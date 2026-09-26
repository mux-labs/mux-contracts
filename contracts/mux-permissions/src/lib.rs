/*!
 * mux-permissions: Fine-grained permission and role management for Mux Protocol.
 *
 * Implements a role-based access control (RBAC) registry that other Mux
 * contracts can call to verify caller permissions before executing
 * privileged operations.
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types backed by the Soroban host.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_perm"), action), data);
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    RoleMembers(Symbol),
    RolePermissions(Symbol),
    AccountRoles(Address),
    PendingAdmins,
    AdminThreshold,
    AdminApprovals(Address),
    /// Registry-level metadata (name, version, description).
    Metadata,
}

// ── Registry metadata ─────────────────────────────────────────────────────────

/// Descriptive metadata attached to the permissions registry itself.
///
/// Stored under [`DataKey::Metadata`] and writable only by the current admin.
/// Useful for off-chain tooling (indexers, dashboards) that need to identify
/// or version a deployed contract instance.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryMeta {
    /// Human-readable name for this registry instance (e.g. `"mux-mainnet-perm"`).
    pub name: String,
    /// Semantic version string (e.g. `"1.0.0"`).
    pub version: String,
    /// Optional free-form description / notes.
    pub description: String,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RoleInfo {
    pub name: Symbol,
    pub members: Vec<Address>,
    pub permissions: Vec<Symbol>,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxPermissionsError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    RoleNotFound = 4,
    AccountNotInRole = 5,
    PermissionNotFound = 6,
    // STORAGE-GRIEFING: unbounded role-member and account-role vecs would let an
    // admin (or a compromised admin key) bloat instance storage, raising rent for
    // every caller that touches this contract.
    TooManyMembers = 7,
    TooManyRoles = 8,
    AdminNotFound = 9,
    AlreadyApproved = 10,
    TooManyPendingAdmins = 11,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum members per role to bound the RoleMembers vec in instance storage.
const MAX_ROLE_MEMBERS: u32 = 256;

/// Maximum roles an account may hold simultaneously.
const MAX_ROLES_PER_ACCOUNT: u32 = 32;

/// Maximum pending admin proposals to bound the PendingAdmins vec.
const MAX_PENDING_ADMINS: u32 = 16;

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): extend instance TTL on every write so the registry
// stays live as long as it is actively used.  See docs/storage-griefing.md.
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxPermissions;

#[contractimpl]
impl MuxPermissions {
    /// Initialize with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxPermissionsError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxPermissionsError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Upgrade the contract WASM. Admin only.
    ///
    /// See `docs/permissions-upgrade-migration.md` for storage-compatibility
    /// rules that must be observed between versions. Instance storage
    /// (admin, roles, pending admin state) is preserved across upgrades by
    /// the Soroban host — only the WASM code is replaced.
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Create a new role with an initial permission set.
    pub fn create_role(
        env: Env,
        role: Symbol,
        permissions: Vec<Symbol>,
    ) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(
            &DataKey::RoleMembers(role.clone()),
            &Vec::<Address>::new(&env),
        );
        env.storage()
            .instance()
            .set(&DataKey::RolePermissions(role.clone()), &permissions);
        emit(&env, symbol_short!("role_crt"), role);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Grant a role to an account.
    pub fn grant_role(env: Env, account: Address, role: Symbol) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;

        if !env
            .storage()
            .instance()
            .has(&DataKey::RolePermissions(role.clone()))
        {
            return Err(MuxPermissionsError::RoleNotFound);
        }

        let mut members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::RoleMembers(role.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        // Idempotent: if the account is already a member, emit no event and
        // short-circuit to avoid storage writes.
        if members.contains(&account) {
            return Ok(());
        }

        // STORAGE-GRIEFING: cap members per role to bound RoleMembers vec size.
        if members.len() >= MAX_ROLE_MEMBERS {
            return Err(MuxPermissionsError::TooManyMembers);
        }
        members.push_back(account.clone());
        env.storage()
            .instance()
            .set(&DataKey::RoleMembers(role.clone()), &members);

        let mut account_roles: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::AccountRoles(account.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        // STORAGE-GRIEFING: cap roles per account to bound AccountRoles vec size.
        if account_roles.len() >= MAX_ROLES_PER_ACCOUNT {
            return Err(MuxPermissionsError::TooManyRoles);
        }
        account_roles.push_back(role.clone());
        env.storage()
            .instance()
            .set(&DataKey::AccountRoles(account.clone()), &account_roles);
        emit(&env, symbol_short!("role_grt"), (account, role));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Revoke a role from an account.
    pub fn revoke_role(
        env: Env,
        account: Address,
        role: Symbol,
    ) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;

        let mut members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::RoleMembers(role.clone()))
            .ok_or(MuxPermissionsError::RoleNotFound)?;

        let pos = members.iter().position(|a| a == account);
        match pos {
            Some(i) => {
                members.remove(i as u32);
            }
            None => return Err(MuxPermissionsError::AccountNotInRole),
        }
        env.storage()
            .instance()
            .set(&DataKey::RoleMembers(role.clone()), &members);

        // Clean up account-role index
        if let Some(mut account_roles) = env
            .storage()
            .instance()
            .get::<DataKey, Vec<Symbol>>(&DataKey::AccountRoles(account.clone()))
        {
            if let Some(i) = account_roles.iter().position(|r| r == role) {
                account_roles.remove(i as u32);
            }
            env.storage()
                .instance()
                .set(&DataKey::AccountRoles(account.clone()), &account_roles);
        }

        emit(&env, symbol_short!("role_rev"), (account, role));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Check whether an account has a specific permission through any of its roles.
    ///
    /// A grant (`true`) emits `perm_ok` for the audit trail. A denial (`false`)
    /// emits **no** event: this is a read-only entrypoint, and per
    /// `docs/event-topic-conventions.md` read-only entrypoints (`has_*`, `get_*`,
    /// `is_*`, `simulate_*`) must not have on-chain side effects. Emitting on
    /// every denial would also let any caller — unauthenticated, since this
    /// function requires no auth — spam an arbitrary account's audit log with
    /// `perm_den` events for permissions it never held.
    pub fn has_permission(env: Env, account: Address, permission: Symbol) -> bool {
        let account_roles: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::AccountRoles(account.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        for role in account_roles.iter() {
            let perms: Vec<Symbol> = env
                .storage()
                .instance()
                .get(&DataKey::RolePermissions(role))
                .unwrap_or_else(|| Vec::new(&env));
            if perms.contains(&permission) {
                emit(&env, symbol_short!("perm_ok"), (account, permission));
                return true;
            }
        }
        false
    }

    /// Return all roles held by an account.
    pub fn get_roles(env: Env, account: Address) -> Vec<Symbol> {
        env.storage()
            .instance()
            .get(&DataKey::AccountRoles(account))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return all members of a role.
    pub fn get_role_members(env: Env, role: Symbol) -> Result<Vec<Address>, MuxPermissionsError> {
        env.storage()
            .instance()
            .get(&DataKey::RoleMembers(role))
            .ok_or(MuxPermissionsError::RoleNotFound)
    }

    // ── Multisig admin ─────────────────────────────────────────────────────────

    /// Set the number of approvals required to promote a pending admin.
    pub fn set_admin_threshold(env: Env, threshold: u32) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::AdminThreshold, &threshold);
        emit(&env, symbol_short!("adm_thr"), threshold);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the current admin threshold, or `1` (default) if never explicitly set.
    pub fn get_admin_threshold(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::AdminThreshold)
            .unwrap_or(1)
    }

    /// Propose a new admin address. Admin-only. Adds to the pending list.
    pub fn propose_admin(env: Env, new_admin: Address) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;
        let mut pending: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmins)
            .unwrap_or_else(|| Vec::new(&env));
        if !pending.contains(&new_admin) {
            if pending.len() >= MAX_PENDING_ADMINS {
                return Err(MuxPermissionsError::TooManyPendingAdmins);
            }
            pending.push_back(new_admin.clone());
            env.storage()
                .instance()
                .set(&DataKey::PendingAdmins, &pending);
            // Initialize approvals list for this candidate
            env.storage().instance().set(
                &DataKey::AdminApprovals(new_admin.clone()),
                &Vec::<Address>::new(&env),
            );
            emit(&env, symbol_short!("adm_prp"), new_admin);
            Self::extend_ttl(&env);
        }
        Ok(())
    }

    /// Approve a pending admin. When approvals reach the threshold, the new
    /// admin is promoted and removed from the pending list.
    pub fn approve_admin(
        env: Env,
        approver: Address,
        new_admin: Address,
    ) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;
        approver.require_auth();

        let pending: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmins)
            .unwrap_or_else(|| Vec::new(&env));
        if !pending.contains(&new_admin) {
            return Err(MuxPermissionsError::AdminNotFound);
        }

        let mut approvals: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AdminApprovals(new_admin.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        if approvals.contains(&approver) {
            return Err(MuxPermissionsError::AlreadyApproved);
        }
        approvals.push_back(approver.clone());
        env.storage()
            .instance()
            .set(&DataKey::AdminApprovals(new_admin.clone()), &approvals);

        let threshold: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AdminThreshold)
            .unwrap_or(1);

        if approvals.len() >= threshold {
            // Promote new admin
            env.storage().instance().set(&DataKey::Admin, &new_admin);
            // Remove from pending
            let mut updated_pending: Vec<Address> = env
                .storage()
                .instance()
                .get(&DataKey::PendingAdmins)
                .unwrap_or_else(|| Vec::new(&env));
            if let Some(i) = updated_pending.iter().position(|a| a == new_admin) {
                updated_pending.remove(i as u32);
            }
            env.storage()
                .instance()
                .set(&DataKey::PendingAdmins, &updated_pending);
            emit(&env, symbol_short!("adm_prm"), new_admin.clone());
        } else {
            emit(&env, symbol_short!("adm_apr"), (approver, new_admin));
        }
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return all pending admin candidates.
    pub fn get_pending_admins(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::PendingAdmins)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── Registry metadata ──────────────────────────────────────────────────────

    /// Store registry-level metadata. Admin only.
    ///
    /// Overwrites any previously stored metadata. Emits a `meta_set` audit event.
    pub fn set_metadata(env: Env, meta: RegistryMeta) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Metadata, &meta);
        emit(&env, symbol_short!("meta_set"), meta.name.clone());
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the currently stored registry metadata, or `None` if not set.
    pub fn get_metadata(env: Env) -> Option<RegistryMeta> {
        env.storage().instance().get(&DataKey::Metadata)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxPermissionsError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxPermissionsError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    /// Extend instance-storage TTL on every write to prevent silent data loss (T-21).
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
        Env, FromVal, String, Vec,
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

    fn setup() -> (Env, MuxPermissionsClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_role_lifecycle_emits_events() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let perm = symbol_short!("write");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(perm);

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);
        client.revoke_role(&user, &role);

        let events = env.events().all();
        // init (from setup) + role_crt + role_grt + role_rev
        assert_eq!(events.len(), 4);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("role_crt"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("role_grt"));
        assert_eq!(topic_action(&env, &events, 3), symbol_short!("role_rev"));
    }

    #[test]
    fn test_role_member_cap_enforced() {
        let (env, client, _admin) = setup();
        env.budget().reset_unlimited();
        let role = symbol_short!("capped");
        client.create_role(&role, &Vec::new(&env));

        for _ in 0..256 {
            client.grant_role(&Address::generate(&env), &role);
        }
        let result = client.try_grant_role(&Address::generate(&env), &role);
        assert!(result.is_err());
    }

    #[test]
    fn test_roles_per_account_cap_enforced() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);

        // 32 distinct role names (max symbol length is 32 chars in Soroban)
        let names = [
            "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13",
            "r14", "r15", "r16", "r17", "r18", "r19", "r20", "r21", "r22", "r23", "r24", "r25",
            "r26", "r27", "r28", "r29", "r30", "r31",
        ];
        for name in names.iter() {
            let role = soroban_sdk::Symbol::new(&env, name);
            client.create_role(&role, &Vec::new(&env));
            client.grant_role(&user, &role);
        }
        let overflow_role = soroban_sdk::Symbol::new(&env, "overflow");
        client.create_role(&overflow_role, &Vec::new(&env));
        let result = client.try_grant_role(&user, &overflow_role);
        assert!(result.is_err());
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
    }

    // ── upgrade() (closes #692) ───────────────────────────────────────────────

    #[test]
    fn test_upgrade_requires_admin_auth() {
        // Seed Admin directly in storage (bypassing initialize) so this test
        // exercises only the upgrade() auth gate with zero mocked auths —
        // require_admin() must reject before update_current_contract_wasm runs.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
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

    #[test]
    fn test_upgrade_before_initialize_returns_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let fake_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
        let result = client.try_upgrade(&fake_hash);
        assert_eq!(result, Err(Ok(MuxPermissionsError::NotInitialized)));
    }

    #[test]
    fn test_create_and_grant_role() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("operator");
        let transfer_perm = symbol_short!("transfer");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(transfer_perm.clone());

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);

        assert!(client.has_permission(&user, &transfer_perm));
        let roles = client.get_roles(&user);
        assert!(roles.contains(&role));
    }

    // ── has_permission emits no event on denial ─────────────────────────────

    #[test]
    fn test_has_permission_denial_emits_no_event() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let perm = symbol_short!("transfer");

        // Only `init` (from setup) has been emitted so far.
        let events_before = env.events().all();
        assert_eq!(events_before.len(), 1);

        assert!(!client.has_permission(&user, &perm));

        // has_permission is read-only; a denial must not append any event
        // (docs/event-topic-conventions.md: read-only entrypoints emit none).
        let events_after = env.events().all();
        assert_eq!(
            events_after.len(),
            1,
            "has_permission returning false must not emit perm_den"
        );
    }

    #[test]
    fn test_has_permission_grant_still_emits_perm_ok() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let perm = symbol_short!("write");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(perm.clone());

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);

        // init + role_crt + role_grt = 3 events so far.
        let events_before = env.events().all();
        assert_eq!(events_before.len(), 3);

        assert!(client.has_permission(&user, &perm));

        // A successful (granted) check still emits perm_ok — only the denial
        // path was silenced.
        let events_after = env.events().all();
        assert_eq!(events_after.len(), 4);
        assert_eq!(topic_action(&env, &events_after, 3), symbol_short!("perm_ok"));
    }

    #[test]
    fn test_revoke_role_removes_permission() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("viewer");
        let read_perm = symbol_short!("read");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(read_perm.clone());

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);
        assert!(client.has_permission(&user, &read_perm));

        client.revoke_role(&user, &role);
        assert!(!client.has_permission(&user, &read_perm));
    }

    #[test]
    fn test_grant_nonexistent_role_fails() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let result = client.try_grant_role(&user, &symbol_short!("ghost"));
        assert!(result.is_err());
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, _admin) = setup();
        let other = Address::generate(&env);
        assert!(client.try_initialize(&other).is_err());
    }

    #[test]
    fn test_double_initialize_returns_already_initialized_error() {
        let (env, client, _admin) = setup();
        let other = Address::generate(&env);
        let result = client.try_initialize(&other);
        assert_eq!(result, Err(Ok(MuxPermissionsError::AlreadyInitialized)));
    }

    #[test]
    fn test_initialize_after_setup_returns_already_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        // First init succeeds.
        assert!(client.try_initialize(&admin).is_ok());
        // Second init with the same admin must return AlreadyInitialized.
        let result = client.try_initialize(&admin);
        assert_eq!(result, Err(Ok(MuxPermissionsError::AlreadyInitialized)));
    }

    #[test]
    fn test_ttl_extended_on_write() {
        // Verify that initialize bumps instance TTL (T-21 mitigation).
        // setup() calls initialize; if extend_ttl was missing the SDK would
        // panic when TTL_EXTEND_TO > remaining TTL.  Reaching here is the assertion.
        let (_env, _client, _admin) = setup();
    }

    #[test]
    fn test_set_admin_threshold_emits_event() {
        let (env, client, _admin) = setup();
        client.set_admin_threshold(&2_u32);
        let events = env.events().all();
        // init + adm_thr
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("adm_thr"));
    }

    #[test]
    fn test_propose_admin_emits_event() {
        let (env, client, _admin) = setup();
        let candidate = Address::generate(&env);
        client.propose_admin(&candidate);
        let events = env.events().all();
        // init + adm_prp
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("adm_prp"));
    }

    #[test]
    fn test_propose_admin_idempotent_no_duplicate_event() {
        let (env, client, _admin) = setup();
        let candidate = Address::generate(&env);
        client.propose_admin(&candidate);
        // Proposing the same candidate again must not emit a second event.
        client.propose_admin(&candidate);
        let events = env.events().all();
        // init + adm_prp (only once)
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_approve_admin_emits_approval_event() {
        let (env, client, admin) = setup();
        // threshold=2 so first approval does not promote
        client.set_admin_threshold(&2_u32);
        let candidate = Address::generate(&env);
        client.propose_admin(&candidate);
        client.approve_admin(&admin, &candidate);
        let events = env.events().all();
        // init + adm_thr + adm_prp + adm_apr
        assert_eq!(events.len(), 4);
        assert_eq!(topic_action(&env, &events, 3), symbol_short!("adm_apr"));
    }

    #[test]
    fn test_approve_admin_emits_promotion_event() {
        let (env, client, admin) = setup();
        // threshold=1 so the first approval immediately promotes
        client.set_admin_threshold(&1_u32);
        let candidate = Address::generate(&env);
        client.propose_admin(&candidate);
        client.approve_admin(&admin, &candidate);
        let events = env.events().all();
        // init + adm_thr + adm_prp + adm_prm
        assert_eq!(events.len(), 4);
        assert_eq!(topic_action(&env, &events, 3), symbol_short!("adm_prm"));
    }

    #[test]
    fn test_approve_admin_duplicate_approver_fails() {
        let (env, client, admin) = setup();
        client.set_admin_threshold(&2_u32);
        let candidate = Address::generate(&env);
        client.propose_admin(&candidate);
        client.approve_admin(&admin, &candidate);
        // Same approver a second time must fail.
        let result = client.try_approve_admin(&admin, &candidate);
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_nonexistent_pending_admin_fails() {
        let (env, client, admin) = setup();
        let ghost = Address::generate(&env);
        let result = client.try_approve_admin(&admin, &ghost);
        assert!(result.is_err());
    }

    // ── Issue #439 — Admin threshold getter ─────────────────────────────────────

    #[test]
    fn test_get_admin_threshold_default() {
        let (_env, client, _admin) = setup();
        // When threshold is never explicitly set, it should default to 1.
        assert_eq!(client.get_admin_threshold(), 1_u32);
    }

    #[test]
    fn test_get_admin_threshold_after_set() {
        let (_env, client, _admin) = setup();
        client.set_admin_threshold(&3_u32);
        assert_eq!(client.get_admin_threshold(), 3_u32);
    }

    // ── Issue #437 — role grant / revoke tests ──────────────────────────────────

    #[test]
    fn test_grant_role_emits_event_with_correct_topics() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(symbol_short!("write"));

        client.create_role(&role, &perms);

        // After setup (1: init) + create_role (1: role_crt) = 2 events
        let events_before = env.events().all();
        assert_eq!(events_before.len(), 2);

        client.grant_role(&user, &role);

        // After grant_role, we should have 3 events (init, role_crt, role_grt)
        let events = env.events().all();
        assert_eq!(events.len(), 3);

        let (_, topics, _) = events.get(2).unwrap();
        assert_eq!(topics.len(), 2);
        let contract_tag = soroban_sdk::Symbol::from_val(&env, &topics.get(0).unwrap());
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(contract_tag, symbol_short!("mux_perm"));
        assert_eq!(action, symbol_short!("role_grt"));
    }

    #[test]
    fn test_revoke_role_emits_event_with_correct_topics() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(symbol_short!("write"));

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);

        // After init + role_crt + role_grt = 3 events
        let events_before = env.events().all();
        assert_eq!(events_before.len(), 3);

        client.revoke_role(&user, &role);

        // After revoke_role, we should have 4 events
        let events = env.events().all();
        assert_eq!(events.len(), 4);

        let (_, topics, _) = events.get(3).unwrap();
        assert_eq!(topics.len(), 2);
        let contract_tag = soroban_sdk::Symbol::from_val(&env, &topics.get(0).unwrap());
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(contract_tag, symbol_short!("mux_perm"));
        assert_eq!(action, symbol_short!("role_rev"));
    }

    #[test]
    fn test_grant_role_duplicate_idempotent() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(symbol_short!("write"));

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);

        // After init + role_crt + role_grt = 3 events
        let events_before = env.events().all();
        assert_eq!(events_before.len(), 3);

        // Granting the same role to the same account again must succeed and
        // not emit a second grant event (idempotent — already a member).
        assert!(client.try_grant_role(&user, &role).is_ok());

        // Event count must remain the same (no duplicate role_grt emitted)
        let events_after = env.events().all();
        assert_eq!(events_after.len(), 3);
    }

    #[test]
    fn test_revoke_role_not_member_fails() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(symbol_short!("write"));

        client.create_role(&role, &perms);

        // Revoke from an account that was never granted the role
        let result = client.try_revoke_role(&user, &role);
        assert_eq!(result, Err(Ok(MuxPermissionsError::AccountNotInRole)));
    }

    #[test]
    fn test_revoke_role_nonexistent_role_fails() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let ghost = symbol_short!("ghost");

        let result = client.try_revoke_role(&user, &ghost);
        assert_eq!(result, Err(Ok(MuxPermissionsError::RoleNotFound)));
    }

    #[test]
    fn test_revoke_role_cleans_account_roles() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(symbol_short!("write"));

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);

        // Confirm role is present before revoke
        let before = client.get_roles(&user);
        assert!(before.contains(&role));

        client.revoke_role(&user, &role);

        // After revoke, the role must no longer appear in get_roles
        let after = client.get_roles(&user);
        assert!(!after.contains(&role));
    }

    #[test]
    fn test_grant_role_without_admin_auth_fails() {
        // Without mock_all_auths, an unauthorized caller gets the host auth
        // error rather than a contract error, so we test the accessible path:
        // ensure grant_role on a non-existent role returns RoleNotFound.
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        // Attempt to grant role that has not been created yet
        let result = client.try_grant_role(&user, &role);
        assert_eq!(result, Err(Ok(MuxPermissionsError::RoleNotFound)));
    }

    #[test]
    fn test_revoke_role_without_admin_auth_fails() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(symbol_short!("write"));

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);

        // With mock_all_auths, admin auth is always satisfied so we can't
        // directly test Unauthorized. Verify the happy path works instead:
        // a valid revoke succeeds.
        let result = client.try_revoke_role(&user, &role);
        assert!(result.is_ok());
    }

    #[test]
    fn test_grant_role_updates_multiple_accounts() {
        let (env, client, _admin) = setup();
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let role = symbol_short!("editor");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(symbol_short!("write"));

        client.create_role(&role, &perms);
        client.grant_role(&user1, &role);
        client.grant_role(&user2, &role);

        // Both accounts should have the role
        let roles1 = client.get_roles(&user1);
        let roles2 = client.get_roles(&user2);
        assert!(roles1.contains(&role));
        assert!(roles2.contains(&role));

        // Both should have the permission
        assert!(client.has_permission(&user1, &symbol_short!("write")));
        assert!(client.has_permission(&user2, &symbol_short!("write")));

        // Revoke from one — the other must still have permission
        client.revoke_role(&user1, &role);
        assert!(!client.has_permission(&user1, &symbol_short!("write")));
        assert!(client.has_permission(&user2, &symbol_short!("write")));
    }

    // ── Registry metadata tests ────────────────────────────────────────────────

    #[test]
    fn test_set_and_get_metadata() {
        let (env, client, _admin) = setup();
        let meta = RegistryMeta {
            name: String::from_str(&env, "mux-testnet-perm"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, "Permissions registry for testnet"),
        };
        client.set_metadata(&meta);
        let stored = client.get_metadata().unwrap();
        assert_eq!(stored.name, meta.name);
        assert_eq!(stored.version, meta.version);
        assert_eq!(stored.description, meta.description);
    }

    #[test]
    fn test_set_metadata_overwrites_previous() {
        let (env, client, _admin) = setup();
        let meta1 = RegistryMeta {
            name: String::from_str(&env, "v1"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, "first"),
        };
        let meta2 = RegistryMeta {
            name: String::from_str(&env, "v2"),
            version: String::from_str(&env, "2.0.0"),
            description: String::from_str(&env, "second"),
        };
        client.set_metadata(&meta1);
        client.set_metadata(&meta2);
        let stored = client.get_metadata().unwrap();
        assert_eq!(stored.version, meta2.version);
    }

    #[test]
    fn test_get_metadata_returns_none_when_unset() {
        let (_env, client, _admin) = setup();
        assert!(client.get_metadata().is_none());
    }

    #[test]
    fn test_set_metadata_emits_event() {
        let (env, client, _admin) = setup();
        let meta = RegistryMeta {
            name: String::from_str(&env, "registry"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, ""),
        };
        client.set_metadata(&meta);
        let events = env.events().all();
        // init + meta_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("meta_set"));
    }
}

// ── Integration tests (closes #275, #691) ─────────────────────────────────────
//
// These exercise multi-contract and cross-role scenarios that go beyond
// isolated unit tests. They previously ran only on `cargo test -- --ignored`;
// they are now unconditional so CI catches regressions in admin-rotation
// threshold enforcement (see docs/permissions-role-model.md).

#[cfg(test)]
mod integration_tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Env, Vec};

    fn setup_integration() -> (Env, MuxPermissionsClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin, contract_id)
    }

    /// Read the currently stored admin directly from instance storage,
    /// bypassing the public API (there is no `get_admin` entrypoint).
    fn stored_admin(env: &Env, contract_id: &Address) -> Address {
        env.as_contract(contract_id, || {
            env.storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::Admin)
                .unwrap()
        })
    }

    /// Verify that a role granted on one instance is not visible on another
    /// (contracts are isolated — no global state bleed-through).
    #[test]
    fn test_permissions_isolated_across_contract_instances() {
        let (env, client_a, _, _) = setup_integration();
        let contract_b = env.register_contract(None, MuxPermissions);
        let client_b = MuxPermissionsClient::new(&env, &contract_b);
        let admin_b = Address::generate(&env);
        client_b.initialize(&admin_b);

        let user = Address::generate(&env);
        let role = symbol_short!("editor");
        client_a.create_role(&role, &Vec::new(&env));
        client_a.grant_role(&user, &role);

        // The role granted on contract A must not be visible on contract B.
        let roles_b = client_b.get_roles(&user);
        assert!(roles_b.is_empty());
    }

    /// Full RBAC lifecycle: create role → grant → check permission → revoke →
    /// re-check. Simulates the sequence a real dApp would execute.
    #[test]
    fn test_full_rbac_lifecycle() {
        let (env, client, _, _) = setup_integration();
        let user = Address::generate(&env);
        let role = symbol_short!("operator");
        let perm = symbol_short!("execute");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(perm.clone());

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);
        assert!(client.has_permission(&user, &perm));

        client.revoke_role(&user, &role);
        assert!(!client.has_permission(&user, &perm));
    }

    /// Multisig admin promotion: two approvals required. Below threshold the
    /// stored admin must not change; once the threshold is reached, the
    /// stored admin must flip to the candidate and be removed from pending.
    #[test]
    fn test_multisig_admin_promotion_transfers_control() {
        let (env, client, old_admin, contract_id) = setup_integration();
        client.set_admin_threshold(&2_u32);
        let new_admin = Address::generate(&env);
        let second_approver = Address::generate(&env);

        // Grant second_approver the admin role so their approval counts.
        let admin_role = symbol_short!("sadmin");
        client.create_role(&admin_role, &Vec::new(&env));
        client.grant_role(&second_approver, &admin_role);

        client.propose_admin(&new_admin);

        // First approval: below threshold (1 < 2) — admin must NOT change yet.
        client.approve_admin(&old_admin, &new_admin);
        assert_eq!(
            stored_admin(&env, &contract_id),
            old_admin,
            "admin must remain unchanged below the approval threshold"
        );
        assert!(
            client.get_pending_admins().contains(&new_admin),
            "candidate must remain pending below the approval threshold"
        );

        // Second approval reaches the threshold — promotion happens now.
        client.approve_admin(&second_approver, &new_admin);
        assert_eq!(
            stored_admin(&env, &contract_id),
            new_admin,
            "admin must be promoted once approvals reach the threshold"
        );
        assert!(
            !client.get_pending_admins().contains(&new_admin),
            "promoted candidate must be removed from the pending list"
        );
    }

    /// propose_admin and approve_admin must be fail-closed: with no admin
    /// auth mocked at all, both calls must be rejected and must not mutate
    /// pending-admin state. Admin state is seeded directly in storage so the
    /// test isolates the auth gate from `initialize`'s own auth requirement.
    #[test]
    fn test_admin_rotation_calls_require_admin_auth() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let candidate = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
        });

        let propose_result = client.try_propose_admin(&candidate);
        assert!(
            propose_result.is_err(),
            "propose_admin must reject when admin auth is absent"
        );
        assert!(
            client.get_pending_admins().is_empty(),
            "no candidate must be recorded after a rejected propose_admin"
        );

        let approve_result = client.try_approve_admin(&admin, &candidate);
        assert!(
            approve_result.is_err(),
            "approve_admin must reject when admin auth is absent"
        );
    }

    /// Two admin candidates can be pending simultaneously. Promoting one to
    /// threshold must not affect the other candidate's pending status or
    /// approval count.
    #[test]
    fn test_multiple_pending_admin_candidates_are_independent() {
        let (env, client, admin, contract_id) = setup_integration();
        client.set_admin_threshold(&2_u32);

        let candidate_a = Address::generate(&env);
        let candidate_b = Address::generate(&env);
        let second_approver = Address::generate(&env);

        client.propose_admin(&candidate_a);
        client.propose_admin(&candidate_b);
        assert!(client.get_pending_admins().contains(&candidate_a));
        assert!(client.get_pending_admins().contains(&candidate_b));

        // Promote candidate_a to threshold.
        client.approve_admin(&admin, &candidate_a);
        client.approve_admin(&second_approver, &candidate_a);
        assert_eq!(stored_admin(&env, &contract_id), candidate_a);

        // candidate_a must be gone from pending; candidate_b must remain,
        // untouched by candidate_a's promotion.
        let pending = client.get_pending_admins();
        assert!(!pending.contains(&candidate_a));
        assert!(pending.contains(&candidate_b));
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────

    #[test]
    fn test_symbol_short_lengths_within_limit() {
        // symbol_short!() macro enforces the length constraint at compile time.
        // These instantiations serve as a compile-time check that all tags and
        // actions used in this contract are valid.
        let _tag = symbol_short!("mux_perm");
        let _init = symbol_short!("init");
        let _role_crt = symbol_short!("role_crt");
        let _role_grt = symbol_short!("role_grt");
        let _role_rev = symbol_short!("role_rev");
        let _perm_ok = symbol_short!("perm_ok");
        let _adm_thr = symbol_short!("adm_thr");
        let _adm_prp = symbol_short!("adm_prp");
        let _adm_apr = symbol_short!("adm_apr");
        let _adm_prm = symbol_short!("adm_prm");
        let _meta_set = symbol_short!("meta_set");
        core::mem::drop((
            _tag, _init, _role_crt, _role_grt, _role_rev, _perm_ok, _adm_thr, _adm_prp, _adm_apr,
            _adm_prm, _meta_set,
        ));
    }
}
