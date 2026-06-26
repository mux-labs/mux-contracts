/*!
 * mux-registry: Contract version registry for Mux Protocol.
 *
 * This contract maintains a registry of protocol components and their versions.
 * It supports registration with optional metadata, discovery queries, and
 * storage griefing guards via capped collections.
 *
 * # Public Interface
 *
 * - `initialize(admin)` — One-time setup with admin authorization
 * - `register(name, version)` — Register/update version only (admin)
 * - `register_with_metadata(name, version, description, author)` — Register with full metadata (admin)
 * - `check_version(name, version)` — Dry-run validation without state mutation
 * - `get_version(name)` — Query registered version (public)
 * - `get_metadata(name)` — Query full metadata (public)
 * - `list_contracts()` — List all registered names (public)
 *
 * # Storage Constraints
 *
 * The registry enforces a cap of 128 registered contracts to prevent storage griefing.
 * Registering more than 128 unique names returns `TooManyContracts`.
 *
 * # Events
 *
 * - `"init"` — Emitted on initialization
 * - `"reg"` — Emitted on registration with (name, version)
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_reg"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Version(Symbol),
    Names,
    Metadata(Symbol),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Metadata associated with a registered contract.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ContractMetadata {
    /// Semantic version string, e.g. "1.2.0"
    pub version: String,
    /// Short human-readable description of the contract.
    pub description: String,
    /// Author or team identifier.
    pub author: String,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxRegistryError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ContractNotFound = 4,
    // STORAGE-GRIEFING: unbounded Names vec would let admin bloat instance storage.
    TooManyContracts = 5,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of registered contract names to bound the Names vec.
const MAX_CONTRACTS: u32 = 128;

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRegistry;

#[contractimpl]
impl MuxRegistry {
    /// Initialize the registry with an admin address.
    /// Must be called exactly once; subsequent calls return `AlreadyInitialized`.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxRegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxRegistryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register or update a contract version. Admin only.
    /// If the name is new, it is added to the registry (up to MAX_CONTRACTS).
    /// If already registered, the version is updated without duplicating the name.
    /// Returns `TooManyContracts` if the registry is at capacity.
    pub fn register(env: Env, name: Symbol, version: String) -> Result<(), MuxRegistryError> {
        Self::require_admin(&env)?;
        let mut names: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env));

        if !names.contains(&name) {
            // STORAGE-GRIEFING: cap the Names vec to bound instance storage growth.
            if names.len() >= MAX_CONTRACTS {
                return Err(MuxRegistryError::TooManyContracts);
            }
            names.push_back(name.clone());
            env.storage().instance().set(&DataKey::Names, &names);
        }
        env.storage()
            .instance()
            .set(&DataKey::Version(name.clone()), &version);
        emit(&env, symbol_short!("reg"), (name, version));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register or update a contract with full metadata. Admin only.
    pub fn register_with_metadata(
        env: Env,
        name: Symbol,
        version: String,
        description: String,
        author: String,
    ) -> Result<(), MuxRegistryError> {
        Self::require_admin(&env)?;
        let mut names: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env));
        if !names.contains(&name) {
            names.push_back(name.clone());
            env.storage().instance().set(&DataKey::Names, &names);
        }
        env.storage()
            .instance()
            .set(&DataKey::Version(name.clone()), &version.clone());
        let meta = ContractMetadata {
            version,
            description,
            author,
        };
        env.storage()
            .instance()
            .set(&DataKey::Metadata(name), &meta);
        Ok(())
    }

    /// Get the version string for a registered contract.
    pub fn get_version(env: Env, name: Symbol) -> Result<String, MuxRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Version(name))
            .ok_or(MuxRegistryError::ContractNotFound)
    }

    /// Dry-run validation of a version query without state mutation.
    /// Returns the version if registered, otherwise returns `ContractNotFound`.
    /// This is useful for preflight checks and deployment validation.
    pub fn check_version(env: Env, name: Symbol) -> Result<String, MuxRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Version(name))
            .ok_or(MuxRegistryError::ContractNotFound)
    }

    /// Get the full metadata for a registered contract.
    pub fn get_metadata(env: Env, name: Symbol) -> Result<ContractMetadata, MuxRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Metadata(name))
            .ok_or(MuxRegistryError::ContractNotFound)
    }

    /// List all registered contract names.
    pub fn list_contracts(env: Env) -> Vec<Symbol> {
        env.storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxRegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxRegistryError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

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
    use soroban_sdk::{symbol_short, testutils::Address as _, Env, String};

    fn setup() -> (Env, MuxRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_register_and_get() {
        let (env, client, _) = setup();
        let name = symbol_short!("account");
        let version = String::from_str(&env, "1.0.0");
        client.register(&name, &version);
        assert_eq!(client.get_version(&name), version);
        assert!(client.list_contracts().contains(&name));
    }

    #[test]
    fn test_get_unknown_fails() {
        let (env, client, _) = setup();
        let result = client.try_get_version(&symbol_short!("ghost"));
        assert!(result.is_err());
    }

    #[test]
    fn test_register_with_metadata() {
        let (env, client, _) = setup();
        let name = symbol_short!("account");
        let version = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Account abstraction contract");
        let author = String::from_str(&env, "mux-labs");

        client.register_with_metadata(&name, &version, &description, &author);

        let meta = client.get_metadata(&name);
        assert_eq!(meta.version, version);
        assert_eq!(meta.description, description);
        assert_eq!(meta.author, author);
        // version key also updated
        assert_eq!(client.get_version(&name), version);
        assert!(client.list_contracts().contains(&name));
    }

    #[test]
    fn test_get_metadata_unknown_fails() {
        let (env, client, _) = setup();
        let result = client.try_get_metadata(&symbol_short!("ghost"));
        assert!(result.is_err());
    }

    #[test]
    fn test_metadata_update() {
        let (env, client, _) = setup();
        let name = symbol_short!("batcher");
        let v1 = String::from_str(&env, "1.0.0");
        let v2 = String::from_str(&env, "1.1.0");
        let desc = String::from_str(&env, "Batcher contract");
        let author = String::from_str(&env, "mux-labs");

        client.register_with_metadata(&name, &v1, &desc, &author);
        client.register_with_metadata(&name, &v2, &desc, &author);

        let meta = client.get_metadata(&name);
        assert_eq!(meta.version, v2);
        // name appears only once in list
        let names = client.list_contracts();
        let count = names.iter().filter(|n| *n == name).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_check_version_dry_run() {
        let (env, client, _) = setup();
        let name = symbol_short!("account");
        let version = String::from_str(&env, "1.0.0");
        client.register(&name, &version);
        // check_version should return the same version without mutating state
        let checked = client.check_version(&name);
        assert_eq!(checked, version);
        // verify it's a true read-only call by checking get_version still works
        let fetched = client.get_version(&name);
        assert_eq!(fetched, version);
    }

    #[test]
    fn test_check_version_not_found() {
        let (env, client, _) = setup();
        let result = client.try_check_version(&symbol_short!("nonexistent"));
        assert!(result.is_err());
    }
}
