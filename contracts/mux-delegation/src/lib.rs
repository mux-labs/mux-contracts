/*!
 * mux-delegation: Delegation system for Mux Protocol.
 *
 * Implements a delegation mechanism that allows accounts to delegate
 * specific permissions or voting power to other accounts, with proper
 * event emission for transparency and auditability.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_deleg"), action), data);
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Delegation(Address, Address), // (delegator, delegate)
    DelegatorDelegates(Address),  // delegator -> Vec<Address>
    DelegateDelegators(Address),  // delegate -> Vec<Address>
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DelegationInfo {
    pub delegator: Address,
    pub delegate: Address,
    pub permissions: Vec<Symbol>,
    pub timestamp: u64,
    pub active: bool,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxDelegationError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    DelegationNotFound = 4,
    SelfDelegation = 5,
    DelegationAlreadyExists = 6,
    InvalidPermission = 7,
    TooManyDelegations = 8,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum delegations per account to prevent storage griefing
const MAX_DELEGATIONS_PER_ACCOUNT: u32 = 64;

// ── Storage TTL ───────────────────────────────────────────────────────────────
// Extend instance TTL on every write to keep the delegation system active
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxDelegation;

#[contractimpl]
impl MuxDelegation {
    /// Initialize the delegation contract with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxDelegationError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxDelegationError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Grant delegation from delegator to delegate with specific permissions.
    pub fn grant_delegation(
        env: Env,
        delegator: Address,
        delegate: Address,
        permissions: Vec<Symbol>,
    ) -> Result<(), MuxDelegationError> {
        delegator.require_auth();

        // Prevent self-delegation
        if delegator == delegate {
            return Err(MuxDelegationError::SelfDelegation);
        }

        // Check if delegation already exists
        if env
            .storage()
            .instance()
            .has(&DataKey::Delegation(delegator.clone(), delegate.clone()))
        {
            return Err(MuxDelegationError::DelegationAlreadyExists);
        }

        // Check delegation limits for delegator
        let delegator_delegates: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::DelegatorDelegates(delegator.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        if delegator_delegates.len() >= MAX_DELEGATIONS_PER_ACCOUNT {
            return Err(MuxDelegationError::TooManyDelegations);
        }

        // Create delegation info
        let delegation = DelegationInfo {
            delegator: delegator.clone(),
            delegate: delegate.clone(),
            permissions: permissions.clone(),
            timestamp: env.ledger().timestamp(),
            active: true,
        };

        // Store delegation
        env.storage().instance().set(
            &DataKey::Delegation(delegator.clone(), delegate.clone()),
            &delegation,
        );

        // Update delegator's delegates list
        let mut updated_delegates = delegator_delegates;
        updated_delegates.push_back(delegate.clone());
        env.storage().instance().set(
            &DataKey::DelegatorDelegates(delegator.clone()),
            &updated_delegates,
        );

        // Update delegate's delegators list
        let mut delegate_delegators: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::DelegateDelegators(delegate.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        delegate_delegators.push_back(delegator.clone());
        env.storage().instance().set(
            &DataKey::DelegateDelegators(delegate.clone()),
            &delegate_delegators,
        );

        // Emit delegate_granted event as required
        emit(
            &env,
            symbol_short!("del_grant"),
            (delegator.clone(), delegate.clone(), permissions),
        );

        Self::extend_ttl(&env);
        Ok(())
    }

    /// Revoke delegation from delegator to delegate.
    pub fn revoke_delegation(
        env: Env,
        delegator: Address,
        delegate: Address,
    ) -> Result<(), MuxDelegationError> {
        delegator.require_auth();

        // Check if delegation exists
        let mut delegation: DelegationInfo = env
            .storage()
            .instance()
            .get(&DataKey::Delegation(delegator.clone(), delegate.clone()))
            .ok_or(MuxDelegationError::DelegationNotFound)?;

        // Mark as inactive
        delegation.active = false;
        env.storage().instance().set(
            &DataKey::Delegation(delegator.clone(), delegate.clone()),
            &delegation,
        );

        // Remove from delegator's delegates list
        let mut delegator_delegates: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::DelegatorDelegates(delegator.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        if let Some(pos) = delegator_delegates.iter().position(|addr| addr == delegate) {
            delegator_delegates.remove(pos as u32);
            env.storage().instance().set(
                &DataKey::DelegatorDelegates(delegator.clone()),
                &delegator_delegates,
            );
        }

        // Remove from delegate's delegators list
        let mut delegate_delegators: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::DelegateDelegators(delegate.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        if let Some(pos) = delegate_delegators.iter().position(|addr| addr == delegator) {
            delegate_delegators.remove(pos as u32);
            env.storage().instance().set(
                &DataKey::DelegateDelegators(delegate.clone()),
                &delegate_delegators,
            );
        }

        emit(&env, symbol_short!("del_revok"), (delegator, delegate));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Get delegation information between delegator and delegate.
    pub fn get_delegation(
        env: Env,
        delegator: Address,
        delegate: Address,
    ) -> Result<DelegationInfo, MuxDelegationError> {
        env.storage()
            .instance()
            .get(&DataKey::Delegation(delegator, delegate))
            .ok_or(MuxDelegationError::DelegationNotFound)
    }

    /// Get all delegates for a delegator.
    pub fn get_delegates(env: Env, delegator: Address) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::DelegatorDelegates(delegator))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get all delegators for a delegate.
    pub fn get_delegators(env: Env, delegate: Address) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::DelegateDelegators(delegate))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Check if a delegation exists and is active.
    pub fn has_delegation(env: Env, delegator: Address, delegate: Address) -> bool {
        if let Some(delegation) = env
            .storage()
            .instance()
            .get::<DataKey, DelegationInfo>(&DataKey::Delegation(delegator, delegate))
        {
            delegation.active
        } else {
            false
        }
    }

    /// Check if delegate has specific permission from delegator.
    pub fn has_delegated_permission(
        env: Env,
        delegator: Address,
        delegate: Address,
        permission: Symbol,
    ) -> bool {
        if let Some(delegation) = env
            .storage()
            .instance()
            .get::<DataKey, DelegationInfo>(&DataKey::Delegation(delegator, delegate))
        {
            delegation.active && delegation.permissions.contains(&permission)
        } else {
            false
        }
    }

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Result<Address, MuxDelegationError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxDelegationError::NotInitialized)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxDelegationError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxDelegationError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    /// Extend instance-storage TTL on every write to prevent silent data loss.
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
        testutils::{Address as _, Events},
        Env, FromVal, Vec,
    };

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

    fn setup() -> (Env, MuxDelegationClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
    }

    #[test]
    fn test_initialize_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, _admin) = setup();
        let other = Address::generate(&env);
        assert!(client.try_initialize(&other).is_err());
    }

    #[test]
    fn test_grant_delegation() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let permission = symbol_short!("vote");
        let mut permissions: Vec<Symbol> = Vec::new(&env);
        permissions.push_back(permission.clone());

        client.grant_delegation(&delegator, &delegate, &permissions);

        let delegation = client.get_delegation(&delegator, &delegate);
        assert_eq!(delegation.delegator, delegator);
        assert_eq!(delegation.delegate, delegate);
        assert!(delegation.active);
        assert!(delegation.permissions.contains(&permission));
    }

    #[test]
    fn test_grant_delegation_emits_delegate_granted_event() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let permission = symbol_short!("vote");
        let mut permissions: Vec<Symbol> = Vec::new(&env);
        permissions.push_back(permission);

        client.grant_delegation(&delegator, &delegate, &permissions);

        let events = env.events().all();
        // init + del_grant
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("del_grant"));
    }

    #[test]
    fn test_self_delegation_fails() {
        let (env, client, _admin) = setup();
        let account = Address::generate(&env);
        let permissions: Vec<Symbol> = Vec::new(&env);

        let result = client.try_grant_delegation(&account, &account, &permissions);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_delegation_fails() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let permissions: Vec<Symbol> = Vec::new(&env);

        client.grant_delegation(&delegator, &delegate, &permissions);
        let result = client.try_grant_delegation(&delegator, &delegate, &permissions);
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_delegation() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let permission = symbol_short!("vote");
        let mut permissions: Vec<Symbol> = Vec::new(&env);
        permissions.push_back(permission.clone());

        client.grant_delegation(&delegator, &delegate, &permissions);
        assert!(client.has_delegation(&delegator, &delegate));

        client.revoke_delegation(&delegator, &delegate);
        assert!(!client.has_delegation(&delegator, &delegate));
    }

    #[test]
    fn test_revoke_delegation_emits_event() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let permissions: Vec<Symbol> = Vec::new(&env);

        client.grant_delegation(&delegator, &delegate, &permissions);
        client.revoke_delegation(&delegator, &delegate);

        let events = env.events().all();
        // init + del_grant + del_revok
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("del_revok"));
    }

    #[test]
    fn test_revoke_nonexistent_delegation_fails() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_revoke_delegation(&delegator, &delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_delegated_permission() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let vote_perm = symbol_short!("vote");
        let admin_perm = symbol_short!("admin");
        let mut permissions: Vec<Symbol> = Vec::new(&env);
        permissions.push_back(vote_perm.clone());

        client.grant_delegation(&delegator, &delegate, &permissions);

        assert!(client.has_delegated_permission(&delegator, &delegate, &vote_perm));
        assert!(!client.has_delegated_permission(&delegator, &delegate, &admin_perm));
    }

    #[test]
    fn test_get_delegates_and_delegators() {
        let (env, client, _admin) = setup();
        let delegator = Address::generate(&env);
        let delegate1 = Address::generate(&env);
        let delegate2 = Address::generate(&env);
        let permissions: Vec<Symbol> = Vec::new(&env);

        client.grant_delegation(&delegator, &delegate1, &permissions);
        client.grant_delegation(&delegator, &delegate2, &permissions);

        let delegates = client.get_delegates(&delegator);
        assert_eq!(delegates.len(), 2);
        assert!(delegates.contains(&delegate1));
        assert!(delegates.contains(&delegate2));

        let delegators1 = client.get_delegators(&delegate1);
        assert_eq!(delegators1.len(), 1);
        assert!(delegators1.contains(&delegator));
    }

    #[test]
    fn test_delegation_limit_enforced() {
        let (env, client, _admin) = setup();
        env.budget().reset_unlimited();
        let delegator = Address::generate(&env);
        let permissions: Vec<Symbol> = Vec::new(&env);

        // Grant maximum allowed delegations
        for _ in 0..64 {
            let delegate = Address::generate(&env);
            client.grant_delegation(&delegator, &delegate, &permissions);
        }

        // Try to grant one more - should fail
        let overflow_delegate = Address::generate(&env);
        let result = client.try_grant_delegation(&delegator, &overflow_delegate, &permissions);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_admin() {
        let (_env, client, admin) = setup();
        let retrieved_admin = client.get_admin();
        assert_eq!(retrieved_admin, admin);
    }
}