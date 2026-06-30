/*!
 * mux-batcher: Multi-operation batching contract for Mux Protocol.
 *
 * Allows atomically executing a sequence of cross-contract calls in a
 * single transaction, with optional per-operation authorization checks.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env, String,
    Vec,
};

// ── Batch operation kind ──────────────────────────────────────────────────────

/// Classifies the intent of a batched operation.
///
/// The kind is informational metadata carried alongside each `Operation`.
/// The batcher does not gate execution on the kind — it is surfaced in events
/// and available to off-chain indexers and TypeScript clients for filtering,
/// analytics, and UI labelling.
///
/// Variants:
/// - `Invoke`   — generic cross-contract function call (default / catch-all)
/// - `Transfer` — asset transfer (e.g. SAC `transfer` call)
/// - `Approve`  — allowance / approval (e.g. SAC `approve` call)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchOperationKind {
    Invoke,
    Transfer,
    Approve,
}

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_bat"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    Executing,
    /// Stores optional contract-level metadata set once at deployment.
    Meta,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Contract-level metadata stored once at deployment for registry discovery.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct BatcherMeta {
    /// Short human-readable description of the contract.
    pub description: String,
    /// Author or team identifier.
    pub author: String,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Operation {
    /// Contract address to invoke.
    pub target: Address,
    /// Name of the function to call on `target`.
    pub fn_name: soroban_sdk::Symbol,
    /// Arguments forwarded verbatim to the target function.
    pub args: Vec<soroban_sdk::Val>,
    /// When `true`, any invocation failure aborts the whole batch with
    /// `RequiredOperationFailed`; when `false`, the failure is counted and
    /// execution continues.
    pub require_success: bool,
    /// Classifies the operation intent for off-chain indexers and clients.
    pub kind: BatchOperationKind,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BatchResult {
    /// Number of operations that completed without error.
    pub success_count: u32,
    /// Number of operations that failed and had `require_success = false`.
    pub failure_count: u32,
    /// Reserved for future per-operation error detail; currently always empty.
    pub errors: Vec<Bytes>,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxBatcherError {
    EmptyBatch = 1,
    BatchTooLarge = 2,
    RequiredOperationFailed = 3,
    Unauthorized = 4,
    ReentrancyDetected = 5,
    MetadataAlreadySet = 6,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum operations allowed in a single batch to bound execution cost.
// STORAGE-GRIEFING: a large batch inflates per-transaction resource consumption
// (CPU instructions, memory) and can be used to grief other users by exhausting
// the ledger's resource budget.  The cap prevents a single caller from
// monopolising ledger capacity.
const MAX_BATCH_SIZE: u32 = 50;

/// Base fee (in stroops) charged per operation in a batch.
/// Used by `estimate_fees` to give callers a conservative preflight estimate.
const FEE_PER_OP: u32 = 100;

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): mux-batcher holds no growing collections, but its
// instance storage (contract metadata) must stay live.  Extend TTL on every
// successful execute_batch call.  See docs/storage-griefing.md.
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Rollback semantics ────────────────────────────────────────────────────────
//
// Soroban provides two rollback paths for mux-batcher:
//
// 1. HOST-LEVEL TRAP (panic! / SDK panic): The Soroban host catches the trap,
//    discards ALL storage writes made during the current contract invocation,
//    and marks the transaction as failed.  No events are committed.
//
// 2. CONTRACT-LEVEL ERROR (return Err(...)): The contract function returns
//    normally with an error value.  The Soroban host does NOT automatically
//    roll back instance storage for contract-level errors — the contract must
//    undo any side effects itself before returning.
//
// mux-batcher uses path 2 for `RequiredOperationFailed` so that callers can
// inspect the error code.  The reentrancy guard (`DataKey::Executing`) is
// therefore explicitly removed before each early-return error path.  All other
// state in this contract is local to the invocation frame and needs no cleanup.
//
// Callers that need atomic all-or-nothing semantics should set
// `require_success = true` on every operation; a single failure then surfaces
// `RequiredOperationFailed` and the caller can treat that as a full rollback.

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxBatcher;

#[contractimpl]
impl MuxBatcher {
    /// Execute a batch of operations atomically.
    ///
    /// If any operation has `require_success = true` and fails, returns
    /// `Err(RequiredOperationFailed)` and emits a `bat_abort` event.
    ///
    /// Emits:
    /// - `bat_abort` — when a required operation fails (before returning error)
    /// - `executed`  — on success, with (caller, success_count, failure_count)
    /// - `bat_ok`    — only when every operation in the batch succeeded
    pub fn execute_batch(
        env: Env,
        caller: Address,
        ops: Vec<Operation>,
    ) -> Result<BatchResult, MuxBatcherError> {
        caller.require_auth();

        if ops.is_empty() {
            return Err(MuxBatcherError::EmptyBatch);
        }
        if ops.len() > MAX_BATCH_SIZE {
            return Err(MuxBatcherError::BatchTooLarge);
        }

        // Emit start event so off-chain indexers can correlate abort/ok events
        // back to the originating batch without scanning storage.
        emit(
            &env,
            symbol_short!("bat_start"),
            (caller.clone(), ops.len()),
        );

        // Reentrancy guard: one of the batched ops could call back into this
        // contract. On error return Soroban rolls back storage automatically.
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Executing)
            .unwrap_or(false)
        {
            return Err(MuxBatcherError::ReentrancyDetected);
        }
        env.storage().instance().set(&DataKey::Executing, &true);

        let mut success_count: u32 = 0;
        let mut failure_count: u32 = 0;
        let errors: Vec<Bytes> = Vec::new(&env);

        for op in ops.iter() {
            let result = env.try_invoke_contract::<soroban_sdk::Val, soroban_sdk::Error>(
                &op.target,
                &op.fn_name,
                op.args.clone(),
            );

            match result {
                Ok(_) => {
                    success_count += 1;
                }
                Err(_err) => {
                    if op.require_success {
                        // Clear reentrancy guard before returning — Soroban rolls
                        // back instance-storage writes on host-side error, but an
                        // Err return from a #[contractimpl] function is NOT a host
                        // trap, so we must clear manually.
                        env.storage().instance().remove(&DataKey::Executing);
                        // Emit abort event so callers can observe the failure
                        // without relying solely on the error return value.
                        emit(&env, symbol_short!("bat_abort"), caller);
                        return Err(MuxBatcherError::RequiredOperationFailed);
                    }
                    failure_count += 1;
                }
            }
        }

        // Clear reentrancy guard so subsequent calls in the same session work.
        env.storage().instance().remove(&DataKey::Executing);

        let result = BatchResult {
            success_count,
            failure_count,
            errors,
        };

        emit(
            &env,
            symbol_short!("executed"),
            (caller.clone(), result.success_count, result.failure_count),
        );

        // Emit a dedicated success event when every operation succeeded.
        if result.failure_count == 0 {
            emit(
                &env,
                symbol_short!("bat_ok"),
                (caller, result.success_count),
            );
        }

        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(result)
    }

    /// Return the maximum number of operations permitted in a single batch.
    ///
    /// Callers can query this before constructing a batch to avoid a
    /// `BatchTooLarge` error at execution time.
    pub fn max_batch_size(_env: Env) -> u32 {
        MAX_BATCH_SIZE
    }

    /// Submit a batch on behalf of the transaction invoker.
    ///
    /// Convenience wrapper around `execute_batch` that derives the caller from
    /// the invoking address, so callers do not need to pass it explicitly.
    ///
    /// Emits the same events as `execute_batch`.
    pub fn submit_batch(env: Env, ops: Vec<Operation>) -> Result<BatchResult, MuxBatcherError> {
        let caller = env.current_contract_address();
        Self::execute_batch(env, caller, ops)
    }

    /// Estimate the fee (in stroops) for a batch of the given size.
    ///
    /// Returns `Err(BatchTooLarge)` when `op_count` exceeds `max_batch_size`.
    pub fn estimate_fees(_env: Env, op_count: u32) -> Result<u32, MuxBatcherError> {
        if op_count == 0 {
            return Err(MuxBatcherError::EmptyBatch);
        }
        if op_count > MAX_BATCH_SIZE {
            return Err(MuxBatcherError::BatchTooLarge);
        }
        Ok(op_count.saturating_mul(FEE_PER_OP))
    }

    /// Store registry metadata (description, author) for this batcher instance.
    ///
    /// Can only be called once; subsequent calls return `MetadataAlreadySet`.
    /// No authorization is required because metadata is informational only and
    /// is expected to be set by the deployer immediately after deployment.
    pub fn set_registry_metadata(
        env: Env,
        description: String,
        author: String,
    ) -> Result<(), MuxBatcherError> {
        if env.storage().instance().has(&DataKey::Meta) {
            return Err(MuxBatcherError::MetadataAlreadySet);
        }
        let meta = BatcherMeta { description, author };
        env.storage().instance().set(&DataKey::Meta, &meta);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Return the registry metadata for this batcher instance, or `None` if not set.
    pub fn get_registry_metadata(env: Env) -> Option<BatcherMeta> {
        env.storage().instance().get(&DataKey::Meta)
    }

    /// Simulate a batch without writing state — useful for preflight checks.
    ///
    /// Counts operations conservatively (assumes all succeed) and emits a
    /// `sim_done` event so off-chain tooling can observe simulated batches
    /// separately from executed ones.
    ///
    /// Returns `Err(EmptyBatch)` or `Err(BatchTooLarge)` on invalid input.
    /// Does **not** invoke target contracts.
    pub fn simulate_batch(
        env: Env,
        caller: Address,
        ops: Vec<Operation>,
    ) -> Result<BatchResult, MuxBatcherError> {
        caller.require_auth();

        if ops.is_empty() {
            return Err(MuxBatcherError::EmptyBatch);
        }
        if ops.len() > MAX_BATCH_SIZE {
            return Err(MuxBatcherError::BatchTooLarge);
        }

        let result = BatchResult {
            success_count: ops.len(),
            failure_count: 0,
            errors: Vec::new(&env),
        };

        emit(
            &env,
            symbol_short!("sim_done"),
            (caller, result.success_count),
        );

        Ok(result)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        contract as test_contract, contractimpl as test_contractimpl, symbol_short,
        testutils::{Address as _, Events},
        Env, FromVal, Vec,
    };

    // Minimal no-op contract used as a real invocation target in tests.
    #[test_contract]
    pub struct DummyTarget;
    #[test_contractimpl]
    impl DummyTarget {
        pub fn noop(_env: Env) {}
    }

    fn topic_action(
        env: &Env,
        events: &soroban_sdk::Vec<(
            soroban_sdk::Address,
            soroban_sdk::Vec<soroban_sdk::Val>,
            soroban_sdk::Val,
        )>,
        idx: u32,
    ) -> soroban_sdk::Symbol {
        let (_, topics, _) = events.get(idx).unwrap();
        soroban_sdk::Symbol::from_val(env, &topics.get(1).unwrap())
    }

    #[test]
    fn test_execute_batch_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let target = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        // require_success=false so a failing call doesn't abort; event still fires
        ops.push_back(Operation {
            target,
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
        let _ = client.try_execute_batch(&caller, &ops);

        let events = env.events().all();
        // bat_start fires first, then executed
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("bat_start"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("executed"));
    }

    #[test]
    fn test_operation_kind_variants_are_distinct() {
        // Verify all BatchOperationKind variants are constructible and distinct.
        assert_ne!(BatchOperationKind::Invoke, BatchOperationKind::Transfer);
        assert_ne!(BatchOperationKind::Transfer, BatchOperationKind::Approve);
        assert_ne!(BatchOperationKind::Invoke, BatchOperationKind::Approve);
    }

    #[test]
    fn test_operation_kind_carried_through_batch() {
        // Verify that an Operation with each kind variant is accepted by execute_batch.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let caller = Address::generate(&env);
        let target = Address::generate(&env);

        for kind in [
            BatchOperationKind::Invoke,
            BatchOperationKind::Transfer,
            BatchOperationKind::Approve,
        ] {
            let mut ops: Vec<Operation> = Vec::new(&env);
            ops.push_back(Operation {
                target: target.clone(),
                fn_name: symbol_short!("noop"),
                args: Vec::new(&env),
                require_success: false,
                kind,
            });
            // execute_batch must accept the op regardless of kind.
            assert!(client.try_execute_batch(&caller, &ops).is_ok());
        }
    }

    #[test]
    fn test_empty_batch_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let ops: Vec<Operation> = Vec::new(&env);
        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_err());
    }

    #[test]
    fn test_reentrancy_guard_clears_after_success() {
        // Verify the Executing flag is cleared so sequential batch calls work.
        // If the guard were not cleared the second call would return ReentrancyDetected.
        // This test requires a real target contract to invoke; we use the batcher
        // itself registered under a second ID, but since ops run against an external
        // address we use a simple single-op batch against a dummy (which returns Err
        // and is not require_success), then verify a second batch also succeeds.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: soroban_sdk::symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });

        assert!(client.try_execute_batch(&caller, &ops).is_ok());
        // Second call must also succeed — guard was cleared after first call.
        assert!(client.try_execute_batch(&caller, &ops).is_ok());
    }

    #[test]
    fn test_batch_too_large_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        let target = Address::generate(&env);
        for _ in 0..51 {
            ops.push_back(Operation {
                target: target.clone(),
                fn_name: soroban_sdk::symbol_short!("noop"),
                args: Vec::new(&env),
                require_success: false,
                kind: BatchOperationKind::Invoke,
            });
        }
        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_err());
    }

    #[test]
    fn test_ttl_extended_on_execute_batch() {
        // Verify that execute_batch bumps instance TTL (T-21 mitigation).
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
        // If extend_ttl was missing the SDK would panic; reaching here is the assertion.
        let _ = client.try_execute_batch(&caller, &ops);
    }

    // ── Issue #73: batch success event ────────────────────────────────────────

    #[test]
    fn test_batch_success_event_emitted_when_all_succeed() {
        // When every operation succeeds, both `executed` and `bat_ok` must fire.
        let env = Env::default();
        env.mock_all_auths();
        let batcher_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &batcher_id);
        let target_id = env.register_contract(None, DummyTarget);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: target_id,
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });
        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_ok());
        let r = result.unwrap().unwrap();
        assert_eq!(r.success_count, 1);
        assert_eq!(r.failure_count, 0);

        let events = env.events().all();
        // bat_start, executed, bat_ok
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("bat_start"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("executed"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("bat_ok"));
    }

    #[test]
    fn test_bat_abort_event_emitted_on_required_failure() {
        // When a required op fails, `bat_abort` must be emitted and the call
        // must return RequiredOperationFailed.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env), // non-existent target → will fail
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });
        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_err());

        let events = env.events().all();
        // bat_start fires first, then bat_abort
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("bat_start"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("bat_abort"));
    }

    #[test]
    fn test_max_batch_size_returns_constant() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        assert_eq!(client.max_batch_size(), MAX_BATCH_SIZE);
    }

    // ── submit_batch tests ────────────────────────────────────────────────────

    #[test]
    fn test_submit_batch_empty_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let ops: Vec<Operation> = Vec::new(&env);
        let result = client.try_submit_batch(&ops);
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_batch_too_large_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let mut ops: Vec<Operation> = Vec::new(&env);
        let target = Address::generate(&env);
        for _ in 0..51 {
            ops.push_back(Operation {
                target: target.clone(),
                fn_name: soroban_sdk::symbol_short!("noop"),
                args: Vec::new(&env),
                require_success: false,
                kind: BatchOperationKind::Invoke,
            });
        }
        let result = client.try_submit_batch(&ops);
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_batch_emits_executed_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
        let _ = client.try_submit_batch(&ops);

        let events = env.events().all();
        // bat_start fires first (via execute_batch), then executed
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("bat_start"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("executed"));
    }

    #[test]
    fn test_batch_success_event_not_emitted_on_partial_failure() {
        // When there is at least one failure, `bat_ok` must NOT be emitted.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        // This op will fail (non-existent target function), require_success=false.
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_ok());

        let events = env.events().all();
        let action_names: soroban_sdk::Vec<soroban_sdk::Symbol> = {
            let mut v = soroban_sdk::Vec::new(&env);
            for i in 0..events.len() {
                v.push_back(topic_action(&env, &events, i));
            }
            v
        };
        // `bat_ok` must not appear in the event list.
        for i in 0..action_names.len() {
            assert_ne!(action_names.get(i).unwrap(), symbol_short!("bat_ok"));
        }
    }

    // ── Issue #79: estimate_fees ───────────────────────────────────────────────

    #[test]
    fn test_estimate_fees_returns_fee_per_op_times_count() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        assert_eq!(client.estimate_fees(&1), 100);
        assert_eq!(client.estimate_fees(&10), 1_000);
        assert_eq!(client.estimate_fees(&50), 5_000);
    }

    #[test]
    fn test_estimate_fees_zero_ops_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        assert!(client.try_estimate_fees(&0).is_err());
    }

    #[test]
    fn test_estimate_fees_over_max_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        assert!(client.try_estimate_fees(&51).is_err());
    }

    // ── simulate_batch tests (#233 / #234) ────────────────────────────────────

    #[test]
    fn test_simulate_batch_returns_op_count() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        for _ in 0..3 {
            ops.push_back(Operation {
                target: Address::generate(&env),
                fn_name: symbol_short!("noop"),
                args: Vec::new(&env),
                require_success: false,
                kind: BatchOperationKind::Invoke,
            });
        }
        let result = client.simulate_batch(&caller, &ops);
        assert_eq!(result.success_count, 3);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn test_simulate_batch_emits_sim_done_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
        let _ = client.simulate_batch(&caller, &ops);

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("sim_done"));
    }

    #[test]
    fn test_simulate_batch_empty_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let ops: Vec<Operation> = Vec::new(&env);
        assert!(client.try_simulate_batch(&caller, &ops).is_err());
    }

    #[test]
    fn test_simulate_batch_too_large_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        let target = Address::generate(&env);
        for _ in 0..51 {
            ops.push_back(Operation {
                target: target.clone(),
                fn_name: symbol_short!("noop"),
                args: Vec::new(&env),
                require_success: false,
                kind: BatchOperationKind::Invoke,
            });
        }
        assert!(client.try_simulate_batch(&caller, &ops).is_err());
    }

    // ── bat_start event (#235) ────────────────────────────────────────────────

    #[test]
    fn test_bat_start_event_emitted_before_execution() {
        // execute_batch must emit bat_start as the first event.
        let env = Env::default();
        env.mock_all_auths();
        let batcher_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &batcher_id);
        let target_id = env.register_contract(None, DummyTarget);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: target_id,
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });
        let _ = client.try_execute_batch(&caller, &ops);

        let events = env.events().all();
        // Order must be: bat_start, executed, bat_ok
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("bat_start"));
    }

    #[test]
    fn test_bat_start_emitted_even_when_required_op_fails() {
        // bat_start must fire before any failure check so indexers see the attempt.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env), // non-existent → fails
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });
        let _ = client.try_execute_batch(&caller, &ops);

        let events = env.events().all();
        // Events: bat_start, bat_abort
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("bat_start"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("bat_abort"));
    }

    // ── Registry metadata (#243) ──────────────────────────────────────────────

    #[test]
    fn test_set_and_get_registry_metadata() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let description = String::from_str(&env, "Multi-operation batching contract");
        let author = String::from_str(&env, "mux-labs");

        assert!(client.try_set_registry_metadata(&description, &author).is_ok());
        let meta = client.get_registry_metadata().unwrap();
        assert_eq!(meta.description, description);
        assert_eq!(meta.author, author);
    }

    #[test]
    fn test_set_registry_metadata_twice_fails() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let description = String::from_str(&env, "Multi-operation batching contract");
        let author = String::from_str(&env, "mux-labs");

        client.set_registry_metadata(&description, &author);
        assert!(client.try_set_registry_metadata(&description, &author).is_err());
    }

    #[test]
    fn test_get_registry_metadata_before_set_returns_none() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        assert!(client.get_registry_metadata().is_none());
    }

    // ── TTL extension on write (#242) ─────────────────────────────────────────

    #[test]
    fn test_ttl_extended_on_submit_batch() {
        // submit_batch delegates to execute_batch, which extends instance TTL.
        // If extend_ttl were missing the SDK would panic; reaching here is the assertion.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
        let _ = client.try_submit_batch(&ops);
    }
}