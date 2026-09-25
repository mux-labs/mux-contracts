/*!
 * mux-registry: Contract version and metadata registry for Mux Protocol.
 *
 * Maintains an on-chain directory of deployed Mux contract crate names and
 * their versions. The registry is used by upgrades, off-chain tooling, and
 * integration tests to verify that the correct contract version is running.
 *
 * # Fail-closed auth
 *
 * Every state-mutating entrypoint requires `require_admin` to pass before
 * any state is modified. `require_admin` calls `admin.require_auth()` on the
 * stored admin address, failing closed: if no admin has been initialised, or
 * if the caller is not the admin, the call reverts with `Unauthorized` before
 * touching any storage.
 *
 * # Audit Events
 *
 * Contract tag: `mux_reg`
 *
 * | Action      | Trigger                    | Data payload                         |
 * |-------------|----------------------------|--------------------------------------|
 * | `init`      | `initialize`               | `admin: Address`                     |
 * | `reg`       | `register`                 | `(name: Symbol, version: String)`    |
 * | `regmeta`   | `register_with_metadata`   | `(name: Symbol, version: String)`    |
 *
 * `get_version`, `check_version`, `get_metadata`, and `list_contracts` are
 * read-only and emit no events.
 * `upgrade` emits no event — the upload/invoke transaction is the audit record.
 *
 * See [`docs/audit-events.md`] and [`docs/registry-contracts-comparison.md`].
 *
 * [`docs/audit-events.md`]: ../../docs/audit-events.md
 * [`docs/registry-contracts-comparison.md`]: ../../docs/registry-contracts-comparison.md
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────

fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_reg"), action), data);
}

// ── TTL constants ─────────────────────────────────────────────────────────────

/// Minimum TTL threshold (ledgers) before auto-extend triggers.
/// At ~5 s per ledger: 17 280 ≈ 1 day.
const TTL_THRESHOLD: u32 = 17_280;

/// Target TTL (ledgers) after auto-extend.
/// At ~5 s per ledger: 518 400 ≈ 30 days.
const TTL_EXTEND_TO: u32 = 518_400;

// ── Error codes ───────────────────────────────────────────────────────────────

/// Stable error codes returned by mux-registry.
///
/// These codes are on-chain ABI — changing an existing variant is a
/// breaking change. New variants may be appended.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// The contract has not been initialised.
    NotInitialized = 1,
    /// The contract has already been initialised.
    AlreadyInitialized = 2,
    /// The caller is not the contract admin.
    Unauthorized = 3,
    /// No entry found for the requested contract name.
    ContractNotFound = 4,
    /// A version string that is empty or otherwise invalid was supplied.
    InvalidVersion = 5,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Storage keys for the mux-registry contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address (`Address`).
    Admin,
    /// Per-contract version record keyed by contract name (`Symbol`).
    Contract(Symbol),
    /// Per-contract extended metadata keyed by contract name (`Symbol`).
    Metadata(Symbol),
    /// Ordered list of registered contract names.
    ContractList,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A version record stored for each registered contract.
#[contracttype]
#[derive(Clone, Debug)]
pub struct VersionRecord {
    /// Semver version string (e.g. `"0.1.0"`).
    pub version: String,
    /// Ledger at which this version was registered.
    pub registered_at: u32,
}

/// Optional extended metadata for a registered contract.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ContractMeta {
    /// Human-readable description of the contract.
    pub description: String,
    /// Additional free-form tags or notes.
    pub notes: String,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRegistry;

#[contractimpl]
impl MuxRegistry {
    // ── Auth helpers ──────────────────────────────────────────────────────────

    /// Fail-closed admin guard.
    ///
    /// Reads the stored admin address and calls `require_auth()` on it.
    /// Reverts with [`RegistryError::NotInitialized`] if no admin is stored,
    /// or with [`RegistryError::Unauthorized`] if the caller is not the admin.
    ///
    /// Every state-mutating entrypoint MUST call `require_admin` before
    /// reading or writing any other storage key.
    fn require_admin(env: &Env) -> Result<Address, RegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();
        Ok(admin)
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    /// Initialise the contract and set the admin address.
    ///
    /// Fails with [`RegistryError::AlreadyInitialized`] if called more than once.
    ///
    /// Emits: `init`
    pub fn initialize(env: Env, admin: Address) -> Result<(), RegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(RegistryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::Admin, &admin);
        // Initialise the contract list as empty.
        let empty: Vec<Symbol> = Vec::new(&env);
        env.storage()
            .instance()
            .set(&DataKey::ContractList, &empty);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("init"), admin);
        Ok(())
    }

    /// Upgrade the contract WASM.
    ///
    /// Requires admin authorisation. Extends instance TTL after the upgrade.
    /// Does not emit an audit event — the upload/invoke transaction is the
    /// on-chain audit record.
    pub fn upgrade(env: Env, new_wasm_hash: soroban_sdk::BytesN<32>) -> Result<(), RegistryError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    // ── Admin entrypoints ─────────────────────────────────────────────────────

    /// Register or update a contract's version entry.
    ///
    /// Requires admin authorisation (fail-closed via [`Self::require_admin`]).
    /// Creates a new entry if `name` is not already registered; otherwise
    /// updates the existing entry.
    ///
    /// Emits: `reg`
    pub fn register(env: Env, name: Symbol, version: String) -> Result<(), RegistryError> {
        Self::require_admin(&env)?;
        if version.len() == 0 {
            return Err(RegistryError::InvalidVersion);
        }
        let record = VersionRecord {
            version: version.clone(),
            registered_at: env.ledger().sequence(),
        };
        let is_new = !env
            .storage()
            .persistent()
            .has(&DataKey::Contract(name.clone()));
        env.storage()
            .persistent()
            .set(&DataKey::Contract(name.clone()), &record);

        if is_new {
            let mut list: Vec<Symbol> = env
                .storage()
                .instance()
                .get(&DataKey::ContractList)
                .unwrap_or_else(|| Vec::new(&env));
            list.push_back(name.clone());
            env.storage()
                .instance()
                .set(&DataKey::ContractList, &list);
        }

        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("reg"), (name, version));
        Ok(())
    }

    /// Register or update a contract entry with extended metadata.
    ///
    /// Behaves identically to [`Self::register`] for the version record and
    /// additionally stores `description` and `notes` in a separate metadata
    /// entry. Requires admin authorisation.
    ///
    /// Emits: `regmeta`
    pub fn register_with_metadata(
        env: Env,
        name: Symbol,
        version: String,
        description: String,
        notes: String,
    ) -> Result<(), RegistryError> {
        Self::require_admin(&env)?;
        if version.len() == 0 {
            return Err(RegistryError::InvalidVersion);
        }
        let record = VersionRecord {
            version: version.clone(),
            registered_at: env.ledger().sequence(),
        };
        let is_new = !env
            .storage()
            .persistent()
            .has(&DataKey::Contract(name.clone()));
        env.storage()
            .persistent()
            .set(&DataKey::Contract(name.clone()), &record);

        let meta = ContractMeta {
            description,
            notes,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Metadata(name.clone()), &meta);

        if is_new {
            let mut list: Vec<Symbol> = env
                .storage()
                .instance()
                .get(&DataKey::ContractList)
                .unwrap_or_else(|| Vec::new(&env));
            list.push_back(name.clone());
            env.storage()
                .instance()
                .set(&DataKey::ContractList, &list);
        }

        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("regmeta"), (name, version));
        Ok(())
    }

    // ── Read-only entrypoints ─────────────────────────────────────────────────
    //
    // None of the following emit events — they are pure reads.

    /// Return the version record for `name`, or `None` if not registered.
    pub fn get_version(env: Env, name: Symbol) -> Option<VersionRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Contract(name))
    }

    /// Return `true` if `name` is registered with exactly `expected_version`.
    pub fn check_version(env: Env, name: Symbol, expected_version: String) -> bool {
        match env
            .storage()
            .persistent()
            .get::<DataKey, VersionRecord>(&DataKey::Contract(name))
        {
            Some(record) => record.version == expected_version,
            None => false,
        }
    }

    /// Return the extended metadata for `name`, or `None` if not stored.
    pub fn get_metadata(env: Env, name: Symbol) -> Option<ContractMeta> {
        env.storage()
            .persistent()
            .get(&DataKey::Metadata(name))
    }

    /// Return all registered contract names in insertion order.
    pub fn list_contracts(env: Env) -> Vec<Symbol> {
        env.storage()
            .instance()
            .get(&DataKey::ContractList)
            .unwrap_or_else(|| Vec::new(&env))
    }
}
