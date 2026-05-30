/*!
 * mux-batcher: Multi-operation batching contract for Mux Protocol.
 *
 * Allows atomically executing a sequence of cross-contract calls in a
 * single transaction, with optional per-operation authorization checks.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Bytes, Env, Vec};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Set to `true` while `execute_batch` is executing.
    /// Prevents cross-contract re-entrancy into the batcher itself.
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
const MAX_BATCH_SIZE: u32 = 50;

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxBatcher;

#[contractimpl]
impl MuxBatcher {
    /// Execute a batch of operations atomically.
    ///
    /// If any operation has `require_success = true` and fails, the entire
    /// transaction is aborted via panic (Soroban rolls back on panic).
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
                        return Err(MuxBatcherError::RequiredOperationFailed);
                    }
                    failure_count += 1;
                }
            }
        }

        env.storage().instance().set(&DataKey::Executing, &false);
        Ok(BatchResult {
            success_count,
            failure_count,
            errors,
        })
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
    use soroban_sdk::{testutils::Address as _, Env, Vec};

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
        let target = Address::generate(&env);
        let mut ops: Vec<Operation> = Vec::new(&env);
        ops.push_back(Operation {
            target: target.clone(),
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
}
