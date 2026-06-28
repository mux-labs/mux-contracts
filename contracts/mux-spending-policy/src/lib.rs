/*!
 * mux-spending-policy: Spending-policy enforcement contract for Mux Protocol.
 *
 * Stores per-account spend limits and validates spend requests against them.
 *
 * ## Audit Events
 *
 * This contract emits the following events:
 * - `initialize`: Emitted when the contract is initialized with an admin address.
 * - `lmt_set`: Emitted when a spending limit policy is created or updated.
 *
 * Events can be queried via the Soroban RPC `getEvents` endpoint with contract ID filter.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_spend"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Key space used by the spending-policy contract.
#[contracttype]
pub enum DataKey {
    /// Instance storage key for the admin address.
    Admin,
    /// Persistent policy record keyed by (account, asset).
    SpendLimit(Address, Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A spend policy describing the maximum spendable amount for a given asset.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SpendLimit {
    /// Asset identifier associated with the policy.
    pub asset: Address,
    /// Maximum amount that may be spent for the asset.
    pub limit: i128,
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Errors returned by the spending-policy contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SpendingPolicyError {
    /// The contract has not been initialized.
    NotInitialized = 1,
    /// The contract was already initialized.
    AlreadyInitialized = 2,
    /// The caller is not authorized to perform the requested action.
    Unauthorized = 3,
    /// No spend policy exists for the requested account/asset pair.
    PolicyNotFound = 4,
    /// The requested spend exceeds the configured policy limit.
    SpendLimitExceeded = 5,
    /// The provided input is invalid (for example a non-positive limit).
    InvalidInput = 6,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxSpendingPolicy;

#[contractimpl]
impl MuxSpendingPolicy {
    /// Initialize the contract with the admin address that may manage policies.
    pub fn initialize(env: Env, admin: Address) -> Result<(), SpendingPolicyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(SpendingPolicyError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        Ok(())
    }

    /// Set or replace the spend limit for an account/asset pair.
    ///
    /// Only the initialized admin can change policies. The configured limit
    /// must be strictly positive.
    pub fn set_policy(
        env: Env,
        account: Address,
        asset: Address,
        limit: i128,
    ) -> Result<(), SpendingPolicyError> {
        Self::require_admin(&env)?;
        if limit <= 0 {
            return Err(SpendingPolicyError::InvalidInput);
        }
        let policy = SpendLimit { asset: asset.clone(), limit };
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(account.clone(), asset.clone()), &policy);
        emit(&env, symbol_short!("lmt_set"), (account, asset, limit));
        Ok(())
    }

    /// Get the spend limit for an account/asset pair.
    pub fn get_policy(
        env: Env,
        account: Address,
        asset: Address,
    ) -> Result<SpendLimit, SpendingPolicyError> {
        env.storage()
            .instance()
            .get(&DataKey::SpendLimit(account, asset))
            .ok_or(SpendingPolicyError::PolicyNotFound)
    }

    /// Check whether `amount` is within the policy limit for `account`/`asset`.
    ///
    /// Returns `Ok(())` when the spend is allowed, `Err(SpendLimitExceeded)`
    /// when the spend exceeds the configured limit, `Err(PolicyNotFound)` when
    /// no policy is configured, and `Err(InvalidInput)` for negative amounts.
    pub fn check_spend(
        env: Env,
        account: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), SpendingPolicyError> {
        if amount < 0 {
            return Err(SpendingPolicyError::InvalidInput);
        }
        let policy: SpendLimit = env
            .storage()
            .instance()
            .get(&DataKey::SpendLimit(account, asset))
            .ok_or(SpendingPolicyError::PolicyNotFound)?;
        if amount > policy.limit {
            return Err(SpendingPolicyError::SpendLimitExceeded);
        }
        Ok(())
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), SpendingPolicyError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SpendingPolicyError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, MuxSpendingPolicyClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_set_and_get_policy() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        let policy = client.get_policy(&account, &asset);
        assert_eq!(policy.limit, 1000);
        assert_eq!(policy.asset, asset);
    }

    #[test]
    fn test_get_policy_not_found() {
        let (env, client, _) = setup();
        let result = client.try_get_policy(&Address::generate(&env), &Address::generate(&env));
        assert!(result.is_err());
    }

    #[test]
    fn test_check_spend_within_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        assert!(client.try_check_spend(&account, &asset, &500).is_ok());
        assert!(client.try_check_spend(&account, &asset, &1000).is_ok());
    }

    #[test]
    fn test_check_spend_exceeds_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        let result = client.try_check_spend(&account, &asset, &1001);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_spend_no_policy() {
        let (env, client, _) = setup();
        let result = client.try_check_spend(&Address::generate(&env), &Address::generate(&env), &1);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_policy_rejects_non_positive_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        let result = client.try_set_policy(&account, &asset, &0);
        assert!(result.is_err());
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, SpendingPolicyError::InvalidInput);
    }

    #[test]
    fn test_check_spend_rejects_negative_amount() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        let result = client.try_check_spend(&account, &asset, &-1);
        assert!(result.is_err());
        let err = result.unwrap_err().unwrap();
        assert_eq!(err, SpendingPolicyError::InvalidInput);
    }

    #[test]
    fn test_invalid_input_error_code() {
        assert_eq!(SpendingPolicyError::InvalidInput as u32, 6);
    }

    // ── Unit Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, admin) = setup();
        // Second attempt to initialize should fail
        let result = client.try_initialize(&admin);
        assert_eq!(result, Err(Ok(SpendingPolicyError::AlreadyInitialized)));
    }

    #[test]
    fn test_set_policy_updates_existing() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Set initial policy
        client.set_policy(&account, &asset, &1000);
        let policy1 = client.get_policy(&account, &asset);
        assert_eq!(policy1.limit, 1000);
        
        // Update the policy to a new limit
        client.set_policy(&account, &asset, &5000);
        let policy2 = client.get_policy(&account, &asset);
        assert_eq!(policy2.limit, 5000);
        assert_eq!(policy2.asset, asset);
    }

    #[test]
    fn test_multiple_accounts_same_asset() {
        let (env, client, _) = setup();
        let asset = Address::generate(&env);
        let account1 = Address::generate(&env);
        let account2 = Address::generate(&env);
        
        // Set policies for two different accounts with the same asset
        client.set_policy(&account1, &asset, &1000);
        client.set_policy(&account2, &asset, &2000);
        
        // Verify each account has its own policy
        let policy1 = client.get_policy(&account1, &asset);
        let policy2 = client.get_policy(&account2, &asset);
        
        assert_eq!(policy1.limit, 1000);
        assert_eq!(policy2.limit, 2000);
        assert_eq!(policy1.asset, asset);
        assert_eq!(policy2.asset, asset);
    }

    #[test]
    fn test_multiple_assets_same_account() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset1 = Address::generate(&env);
        let asset2 = Address::generate(&env);
        
        // Set policies for the same account with two different assets
        client.set_policy(&account, &asset1, &1000);
        client.set_policy(&account, &asset2, &5000);
        
        // Verify each asset has its own policy for the same account
        let policy1 = client.get_policy(&account, &asset1);
        let policy2 = client.get_policy(&account, &asset2);
        
        assert_eq!(policy1.limit, 1000);
        assert_eq!(policy1.asset, asset1);
        assert_eq!(policy2.limit, 5000);
        assert_eq!(policy2.asset, asset2);
    }

    #[test]
    fn test_policy_boundary_values() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Test with very large limit (max i128)
        let max_limit = i128::MAX;
        client.set_policy(&account, &asset, &max_limit);
        let policy = client.get_policy(&account, &asset);
        assert_eq!(policy.limit, max_limit);
        
        // Test check_spend at exactly the limit
        assert!(client.try_check_spend(&account, &asset, &max_limit).is_ok());
    }

    #[test]
    fn test_check_spend_at_exact_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        let limit = 1000;
        
        client.set_policy(&account, &asset, &limit);
        
        // Spending exactly at the limit should succeed
        assert!(client.try_check_spend(&account, &asset, &limit).is_ok());
        
        // Spending 1 more should fail
        assert!(client.try_check_spend(&account, &asset, &(limit + 1)).is_err());
    }

    #[test]
    fn test_check_spend_zero_amount() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        client.set_policy(&account, &asset, &1000);
        
        // Spending zero should be allowed (no validation against zero in current implementation)
        assert!(client.try_check_spend(&account, &asset, &0).is_ok());
    }

    #[test]
    fn test_error_codes_mapping() {
        // Verify all error codes have expected discriminant values
        assert_eq!(SpendingPolicyError::NotInitialized as u32, 1);
        assert_eq!(SpendingPolicyError::AlreadyInitialized as u32, 2);
        assert_eq!(SpendingPolicyError::Unauthorized as u32, 3);
        assert_eq!(SpendingPolicyError::PolicyNotFound as u32, 4);
        assert_eq!(SpendingPolicyError::SpendLimitExceeded as u32, 5);
    }

    // ── Negative Path Tests ────────────────────────────────────────────────────

    #[test]
    fn test_set_policy_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Try to set policy before initialization
        let result = client.try_set_policy(&account, &asset, &1000);
        assert_eq!(result, Err(Ok(SpendingPolicyError::NotInitialized)));
    }

    #[test]
    fn test_check_spend_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Try to check spend before initialization
        let result = client.try_check_spend(&account, &asset, &100);
        assert_eq!(result, Err(Ok(SpendingPolicyError::NotInitialized)));
    }

    #[test]
    fn test_check_spend_with_negative_amount() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        client.set_policy(&account, &asset, &1000);
        
        // Check spend with negative amount should fail (no policy for this amount)
        // This tests handling of negative amounts in the comparison logic
        let result = client.try_check_spend(&account, &asset, &-500);
        assert!(result.is_ok()); // Negative amounts are less than positive limits, so they pass
    }

    #[test]
    fn test_get_policy_not_found_specific_error() {
        let (env, client, _) = setup();
        let nonexistent_account = Address::generate(&env);
        let nonexistent_asset = Address::generate(&env);
        
        // Verify that getting a non-existent policy returns PolicyNotFound error
        let result = client.try_get_policy(&nonexistent_account, &nonexistent_asset);
        assert_eq!(result, Err(Ok(SpendingPolicyError::PolicyNotFound)));
    }

    #[test]
    fn test_check_spend_policy_not_found_error() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Try to check spend on account/asset pair with no policy
        let result = client.try_check_spend(&account, &asset, &500);
        assert_eq!(result, Err(Ok(SpendingPolicyError::PolicyNotFound)));
    }

    #[test]
    fn test_check_spend_exceeds_limit_error() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        client.set_policy(&account, &asset, &1000);
        
        // Verify SpendLimitExceeded error when spending exceeds limit
        let result = client.try_check_spend(&account, &asset, &1001);
        assert_eq!(result, Err(Ok(SpendingPolicyError::SpendLimitExceeded)));
    }

    #[test]
    fn test_check_spend_far_exceeds_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        client.set_policy(&account, &asset, &1000);
        
        // Test spending far exceeding the limit
        let result = client.try_check_spend(&account, &asset, &1_000_000);
        assert_eq!(result, Err(Ok(SpendingPolicyError::SpendLimitExceeded)));
    }

    #[test]
    fn test_set_policy_overwrites_completely() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset1 = Address::generate(&env);
        let asset2 = Address::generate(&env);
        
        // Set policy with asset1 and high limit
        client.set_policy(&account, &asset1, &10000);
        
        // Verify first policy exists
        let policy1 = client.get_policy(&account, &asset1);
        assert_eq!(policy1.asset, asset1);
        assert_eq!(policy1.limit, 10000);
        
        // Create new policy with different asset - should not affect previous
        client.set_policy(&account, &asset2, &5000);
        
        // Verify both policies exist independently
        let policy1_check = client.get_policy(&account, &asset1);
        let policy2_check = client.get_policy(&account, &asset2);
        
        assert_eq!(policy1_check.limit, 10000);
        assert_eq!(policy2_check.limit, 5000);
    }

    #[test]
    fn test_get_policy_returns_correct_asset() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        client.set_policy(&account, &asset, &2500);
        let policy = client.get_policy(&account, &asset);
        
        // Verify the returned policy has the correct asset address
        assert_eq!(policy.asset, asset);
        assert_eq!(policy.limit, 2500);
    }

    #[test]
    fn test_sequential_check_spend_calls() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        client.set_policy(&account, &asset, &1000);
        
        // Multiple successful checks should not affect limit enforcement
        assert!(client.try_check_spend(&account, &asset, &100).is_ok());
        assert!(client.try_check_spend(&account, &asset, &200).is_ok());
        assert!(client.try_check_spend(&account, &asset, &500).is_ok());
        
        // Limit should still be enforced for new checks
        assert!(client.try_check_spend(&account, &asset, &800).is_err());
    }

    #[test]
    fn test_policy_with_min_positive_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Set policy with minimum positive limit (1)
        client.set_policy(&account, &asset, &1);
        let policy = client.get_policy(&account, &asset);
        
        assert_eq!(policy.limit, 1);
        assert!(client.try_check_spend(&account, &asset, &1).is_ok());
        assert!(client.try_check_spend(&account, &asset, &2).is_err());
    }

    // ── Audit Event Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_initialize_emits_event() {
        use soroban_sdk::testutils::Events;
        
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        
        client.initialize(&admin);
        
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        
        let (_, topics, data) = events.get(0).unwrap();
        assert_eq!(topics.len(), 2);
        
        // Verify topics
        let contract_tag = soroban_sdk::Symbol::from_val(&env, &topics.get(0).unwrap());
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(contract_tag, symbol_short!("mux_spend"));
        assert_eq!(action, symbol_short!("init"));
    }

    #[test]
    fn test_set_policy_emits_event() {
        use soroban_sdk::testutils::Events;
        
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Clear events from setup (initialize event)
        env.events().all();
        
        client.set_policy(&account, &asset, &1000);
        
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        
        let (_, topics, _) = events.get(0).unwrap();
        
        // Verify topics
        let contract_tag = soroban_sdk::Symbol::from_val(&env, &topics.get(0).unwrap());
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(contract_tag, symbol_short!("mux_spend"));
        assert_eq!(action, symbol_short!("lmt_set"));
    }

    #[test]
    fn test_multiple_events_emitted() {
        use soroban_sdk::testutils::Events;
        
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let account1 = Address::generate(&env);
        let asset1 = Address::generate(&env);
        let account2 = Address::generate(&env);
        let asset2 = Address::generate(&env);
        
        // Initialize
        client.initialize(&admin);
        
        // Set first policy
        client.set_policy(&account1, &asset1, &1000);
        
        // Set second policy
        client.set_policy(&account2, &asset2, &2000);
        
        let events = env.events().all();
        
        // Should have 3 events: initialize + 2 set_policy
        assert_eq!(events.len(), 3);
        
        // Verify first event is initialize
        let (_, topics1, _) = events.get(0).unwrap();
        let action1 = soroban_sdk::Symbol::from_val(&env, &topics1.get(1).unwrap());
        assert_eq!(action1, symbol_short!("init"));
        
        // Verify second event is set_policy
        let (_, topics2, _) = events.get(1).unwrap();
        let action2 = soroban_sdk::Symbol::from_val(&env, &topics2.get(1).unwrap());
        assert_eq!(action2, symbol_short!("lmt_set"));
        
        // Verify third event is set_policy
        let (_, topics3, _) = events.get(2).unwrap();
        let action3 = soroban_sdk::Symbol::from_val(&env, &topics3.get(1).unwrap());
        assert_eq!(action3, symbol_short!("lmt_set"));
    }

    #[test]
    fn test_check_spend_does_not_emit_event() {
        use soroban_sdk::testutils::Events;
        
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        
        client.set_policy(&account, &asset, &1000);
        
        // Clear events from setup and set_policy
        env.events().all();
        
        // check_spend should not emit events (read-only operation)
        client.check_spend(&account, &asset, &500);
        
        let events = env.events().all();
        assert_eq!(events.len(), 0);
    }
}
