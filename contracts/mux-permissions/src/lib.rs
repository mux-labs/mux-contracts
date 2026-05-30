/*!
 * mux-permissions: Fine-grained permission and role management for Mux Protocol.
 *
 * Implements a role-based access control (RBAC) registry that other Mux
 * contracts can call to verify caller permissions before executing
 * privileged operations.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    RoleMembers(Symbol),
    RolePermissions(Symbol),
    AccountRoles(Address),
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
}

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
            .set(&DataKey::RolePermissions(role), &permissions);
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
            account_roles.push_back(role.clone());
        }
        env.storage()
            .instance()
            .set(&DataKey::AccountRoles(account), &account_roles);

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
                .set(&DataKey::AccountRoles(account), &account_roles);
        }

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
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Env, Vec};

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
}
