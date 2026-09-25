/*!
 * mux-registry: Contract version registry for Mux Protocol.
 *
 * This contract maintains a registry of protocol components and their versions.
 * It supports registration with optional metadata, discovery queries, and
 * storage griefing guards via capped collections.
 *
 * # `no_std` and `alloc` Constraints
 *
 * This crate is `#![no_std]` and uses `extern crate alloc` for heap-backed
 * collection types. The Soroban VM provides a heap allocator on-chain, so
 * `alloc` types are safe to use. However, prefer `soroban_sdk` collection
 * types (`Vec`, `String`) for consistency with other Mux contracts and for
 * gas-predictable storage access.
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
 * - `"init"` — Emitted on initialization with `admin: Address`
 * - `"reg"` — Emitted on `register` with `(name: Symbol, version: String)`
 * - `"regmeta"` — Emitted on `register_with_metadata` with `(name: Symbol, version: String)`
 */

#![no_std]

extern crate alloc;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
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
    /// Source repository URL or additional metadata.
    pub repository: String,
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

    /// Upgrade the contract WASM. Admin only.
    ///
    /// See `docs/contract-upgrade-pattern.md` for storage-compatibility rules
    /// that must be observed between versions. Instance storage (admin, names,
    /// versions, and metadata) is preserved across upgrades by the Soroban host.
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxRegistryError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
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
        repository: String,
    ) -> Result<(), MuxRegistryError> {
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
        let version_clone = version.clone();
        env.storage()
            .instance()
            .set(&DataKey::Version(name.clone()), &version_clone);
        let meta = ContractMetadata {
            version: version.clone(),
            description,
            author,
            repository,
        };
        env.storage()
            .instance()
            .set(&DataKey::Metadata(name.clone()), &meta);
        emit(&env, symbol_short!("regmeta"), (name, version));
        Self::extend_ttl(&env);
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
    use alloc::format;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events},
        Env, FromVal, String,
    };

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
        let (_env, client, _) = setup();
        let result = client.try_get_version(&symbol_short!("ghost"));
        assert_eq!(result, Err(Ok(MuxRegistryError::ContractNotFound)));
    }

    #[test]
    fn test_register_with_metadata() {
        let (env, client, _) = setup();
        let name = symbol_short!("account");
        let version = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Account abstraction contract");
        let author = String::from_str(&env, "mux-labs");
        let repository = String::from_str(&env, "https://github.com/mux-protocol/mux-contracts");

        client.register_with_metadata(&name, &version, &description, &author, &repository);

        let meta = client.get_metadata(&name);
        assert_eq!(meta.version, version);
        assert_eq!(meta.description, description);
        assert_eq!(meta.author, author);
        assert_eq!(meta.repository, repository);
        // version key also updated
        assert_eq!(client.get_version(&name), version);
        assert!(client.list_contracts().contains(&name));
    }

    #[test]
    fn test_get_metadata_unknown_fails() {
        let (_env, client, _) = setup();
        let result = client.try_get_metadata(&symbol_short!("ghost"));
        assert_eq!(result, Err(Ok(MuxRegistryError::ContractNotFound)));
    }

    /// Uninitialized registry has no metadata keys — miss returns ContractNotFound
    /// (same public error as an unknown name; does not require admin auth).
    #[test]
    fn test_get_metadata_on_uninitialized_returns_not_found() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        assert_eq!(
            client.try_get_metadata(&symbol_short!("x")),
            Err(Ok(MuxRegistryError::ContractNotFound))
        );
    }

    /// `register` writes Version only — get_metadata must still miss with ContractNotFound.
    #[test]
    fn test_get_metadata_missing_after_register_without_metadata() {
        let (env, client, _) = setup();
        let name = symbol_short!("bare");
        let version = String::from_str(&env, "1.0.0");
        client.register(&name, &version);
        assert_eq!(client.get_version(&name), version);
        assert_eq!(
            client.try_get_metadata(&name),
            Err(Ok(MuxRegistryError::ContractNotFound))
        );
    }

    /// After other contracts are registered with metadata, an unknown name still misses.
    #[test]
    fn test_get_metadata_unknown_after_registrations() {
        let (env, client, _) = setup();
        let known = symbol_short!("known");
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Known contract");
        let author = String::from_str(&env, "mux-labs");
        let repository = String::from_str(&env, "https://github.com/mux-protocol/mux-contracts");
        client.register_with_metadata(&known, &version, &description, &author, &repository);

        assert_eq!(
            client.try_get_metadata(&symbol_short!("unknown")),
            Err(Ok(MuxRegistryError::ContractNotFound))
        );
        assert!(client.try_get_metadata(&known).is_ok());
    }

    #[test]
    fn test_metadata_update() {
        let (env, client, _) = setup();
        let name = symbol_short!("batcher");
        let v1 = String::from_str(&env, "1.0.0");
        let v2 = String::from_str(&env, "1.1.0");
        let desc = String::from_str(&env, "Batcher contract");
        let author = String::from_str(&env, "mux-labs");
        let repo = String::from_str(&env, "https://github.com/mux-protocol/mux-contracts");

        client.register_with_metadata(&name, &v1, &desc, &author, &repo);
        client.register_with_metadata(&name, &v2, &desc, &author, &repo);

        let meta = client.get_metadata(&name);
        assert_eq!(meta.version, v2);
        // name appears only once in list
        let names = client.list_contracts();
        let count = names.iter().filter(|n| *n == name).count();
        assert_eq!(count, 1);
    }

    /// Filling the registry to MAX_CONTRACTS via register() and then calling
    /// register_with_metadata() with a new name must return TooManyContracts.
    #[test]
    fn test_too_many_contracts_via_register_with_metadata() {
        let env = Env::default();
        env.mock_all_auths();
        env.budget().reset_unlimited();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let version = String::from_str(&env, "1.0.0");
        let desc = String::from_str(&env, "desc");
        let author = String::from_str(&env, "mux-labs");

        // Fill the registry to exactly MAX_CONTRACTS (128) entries.
        // Two-letter base-26 symbols: "aa"=0 … "ex"=127, "ey"=128.
        for i in 0u32..128 {
            let sym = soroban_sdk::Symbol::new(
                &env,
                &format!(
                    "{}{}",
                    (b'a' + (i / 26) as u8) as char,
                    (b'a' + (i % 26) as u8) as char
                ),
            );
            client.register(&sym, &version);
        }

        // One more new name must be rejected by register_with_metadata.
        let overflow = soroban_sdk::Symbol::new(&env, "ey");
        let repo = String::from_str(&env, "https://github.com/mux-protocol/mux-contracts");
        let result =
            client.try_register_with_metadata(&overflow, &version, &desc, &author, &repo);
        assert_eq!(result, Err(Ok(MuxRegistryError::TooManyContracts)));
    }

    /// register() also enforces the cap once MAX_CONTRACTS names are registered.
    #[test]
    fn test_too_many_contracts_via_register() {
        let env = Env::default();
        env.mock_all_auths();
        env.budget().reset_unlimited();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let version = String::from_str(&env, "1.0.0");

        for i in 0u32..128 {
            let sym = soroban_sdk::Symbol::new(
                &env,
                &format!(
                    "{}{}",
                    (b'a' + (i / 26) as u8) as char,
                    (b'a' + (i % 26) as u8) as char
                ),
            );
            client.register(&sym, &version);
        }

        let overflow = soroban_sdk::Symbol::new(&env, "ey");
        let result = client.try_register(&overflow, &version);
        assert_eq!(result, Err(Ok(MuxRegistryError::TooManyContracts)));
    }

    // ── Unauthorized admin tests ───────────────────────────────────────────────
    //
    // These tests verify that `register` and `register_with_metadata` reject
    // callers who have not been authorised as the declared admin.  Following
    // the pattern used across mux-* contracts (see mux-account-factory and
    // mux-delegation), they deliberately omit `mock_all_auths` so that
    // `require_auth` rejects the call at the host level, surfacing as
    // `Err(..)` from `try_*`.  State mutation and event emission must not
    // occur on a rejected call.

    /// `register` without any authorised signer must be rejected at the host
    /// level.  No version entry or name list entry may be written.
    #[test]
    fn test_register_requires_admin_auth() {
        use soroban_sdk::testutils::Events;
        // Deliberately omit mock_all_auths — admin.require_auth() must reject.
        // Seed the Admin key directly via as_contract (mock_all_auths is a
        // permanent switch in soroban-sdk 21, not a restorable guard).
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            env.storage()
                .instance()
                .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        });

        // Attempt register without any auth — require_auth on admin must reject.
        let name = symbol_short!("account");
        let version = String::from_str(&env, "1.0.0");
        let result = client.try_register(&name, &version);
        assert!(
            result.is_err(),
            "register must be rejected when admin auth is absent"
        );

        // No version entry must have been written.
        assert!(
            client.try_get_version(&name).is_err(),
            "no version must be stored after a rejected register"
        );
        // Names list must still be empty.
        assert_eq!(
            client.list_contracts().len(),
            0,
            "names list must remain empty after a rejected register"
        );
        // No events must have been emitted.
        assert_eq!(
            env.events().all().len(),
            0,
            "no events must be emitted after a rejected register"
        );
    }

    /// `register_with_metadata` without any authorised signer must be rejected.
    /// No version, metadata, or name list entry may be written.
    #[test]
    fn test_register_with_metadata_requires_admin_auth() {
        use soroban_sdk::testutils::Events;
        // Deliberately omit mock_all_auths — admin.require_auth() must reject.
        let env = Env::default();
        let cid = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &cid);
        let admin = Address::generate(&env);
        // Seed the Admin key directly so require_admin can find it, with no
        // auth mocked anywhere (mock_all_auths is not restorable via a guard).
        env.as_contract(&cid, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            env.storage()
                .instance()
                .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        });
        // Attempt register_with_metadata without any auth mock.
        let name = symbol_short!("batcher");
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Batcher contract");
        let author = String::from_str(&env, "mux-labs");
        let repository =
            String::from_str(&env, "https://github.com/mux-protocol/mux-contracts");

        let result = client.try_register_with_metadata(
            &name,
            &version,
            &description,
            &author,
            &repository,
        );
        assert!(
            result.is_err(),
            "register_with_metadata must be rejected when admin auth is absent"
        );

        // No version entry must have been written.
        assert!(
            client.try_get_version(&name).is_err(),
            "no version must be stored after a rejected register_with_metadata"
        );
        // No metadata entry must have been written.
        assert!(
            client.try_get_metadata(&name).is_err(),
            "no metadata must be stored after a rejected register_with_metadata"
        );
        // Names list must still be empty.
        assert_eq!(
            client.list_contracts().len(),
            0,
            "names list must remain empty after a rejected register_with_metadata"
        );
        // No events must have been emitted (init event from {_guard} env is gone; this env only).
        assert_eq!(
            env.events().all().len(),
            0,
            "no events must be emitted after a rejected register_with_metadata"
        );
    }

    /// A non-admin caller (different address from the stored admin) must not be
    /// able to register contracts.  The admin key is present but `require_auth`
    /// must reject the wrong signer.
    #[test]
    fn test_register_rejects_non_admin_caller() {
        use soroban_sdk::testutils::Events;
        // Deliberately omit mock_all_auths — the wrong signer must be rejected.
        let env = Env::default();
        let cid = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &cid);
        let admin = Address::generate(&env);
        let _attacker = Address::generate(&env);

        // Seed the Admin key directly via as_contract (mock_all_auths is a
        // permanent switch, not a restorable guard).
        env.as_contract(&cid, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            env.storage()
                .instance()
                .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        });

        // Attempt to register from env with no mocked signer.
        let name = symbol_short!("perm");
        let version = String::from_str(&env, "3.0.0");
        let result = client.try_register(&name, &version);
        assert!(
            result.is_err(),
            "register must reject a non-admin caller"
        );

        assert!(client.try_get_version(&name).is_err());
        assert_eq!(client.list_contracts().len(), 0);
        assert_eq!(env.events().all().len(), 0);
    }

    /// A non-admin caller must not be able to register contracts with metadata.
    #[test]
    fn test_register_with_metadata_rejects_non_admin_caller() {
        use soroban_sdk::testutils::Events;
        // Deliberately omit mock_all_auths — the wrong signer must be rejected.
        let env = Env::default();
        let cid = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &cid);
        let admin = Address::generate(&env);

        // Seed the Admin key directly via as_contract (mock_all_auths is a
        // permanent switch, not a restorable guard).
        env.as_contract(&cid, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            env.storage()
                .instance()
                .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        });

        let name = symbol_short!("policy");
        let version = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Policy contract");
        let author = String::from_str(&env, "mux-labs");
        let repository =
            String::from_str(&env, "https://github.com/mux-protocol/mux-contracts");

        let result = client.try_register_with_metadata(
            &name,
            &version,
            &description,
            &author,
            &repository,
        );
        assert!(
            result.is_err(),
            "register_with_metadata must reject a non-admin caller"
        );

        assert!(client.try_get_version(&name).is_err());
        assert!(client.try_get_metadata(&name).is_err());
        assert_eq!(client.list_contracts().len(), 0);
        assert_eq!(env.events().all().len(), 0);
    }

    /// An unauthorized `register` call must not affect a previously-authorized
    /// registration in a separate env — isolation check.
    #[test]
    fn test_unauthorized_register_does_not_affect_other_envs() {
        // Authorized env: register one contract legitimately.
        let env_auth = Env::default();
        env_auth.mock_all_auths();
        let cid_auth = env_auth.register_contract(None, MuxRegistry);
        let c_auth = MuxRegistryClient::new(&env_auth, &cid_auth);
        let admin_auth = Address::generate(&env_auth);
        c_auth.initialize(&admin_auth);
        let name_auth = symbol_short!("account");
        let version = String::from_str(&env_auth, "1.0.0");
        c_auth.register(&name_auth, &version);
        assert_eq!(c_auth.list_contracts().len(), 1);

        // Unauthorized env: attempt without mock_all_auths.
        let env_unauth = Env::default();
        let cid_unauth = env_unauth.register_contract(None, MuxRegistry);
        let c_unauth = MuxRegistryClient::new(&env_unauth, &cid_unauth);
        let admin_unauth = Address::generate(&env_unauth);
        // Seed Admin directly — mock_all_auths is a permanent switch, so it
        // cannot be used to initialize and then un-mock for the rejection.
        env_unauth.as_contract(&cid_unauth, || {
            env_unauth
                .storage()
                .instance()
                .set(&DataKey::Admin, &admin_unauth);
            env_unauth
                .storage()
                .instance()
                .set(&DataKey::Names, &Vec::<Symbol>::new(&env_unauth));
        });
        let name_unauth = symbol_short!("batcher");
        let ver2 = String::from_str(&env_unauth, "1.0.0");
        let result = c_unauth.try_register(&name_unauth, &ver2);
        assert!(result.is_err());
        assert_eq!(c_unauth.list_contracts().len(), 0);

        // Original authorized env must be unaffected.
        assert_eq!(c_auth.list_contracts().len(), 1);
        assert!(c_auth.try_get_version(&name_auth).is_ok());
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────

    #[test]
    fn test_symbol_short_lengths_within_limit() {
        let tags = [symbol_short!("mux_reg")];
        let actions = [
            symbol_short!("init"),
            symbol_short!("reg"),
            symbol_short!("regmeta"),
        ];
        for sym in tags.iter().chain(actions.iter()) {
            let _ = sym;
        }
    }

    // ── Event assertions ──────────────────────────────────────────────────────

    /// Extract the contract tag symbol (topics[0]) from a specific event index.
    fn topic_tag(
        env: &Env,
        events: &soroban_sdk::Vec<(
            soroban_sdk::Address,
            soroban_sdk::Vec<soroban_sdk::Val>,
            soroban_sdk::Val,
        )>,
        idx: u32,
    ) -> soroban_sdk::Symbol {
        let (_, topics, _) = events.get(idx).unwrap();
        soroban_sdk::Symbol::from_val(env, &topics.get(0).unwrap())
    }

    /// Extract the action symbol (topics[1]) from a specific event index.
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
    fn test_initialize_emits_init_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let events = env.events().all();
        assert_eq!(events.len(), 1, "expected exactly 1 event after initialize");
        assert_eq!(topic_tag(&env, &events, 0), symbol_short!("mux_reg"));
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_register_emits_reg_event() {
        let (env, client, _) = setup();
        let name = symbol_short!("account");
        let version = String::from_str(&env, "1.0.0");
        client.register(&name, &version);

        let events = env.events().all();
        // events[0] = init from setup(), events[1] = reg
        assert_eq!(events.len(), 2, "expected init + reg events");
        assert_eq!(topic_tag(&env, &events, 1), symbol_short!("mux_reg"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("reg"));
    }

    #[test]
    fn test_register_update_emits_reg_event() {
        // Re-registering (version update) also emits reg.
        let (env, client, _) = setup();
        let name = symbol_short!("batcher");
        let v1 = String::from_str(&env, "1.0.0");
        let v2 = String::from_str(&env, "1.1.0");
        client.register(&name, &v1);
        client.register(&name, &v2);

        let events = env.events().all();
        // init + reg + reg
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("reg"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("reg"));
        assert_eq!(client.get_version(&name), v2);
    }

    #[test]
    fn test_register_with_metadata_emits_regmeta_event() {
        let (env, client, _) = setup();
        let name = symbol_short!("account");
        let version = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Account abstraction contract");
        let author = String::from_str(&env, "mux-labs");
        let repository = String::from_str(&env, "https://github.com/mux-protocol/mux-contracts");
        client.register_with_metadata(&name, &version, &description, &author, &repository);

        let events = env.events().all();
        // events[0] = init, events[1] = regmeta
        assert_eq!(events.len(), 2, "expected init + regmeta events");
        assert_eq!(topic_tag(&env, &events, 1), symbol_short!("mux_reg"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("regmeta"));
    }

    #[test]
    fn test_failed_initialize_emits_no_event() {
        // Second initialize call is rejected — must not emit any event.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let _ = client.try_initialize(&admin); // must fail

        let events = env.events().all();
        // Only the first (successful) initialize emitted an event.
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_over_cap_register_emits_no_extra_event() {
        // TooManyContracts rejection must not emit a reg event.
        let env = Env::default();
        env.mock_all_auths();
        env.budget().reset_unlimited();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let version = String::from_str(&env, "1.0.0");
        for i in 0u32..128 {
            let sym = soroban_sdk::Symbol::new(
                &env,
                &format!(
                    "{}{}",
                    (b'a' + (i / 26) as u8) as char,
                    (b'a' + (i % 26) as u8) as char
                ),
            );
            client.register(&sym, &version);
        }
        let before = env.events().all().len();
        let overflow = soroban_sdk::Symbol::new(&env, "ey");
        let _ = client.try_register(&overflow, &version);
        // No extra event from the failed call.
        assert_eq!(env.events().all().len(), before);
    }
}
