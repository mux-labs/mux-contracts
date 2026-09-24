/*!
 * mux-delegation: Scoped delegate permission management for Mux Protocol.
 *
 * An *owner* (any Soroban [`Address`]) can grant a named set of *permissions*
 * to a *delegate* address. Delegates act on behalf of the owner **only within
 * the granted permission set** — they cannot self-escalate or sub-grant.
 *
 * Each owner may register up to [`MAX_DELEGATES_PER_OWNER`] (128) delegates.
 * Each delegate may hold up to [`MAX_DELEGATE_PERMS`] (64) permissions. All
 * state-mutating operations require owner authorization and emit an audit
 * event under the `mux_dlg` contract tag.
 *
 * # Permission Model
 *
 * Permissions are opaque [`Symbol`] values chosen by the application layer
 * (e.g. `"transfer"`, `"read"`, `"swap"`). Calling `grant_delegate` with a
 * new permission list **replaces** any prior grant for the same
 * `(owner, delegate)` pair — there is no append mode.
 *
 * See [`docs/delegation-permission-model.md`] for the full security model,
 * storage layout, bounds rationale, and TypeScript binding notes.
 *
 * # Public Interface
 *
 * | Entrypoint                  | Description                                                   |
 * |-----------------------------|---------------------------------------------------------------|
 * | `grant_delegate`            | Grant a set of permissions from owner to delegate (owner auth required). |
 * | `revoke_delegate`           | Revoke all permissions for an (owner, delegate) pair (owner auth required). |
 * | `get_delegate_permissions`  | Return the `Vec<Symbol>` of permissions granted to a delegate. |
 * | `is_delegate`               | Return `true` if owner has granted a specific permission to delegate. |
 * | `get_delegates`             | Return all delegate addresses registered under an owner.     |
 * | `check_delegate`            | Read-only convenience check: `Ok(())` if permission is granted, `Err(NotADelegate)` otherwise. |
 * | `link_contract_id`          | Store the on-chain address of this delegation contract in instance storage for registry discoverability (admin auth required). |
 * | `get_contract_id`           | Return the linked contract address, or `None` if not yet set. |
 *
 * # Storage Layout
 *
 * | Key                              | Value          | TTL        |
 * |----------------------------------|----------------|------------|
 * | `DelegatePerms(owner, delegate)` | `Vec<Symbol>`  | Persistent |
 * | `OwnerDelegates(owner)`          | `Vec<Address>` | Persistent |
 * | `ContractId`                     | `Address`      | Instance   |
 *
 * # Bounds
 *
 * - [`MAX_DELEGATE_PERMS`] = 64 — maximum permissions per `(owner, delegate)` pair.
 * - [`MAX_DELEGATES_PER_OWNER`] = 128 — maximum delegate addresses per owner
 *   (storage-griefing guard).
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types backed by the Soroban host.
 *
 * # Error Codes (Stable ABI)
 *
 * Error codes 6001–6004 are stable ABI — coordinate changes with a registry
 * version bump.
 *
 * | Code | Variant              | Description                                         |
 * |------|----------------------|-----------------------------------------------------|
 * | 6001 | `NotADelegate`       | No grant exists for the `(owner, delegate)` pair    |
 * | 6002 | `TooManyPermissions` | `permissions` list exceeds the 64-entry cap         |
 * | 6003 | `EmptyPermissions`   | `permissions` is empty; at least one required       |
 * | 6004 | `TooManyDelegates`   | Owner already has 128 delegates registered          |
 *
 * [`docs/delegation-permission-model.md`]: ../../docs/delegation-permission-model.md
 * [`docs/audit-events.md`]: ../../docs/audit-events.md
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
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
    /// Stores the on-chain Address of this delegation contract instance
    /// for registry discoverability (see `link_contract_id`).
    ContractId,
    /// Upgrade authority, set once by `initialize`. Independent of the
    /// caller-supplied `admin` parameter accepted by `link_contract_id` —
    /// see `docs/delegation-upgrade.md` for why the two are not unified.
    Admin,
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
    /// A contract address has already been linked; `link_contract_id` is write-once.
    ContractIdAlreadySet = 6005,
    /// `upgrade` was called before `initialize` — there is no admin to authorise it.
    NotInitialized = 6006,
    /// `initialize` was called more than once.
    AlreadyInitialized = 6007,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxDelegation;

#[contractimpl]
impl MuxDelegation {
    /// Initialize the contract with an upgrade admin. Optional: a delegation
    /// contract that is never initialized behaves exactly as before this
    /// admin was introduced — `grant_delegate` / `revoke_delegate` /
    /// `link_contract_id` are unaffected. Only `upgrade()` requires this to
    /// have been called first.
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

    /// Upgrade the contract WASM. Admin only.
    ///
    /// See `docs/delegation-upgrade.md` for storage-compatibility rules that
    /// must be observed between versions. Requires `initialize` to have been
    /// called first; returns `NotInitialized` otherwise (fail-closed — there
    /// is no admin to authorise the replace).
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxDelegationError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Self::extend_ttl(&env);
        Ok(())
    }

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
        let perms_key = DataKey::DelegatePerms(owner.clone(), delegate.clone());
        env.storage().persistent().set(
            &perms_key,
            &permissions,
        );
        // Extend per-entry TTL so this DelegatePerms record stays live
        // independently of the contract instance TTL (closes #407).
        Self::extend_entry_ttl(&env, &perms_key);

        // Track delegate in owner's delegate list.
        let delegates_key = DataKey::OwnerDelegates(owner.clone());
        let mut delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&delegates_key)
            .unwrap_or_else(|| Vec::new(&env));
        if !delegates.contains(&delegate) {
            if delegates.len() >= MAX_DELEGATES_PER_OWNER {
                return Err(MuxDelegationError::TooManyDelegates);
            }
            delegates.push_back(delegate.clone());
            env.storage()
                .persistent()
                .set(&delegates_key, &delegates);
            // Extend per-entry TTL for the owner delegate list as well.
            Self::extend_entry_ttl(&env, &delegates_key);
        } else {
            // Refresh TTL even when the delegate is already tracked (re-grant).
            Self::extend_entry_ttl(&env, &delegates_key);
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
        let delegates_key = DataKey::OwnerDelegates(owner.clone());
        if let Some(mut delegates) = env
            .storage()
            .persistent()
            .get::<DataKey, Vec<Address>>(&delegates_key)
        {
            if let Some(i) = delegates.iter().position(|a| a == delegate) {
                delegates.remove(i as u32);
            }
            env.storage()
                .persistent()
                .set(&delegates_key, &delegates);
            // Refresh per-entry TTL after mutation (closes #407).
            Self::extend_entry_ttl(&env, &delegates_key);
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

    /// Returns Ok(()) if owner has granted permission to delegate, Err(NotADelegate) otherwise.
    ///
    /// This is a read-only convenience check. No authentication is required and
    /// no state is mutated. Callers that only need a boolean can use `is_delegate`
    /// instead; `check_delegate` is useful when an error value is needed for
    /// chained authorization checks.
    pub fn check_delegate(
        env: Env,
        owner: Address,
        delegate: Address,
        permission: Symbol,
    ) -> Result<(), MuxDelegationError> {
        let perms: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::DelegatePerms(owner, delegate))
            .unwrap_or_else(|| Vec::new(&env));
        if perms.contains(&permission) {
            Ok(())
        } else {
            Err(MuxDelegationError::NotADelegate)
        }
    }

    /// Store the on-chain address of this delegation contract instance in
    /// instance storage so off-chain indexers, the mux-registry, and
    /// TypeScript clients can discover it without an external config file.
    ///
    /// Write-once: subsequent calls return `ContractIdAlreadySet` (error 6005).
    /// `admin` must authorise the call to prevent an unauthenticated party from
    /// overwriting the contract's own identity before the deployer can set it.
    ///
    /// Emits `dlg_link` on success.
    ///
    /// # Errors
    /// - [`MuxDelegationError::ContractIdAlreadySet`] — already linked.
    pub fn link_contract_id(
        env: Env,
        admin: Address,
        contract_id: Address,
    ) -> Result<(), MuxDelegationError> {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::ContractId) {
            return Err(MuxDelegationError::ContractIdAlreadySet);
        }
        env.storage()
            .instance()
            .set(&DataKey::ContractId, &contract_id);
        Self::extend_ttl(&env);
        emit(&env, symbol_short!("dlg_link"), (admin, contract_id));
        Ok(())
    }

    /// Return the linked contract address, or `None` if `link_contract_id`
    /// has not been called yet.
    pub fn get_contract_id(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::ContractId)
    }

    fn extend_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    fn require_admin(env: &Env) -> Result<(), MuxDelegationError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxDelegationError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    /// Extend the TTL for a single persistent `DelegatePerms` or `OwnerDelegates`
    /// entry. Called after every write so individual entries do not expire while
    /// the contract instance is still live.
    ///
    /// This is the per-entry counterpart to `extend_ttl`, which only refreshes
    /// the contract *instance* storage. Persistent entries have their own TTL
    /// clock and must be bumped independently (see docs/storage-griefing.md).
    fn extend_entry_ttl(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
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

    // ── get_delegates enumeration ─────────────────────────────────────────────

    #[test]
    fn test_get_delegates_empty_for_unknown_owner() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), 0);
    }

    #[test]
    fn test_get_delegates_preserves_grant_order() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let d0 = Address::generate(&env);
        let d1 = Address::generate(&env);
        let d2 = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        client.grant_delegate(&owner, &d0, &perms);
        client.grant_delegate(&owner, &d1, &perms);
        client.grant_delegate(&owner, &d2, &perms);

        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), 3);
        assert_eq!(delegates.get(0).unwrap(), d0);
        assert_eq!(delegates.get(1).unwrap(), d1);
        assert_eq!(delegates.get(2).unwrap(), d2);
    }

    #[test]
    fn test_get_delegates_isolated_per_owner() {
        let (env, client) = setup();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        let delegate_a = Address::generate(&env);
        let delegate_b = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        client.grant_delegate(&owner_a, &delegate_a, &perms);
        client.grant_delegate(&owner_b, &delegate_b, &perms);

        let list_a = client.get_delegates(&owner_a);
        let list_b = client.get_delegates(&owner_b);
        assert_eq!(list_a.len(), 1);
        assert_eq!(list_b.len(), 1);
        assert_eq!(list_a.get(0).unwrap(), delegate_a);
        assert_eq!(list_b.get(0).unwrap(), delegate_b);
        assert!(!list_a.contains(&delegate_b));
        assert!(!list_b.contains(&delegate_a));
    }

    #[test]
    fn test_get_delegates_regrant_does_not_duplicate() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm_a = symbol_short!("read");
        let perm_b = symbol_short!("write");

        client.grant_delegate(&owner, &delegate, &vec![&env, perm_a]);
        client.grant_delegate(&owner, &delegate, &vec![&env, perm_b]);

        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), 1);
        assert_eq!(delegates.get(0).unwrap(), delegate);
    }

    #[test]
    fn test_get_delegates_after_middle_revoke() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let d0 = Address::generate(&env);
        let d1 = Address::generate(&env);
        let d2 = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        client.grant_delegate(&owner, &d0, &perms);
        client.grant_delegate(&owner, &d1, &perms);
        client.grant_delegate(&owner, &d2, &perms);

        client.revoke_delegate(&owner, &d1);

        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), 2);
        assert!(delegates.contains(&d0));
        assert!(!delegates.contains(&d1));
        assert!(delegates.contains(&d2));
    }

    #[test]
    fn test_get_delegates_enumerates_up_to_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];
        let mut expected: Vec<Address> = Vec::new(&env);

        for _ in 0..MAX_DELEGATES_PER_OWNER {
            let d = Address::generate(&env);
            client.grant_delegate(&owner, &d, &perms);
            expected.push_back(d);
        }

        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), MAX_DELEGATES_PER_OWNER);
        for i in 0..MAX_DELEGATES_PER_OWNER {
            assert_eq!(delegates.get(i).unwrap(), expected.get(i).unwrap());
        }
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
    fn test_grant_overwrite_updates_is_delegate_checks() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm_a = symbol_short!("read");
        let perm_b = symbol_short!("write");

        client.grant_delegate(&owner, &delegate, &vec![&env, perm_a.clone()]);
        assert!(client.is_delegate(&owner, &delegate, &perm_a));
        assert!(!client.is_delegate(&owner, &delegate, &perm_b));

        client.grant_delegate(&owner, &delegate, &vec![&env, perm_b.clone()]);
        assert!(!client.is_delegate(&owner, &delegate, &perm_a));
        assert!(client.is_delegate(&owner, &delegate, &perm_b));
    }

    #[test]
    fn test_grant_overwrite_does_not_duplicate_owner_delegates() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.grant_delegate(&owner, &delegate, &vec![&env, symbol_short!("read")]);
        client.grant_delegate(&owner, &delegate, &vec![&env, symbol_short!("write")]);
        client.grant_delegate(
            &owner,
            &delegate,
            &vec![&env, symbol_short!("swap"), symbol_short!("vote")],
        );

        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), 1);
        assert!(delegates.contains(&delegate));
    }

    #[test]
    fn test_grant_overwrite_replaces_full_permission_set() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let old_a = symbol_short!("read");
        let old_b = symbol_short!("write");
        let new_a = symbol_short!("swap");
        let new_b = symbol_short!("vote");

        client.grant_delegate(&owner, &delegate, &vec![&env, old_a.clone(), old_b.clone()]);
        client.grant_delegate(&owner, &delegate, &vec![&env, new_a.clone(), new_b.clone()]);

        let stored = client.get_delegate_permissions(&owner, &delegate);
        assert_eq!(stored.len(), 2);
        assert!(!stored.contains(&old_a));
        assert!(!stored.contains(&old_b));
        assert!(stored.contains(&new_a));
        assert!(stored.contains(&new_b));
    }

    #[test]
    fn test_grant_overwrite_with_empty_permissions_fails() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.grant_delegate(&owner, &delegate, &vec![&env, symbol_short!("read")]);
        let result = client.try_grant_delegate(&owner, &delegate, &Vec::new(&env));
        assert_eq!(result, Err(Ok(MuxDelegationError::EmptyPermissions)));

        // Prior grant must remain intact after the rejected overwrite.
        assert!(client.is_delegate(&owner, &delegate, &symbol_short!("read")));
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
    fn test_error_code_too_many_delegates_cap() {
        assert_eq!(MuxDelegationError::TooManyDelegates as u32, 6004);
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────
    // symbol_short! enforces ≤ 8 chars at compile time; these declarations
    // confirm all event tag/action strings compile without truncation.
    #[test]
    fn test_symbol_short_lengths_within_limit() {
        let _mux_dlg = symbol_short!("mux_dlg");
        let _dlg_grant = symbol_short!("dlg_grant");
        let _dlg_rev = symbol_short!("dlg_rev");
        let _dlg_link = symbol_short!("dlg_link");
        let _init = symbol_short!("init");
    }

    // ── check_delegate ────────────────────────────────────────────────────────

    #[test]
    fn test_check_delegate_ok_for_granted_permission() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm = symbol_short!("read");

        client.grant_delegate(&owner, &delegate, &vec![&env, perm.clone()]);

        let result = client.try_check_delegate(&owner, &delegate, &perm);
        assert_eq!(result, Ok(Ok(())));
    }

    #[test]
    fn test_check_delegate_err_for_ungrant_permission() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let granted = symbol_short!("read");
        let ungrated = symbol_short!("write");

        client.grant_delegate(&owner, &delegate, &vec![&env, granted]);

        let result = client.try_check_delegate(&owner, &delegate, &ungrated);
        assert_eq!(result, Err(Ok(MuxDelegationError::NotADelegate)));
    }

    #[test]
    fn test_check_delegate_err_for_unregistered_delegate() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm = symbol_short!("read");

        // No grant has been made at all.
        let result = client.try_check_delegate(&owner, &delegate, &perm);
        assert_eq!(result, Err(Ok(MuxDelegationError::NotADelegate)));
    }

    // ── Unauthorized delegate denial tests (closes #408) ─────────────────────
    //
    // These tests verify that `grant_delegate` and `revoke_delegate` reject
    // callers who have not been authorised as the declared `owner`.  Following
    // the pattern used across mux-* contracts (see mux-account-factory), they
    // deliberately omit `mock_all_auths` so that `require_auth` rejects the
    // call at the host level, surfacing as `Err(..)` from `try_*`.

    /// Calling grant_delegate without any authorised signer must be rejected.
    #[test]
    fn test_grant_delegate_requires_owner_auth() {
        // No mock_all_auths — require_auth must reject.
        let env = Env::default();
        let id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &id);

        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        let result = client.try_grant_delegate(&owner, &delegate, &perms);
        assert!(
            result.is_err(),
            "grant_delegate must reject when owner auth is absent"
        );

        // No storage must have been written.
        assert!(
            client.get_delegates(&owner).is_empty(),
            "no delegate must be registered after a rejected grant"
        );
        assert!(
            !client.is_delegate(&owner, &delegate, &symbol_short!("read")),
            "is_delegate must return false after a rejected grant"
        );
    }

    /// Calling revoke_delegate without authorisation must be rejected.
    /// require_auth is checked before any storage read, so the call fails
    /// on auth even when no grant exists yet.
    #[test]
    fn test_revoke_delegate_requires_owner_auth() {
        // No mock_all_auths — require_auth must reject before any storage access.
        let env = Env::default();
        let id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &id);

        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_revoke_delegate(&owner, &delegate);
        assert!(
            result.is_err(),
            "revoke_delegate must reject when owner auth is absent"
        );
    }

    /// is_delegate must return false for a (owner, delegate) pair that was
    /// never granted — no auth required for this read-only query.
    #[test]
    fn test_is_delegate_returns_false_for_never_granted() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let stranger = Address::generate(&env);

        // No grant has ever been made between owner and stranger.
        assert!(
            !client.is_delegate(&owner, &stranger, &symbol_short!("read")),
            "is_delegate must return false for a never-granted pair"
        );
        assert!(
            !client.is_delegate(&owner, &stranger, &symbol_short!("transfer")),
            "is_delegate must return false regardless of the queried permission"
        );
    }

    /// is_delegate returns false after a grant is revoked (post-revoke denial).
    #[test]
    fn test_is_delegate_false_after_revoke() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm = symbol_short!("trade");

        client.grant_delegate(&owner, &delegate, &vec![&env, perm.clone()]);
        assert!(client.is_delegate(&owner, &delegate, &perm));

        client.revoke_delegate(&owner, &delegate);
        assert!(
            !client.is_delegate(&owner, &delegate, &perm),
            "is_delegate must return false after the grant is revoked"
        );
    }

    /// get_delegate_permissions returns an empty vec for a never-granted pair.
    #[test]
    fn test_get_delegate_permissions_empty_for_never_granted() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let stranger = Address::generate(&env);

        let perms = client.get_delegate_permissions(&owner, &stranger);
        assert_eq!(
            perms.len(),
            0,
            "get_delegate_permissions must return empty vec for unknown pair"
        );
    }

    // ── link_contract_id / get_contract_id (closes #411) ─────────────────────

    /// Linking a contract address stores it and returns it via get_contract_id.
    #[test]
    fn test_link_contract_id_stores_address() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        let contract_addr = Address::generate(&env);

        assert!(client
            .try_link_contract_id(&admin, &contract_addr)
            .is_ok());
        assert_eq!(client.get_contract_id(), Some(contract_addr));
    }

    /// Before link_contract_id is called, get_contract_id returns None.
    #[test]
    fn test_get_contract_id_returns_none_before_link() {
        let (env, client) = setup();
        let _ = env;
        assert_eq!(client.get_contract_id(), None);
    }

    /// link_contract_id is write-once: a second call returns ContractIdAlreadySet.
    #[test]
    fn test_link_contract_id_is_write_once() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        let contract_addr_a = Address::generate(&env);
        let contract_addr_b = Address::generate(&env);

        client.link_contract_id(&admin, &contract_addr_a);

        let result = client.try_link_contract_id(&admin, &contract_addr_b);
        assert_eq!(
            result,
            Err(Ok(MuxDelegationError::ContractIdAlreadySet)),
            "second link_contract_id call must return ContractIdAlreadySet"
        );

        // Original value must remain intact.
        assert_eq!(client.get_contract_id(), Some(contract_addr_a));
    }

    /// link_contract_id emits a dlg_link event on success.
    #[test]
    fn test_link_contract_id_emits_event() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        let contract_addr = Address::generate(&env);

        client.link_contract_id(&admin, &contract_addr);

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("dlg_link"));
    }

    /// link_contract_id requires admin authorisation; unauthenticated calls
    /// must be rejected without writing any state.
    #[test]
    fn test_link_contract_id_requires_admin_auth() {
        // No mock_all_auths — require_auth must reject.
        let env = Env::default();
        let id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &id);

        let admin = Address::generate(&env);
        let contract_addr = Address::generate(&env);

        let result = client.try_link_contract_id(&admin, &contract_addr);
        assert!(
            result.is_err(),
            "link_contract_id must reject when admin auth is absent"
        );

        // Storage must remain empty.
        assert_eq!(
            client.get_contract_id(),
            None,
            "get_contract_id must return None after a rejected link attempt"
        );
    }

    /// Error code 6005 is stable ABI for ContractIdAlreadySet.
    #[test]
    fn test_error_code_contract_id_already_set() {
        assert_eq!(MuxDelegationError::ContractIdAlreadySet as u32, 6005);
    }

    /// Existing delegation operations are unaffected after a contract address
    /// is linked — isolation check.
    #[test]
    fn test_link_contract_id_does_not_affect_delegation_state() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perm = symbol_short!("read");
        let contract_addr = Address::generate(&env);

        // Grant a delegation first.
        client.grant_delegate(&owner, &delegate, &vec![&env, perm.clone()]);
        assert!(client.is_delegate(&owner, &delegate, &perm));

        // Link the contract address.
        client.link_contract_id(&admin, &contract_addr);

        // Prior delegation must be unaffected.
        assert!(client.is_delegate(&owner, &delegate, &perm));
        assert_eq!(client.get_delegates(&owner).len(), 1);
    }

    // ── initialize / upgrade (closes #693) ────────────────────────────────────

    #[test]
    fn test_initialize_stores_admin_and_emits_event() {
        let (env, client) = setup();
        let admin = Address::generate(&env);

        assert!(client.try_initialize(&admin).is_ok());
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);

        client.initialize(&admin);
        let result = client.try_initialize(&admin);
        assert_eq!(result, Err(Ok(MuxDelegationError::AlreadyInitialized)));
    }

    #[test]
    fn test_initialize_requires_admin_auth() {
        // No mock_all_auths — require_auth must reject.
        let env = Env::default();
        let id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &id);
        let admin = Address::generate(&env);

        let result = client.try_initialize(&admin);
        assert!(
            result.is_err(),
            "initialize must reject when admin auth is absent"
        );
    }

    #[test]
    fn test_upgrade_before_initialize_returns_not_initialized() {
        let (env, client) = setup();
        let fake_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        let result = client.try_upgrade(&fake_hash);
        assert_eq!(result, Err(Ok(MuxDelegationError::NotInitialized)));
    }

    #[test]
    fn test_upgrade_requires_admin_auth() {
        // Seed Admin directly in storage (bypassing initialize) so this test
        // exercises only the upgrade() auth gate with zero mocked auths.
        let env = Env::default();
        let id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &id);
        let admin = Address::generate(&env);
        env.as_contract(&id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
        });

        let fake_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
        let result = client.try_upgrade(&fake_hash);
        assert!(
            result.is_err(),
            "upgrade must reject when admin auth is absent"
        );
    }

    #[test]
    fn test_grant_delegate_does_not_require_initialize() {
        // Delegation grants must keep working exactly as before for
        // contracts that never call initialize() — the upgrade admin is
        // optional and orthogonal to the delegation grant/revoke flow.
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let perms = vec![&env, symbol_short!("read")];

        assert!(client.try_grant_delegate(&owner, &delegate, &perms).is_ok());
    }

    #[test]
    fn test_error_code_not_initialized() {
        assert_eq!(MuxDelegationError::NotInitialized as u32, 6006);
    }

    #[test]
    fn test_error_code_already_initialized() {
        assert_eq!(MuxDelegationError::AlreadyInitialized as u32, 6007);
    }
}
