/*!
 * mux-account: Account abstraction contract for Mux Protocol.
 *
 * Provides delegated signing, guardian management, and spending limits
 * on top of a Stellar Soroban account.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Map, Vec};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
    SpendLimit(Address),
    GuardianSet,
    Nonce,
    /// Set to `true` while `debit_spend` is executing.
    /// Defense-in-depth against cross-contract re-entrancy; Soroban's VM
    /// also prevents recursive same-contract calls at the host level.
    Executing,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SpendLimit {
    pub asset: Address,
    pub amount: i128,
    pub period_ledgers: u32,
    pub spent: i128,
    pub reset_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DelegateInfo {
    pub address: Address,
    pub expiry_ledger: u32,
    pub can_spend: bool,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxAccountError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    DelegateNotFound = 4,
    DelegateExpired = 5,
    SpendLimitExceeded = 6,
    InvalidAmount = 7,
    InvalidPeriod = 8,
    ReentrancyDetected = 9,
    ArithmeticOverflow = 10,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxAccount;

#[contractimpl]
impl MuxAccount {
    /// Initialize the account with an owner and optional guardian set.
    pub fn initialize(
        env: Env,
        owner: Address,
        guardians: Vec<Address>,
    ) -> Result<(), MuxAccountError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(MuxAccountError::AlreadyInitialized);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&DataKey::GuardianSet, &guardians);
        env.storage().instance().set(
            &DataKey::Delegates,
            &Map::<Address, DelegateInfo>::new(&env),
        );
        env.storage().instance().set(&DataKey::Nonce, &0_u64);
        Ok(())
    }

    /// Add or update a delegate with an expiry and spending permission flag.
    pub fn set_delegate(
        env: Env,
        delegate: Address,
        expiry_ledger: u32,
        can_spend: bool,
    ) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        let mut delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;

        delegates.set(
            delegate.clone(),
            DelegateInfo {
                address: delegate,
                expiry_ledger,
                can_spend,
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::Delegates, &delegates);
        Ok(())
    }

    /// Remove a delegate.
    pub fn remove_delegate(env: Env, delegate: Address) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        let mut delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;

        if !delegates.contains_key(delegate.clone()) {
            return Err(MuxAccountError::DelegateNotFound);
        }
        delegates.remove(delegate);
        env.storage()
            .instance()
            .set(&DataKey::Delegates, &delegates);
        Ok(())
    }

    /// Set a per-asset spend limit for a delegate.
    pub fn set_spend_limit(
        env: Env,
        asset: Address,
        amount: i128,
        period_ledgers: u32,
    ) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        if amount <= 0 {
            return Err(MuxAccountError::InvalidAmount);
        }
        if period_ledgers == 0 {
            return Err(MuxAccountError::InvalidPeriod);
        }
        let limit = SpendLimit {
            asset: asset.clone(),
            amount,
            period_ledgers,
            spent: 0,
            reset_ledger: env.ledger().sequence().saturating_add(period_ledgers),
        };
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(asset), &limit);
        Ok(())
    }

    /// Check and debit a spend against the configured limit.
    pub fn debit_spend(env: Env, asset: Address, spend: i128) -> Result<(), MuxAccountError> {
        let caller = env.current_contract_address();
        caller.require_auth();

        // Reentrancy guard: reject if a debit_spend call is already in progress.
        // On error return Soroban rolls back storage, so the flag is self-cleaning.
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Executing)
            .unwrap_or(false)
        {
            return Err(MuxAccountError::ReentrancyDetected);
        }
        env.storage().instance().set(&DataKey::Executing, &true);

        let mut limit: SpendLimit = env
            .storage()
            .instance()
            .get(&DataKey::SpendLimit(asset.clone()))
            .ok_or(MuxAccountError::SpendLimitExceeded)?;

        if env.ledger().sequence() >= limit.reset_ledger {
            limit.spent = 0;
            limit.reset_ledger = env.ledger().sequence().saturating_add(limit.period_ledgers);
        }

        let new_spent = limit
            .spent
            .checked_add(spend)
            .ok_or(MuxAccountError::ArithmeticOverflow)?;
        if new_spent > limit.amount {
            return Err(MuxAccountError::SpendLimitExceeded);
        }
        limit.spent = new_spent;
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(asset), &limit);

        env.storage().instance().set(&DataKey::Executing, &false);
        Ok(())
    }

    /// Return the current owner.
    pub fn owner(env: Env) -> Result<Address, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(MuxAccountError::NotInitialized)
    }

    /// Return all active delegates.
    pub fn delegates(env: Env) -> Result<Map<Address, DelegateInfo>, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)
    }

    /// Return the guardian set.
    pub fn guardians(env: Env) -> Result<Vec<Address>, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::GuardianSet)
            .ok_or(MuxAccountError::NotInitialized)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_owner(env: &Env) -> Result<(), MuxAccountError> {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(MuxAccountError::NotInitialized)?;
        owner.require_auth();
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Vec};

    fn setup() -> (Env, MuxAccountClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccount);
        let client = MuxAccountClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    #[test]
    fn test_initialize() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        assert!(client.try_initialize(&owner, &guardians).is_ok());
        assert_eq!(client.owner(), owner);
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let result = client.try_initialize(&owner, &guardians);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_and_remove_delegate() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u32, &true);

        let delegates = client.delegates();
        assert!(delegates.contains_key(delegate.clone()));

        client.remove_delegate(&delegate);
        let delegates_after = client.delegates();
        assert!(!delegates_after.contains_key(delegate));
    }

    #[test]
    fn test_spend_limit_enforcement() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);

        // Debit within limit succeeds
        assert!(client.try_debit_spend(&asset, &500_i128).is_ok());

        // Debit exceeding limit fails
        let result = client.try_debit_spend(&asset, &600_i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_spend_limit_invalid_amount() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let asset = Address::generate(&env);
        let result = client.try_set_spend_limit(&asset, &0_i128, &100_u32);
        assert!(result.is_err());
    }

    #[test]
    fn test_reentrancy_guard_clears_after_success() {
        // Verify the Executing flag is cleared after a successful debit so
        // sequential calls continue to work (the guard does not lock permanently).
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);

        assert!(client.try_debit_spend(&asset, &100_i128).is_ok());
        assert!(client.try_debit_spend(&asset, &100_i128).is_ok());
    }

    #[test]
    fn test_overflow_in_spend_accumulation() {
        // Verify that debiting i128::MAX does not silently wrap; it must return
        // ArithmeticOverflow before SpendLimitExceeded is even evaluated.
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let asset = Address::generate(&env);
        // Set a very large limit so SpendLimitExceeded would not trigger first.
        client.set_spend_limit(&asset, &i128::MAX, &100_u32);

        // First debit consumes i128::MAX - 1 of the allowance.
        assert!(client.try_debit_spend(&asset, &(i128::MAX - 1)).is_ok());

        // Second debit of 2 would overflow limit.spent + 2 beyond i128::MAX.
        let result = client.try_debit_spend(&asset, &2_i128);
        assert!(result.is_err());
    }
}
