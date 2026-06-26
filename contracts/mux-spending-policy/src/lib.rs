/*!
 * mux-spending-policy: Spending-policy enforcement contract for Mux Protocol.
 *
 * Stores per-account spend limits and validates spend requests against them.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    /// SpendLimit(account, asset) -> SpendLimit
    SpendLimit(Address, Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SpendLimit {
    pub asset: Address,
    pub limit: i128,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SpendingPolicyError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    PolicyNotFound = 4,
    SpendLimitExceeded = 5,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxSpendingPolicy;

#[contractimpl]
impl MuxSpendingPolicy {
    pub fn initialize(env: Env, admin: Address) -> Result<(), SpendingPolicyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(SpendingPolicyError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Set a spend limit for an account/asset pair. Admin only.
    pub fn set_policy(
        env: Env,
        account: Address,
        asset: Address,
        limit: i128,
    ) -> Result<(), SpendingPolicyError> {
        Self::require_admin(&env)?;
        let policy = SpendLimit { asset: asset.clone(), limit };
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(account, asset), &policy);
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
    /// Returns Ok(()) if allowed, Err(SpendLimitExceeded) if over limit,
    /// or Err(PolicyNotFound) if no policy is set.
    pub fn check_spend(
        env: Env,
        account: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), SpendingPolicyError> {
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
    fn test_spend_limit_exceeded_error_code() {
        // Verify SpendLimitExceeded has the expected discriminant value (5)
        assert_eq!(SpendingPolicyError::SpendLimitExceeded as u32, 5);
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
}
