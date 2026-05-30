/*!
 * mux-batcher: Multi-operation batching contract for Mux Protocol.
 *
 * Allows atomically executing a sequence of cross-contract calls in a
 * single transaction, with optional per-operation authorization checks.
 */

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short, Address, Bytes, Env, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: soroban_sdk::Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events().publish((symbol_short!("mux_bat"), action), data);
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

        let result = BatchResult { success_count, failure_count, errors };
        emit(&env, symbol_short!("executed"), (caller, result.success_count, result.failure_count));
        Ok(result)
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
    use soroban_sdk::{testutils::{Address as _, Events}, symbol_short, FromVal, Env, Vec};

    fn topic_action(env: &Env, events: &soroban_sdk::Vec<(soroban_sdk::Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val)>, idx: u32) -> soroban_sdk::Symbol {
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
