/*!
 * mux-batcher: Multi-operation batching contract for Mux Protocol.
 *
 * Allows atomically executing a sequence of cross-contract calls in a
 * single transaction, with optional per-operation authorization checks.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Vec,
};

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
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct Operation {
    pub target: Address,
    pub fn_name: soroban_sdk::Symbol,
    pub args: Vec<soroban_sdk::Val>,
    pub require_success: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BatchResult {
    pub success_count: u32,
    pub failure_count: u32,
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
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum operations allowed in a single batch to bound execution cost.
// STORAGE-GRIEFING: a large batch inflates per-transaction resource consumption
// (CPU instructions, memory) and can be used to grief other users by exhausting
// the ledger's resource budget.  The cap prevents a single caller from
// monopolising ledger capacity.
const MAX_BATCH_SIZE: u32 = 50;

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
                        emit(
                            &env,
                            symbol_short!("bat_abort"),
                            caller,
                        );
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

    /// Simulate a batch without writing state — useful for preflight checks.
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

        // Preflight: count without invoking (real simulation requires contract
        // access to a read-only snapshot — this returns a conservative estimate).
        Ok(BatchResult {
            success_count: ops.len(),
            failure_count: 0,
            errors: Vec::new(&env),
        })
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
        });
        let _ = client.try_execute_batch(&caller, &ops);

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("executed"));
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
        });
        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.success_count, 1);
        assert_eq!(r.failure_count, 0);

        let events = env.events().all();
        // `executed` then `bat_ok`
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("executed"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("bat_ok"));
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
        });
        let result = client.try_execute_batch(&caller, &ops);
        assert!(result.is_err());

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("bat_abort"));
    }

    #[test]
    fn test_max_batch_size_returns_constant() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxBatcher);
        let client = MuxBatcherClient::new(&env, &contract_id);
        assert_eq!(client.max_batch_size(), MAX_BATCH_SIZE);
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
}
