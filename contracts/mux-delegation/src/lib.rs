/*!
 * mux-delegation: Delegate permission management for Mux Protocol.
 *
 * Allows an owner to grant or revoke scoped permissions to a delegate
 * address. Delegates act on behalf of owners only within the granted
 * permission set.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_dlg"), action), data);
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280;
const TTL_EXTEND_TO: u32 = 518_400;

/// Maximum permissions that can be granted to a single delegate.
const MAX_DELEGATE_PERMS: u32 = 64;

/// Maximum delegates an owner can register (storage griefing guard).
const MAX_DELEGATES_PER_OWNER: u32 = 128;

// ── Types ─────────────────────────────────────────────────────────────────────

// Issue #83: Store delegate permissions map.
// Key: (owner, delegate) tuple -> Vec<Symbol> of granted permissions.
#[contracttype]
pub enum DataKey {
    /// Maps (owner, delegate) -> Vec<Symbol> of granted permissions.
    DelegatePerms(Address, Address),
    /// Maps owner -> Vec<Address> of all delegates (for enumeration).
    OwnerDelegates(Address),
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxDelegationError {
    NotADelegate = 6001,
    TooManyPermissions = 6002,
    EmptyPermissions = 6003,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxDelegation;

#[contractimpl]
impl MuxDelegation {
    // Issue #81: Add grant_delegate function.
    /// Grant a set of permissions from `owner` to `delegate`.
    /// The owner must authorize this call. Overwrites any prior grant.
    pub fn grant_delegate(
        env: Env,
        owner: Address,
        delegate: Address,
        permissions: Vec<Symbol>,
    ) -> Result<(), MuxDelegationError> {
        owner.require_auth();

        if permissions.is_empty() {
            return Err(MuxDelegationError::EmptyPermissions);
        }
        if permissions.len() > MAX_DELEGATE_PERMS {
            return Err(MuxDelegationError::TooManyPermissions);
        }

        // Persist the permissions map (issue #83).
        env.storage().persistent().set(
            &DataKey::DelegatePerms(owner.clone(), delegate.clone()),
            &permissions,
        );

        // Track delegate in owner's delegate list.
        let mut delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerDelegates(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        if !delegates.contains(&delegate) {
            if delegates.len() >= MAX_DELEGATES_PER_OWNER {
                return Err(MuxDelegationError::TooManyDelegates);
            }
            delegates.push_back(delegate.clone());
            env.storage()
                .persistent()
                .set(&DataKey::OwnerDelegates(owner.clone()), &delegates);
        }

        Self::extend_ttl(&env);
        emit(&env, symbol_short!("dlg_grant"), (owner, delegate));
        Ok(())
    }

    // Issue #82: Add revoke_delegate function.
    /// Revoke all delegated permissions from `delegate` granted by `owner`.
    pub fn revoke_delegate(
        env: Env,
        owner: Address,
        delegate: Address,
    ) -> Result<(), MuxDelegationError> {
        owner.require_auth();

        let key = DataKey::DelegatePerms(owner.clone(), delegate.clone());
        if !env.storage().persistent().has(&key) {
            return Err(MuxDelegationError::NotADelegate);
        }

        env.storage().persistent().remove(&key);

        // Remove delegate from owner's delegate list.
        if let Some(mut delegates) = env
            .storage()
            .persistent()
            .get::<DataKey, Vec<Address>>(&DataKey::OwnerDelegates(owner.clone()))
        {
            if let Some(i) = delegates.iter().position(|a| a == delegate) {
                delegates.remove(i as u32);
            }
            env.storage()
                .persistent()
                .set(&DataKey::OwnerDelegates(owner.clone()), &delegates);
        }

        Self::extend_ttl(&env);
        emit(&env, symbol_short!("dlg_rev"), (owner, delegate));
        Ok(())
    }

    /// Return the permissions granted by `owner` to `delegate`.
    pub fn get_delegate_permissions(env: Env, owner: Address, delegate: Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::DelegatePerms(owner, delegate))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Check whether `delegate` holds a specific permission from `owner`.
    pub fn is_delegate(env: Env, owner: Address, delegate: Address, permission: Symbol) -> bool {
        let perms: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::DelegatePerms(owner, delegate))
            .unwrap_or_else(|| Vec::new(&env));
        perms.contains(&permission)
    }

    /// Return all delegates registered under `owner`.
    pub fn get_delegates(env: Env, owner: Address) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerDelegates(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    fn extend_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

// ── Tests (Issue #84) ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events},
        vec, Env,
    };

    fn setup() -> (Env, MuxDelegationClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &id);
        (env, client)
    }

    #[test]
    fn test_grant_delegate_stores_permissions() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm = symbol_short!("transfer");
        let perms = vec![&env, perm.clone()];

        client.grant_delegate(&owner, &delegate, &perms);

        let stored = client.get_delegate_permissions(&owner, &delegate);
        assert_eq!(stored.len(), 1);
        assert!(stored.contains(&perm));
    }

    #[test]
    fn test_is_delegate_returns_true_for_granted_permission() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm = symbol_short!("read");
        let perms = vec![&env, perm.clone()];

        client.grant_delegate(&owner, &delegate, &perms);

        assert!(client.is_delegate(&owner, &delegate, &perm));
        assert!(!client.is_delegate(&owner, &delegate, &symbol_short!("write")));
    }

    #[test]
    fn test_revoke_delegate_removes_permissions() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm = symbol_short!("swap");
        let perms = vec![&env, perm.clone()];

        client.grant_delegate(&owner, &delegate, &perms);
        assert!(client.is_delegate(&owner, &delegate, &perm));

        client.revoke_delegate(&owner, &delegate);
        assert!(!client.is_delegate(&owner, &delegate, &perm));
    }

    #[test]
    fn test_revoke_nonexistent_delegate_fails() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_revoke_delegate(&owner, &delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_grant_empty_permissions_fails() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_grant_delegate(&owner, &delegate, &Vec::new(&env));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_delegates_tracks_all_delegates() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate_a = Address::generate(&env);
        let delegate_b = Address::generate(&env);
        let perm = symbol_short!("vote");
        let perms = vec![&env, perm];

        client.grant_delegate(&owner, &delegate_a, &perms);
        client.grant_delegate(&owner, &delegate_b, &perms);

        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), 2);
        assert!(delegates.contains(&delegate_a));
        assert!(delegates.contains(&delegate_b));
    }

    #[test]
    fn test_revoke_removes_from_delegates_list() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perms = vec![&env, symbol_short!("trade")];

        client.grant_delegate(&owner, &delegate, &perms);
        assert_eq!(client.get_delegates(&owner).len(), 1);

        client.revoke_delegate(&owner, &delegate);
        assert_eq!(client.get_delegates(&owner).len(), 0);
    }

    #[test]
    fn test_grant_overwrites_prior_permissions() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm_a = symbol_short!("read");
        let perm_b = symbol_short!("write");

        client.grant_delegate(&owner, &delegate, &vec![&env, perm_a.clone()]);
        client.grant_delegate(&owner, &delegate, &vec![&env, perm_b.clone()]);

        let stored = client.get_delegate_permissions(&owner, &delegate);
        assert!(!stored.contains(&perm_a));
        assert!(stored.contains(&perm_b));
    }

    #[test]
    fn test_error_code_not_a_delegate() {
        assert_eq!(MuxDelegationError::NotADelegate as u32, 6001);
    }

    #[test]
    fn test_error_code_too_many_permissions() {
        assert_eq!(MuxDelegationError::TooManyPermissions as u32, 6002);
    }

    #[test]
    fn test_error_code_empty_permissions() {
        assert_eq!(MuxDelegationError::EmptyPermissions as u32, 6003);
    }

    #[test]
    fn test_grant_emits_event() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        client.grant_delegate(&owner, &delegate, &perms);

        let events = env.events().all();
        assert!(!events.is_empty());
    }

    #[test]
    fn test_revoke_emits_event() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        client.grant_delegate(&owner, &delegate, &perms);
        let before = env.events().all().len();

        client.revoke_delegate(&owner, &delegate);
        assert!(env.events().all().len() > before);
    }
}
