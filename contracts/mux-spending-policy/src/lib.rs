/*!
 * mux-spending-policy: Spending-policy enforcement contract for Mux Protocol.
 *
 * Stores per-account spend limits and validates spend requests against them.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

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
}
