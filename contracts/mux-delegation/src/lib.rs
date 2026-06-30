/*!
 * mux-delegation: Delegate permission management for Mux Protocol.
 *
 * Allows an owner to grant or revoke scoped permissions to a delegate
 * address. Delegates act on behalf of owners only within the granted
 * permission set.
 *
 * Each owner may register up to 128 delegates. Each delegate may hold up to
 * 64 permissions. All state-mutating operations require owner authorization
 * and emit an audit event under the `mux_dlg` contract tag.
 *
 * Error codes 6001–6004 are stable ABI — coordinate changes with a registry
 * version bump.
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
    /// No grant exists for the given (owner, delegate) pair.
    NotADelegate = 6001,
    /// The permission list exceeds the 64-entry cap enforced at grant time.
    TooManyPermissions = 6002,
    /// The permission list is empty; at least one permission must be specified.
    EmptyPermissions = 6003,
    /// The owner already has 128 delegates registered (storage-griefing guard).
    TooManyDelegates = 6004,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxDelegation;

#[contractimpl]
impl MuxDelegation {
    /// Grant `permissions` from `owner` to `delegate`. Requires `owner` auth.
    ///
    /// If a prior grant exists for the same `(owner, delegate)` pair it is
    /// fully replaced — there is no append mode. Emits `dlg_grant` on success.
    ///
    /// # Errors
    /// - [`MuxDelegationError::EmptyPermissions`] — `permissions` is empty.
    /// - [`MuxDelegationError::TooManyPermissions`] — more than 64 entries.
    /// - [`MuxDelegationError::TooManyDelegates`] — owner already has 128 delegates.
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

    /// Revoke all permissions granted by `owner` to `delegate`. Requires `owner` auth.
    ///
    /// Removes the permission set and removes the delegate from the owner's
    /// delegate list. Emits `dlg_rev` on success.
    ///
    /// # Errors
    /// - [`MuxDelegationError::NotADelegate`] — no grant exists for the pair.
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
    ///
    /// Returns an empty list if no grant exists for the pair.
    pub fn get_delegate_permissions(env: Env, owner: Address, delegate: Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::DelegatePerms(owner, delegate))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return `true` if `owner` has granted `permission` to `delegate`.
    pub fn is_delegate(env: Env, owner: Address, delegate: Address, permission: Symbol) -> bool {
        let perms: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::DelegatePerms(owner, delegate))
            .unwrap_or_else(|| Vec::new(&env));
        perms.contains(&permission)
    }

    /// Return all delegates registered under `owner`, or an empty list if none.
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
        vec, Env, FromVal,
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
    fn test_grant_too_many_permissions_fails() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let mut perms: Vec<Symbol> = Vec::new(&env);
        for _ in 0..=MAX_DELEGATE_PERMS {
            perms.push_back(symbol_short!("x"));
        }
        let result = client.try_grant_delegate(&owner, &delegate, &perms);
        assert!(result.is_err());
    }

    #[test]
    fn test_grant_too_many_delegates_fails() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];
        for _ in 0..MAX_DELEGATES_PER_OWNER {
            client.grant_delegate(&owner, &Address::generate(&env), &perms);
        }
        let result = client.try_grant_delegate(&owner, &Address::generate(&env), &perms);
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
    fn test_error_code_too_many_delegates() {
        assert_eq!(MuxDelegationError::TooManyDelegates as u32, 6004);
    }

    #[test]
    fn test_grant_emits_event() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        client.grant_delegate(&owner, &delegate, &perms);

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("dlg_grant"));
    }

    #[test]
    fn test_revoke_emits_event() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        client.grant_delegate(&owner, &delegate, &perms);
        client.revoke_delegate(&owner, &delegate);

        let events = env.events().all();
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("dlg_rev"));
    }

    // ── Delegate count cap (#252) ─────────────────────────────────────────────

    #[test]
    fn test_too_many_delegates_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        env.budget().reset_unlimited();
        let id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &id);
        let owner = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        for _ in 0..MAX_DELEGATES_PER_OWNER {
            client.grant_delegate(&owner, &Address::generate(&env), &perms);
        }

        let result = client.try_grant_delegate(&owner, &Address::generate(&env), &perms);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_code_too_many_delegates() {
        assert_eq!(MuxDelegationError::TooManyDelegates as u32, 6004);
    }
}
