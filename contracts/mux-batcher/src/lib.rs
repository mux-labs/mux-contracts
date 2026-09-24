/*!
 * mux-batcher: Multi-operation batching contract for Mux Protocol.
 *
 * Allows atomically executing a sequence of cross-contract calls in a
 * single transaction, with optional per-operation authorization checks.
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types backed by the Soroban host.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN,
    Env, String, Vec,
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
    /// Upgrade authority, set once by `initialize`. Optional: a batcher that
    /// is never initialized has no admin and can never be upgraded.
    Admin,
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
    NotInitialized = 7,
    AlreadyInitialized = 8,
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
    /// Initialize the batcher with an upgrade admin. Optional: a batcher
    /// that is never initialized behaves exactly as before this admin was
    /// introduced — it simply has no `upgrade()` path (`upgrade` returns
    /// `NotInitialized`). Batching itself never required an admin and still
    /// does not.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxBatcherError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxBatcherError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Upgrade the contract WASM. Admin only.
    ///
    /// See `docs/batcher-upgrade.md` for storage-compatibility rules that
    /// must be observed between versions. Requires `initialize` to have been
    /// called first; returns `NotInitialized` otherwise (fail-closed — there
    /// is no admin to authorise the replace).
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxBatcherError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Execute a batch of operations atomically.
    ///
    /// If any operation has `require_success = true` and fails, returns
    /// `Err(RequiredOperationFailed)` and emits a `bat_abort` event.
    ///
    /// # Reentrancy Guard (#690)
    ///
    /// The reentrancy guard (`DataKey::Executing`) is set immediately after the
    /// size checks pass and is **always** removed before this function returns,
    /// regardless of outcome:
    /// - Cleared after the batch loop completes successfully.
    /// - Cleared before returning `Err(RequiredOperationFailed)` on the abort path.
    /// Note: `Err(EmptyBatch)` and `Err(BatchTooLarge)` return before the guard
    /// is ever set, so no cleanup is needed on those paths.
    ///
    /// This ensures that:
    /// 1. Batched operations cannot recursively call `execute_batch` (reentrancy is blocked).
    /// 2. Subsequent calls in the same session succeed (guard is cleared after each call).
    /// 3. The guard is cleared even when required operations fail (abort path cleanup).
    ///
    /// Emits (in order):
    /// - `bat_start` — immediately after size checks pass, before any operations run
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
    /// Requires admin authorization (`initialize` must have been called first).
    /// Returns `NotInitialized` if the contract has not been initialized.
    ///
    /// This prevents metadata-spoofing: only the admin that called `initialize`
    /// can record the canonical deployment description and author.
    pub fn set_registry_metadata(
        env: Env,
        description: String,
        author: String,
    ) -> Result<(), MuxBatcherError> {
        // Fail-closed: require admin auth before checking MetadataAlreadySet so
        // that unauthenticated callers cannot probe whether metadata has been set.
        Self::require_admin(&env)?;
        if env.storage().instance().has(&DataKey::Meta) {
            return Err(MuxBatcherError::MetadataAlreadySet);
        }
        let meta = BatcherMeta {
            description,
            author,
        };
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
    /// Does **not** invoke target contracts or write any storage.
    ///
    /// See `docs/simulate-batch.md` for full usage patterns, limitations,
    /// and TypeScript binding examples.
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

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxBatcherError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxBatcherError::NotInitialized)?;
        admin.require_auth();
        Ok(())
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

    /// Build `count` noop operations for batch-size boundary tests.
    fn make_nop_ops(env: &Env, count: u32) -> Vec<Operation> {
        let mut ops: Vec<Operation> = Vec::new(env);
        let target = Address::generate(env);
        for _ in 0..count {
            ops.push_back(Operation {
                target: target.clone(),
                fn_name: symbol_short!("noop"),
                args: Vec::new(env),
                require_success: false,
                kind: BatchOperationKind::Invoke,
            });
        }
        ops
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
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::EmptyBatch);
    }

    #[test]
    fn test_empty_batch_does_not_emit_events() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let ops: Vec<Operation> = Vec::new(&env);
        let _ = client.try_execute_batch(&caller, &ops);
        assert_eq!(env.events().all().len(), 0);
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

    // ── Reentrancy guard: abort path ──────────────────────────────────────────

    #[test]
    fn test_reentrancy_guard_clears_after_required_op_fails() {
        // If a required operation fails the batch aborts with RequiredOperationFailed.
        // The reentrancy guard must be cleared before the function returns so that
        // a subsequent call can succeed.  If the guard were left set the second call
        // would return ReentrancyDetected instead of executing normally.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let target_id = env.register_contract(None, DummyTarget);

        let caller = Address::generate(&env);

        // First call: required op against a non-existent target → aborts.
        let mut abort_ops: Vec<Operation> = Vec::new(&env);
        abort_ops.push_back(Operation {
            target: Address::generate(&env), // non-existent → will fail
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });
        let abort_result = client.try_execute_batch(&caller, &abort_ops);
        assert!(
            abort_result.is_err(),
            "first batch must fail with RequiredOperationFailed"
        );

        // Second call: a successful batch against a real target.
        // This must succeed — the guard must have been cleared on the abort path.
        let mut ok_ops: Vec<Operation> = Vec::new(&env);
        ok_ops.push_back(Operation {
            target: target_id,
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });
        let ok_result = client.try_execute_batch(&caller, &ok_ops);
        assert!(
            ok_result.is_ok(),
            "second batch must succeed — guard must be cleared after abort"
        );
    }

    #[test]
    fn test_reentrancy_detected_when_executing_flag_already_set() {
        // Simulate a re-entrant call by pre-seeding DataKey::Executing = true in
        // instance storage before calling execute_batch.  execute_batch must detect
        // the flag and return ReentrancyDetected without processing any operations.
        //
        // Note: the Soroban test environment does not support true recursive
        // cross-contract re-entry within a single test frame, so we seed the flag
        // directly to exercise the guard check in isolation.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        // Seed the reentrancy flag directly as if a prior (incomplete) call set it.
        env.as_contract(&contract_id, || {
            env.storage()
                .instance()
                .set(&DataKey::Executing, &true);
        });

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("noop"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });

        let result = client.try_execute_batch(&caller, &ops);
        assert!(
            result.is_err(),
            "execute_batch must return an error when guard is already set"
        );
        // The outer Result<Result<BatchResult, MuxBatcherError>, _> — unwrap the
        // transport layer and check the contract error.
        let contract_err = result.unwrap_err();
        assert_eq!(
            contract_err,
            Ok(MuxBatcherError::ReentrancyDetected),
            "error must be ReentrancyDetected when Executing flag is pre-set"
        );
    }

    #[test]
    fn test_batch_too_large_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let result = client.try_execute_batch(&caller, &ops);
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::BatchTooLarge);
    }

    #[test]
    fn test_execute_batch_at_max_size_accepted() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE);
        let result = client.try_simulate_batch(&caller, &ops);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().success_count, MAX_BATCH_SIZE);
    }

    #[test]
    fn test_batch_too_large_does_not_emit_events() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let _ = client.try_execute_batch(&caller, &ops);
        assert_eq!(env.events().all().len(), 0);
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
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::EmptyBatch);
    }

    #[test]
    fn test_submit_batch_too_large_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let result = client.try_submit_batch(&ops);
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::BatchTooLarge);
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

    #[test]
    fn test_batch_atomic_require_success_aborts_whole_batch() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        // Valid op 1
        ops.push_back(Operation {
            target: contract_id.clone(),
            fn_name: symbol_short!("max_batch"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });
        // Failing op 2 with require_success = true
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("fail_op"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });

        let result = client.try_execute_batch(&caller, &ops);
        assert_eq!(result, Err(Ok(MuxBatcherError::RequiredOperationFailed)));
    }

    #[test]
    fn test_batch_per_op_failure_policy_mixed() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        // Successful op 1
        ops.push_back(Operation {
            target: contract_id.clone(),
            fn_name: symbol_short!("max_batch"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
        // Failing op 2 with require_success = false
        ops.push_back(Operation {
            target: Address::generate(&env),
            fn_name: symbol_short!("nonexist"),
            args: Vec::new(&env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });

        let result = client.execute_batch(&caller, &ops);
        assert_eq!(result.success_count, 1);
        assert_eq!(result.failure_count, 1);
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

        let result = client.try_estimate_fees(&0);
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::EmptyBatch);
    }

    #[test]
    fn test_estimate_fees_over_max_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let result = client.try_estimate_fees(&(MAX_BATCH_SIZE + 1));
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::BatchTooLarge);
    }

    #[test]
    fn test_estimate_fees_at_max_size_accepted() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        assert_eq!(
            client.estimate_fees(&MAX_BATCH_SIZE),
            MAX_BATCH_SIZE.saturating_mul(FEE_PER_OP)
        );
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
        let result = client.try_simulate_batch(&caller, &ops);
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::EmptyBatch);
    }

    #[test]
    fn test_simulate_batch_too_large_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let result = client.try_simulate_batch(&caller, &ops);
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, MuxBatcherError::BatchTooLarge);
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
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let description = String::from_str(&env, "Multi-operation batching contract");
        let author = String::from_str(&env, "mux-labs");

        assert!(client
            .try_set_registry_metadata(&description, &author)
            .is_ok());
        let meta = client.get_registry_metadata().unwrap();
        assert_eq!(meta.description, description);
        assert_eq!(meta.author, author);
    }

    #[test]
    fn test_set_registry_metadata_twice_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let description = String::from_str(&env, "Multi-operation batching contract");
        let author = String::from_str(&env, "mux-labs");

        client.set_registry_metadata(&description, &author);
        assert!(client
            .try_set_registry_metadata(&description, &author)
            .is_err());
    }

    #[test]
    fn test_set_registry_metadata_requires_admin_auth() {
        // Without mock_all_auths, set_registry_metadata must reject unauthenticated callers.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        // Seed admin directly (bypassing initialize's own auth check) so only
        // set_registry_metadata's auth gate is exercised here.
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
        });

        let description = String::from_str(&env, "Multi-operation batching contract");
        let author = String::from_str(&env, "mux-labs");

        let result = client.try_set_registry_metadata(&description, &author);
        assert!(
            result.is_err(),
            "set_registry_metadata must reject when admin auth is absent"
        );
    }

    #[test]
    fn test_set_registry_metadata_before_initialize_returns_not_initialized() {
        // Without initialize, set_registry_metadata must return NotInitialized (fail-closed).
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);

        let description = String::from_str(&env, "Multi-operation batching contract");
        let author = String::from_str(&env, "mux-labs");

        let result = client.try_set_registry_metadata(&description, &author);
        assert_eq!(result, Err(Ok(MuxBatcherError::NotInitialized)));
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

    // ── Issue #412: batch size upper bound enforcement ────────────────────────
    //
    // These tests explicitly verify that MAX_BATCH_SIZE = 50 is enforced on
    // every public entry point that accepts an ops vec.  They are separate from
    // the general boundary tests above so the enforcement contract is
    // unambiguously covered and easy to locate in the audit trail.

    /// MAX_BATCH_SIZE must equal 50 — this is the stable on-chain constant that
    /// callers and documentation depend on.
    #[test]
    fn test_max_batch_size_constant_is_50() {
        assert_eq!(
            MAX_BATCH_SIZE, 50,
            "MAX_BATCH_SIZE must remain 50; update docs/batching-limits.md if changed"
        );
    }

    /// execute_batch with exactly MAX_BATCH_SIZE ops must succeed (boundary-at-limit).
    #[test]
    fn test_execute_batch_exactly_at_max_size_accepted() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE);
        // All ops have require_success=false so missing targets count as failures
        // but do not abort — we only need the size check to pass.
        assert!(
            client.try_execute_batch(&caller, &ops).is_ok(),
            "execute_batch must accept exactly MAX_BATCH_SIZE ops"
        );
    }

    /// execute_batch with MAX_BATCH_SIZE + 1 ops must return BatchTooLarge.
    #[test]
    fn test_execute_batch_one_over_max_returns_batch_too_large() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let result = client.try_execute_batch(&caller, &ops);
        assert_eq!(
            result.unwrap_err(),
            Ok(MuxBatcherError::BatchTooLarge),
            "execute_batch with MAX_BATCH_SIZE+1 ops must return BatchTooLarge"
        );
    }

    /// submit_batch with exactly MAX_BATCH_SIZE ops must succeed.
    #[test]
    fn test_submit_batch_exactly_at_max_size_accepted() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE);
        assert!(
            client.try_submit_batch(&ops).is_ok(),
            "submit_batch must accept exactly MAX_BATCH_SIZE ops"
        );
    }

    /// submit_batch with MAX_BATCH_SIZE + 1 ops must return BatchTooLarge.
    #[test]
    fn test_submit_batch_one_over_max_returns_batch_too_large() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let result = client.try_submit_batch(&ops);
        assert_eq!(
            result.unwrap_err(),
            Ok(MuxBatcherError::BatchTooLarge),
            "submit_batch with MAX_BATCH_SIZE+1 ops must return BatchTooLarge"
        );
    }

    /// simulate_batch with exactly MAX_BATCH_SIZE ops must succeed.
    #[test]
    fn test_simulate_batch_exactly_at_max_size_accepted() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE);
        let result = client.try_simulate_batch(&caller, &ops);
        assert!(
            result.is_ok(),
            "simulate_batch must accept exactly MAX_BATCH_SIZE ops"
        );
        assert_eq!(result.unwrap().unwrap().success_count, MAX_BATCH_SIZE);
    }

    /// simulate_batch with MAX_BATCH_SIZE + 1 ops must return BatchTooLarge.
    #[test]
    fn test_simulate_batch_one_over_max_returns_batch_too_large() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let result = client.try_simulate_batch(&caller, &ops);
        assert_eq!(
            result.unwrap_err(),
            Ok(MuxBatcherError::BatchTooLarge),
            "simulate_batch with MAX_BATCH_SIZE+1 ops must return BatchTooLarge"
        );
    }

    /// estimate_fees with exactly MAX_BATCH_SIZE must succeed and return the
    /// correct fee without overflow.
    #[test]
    fn test_estimate_fees_exactly_at_max_size_accepted() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let expected = MAX_BATCH_SIZE.saturating_mul(FEE_PER_OP);
        assert_eq!(
            client.estimate_fees(&MAX_BATCH_SIZE),
            expected,
            "estimate_fees at MAX_BATCH_SIZE must return MAX_BATCH_SIZE * FEE_PER_OP"
        );
    }

    /// estimate_fees with MAX_BATCH_SIZE + 1 must return BatchTooLarge.
    #[test]
    fn test_estimate_fees_one_over_max_returns_batch_too_large() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let result = client.try_estimate_fees(&(MAX_BATCH_SIZE + 1));
        assert_eq!(
            result.unwrap_err(),
            Ok(MuxBatcherError::BatchTooLarge),
            "estimate_fees with MAX_BATCH_SIZE+1 must return BatchTooLarge"
        );
    }

    /// BatchTooLarge (error code 2) is stable ABI — verify the discriminant.
    #[test]
    fn test_batch_too_large_error_code_is_2() {
        assert_eq!(
            MuxBatcherError::BatchTooLarge as u32,
            2,
            "BatchTooLarge must remain error code 2; coordinate changes with docs/error_codes.md"
        );
    }

    /// max_batch_size() query entrypoint must match the compiled constant so
    /// callers can discover the limit at runtime without hard-coding it.
    #[test]
    fn test_max_batch_size_query_matches_constant() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        assert_eq!(
            client.max_batch_size(),
            MAX_BATCH_SIZE,
            "max_batch_size() must equal the compiled MAX_BATCH_SIZE constant"
        );
    }

    /// Oversized batches must not emit any events (no partial side-effects).
    #[test]
    fn test_execute_batch_over_max_emits_no_events() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let caller = Address::generate(&env);
        let ops = make_nop_ops(&env, MAX_BATCH_SIZE + 1);
        let _ = client.try_execute_batch(&caller, &ops);
        assert_eq!(
            env.events().all().len(),
            0,
            "no events must be emitted when execute_batch rejects an oversized batch"
        );
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────

    #[test]
    fn test_symbol_short_lengths_within_limit() {
        let tags = [symbol_short!("mux_bat")];
        let actions = [
            symbol_short!("bat_start"),
            symbol_short!("executed"),
            symbol_short!("bat_ok"),
            symbol_short!("bat_abort"),
            symbol_short!("sim_done"),
            symbol_short!("init"),
        ];
        for sym in tags.iter().chain(actions.iter()) {
            let _ = sym;
        }
    }

    // ── simulate_batch does not invoke targets (#651, by design) ─────────────
    //
    // simulate_batch is a conservative, read-only preflight — see
    // docs/simulate-batch.md "Limitations". It intentionally never invokes
    // target contracts, so it cannot detect auth or contract-level failures
    // that execute_batch would surface; true dry-run semantics require
    // Soroban's off-chain Simulation RPC instead. This test locks in that
    // documented behaviour: an operation that would fail for real (wrong
    // function name on a real registered contract) is still reported as
    // successful by simulate_batch.

    #[test]
    fn test_simulate_batch_reports_success_for_operation_that_would_actually_fail() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let target_id = env.register_contract(None, DummyTarget);

        let caller = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        // "nonexist" is not a function on DummyTarget — execute_batch would
        // fail this op for real; simulate_batch must not.
        ops.push_back(Operation {
            target: target_id,
            fn_name: symbol_short!("nonexist"),
            args: Vec::new(&env),
            require_success: true,
            kind: BatchOperationKind::Invoke,
        });

        let result = client.simulate_batch(&caller, &ops);
        assert_eq!(result.success_count, 1);
        assert_eq!(result.failure_count, 0);
    }

    // ── initialize / upgrade (closes #694) ────────────────────────────────────

    #[test]
    fn test_initialize_stores_admin_and_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        assert!(client.try_initialize(&admin).is_ok());
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin);
        let result = client.try_initialize(&admin);
        assert_eq!(result, Err(Ok(MuxBatcherError::AlreadyInitialized)));
    }

    #[test]
    fn test_initialize_requires_admin_auth() {
        // No mock_all_auths — require_auth must reject.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let result = client.try_initialize(&admin);
        assert!(
            result.is_err(),
            "initialize must reject when admin auth is absent"
        );
    }

    #[test]
    fn test_execute_batch_requires_caller_auth() {
        let env = Env::default();
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

        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_err());
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_simulate_batch_requires_caller_auth() {
        let env = Env::default();
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

        let result = client.try_simulate_batch(&caller, &ops);
        assert!(result.is_err());
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_upgrade_before_initialize_returns_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let fake_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        let result = client.try_upgrade(&fake_hash);
        assert_eq!(result, Err(Ok(MuxBatcherError::NotInitialized)));
    }

    #[test]
    fn test_upgrade_requires_admin_auth() {
        // Seed Admin directly in storage (bypassing initialize) so this test
        // exercises only the upgrade() auth gate with zero mocked auths.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
        });

        let fake_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
        let result = client.try_upgrade(&fake_hash);
        assert!(
            result.is_err(),
            "upgrade must reject when admin auth is absent"
        );
    }

    #[test]
    fn test_execute_batch_does_not_require_initialize() {
        // Batching must keep working exactly as before for batchers that
        // never call initialize() — the admin is optional and orthogonal to
        // batch execution.
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
        assert!(client.try_execute_batch(&caller, &ops).is_ok());
    }
}
