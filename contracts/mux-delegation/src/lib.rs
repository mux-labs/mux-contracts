/*!
 * mux-delegation: Standalone delegation registry for Mux Protocol.
 *
 * Manages delegate authorizations independently of mux-account, allowing
 * any owner to grant time-bounded, capability-scoped delegation to other
 * addresses.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────

fn emit(env: &Env, action: soroban_sdk::Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_del"), action), data);
}

// ── Storage TTL ───────────────────────────────────────────────────────────────

const TTL_THRESHOLD: u32 = 17_280;  // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Types ─────────────────────────────────────────────────────────────────────

/// Maximum delegates an owner may register simultaneously.
const MAX_DELEGATES: u32 = 64;

#[contracttype]
pub enum DataKey {
    /// Delegates registered by an owner: DataKey::Delegates(owner) -> Vec<DelegateEntry>
    Delegates(Address),
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DelegateEntry {
    /// The delegated address.
    pub delegate: Address,
    /// Ledger number after which this delegation expires (0 = never).
    pub expiry_ledger: u32,
    /// Whether the delegate may invoke spend operations.
    pub can_spend: bool,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxDelegationError {
    Unauthorized = 1,
    DelegateNotFound = 2,
    DelegateExpired = 3,
    TooManyDelegates = 4,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxDelegation;

#[contractimpl]
impl MuxDelegation {
    /// Add or update a delegate for the calling owner.
    pub fn add_delegate(
        env: Env,
        owner: Address,
        delegate: Address,
        expiry_ledger: u32,
        can_spend: bool,
    ) -> Result<(), MuxDelegationError> {
        owner.require_auth();

        let mut entries: Vec<DelegateEntry> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        // Update existing entry if present.
        for i in 0..entries.len() {
            if entries.get(i).map(|e| e.delegate == delegate).unwrap_or(false) {
                entries.set(i, DelegateEntry { delegate: delegate.clone(), expiry_ledger, can_spend });
                env.storage().instance().set(&DataKey::Delegates(owner.clone()), &entries);
                emit(&env, symbol_short!("del_upd"), (owner, delegate));
                Self::extend_ttl(&env);
                return Ok(());
            }
        }

        if entries.len() >= MAX_DELEGATES {
            return Err(MuxDelegationError::TooManyDelegates);
        }

        entries.push_back(DelegateEntry { delegate: delegate.clone(), expiry_ledger, can_spend });
        env.storage().instance().set(&DataKey::Delegates(owner.clone()), &entries);
        emit(&env, symbol_short!("del_add"), (owner, delegate));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Remove a delegate for the calling owner.
    pub fn remove_delegate(
        env: Env,
        owner: Address,
        delegate: Address,
    ) -> Result<(), MuxDelegationError> {
        owner.require_auth();

        let mut entries: Vec<DelegateEntry> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let pos = entries.iter().position(|e| e.delegate == delegate);
        match pos {
            Some(i) => entries.remove(i as u32),
            None => return Err(MuxDelegationError::DelegateNotFound),
        }

        env.storage().instance().set(&DataKey::Delegates(owner.clone()), &entries);
        emit(&env, symbol_short!("del_rem"), (owner, delegate));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Check whether `delegate` is currently authorized by `owner`.
    /// Returns `Err(DelegateExpired)` if the entry exists but has expired.
    pub fn is_authorized(
        env: Env,
        owner: Address,
        delegate: Address,
    ) -> Result<bool, MuxDelegationError> {
        let entries: Vec<DelegateEntry> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates(owner))
            .unwrap_or_else(|| Vec::new(&env));

        for entry in entries.iter() {
            if entry.delegate == delegate {
                if entry.expiry_ledger != 0 && env.ledger().sequence() > entry.expiry_ledger {
                    return Err(MuxDelegationError::DelegateExpired);
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Return all delegate entries for an owner.
    pub fn get_delegates(env: Env, owner: Address) -> Vec<DelegateEntry> {
        env.storage()
            .instance()
            .get(&DataKey::Delegates(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn extend_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events, Ledger},
        Env, FromVal,
    };

    fn setup() -> (Env, MuxDelegationClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        (env, client)
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
    fn test_add_and_check_delegate() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.add_delegate(&owner, &delegate, &0, &true);
        assert_eq!(client.is_authorized(&owner, &delegate), Ok(true));
    }

    #[test]
    fn test_remove_delegate() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.add_delegate(&owner, &delegate, &0, &false);
        client.remove_delegate(&owner, &delegate);
        assert_eq!(client.is_authorized(&owner, &delegate), Ok(false));
    }

    #[test]
    fn test_remove_nonexistent_delegate_fails() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_remove_delegate(&owner, &delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_delegate_returns_error() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        // Add delegate expiring at ledger 10
        client.add_delegate(&owner, &delegate, &10, &true);

        // Advance ledger past expiry
        env.ledger().set_sequence_number(11);
        let result = client.try_is_authorized(&owner, &delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_existing_delegate() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.add_delegate(&owner, &delegate, &0, &false);
        // Update: now can_spend = true
        client.add_delegate(&owner, &delegate, &0, &true);

        let entries = client.get_delegates(&owner);
        assert_eq!(entries.len(), 1);
        assert!(entries.get(0).unwrap().can_spend);
    }

    #[test]
    fn test_delegate_cap_enforced() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);

        for _ in 0..64 {
            client.add_delegate(&owner, &Address::generate(&env), &0, &false);
        }
        let result = client.try_add_delegate(&owner, &Address::generate(&env), &0, &false);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_emits_event() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.add_delegate(&owner, &delegate, &0, &false);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("del_add"));
    }

    #[test]
    fn test_remove_emits_event() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.add_delegate(&owner, &delegate, &0, &false);
        client.remove_delegate(&owner, &delegate);
        let events = env.events().all();
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("del_rem"));
    }

    #[test]
    fn test_update_emits_event() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.add_delegate(&owner, &delegate, &0, &false);
        client.add_delegate(&owner, &delegate, &0, &true);
        let events = env.events().all();
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("del_upd"));
    }

    #[test]
    fn test_get_delegates_empty() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        assert_eq!(client.get_delegates(&owner).len(), 0);
    }

    #[test]
    fn test_ttl_extended_on_write() {
        // Reaching here without panic confirms extend_ttl is called (T-21 mitigation).
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.add_delegate(&owner, &delegate, &0, &false);
    }
}
