/*!
 * mux-delegation: Scoped delegate permission management for Mux Protocol.
 *
 * An owner grants a named permission set (`Vec<Symbol>`) to a delegate
 * address; granting again for the same `(owner, delegate)` pair fully
 * replaces the prior set (no append mode).
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types backed by the Soroban host.
 *
 * # Upgrade path
 *
 * See `docs/delegation-upgrade.md` for storage-compatibility rules.
 * `upgrade()` is admin-gated and requires `initialize()` to have been called
 * first. Delegation operations (`grant_delegate`, `revoke_delegate`, etc.)
 * never require an admin and are unaffected by whether `initialize` has been
 * called.
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

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Storage key discriminants for the delegation contract.
///
/// Two independent "admin" concepts exist in this contract:
/// - `Admin` (instance, set by `initialize`) — controls `upgrade()` only.
/// - The `admin` parameter of `link_contract_id` — checked inline via
///   `admin.require_auth()` and **not** stored or related to `DataKey::Admin`.
///
/// See `docs/delegation-upgrade.md` for the full breakdown.
#[contracttype]
pub enum DataKey {
    /// Granted permission set for an `(owner, delegate)` pair.
    /// Storage: Persistent.
    DelegatePerms(Address, Address),
    /// All delegate addresses registered under an owner.
    /// Storage: Persistent.
    OwnerDelegates(Address),
    /// Write-once self-registration address set by `link_contract_id`.
    /// Storage: Instance.
    ContractId,
    /// Optional upgrade authority, set once by `initialize`.
    /// Absent unless `initialize` was called — `upgrade()` returns
    /// `NotInitialized` in that case.
    /// Storage: Instance.
    Admin,
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Error codes for the delegation contract.
///
/// Codes 6001–6007 are stable ABI — coordinate changes with a registry
/// version bump and update `docs/error_codes.md`.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxDelegationError {
    /// No grant exists for the `(owner, delegate)` pair.
    NotADelegate = 6001,
    /// `permissions` exceeds `MAX_DELEGATE_PERMS` (64).
    TooManyPermissions = 6002,
    /// `permissions` is empty.
    EmptyPermissions = 6003,
    /// Owner has reached `MAX_DELEGATES_PER_OWNER` (128).
    TooManyDelegates = 6004,
    /// `link_contract_id` called after the address was already set.
    ContractIdAlreadySet = 6005,
    /// `upgrade` called before `initialize`; no admin to authorise it.
    NotInitialized = 6006,
    /// `initialize` called more than once.
    AlreadyInitialized = 6007,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum permissions per `(owner, delegate)` pair (enforced at grant time).
/// See `docs/delegation-upgrade.md#changing-max_delegate_perms`.
// STORAGE-GRIEFING: unbounded permission vecs would let any owner bloat
// persistent storage, raising rent for every caller.  The cap prevents a
// single owner from monopolising ledger capacity.
const MAX_DELEGATE_PERMS: u32 = 64;

/// Maximum delegate addresses per owner.
// STORAGE-GRIEFING: bounds the OwnerDelegates vec in persistent storage.
const MAX_DELEGATES_PER_OWNER: u32 = 128;

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): extend instance TTL on every write so the contract
// stays live as long as it is actively used.  See docs/storage-griefing.md.
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxDelegation;

#[contractimpl]
impl MuxDelegation {
    // ── Upgrade / initialize ───────────────────────────────────────────────────

    /// Initialize the contract with an upgrade admin. Optional: the delegation
    /// operations work without an admin — `initialize` only enables `upgrade()`.
    ///
    /// Returns `AlreadyInitialized` if called more than once.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxDelegationError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxDelegationError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Upgrade the contract WASM. Admin only.
    ///
    /// See `docs/delegation-upgrade.md` for storage-compatibility rules.
    /// Returns `NotInitialized` if `initialize` was never called (fail-closed).
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxDelegationError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    // ── Delegation entrypoints ─────────────────────────────────────────────────

    /// Grant `permissions` to `delegate` on behalf of `owner`.
    ///
    /// - Requires `owner.require_auth()`.
    /// - Replaces any prior grant for the `(owner, delegate)` pair (no append).
    /// - Returns `EmptyPermissions` if `permissions` is empty.
    /// - Returns `TooManyPermissions` if `permissions.len() > MAX_DELEGATE_PERMS`.
    /// - Returns `TooManyDelegates` if `owner` already has `MAX_DELEGATES_PER_OWNER`
    ///   unique delegates.
    ///
    /// Emits `dlg_grant` on success.
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

        // Track whether this delegate already exists in the owner's list
        let mut delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerDelegates(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        if !delegates.contains(&delegate) {
            // STORAGE-GRIEFING: cap delegates per owner
            if delegates.len() >= MAX_DELEGATES_PER_OWNER {
                return Err(MuxDelegationError::TooManyDelegates);
            }
            delegates.push_back(delegate.clone());
            env.storage()
                .persistent()
                .set(&DataKey::OwnerDelegates(owner.clone()), &delegates);
        }

        // Replace the permission set (idempotent re-grant is intentional)
        env.storage().persistent().set(
            &DataKey::DelegatePerms(owner.clone(), delegate.clone()),
            &permissions,
        );

        emit(
            &env,
            symbol_short!("dlg_grant"),
            (owner, delegate),
        );
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Revoke all permissions for `delegate` from `owner`.
    ///
    /// - Requires `owner.require_auth()`.
    /// - Returns `NotADelegate` if no grant exists for the pair.
    ///
    /// Emits `dlg_rev` on success.
    pub fn revoke_delegate(
        env: Env,
        owner: Address,
        delegate: Address,
    ) -> Result<(), MuxDelegationError> {
        owner.require_auth();

        if !env
            .storage()
            .persistent()
            .has(&DataKey::DelegatePerms(owner.clone(), delegate.clone()))
        {
            return Err(MuxDelegationError::NotADelegate);
        }

        // Remove from the permission map
        env.storage()
            .persistent()
            .remove(&DataKey::DelegatePerms(owner.clone(), delegate.clone()));

        // Remove from the owner's delegate list
        let mut delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerDelegates(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        if let Some(i) = delegates.iter().position(|a| a == delegate) {
            delegates.remove(i as u32);
            env.storage()
                .persistent()
                .set(&DataKey::OwnerDelegates(owner.clone()), &delegates);
        }

        emit(
            &env,
            symbol_short!("dlg_rev"),
            (owner, delegate),
        );
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Return the permission set granted to `delegate` by `owner`.
    ///
    /// Returns an empty `Vec` if no grant exists (does not error).
    pub fn get_delegate_permissions(env: Env, owner: Address, delegate: Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::DelegatePerms(owner, delegate))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return whether `permission` is in the grant for `(owner, delegate)`.
    ///
    /// Read-only; no events emitted, no auth required.
    pub fn is_delegate(env: Env, owner: Address, delegate: Address, permission: Symbol) -> bool {
        let perms: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::DelegatePerms(owner, delegate))
            .unwrap_or_else(|| Vec::new(&env));
        perms.contains(&permission)
    }

    /// Return all delegate addresses registered under `owner`.
    ///
    /// Returns an empty `Vec` if `owner` has no delegates.
    pub fn get_delegates(env: Env, owner: Address) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerDelegates(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Read-only auth check: `Ok(())` if `permission` is granted to `delegate`
    /// by `owner`; `Err(NotADelegate)` otherwise.
    ///
    /// Equivalent to `is_delegate` but returns a typed error so callers can
    /// chain this as a guard in other contracts.
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

    /// Write-once self-registration of this contract's own address.
    ///
    /// The `admin` parameter is the caller authorising this specific call
    /// (`admin.require_auth()`) and is **not** the same as the `DataKey::Admin`
    /// set by `initialize`. Any address may call `link_contract_id` by naming
    /// itself as `admin`.
    ///
    /// Returns `ContractIdAlreadySet` if called more than once.
    ///
    /// Emits `dlg_link` on success.
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

        emit(
            &env,
            symbol_short!("dlg_link"),
            (admin, contract_id),
        );
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Return the linked contract address, or `None` if not yet set.
    pub fn get_contract_id(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::ContractId)
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

    fn perms(env: &Env, names: &[&str]) -> Vec<Symbol> {
        let mut v: Vec<Symbol> = Vec::new(env);
        for name in names {
            v.push_back(Symbol::new(env, name));
        }
        v
    }

    fn sym(env: &Env, name: &str) -> Symbol {
        Symbol::new(env, name)
    }

    // ── initialize / upgrade ──────────────────────────────────────────────────

    #[test]
    fn test_initialize_stores_admin_and_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        assert!(client.try_initialize(&admin).is_ok());
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_returns_already_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin);
        let result = client.try_initialize(&admin);
        assert_eq!(result, Err(Ok(MuxDelegationError::AlreadyInitialized)));
    }

    #[test]
    fn test_initialize_requires_admin_auth() {
        let env = Env::default(); // no mock_all_auths
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let result = client.try_initialize(&admin);
        assert!(
            result.is_err(),
            "initialize must reject when admin auth is absent"
        );
    }

    #[test]
    fn test_upgrade_before_initialize_returns_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let fake_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        let result = client.try_upgrade(&fake_hash);
        assert_eq!(result, Err(Ok(MuxDelegationError::NotInitialized)));
    }

    #[test]
    fn test_upgrade_requires_admin_auth() {
        let env = Env::default(); // no mock_all_auths
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
        });

        let fake_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
        let result = client.try_upgrade(&fake_hash);
        assert!(
            result.is_err(),
            "upgrade must reject when admin auth is absent"
        );
    }

    // ── grant_delegate ────────────────────────────────────────────────────────

    #[test]
    fn test_grant_delegate_emits_dlg_grant_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        assert!(client.try_grant_delegate(&owner, &delegate, &p).is_ok());
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("dlg_grant"));
    }

    #[test]
    fn test_grant_delegate_empty_permissions_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p: Vec<Symbol> = Vec::new(&env);

        let result = client.try_grant_delegate(&owner, &delegate, &p);
        assert_eq!(result, Err(Ok(MuxDelegationError::EmptyPermissions)));
    }

    #[test]
    fn test_grant_delegate_too_many_permissions_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        // Build MAX_DELEGATE_PERMS + 1 permission symbols (one over the limit).
        // All entries use the same symbol to keep the vec size valid — the cap
        // is on the vec length, not on uniqueness.
        let mut p: Vec<Symbol> = Vec::new(&env);
        for _ in 0..=(MAX_DELEGATE_PERMS) {
            p.push_back(sym(&env, "transfer"));
        }

        let result = client.try_grant_delegate(&owner, &delegate, &p);
        assert_eq!(result, Err(Ok(MuxDelegationError::TooManyPermissions)));
    }

    #[test]
    fn test_grant_delegate_replaces_prior_grant() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let p1 = perms(&env, &["transfer"]);
        let p2 = perms(&env, &["approve"]);

        client.grant_delegate(&owner, &delegate, &p1);
        client.grant_delegate(&owner, &delegate, &p2);

        let got = client.get_delegate_permissions(&owner, &delegate);
        // p2 must have replaced p1 — only "approve" should be present
        assert!(got.contains(&sym(&env, "approve")));
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn test_grant_delegate_requires_owner_auth() {
        let env = Env::default(); // no mock_all_auths
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        let result = client.try_grant_delegate(&owner, &delegate, &p);
        assert!(
            result.is_err(),
            "grant_delegate must reject when owner auth is absent"
        );
    }

    // ── revoke_delegate ───────────────────────────────────────────────────────

    #[test]
    fn test_revoke_delegate_emits_dlg_rev_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        client.grant_delegate(&owner, &delegate, &p);
        assert!(client.try_revoke_delegate(&owner, &delegate).is_ok());

        let events = env.events().all();
        // dlg_grant + dlg_rev
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("dlg_rev"));
    }

    #[test]
    fn test_revoke_delegate_not_a_delegate_returns_error() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_revoke_delegate(&owner, &delegate);
        assert_eq!(result, Err(Ok(MuxDelegationError::NotADelegate)));
    }

    #[test]
    fn test_revoke_delegate_requires_owner_auth() {
        let env = Env::default(); // no mock_all_auths
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_revoke_delegate(&owner, &delegate);
        assert!(
            result.is_err(),
            "revoke_delegate must reject when owner auth is absent"
        );
    }

    // ── get_delegate_permissions / is_delegate / check_delegate ──────────────

    #[test]
    fn test_get_delegate_permissions_returns_empty_for_unknown_pair() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let got = client.get_delegate_permissions(&owner, &delegate);
        assert!(got.is_empty());
    }

    #[test]
    fn test_is_delegate_true_after_grant() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        client.grant_delegate(&owner, &delegate, &p);
        assert!(client.is_delegate(&owner, &delegate, &sym(&env, "transfer")));
    }

    #[test]
    fn test_is_delegate_false_after_revoke() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        client.grant_delegate(&owner, &delegate, &p);
        client.revoke_delegate(&owner, &delegate);
        assert!(!client.is_delegate(&owner, &delegate, &sym(&env, "transfer")));
    }

    #[test]
    fn test_check_delegate_ok_when_granted() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        client.grant_delegate(&owner, &delegate, &p);
        assert!(
            client
                .try_check_delegate(&owner, &delegate, &sym(&env, "transfer"))
                .is_ok()
        );
    }

    #[test]
    fn test_check_delegate_err_when_not_granted() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = client.try_check_delegate(&owner, &delegate, &sym(&env, "transfer"));
        assert_eq!(result, Err(Ok(MuxDelegationError::NotADelegate)));
    }

    // ── get_delegates ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_delegates_returns_all_delegates() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let d1 = Address::generate(&env);
        let d2 = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        client.grant_delegate(&owner, &d1, &p);
        client.grant_delegate(&owner, &d2, &p);

        let delegates = client.get_delegates(&owner);
        assert_eq!(delegates.len(), 2);
        assert!(delegates.contains(&d1));
        assert!(delegates.contains(&d2));
    }

    #[test]
    fn test_get_delegates_empty_for_unknown_owner() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);

        assert!(client.get_delegates(&owner).is_empty());
    }

    #[test]
    fn test_revoke_removes_from_get_delegates() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        client.grant_delegate(&owner, &delegate, &p);
        client.revoke_delegate(&owner, &delegate);

        assert!(client.get_delegates(&owner).is_empty());
    }

    // ── link_contract_id ──────────────────────────────────────────────────────

    #[test]
    fn test_link_contract_id_stores_and_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let linked = Address::generate(&env);

        assert!(client.try_link_contract_id(&admin, &linked).is_ok());
        assert_eq!(client.get_contract_id(), Some(linked));

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("dlg_link"));
    }

    #[test]
    fn test_link_contract_id_twice_returns_already_set() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let linked = Address::generate(&env);

        client.link_contract_id(&admin, &linked);
        let result = client.try_link_contract_id(&admin, &linked);
        assert_eq!(result, Err(Ok(MuxDelegationError::ContractIdAlreadySet)));
    }

    #[test]
    fn test_link_contract_id_requires_admin_auth() {
        let env = Env::default(); // no mock_all_auths
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let linked = Address::generate(&env);

        let result = client.try_link_contract_id(&admin, &linked);
        assert!(
            result.is_err(),
            "link_contract_id must reject when admin auth is absent"
        );
    }

    #[test]
    fn test_get_contract_id_returns_none_when_not_set() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        assert!(client.get_contract_id().is_none());
    }

    // ── Delegation does not require initialize ────────────────────────────────

    #[test]
    fn test_grant_and_revoke_work_without_initialize() {
        // Delegation operations must work independently of the admin / upgrade path.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let delegate = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        assert!(client.try_grant_delegate(&owner, &delegate, &p).is_ok());
        assert!(client.try_revoke_delegate(&owner, &delegate).is_ok());
    }

    // ── Storage-cap griefing guards ───────────────────────────────────────────

    #[test]
    fn test_too_many_delegates_per_owner_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        env.budget().reset_unlimited();
        let contract_id = env.register_contract(None, MuxDelegation);
        let client = MuxDelegationClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let p = perms(&env, &["transfer"]);

        for _ in 0..MAX_DELEGATES_PER_OWNER {
            client.grant_delegate(&owner, &Address::generate(&env), &p);
        }

        let overflow = Address::generate(&env);
        let result = client.try_grant_delegate(&owner, &overflow, &p);
        assert_eq!(result, Err(Ok(MuxDelegationError::TooManyDelegates)));
    }

    // ── Error code stability (stable ABI) ─────────────────────────────────────

    #[test]
    fn test_error_code_discriminants_are_stable() {
        assert_eq!(MuxDelegationError::NotADelegate as u32, 6001);
        assert_eq!(MuxDelegationError::TooManyPermissions as u32, 6002);
        assert_eq!(MuxDelegationError::EmptyPermissions as u32, 6003);
        assert_eq!(MuxDelegationError::TooManyDelegates as u32, 6004);
        assert_eq!(MuxDelegationError::ContractIdAlreadySet as u32, 6005);
        assert_eq!(MuxDelegationError::NotInitialized as u32, 6006);
        assert_eq!(MuxDelegationError::AlreadyInitialized as u32, 6007);
    }

    // ── symbol_short length audit ─────────────────────────────────────────────

    #[test]
    fn test_symbol_short_lengths_within_limit() {
        // symbol_short!() enforces the length constraint at compile time.
        let _tag = symbol_short!("mux_dlg");
        let _init = symbol_short!("init");
        let _grant = symbol_short!("dlg_grant");
        let _rev = symbol_short!("dlg_rev");
        let _link = symbol_short!("dlg_link");
        core::mem::drop((_tag, _init, _grant, _rev, _link));
    }
}
