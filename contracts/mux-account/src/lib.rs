/*!
 * mux-account: Account abstraction contract for Mux Protocol.
 *
 * Provides delegated signing, guardian management, and spending limits
 * on top of a Stellar Soroban account.
 */

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short, Address, Env, Map, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
// All state-mutating operations publish a structured event:
//   topics: [contract_name, action]
//   data:   action-specific payload (see docs/audit-events.md)

fn emit(env: &Env, action: soroban_sdk::Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events().publish((symbol_short!("mux_acct"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
    SpendLimit(Address),
    GuardianSet,
    Nonce,
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
        env.storage().instance().set(&DataKey::GuardianSet, &guardians);
        env.storage().instance().set(&DataKey::Delegates, &Map::<Address, DelegateInfo>::new(&env));
        env.storage().instance().set(&DataKey::Nonce, &0_u64);
        emit(&env, symbol_short!("init"), owner);
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
            DelegateInfo { address: delegate.clone(), expiry_ledger, can_spend },
        );
        env.storage().instance().set(&DataKey::Delegates, &delegates);
        emit(&env, symbol_short!("dlg_set"), (delegate, expiry_ledger, can_spend));
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
        delegates.remove(delegate.clone());
        env.storage().instance().set(&DataKey::Delegates, &delegates);
        emit(&env, symbol_short!("dlg_rm"), delegate);
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
            reset_ledger: env.ledger().sequence() + period_ledgers,
        };
        env.storage().instance().set(&DataKey::SpendLimit(asset.clone()), &limit);
        emit(&env, symbol_short!("lmt_set"), (asset, amount, period_ledgers));
        Ok(())
    }

    /// Check and debit a spend against the configured limit.
    pub fn debit_spend(env: Env, asset: Address, spend: i128) -> Result<(), MuxAccountError> {
        let caller = env.current_contract_address();
        caller.require_auth();

        let mut limit: SpendLimit = env
            .storage()
            .instance()
            .get(&DataKey::SpendLimit(asset.clone()))
            .ok_or(MuxAccountError::SpendLimitExceeded)?;

        if env.ledger().sequence() >= limit.reset_ledger {
            limit.spent = 0;
            limit.reset_ledger = env.ledger().sequence() + limit.period_ledgers;
        }

        let new_spent = limit.spent + spend;
        if new_spent > limit.amount {
            return Err(MuxAccountError::SpendLimitExceeded);
        }
        limit.spent = new_spent;
        env.storage().instance().set(&DataKey::SpendLimit(asset.clone()), &limit);
        emit(&env, symbol_short!("debited"), (asset, spend));
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
    use soroban_sdk::{testutils::{Address as _, Events}, symbol_short, FromVal, Env, Vec};

    fn topic_action(env: &Env, events: &soroban_sdk::Vec<(soroban_sdk::Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val)>, idx: u32) -> soroban_sdk::Symbol {
        let (_, topics, _) = events.get(idx).unwrap();
        soroban_sdk::Symbol::from_val(env, &topics.get(1).unwrap())
    }

    fn setup() -> (Env, MuxAccountClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccount);
        let client = MuxAccountClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    #[test]
    fn test_initialize_emits_event() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_set_delegate_emits_event() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u32, &true);
        let events = env.events().all();
        // init + dlg_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("dlg_set"));
    }

    #[test]
    fn test_remove_delegate_emits_event() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u32, &false);
        client.remove_delegate(&delegate);
        let events = env.events().all();
        // init + dlg_set + dlg_rm
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("dlg_rm"));
    }

    #[test]
    fn test_spend_limit_emits_events() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);
        client.try_debit_spend(&asset, &200_i128).unwrap();
        let events = env.events().all();
        // init + lmt_set + debited
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("lmt_set"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("debited"));
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
}
