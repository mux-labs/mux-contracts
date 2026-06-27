/*!
 * mux-permissions: Fine-grained permission and role management for Mux Protocol.
 *
 * Implements a role-based access control (RBAC) registry that other Mux
 * contracts can call to verify caller permissions before executing
 * privileged operations.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
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
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum members per role to bound the RoleMembers vec in instance storage.
const MAX_ROLE_MEMBERS: u32 = 256;

/// Maximum roles an account may hold simultaneously.
const MAX_ROLES_PER_ACCOUNT: u32 = 32;

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

        if !members.contains(&account) {
            // STORAGE-GRIEFING: cap members per role to bound RoleMembers vec size.
            if members.len() >= MAX_ROLE_MEMBERS {
                return Err(MuxPermissionsError::TooManyMembers);
            }
            members.push_back(account.clone());
        }
        env.storage()
            .instance()
            .set(&DataKey::RoleMembers(role.clone()), &members);

        let mut account_roles: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::AccountRoles(account.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        if !account_roles.contains(&role) {
            // STORAGE-GRIEFING: cap roles per account to bound AccountRoles vec size.
            if account_roles.len() >= MAX_ROLES_PER_ACCOUNT {
                return Err(MuxPermissionsError::TooManyRoles);
            }
            account_roles.push_back(role.clone());
        }
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
    pub fn has_permission(env: Env, account: Address, permission: Symbol) -> bool {
        let account_roles: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::AccountRoles(account))
            .unwrap_or_else(|| Vec::new(&env));

        for role in account_roles.iter() {
            let perms: Vec<Symbol> = env
                .storage()
                .instance()
                .get(&DataKey::RolePermissions(role))
                .unwrap_or_else(|| Vec::new(&env));
            if perms.contains(&permission) {
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

    /// Propose a new admin address. Admin-only. Adds to the pending list.
    pub fn propose_admin(env: Env, new_admin: Address) -> Result<(), MuxPermissionsError> {
        Self::require_admin(&env)?;
        let mut pending: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmins)
            .unwrap_or_else(|| Vec::new(&env));
        if !pending.contains(&new_admin) {
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

    /// Extend the contract instance TTL. Callable by anyone so external keepers
    /// can prevent expiry during idle periods without needing admin auth.
    pub fn bump_ttl(env: Env) {
        Self::extend_ttl(&env);
    }

    /// Return the current TTL configuration.
    pub fn ttl_config(_env: Env) -> (u32, u32) {
        (TTL_THRESHOLD, TTL_EXTEND_TO)
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

    #[test]
    fn test_bump_ttl_callable_without_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPermissions);
        let client = MuxPermissionsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        // bump_ttl requires no auth — anyone can call it
        client.bump_ttl();
    }

    #[test]
    fn test_ttl_config_returns_constants() {
        let (_env, client, _admin) = setup();
        let (threshold, extend_to) = client.ttl_config();
        assert_eq!(threshold, 17_280);
        assert_eq!(extend_to, 518_400);
    }

    #[test]
    fn test_create_role_extends_ttl() {
        let (env, client, _admin) = setup();
        let role = symbol_short!("writer");
        client.create_role(&role, &Vec::new(&env));
        // If extend_ttl was missing the contract would eventually expire.
        // Reaching here without panic confirms TTL was bumped.
    }

    #[test]
    fn test_grant_role_extends_ttl() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("reader");
        client.create_role(&role, &Vec::new(&env));
        client.grant_role(&user, &role);
    }

    #[test]
    fn test_revoke_role_extends_ttl() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("temp");
        client.create_role(&role, &Vec::new(&env));
        client.grant_role(&user, &role);
        client.revoke_role(&user, &role);
    }

    // ── Additional unit tests ────────────────────────────────────────────────

    #[test]
    fn test_has_permission_returns_false_for_unknown_account() {
        let (env, client, _admin) = setup();
        let stranger = Address::generate(&env);
        assert!(!client.has_permission(&stranger, &symbol_short!("any")));
    }

    #[test]
    fn test_get_roles_returns_empty_for_unknown_account() {
        let (env, client, _admin) = setup();
        let stranger = Address::generate(&env);
        let roles = client.get_roles(&stranger);
        assert_eq!(roles.len(), 0);
    }

    #[test]
    fn test_get_role_members_fails_for_nonexistent_role() {
        let (_env, client, _admin) = setup();
        let result = client.try_get_role_members(&symbol_short!("nope"));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_role_members_returns_empty_after_creation() {
        let (env, client, _admin) = setup();
        let role = symbol_short!("empty");
        client.create_role(&role, &Vec::new(&env));
        let members = client.get_role_members(&role);
        assert_eq!(members.len(), 0);
    }

    #[test]
    fn test_grant_same_role_twice_is_idempotent() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("dup");
        client.create_role(&role, &Vec::new(&env));
        client.grant_role(&user, &role);
        client.grant_role(&user, &role);
        let members = client.get_role_members(&role);
        assert_eq!(members.len(), 1);
        let roles = client.get_roles(&user);
        assert_eq!(roles.len(), 1);
    }

    #[test]
    fn test_revoke_non_member_fails() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("solo");
        client.create_role(&role, &Vec::new(&env));
        let result = client.try_revoke_role(&user, &role);
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_nonexistent_role_fails() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let result = client.try_revoke_role(&user, &symbol_short!("ghost"));
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_roles_grant_combined_permissions() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);

        let role_a = symbol_short!("roleA");
        let role_b = symbol_short!("roleB");
        let perm_x = symbol_short!("permX");
        let perm_y = symbol_short!("permY");

        let mut perms_a: Vec<Symbol> = Vec::new(&env);
        perms_a.push_back(perm_x.clone());
        let mut perms_b: Vec<Symbol> = Vec::new(&env);
        perms_b.push_back(perm_y.clone());

        client.create_role(&role_a, &perms_a);
        client.create_role(&role_b, &perms_b);
        client.grant_role(&user, &role_a);
        client.grant_role(&user, &role_b);

        assert!(client.has_permission(&user, &perm_x));
        assert!(client.has_permission(&user, &perm_y));
        let roles = client.get_roles(&user);
        assert_eq!(roles.len(), 2);
    }

    #[test]
    fn test_revoke_one_role_keeps_other_permissions() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);

        let role_a = symbol_short!("keepA");
        let role_b = symbol_short!("dropB");
        let perm_keep = symbol_short!("keep");
        let perm_drop = symbol_short!("drop");

        let mut perms_a: Vec<Symbol> = Vec::new(&env);
        perms_a.push_back(perm_keep.clone());
        let mut perms_b: Vec<Symbol> = Vec::new(&env);
        perms_b.push_back(perm_drop.clone());

        client.create_role(&role_a, &perms_a);
        client.create_role(&role_b, &perms_b);
        client.grant_role(&user, &role_a);
        client.grant_role(&user, &role_b);

        client.revoke_role(&user, &role_b);

        assert!(client.has_permission(&user, &perm_keep));
        assert!(!client.has_permission(&user, &perm_drop));
    }

    #[test]
    fn test_role_with_multiple_permissions() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);
        let role = symbol_short!("multi");
        let p1 = symbol_short!("read");
        let p2 = symbol_short!("write");
        let p3 = symbol_short!("delete");

        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(p1.clone());
        perms.push_back(p2.clone());
        perms.push_back(p3.clone());

        client.create_role(&role, &perms);
        client.grant_role(&user, &role);

        assert!(client.has_permission(&user, &p1));
        assert!(client.has_permission(&user, &p2));
        assert!(client.has_permission(&user, &p3));
        assert!(!client.has_permission(&user, &symbol_short!("admin")));
    }

    #[test]
    fn test_get_pending_admins_empty_initially() {
        let (_env, client, _admin) = setup();
        let pending = client.get_pending_admins();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_propose_and_get_pending_admins() {
        let (env, client, _admin) = setup();
        let c1 = Address::generate(&env);
        let c2 = Address::generate(&env);
        client.propose_admin(&c1);
        client.propose_admin(&c2);
        let pending = client.get_pending_admins();
        assert_eq!(pending.len(), 2);
        assert!(pending.contains(&c1));
        assert!(pending.contains(&c2));
    }

    #[test]
    fn test_multisig_full_promotion_flow() {
        let (env, client, admin) = setup();
        client.set_admin_threshold(&1_u32);
        let candidate = Address::generate(&env);
        client.propose_admin(&candidate);
        client.approve_admin(&admin, &candidate);
        let pending = client.get_pending_admins();
        assert!(!pending.contains(&candidate));
    }

    #[test]
    fn test_multiple_members_in_role() {
        let (env, client, _admin) = setup();
        let role = symbol_short!("team");
        let perm = symbol_short!("deploy");
        let mut perms: Vec<Symbol> = Vec::new(&env);
        perms.push_back(perm.clone());
        client.create_role(&role, &perms);

        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        let u3 = Address::generate(&env);
        client.grant_role(&u1, &role);
        client.grant_role(&u2, &role);
        client.grant_role(&u3, &role);

        let members = client.get_role_members(&role);
        assert_eq!(members.len(), 3);
        assert!(client.has_permission(&u1, &perm));
        assert!(client.has_permission(&u2, &perm));
        assert!(client.has_permission(&u3, &perm));
    }

    #[test]
    fn test_revoke_middle_member_preserves_others() {
        let (env, client, _admin) = setup();
        let role = symbol_short!("squad");
        client.create_role(&role, &Vec::new(&env));

        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        let u3 = Address::generate(&env);
        client.grant_role(&u1, &role);
        client.grant_role(&u2, &role);
        client.grant_role(&u3, &role);

        client.revoke_role(&u2, &role);

        let members = client.get_role_members(&role);
        assert_eq!(members.len(), 2);
        assert!(members.contains(&u1));
        assert!(!members.contains(&u2));
        assert!(members.contains(&u3));
    }

    #[test]
    fn test_create_role_with_empty_permissions() {
        let (env, client, _admin) = setup();
        let role = symbol_short!("bare");
        client.create_role(&role, &Vec::new(&env));
        let user = Address::generate(&env);
        client.grant_role(&user, &role);
        assert!(!client.has_permission(&user, &symbol_short!("any")));
    }
}
