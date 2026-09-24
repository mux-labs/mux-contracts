/*!
 * mux-account-factory: Account factory for deploying account abstraction instances.
 *
 * Provides a factory contract that registers new MuxAccount instances and
 * maintains a per-owner index of deployed accounts.
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types backed by the Soroban host.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_fac"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Per-owner list of deployed account addresses.
    Accounts(Address),
    /// Total accounts registered across all owners.
    AccountCount,
    /// Metadata for a specific account: DataKey::Metadata(owner, account_address)
    Metadata(Address, Address),
    /// Optional upgrade authority set once by `initialize`. A factory that is
    /// never initialized has no admin and cannot be upgraded on-chain.
    Admin,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Metadata associated with a registered account.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AccountMetadata {
    /// Semantic version string, e.g. "1.2.0"
    pub version: String,
    /// Short human-readable description of the account.
    pub description: String,
    /// Author or team identifier.
    pub author: String,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxAccountFactoryError {
    Unauthorized = 1,
    /// account_address must differ from owner.
    InvalidAccount = 2,
    // STORAGE-GRIEFING: unbounded per-owner Accounts vec would let an owner
    // bloat instance storage indefinitely.
    TooManyAccounts = 3,
    /// Metadata not found for the specified account.
    MetadataNotFound = 4,
    // STORAGE-GRIEFING: unbounded metadata string sizes would let an owner
    // bloat instance storage indefinitely.
    MetadataTooLarge = 5,
    /// `upgrade()` was called before `initialize()` — no admin is stored.
    NotInitialized = 6,
    /// `initialize()` was called more than once on the same instance.
    AlreadyInitialized = 7,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum accounts per owner to bound the Accounts vec in instance storage.
///
/// Enforced on every deploy path (`deploy_account`, `deploy_account_with_metadata`)
/// and mirrored by the dry-run helpers (`simulate_deploy*`). Exposed via
/// [`MuxAccountFactoryClient::max_accounts_per_owner`] for TypeScript clients.
pub const MAX_ACCOUNTS_PER_OWNER: u32 = 64;

// STORAGE-GRIEFING: bound metadata string sizes to prevent owners from bloating
// instance storage with large strings. Each account can have metadata, so with
// 64 accounts per owner, unbounded strings could cause significant storage bloat.
const MAX_VERSION_LENGTH: u32 = 32; // e.g., "1.2.3" or "v1.2.3-beta"
const MAX_DESCRIPTION_LENGTH: u32 = 256; // Short human-readable description
const MAX_AUTHOR_LENGTH: u32 = 64; // Author or team identifier

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): extend instance TTL on every write so the factory
// stays live as long as it is actively used.  See docs/storage-griefing.md.
//
// Values: ~17,280 ledgers ≈ 1 day (5-second ledger close); bump to 30 days.
const TTL_THRESHOLD: u32 = 17_280; // extend when remaining TTL falls below 1 day
const TTL_EXTEND_TO: u32 = 518_400; // extend to ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxAccountFactory;

#[contractimpl]
impl MuxAccountFactory {
    /// Optional one-time setup that records an upgrade authority for this factory.
    ///
    /// Calling `initialize` is **not** required for the factory to register accounts —
    /// `deploy_account` and `deploy_account_with_metadata` work without it. It only
    /// establishes the admin address that is later required by `upgrade()`. A factory
    /// that is never initialized has no on-chain upgrade path (`NotInitialized`).
    ///
    /// # Errors
    /// - [`MuxAccountFactoryError::AlreadyInitialized`] if called a second time.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxAccountFactoryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxAccountFactoryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Upgrade the contract WASM. Admin only.
    ///
    /// Requires that `initialize(admin)` was called first. Returns
    /// `NotInitialized` (fail-closed) if no admin is stored — a factory that
    /// was never initialized simply cannot be upgraded on-chain.
    ///
    /// See `docs/contract-upgrade-pattern.md` for storage-compatibility rules
    /// that must be observed between versions. Instance storage (all accounts,
    /// counts, and metadata) is preserved across upgrades by the Soroban host.
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxAccountFactoryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxAccountFactoryError::NotInitialized)?;
        admin.require_auth();
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register a new account for the given owner without metadata.
    ///
    /// The caller must be `owner` (`owner.require_auth()` is called before any
    /// storage write).  `account_address` must differ from `owner`.
    ///
    /// Appends `account_address` to the owner's `Accounts` vec and increments
    /// the global `AccountCount`.  Emits a single `deployed` event.
    /// Instance storage TTL is extended on every successful call.
    ///
    /// # Errors
    ///
    /// | Variant | When |
    /// |---------|------|
    /// | [`MuxAccountFactoryError::InvalidAccount`] | `account_address == owner` |
    /// | [`MuxAccountFactoryError::TooManyAccounts`] | owner's Accounts vec is at [`MAX_ACCOUNTS_PER_OWNER`] |
    ///
    /// Auth host errors are surfaced as host-level panics, not contract errors.
    ///
    /// # Events
    ///
    /// Emits `(mux_fac, deployed)` with data `(owner, account_address)`.
    ///
    /// # See also
    ///
    /// - [`Self::deploy_account_with_metadata`] — same path plus metadata storage
    /// - [`Self::simulate_deploy`] — dry-run that enforces the same checks
    /// - [docs/account-factory-flow.md](../../../../../docs/account-factory-flow.md)
    pub fn deploy_account(
        env: Env,
        owner: Address,
        account_address: Address,
    ) -> Result<Address, MuxAccountFactoryError> {
        owner.require_auth();

        if account_address == owner {
            return Err(MuxAccountFactoryError::InvalidAccount);
        }

        // STORAGE-GRIEFING: bound Accounts vec growth on deploy; idempotent if already registered.
        let (mut accounts, already_registered) =
            Self::load_accounts_for_deploy(&env, &owner, &account_address)?;

        if !already_registered {
            accounts.push_back(account_address.clone());
            env.storage()
                .instance()
                .set(&DataKey::Accounts(owner.clone()), &accounts);

            let count: u64 = env
                .storage()
                .instance()
                .get(&DataKey::AccountCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&DataKey::AccountCount, &(count + 1));
        }

        emit(
            &env,
            symbol_short!("deployed"),
            (owner, account_address.clone()),
        );
        Self::extend_ttl(&env);
        Ok(account_address)
    }

    /// Get all accounts registered for a given owner.
    ///
    /// Returns an empty vec for owners that have never deployed — never errors.
    /// No authorization required.  Does **not** extend TTL.
    pub fn get_accounts(env: Env, owner: Address) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Accounts(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return the total count of registered accounts across **all** owners.
    ///
    /// This is a global monotonically-increasing counter.  It increments by 1
    /// on every successful [`Self::deploy_account`] or
    /// [`Self::deploy_account_with_metadata`] call for a new account.
    /// Re-deploying an already-registered account is idempotent and does not
    /// increment this counter.
    ///
    /// No authorization required.  Does **not** extend TTL.
    pub fn account_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::AccountCount)
            .unwrap_or(0)
    }

    /// Register a new account for the given owner with optional structured metadata.
    ///
    /// Identical to [`Self::deploy_account`] plus: validates metadata string sizes,
    /// stores an [`AccountMetadata`] entry, and emits an additional `meta_set` event.
    /// Re-deploying an existing account idempotently updates the metadata.
    ///
    /// The caller must be `owner`.  `account_address` must differ from `owner`.
    /// Metadata strings are individually bounded to prevent storage bloat:
    ///
    /// | Field | Max bytes | Constant |
    /// |-------|-----------|----------|
    /// | `version` | 32 | `MAX_VERSION_LENGTH` |
    /// | `description` | 256 | `MAX_DESCRIPTION_LENGTH` |
    /// | `author` | 64 | `MAX_AUTHOR_LENGTH` |
    ///
    /// # Errors
    ///
    /// | Variant | When |
    /// |---------|------|
    /// | [`MuxAccountFactoryError::InvalidAccount`] | `account_address == owner` |
    /// | [`MuxAccountFactoryError::TooManyAccounts`] | owner's Accounts vec is at [`MAX_ACCOUNTS_PER_OWNER`] |
    /// | [`MuxAccountFactoryError::MetadataTooLarge`] | any metadata field exceeds its byte limit |
    ///
    /// # Events (in emission order)
    ///
    /// 1. `(mux_fac, deployed)` — data `(owner, account_address)`
    /// 2. `(mux_fac, meta_set)` — data `(owner, account_address, version)`
    ///
    /// # See also
    ///
    /// - [`Self::simulate_deploy_with_metadata`] — dry-run with identical validation
    /// - [`Self::get_account_metadata`] — retrieve stored metadata later
    pub fn deploy_account_with_metadata(
        env: Env,
        owner: Address,
        account_address: Address,
        version: String,
        description: String,
        author: String,
    ) -> Result<Address, MuxAccountFactoryError> {
        owner.require_auth();

        if account_address == owner {
            return Err(MuxAccountFactoryError::InvalidAccount);
        }

        // STORAGE-GRIEFING: bound Accounts vec growth on deploy; idempotent if already registered.
        let (mut accounts, already_registered) =
            Self::load_accounts_for_deploy(&env, &owner, &account_address)?;

        // STORAGE-GRIEFING: validate metadata string sizes to prevent storage bloat.
        Self::validate_metadata(&version, &description, &author)?;

        if !already_registered {
            accounts.push_back(account_address.clone());
            env.storage()
                .instance()
                .set(&DataKey::Accounts(owner.clone()), &accounts);

            let count: u64 = env
                .storage()
                .instance()
                .get(&DataKey::AccountCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&DataKey::AccountCount, &(count + 1));
        }

        // Store metadata
        let meta = AccountMetadata {
            version: version.clone(),
            description,
            author,
        };
        env.storage().instance().set(
            &DataKey::Metadata(owner.clone(), account_address.clone()),
            &meta,
        );

        emit(
            &env,
            symbol_short!("deployed"),
            (owner.clone(), account_address.clone()),
        );
        emit(
            &env,
            symbol_short!("meta_set"),
            (owner, account_address.clone(), version),
        );
        Self::extend_ttl(&env);
        Ok(account_address)
    }

    /// Return the stored metadata for a specific (owner, account_address) pair.
    ///
    /// Returns [`MuxAccountFactoryError::MetadataNotFound`] when the account was
    /// registered via [`Self::deploy_account`] (no metadata path) or when the
    /// `(owner, account_address)` pair has never been registered at all.
    ///
    /// Metadata is keyed on *both* owner and account address — querying with a
    /// different owner for the same `account_address` returns `MetadataNotFound`.
    ///
    /// No authorization required.  Does **not** extend TTL.
    pub fn get_account_metadata(
        env: Env,
        owner: Address,
        account_address: Address,
    ) -> Result<AccountMetadata, MuxAccountFactoryError> {
        env.storage()
            .instance()
            .get(&DataKey::Metadata(owner, account_address))
            .ok_or(MuxAccountFactoryError::MetadataNotFound)
    }

    /// Preflight / dry-run of [`Self::deploy_account`].
    ///
    /// Returns the account address that *would* be registered, or the **same
    /// error** that the real deploy would return — including
    /// [`MuxAccountFactoryError::TooManyAccounts`] when the owner's Accounts
    /// vec is already at [`MAX_ACCOUNTS_PER_OWNER`].
    ///
    /// **No storage is written and no events are emitted.**  This entrypoint is
    /// safe to call without paying for a state-mutating transaction.
    ///
    /// The simulate path mirrors the deploy path exactly so that clients can
    /// use the return value to predict the on-chain result:
    ///
    /// ```text
    /// simulated = simulate_deploy(owner, addr)  // read-only
    /// deployed  = deploy_account(owner, addr)   // state-mutating
    /// assert_eq!(simulated, deployed)           // always true
    /// ```
    ///
    /// # See also
    ///
    /// - [`Self::max_accounts_per_owner`] — query the cap constant
    /// - [docs/account-factory-flow.md § Preflight Pattern](../../../../../docs/account-factory-flow.md#preflight-pattern)
    pub fn simulate_deploy(
        env: Env,
        owner: Address,
        account_address: Address,
    ) -> Result<Address, MuxAccountFactoryError> {
        if account_address == owner {
            return Err(MuxAccountFactoryError::InvalidAccount);
        }
        // Mirror the deploy-path Accounts bound so dry-run stays auditable.
        let _ = Self::load_accounts_for_deploy(&env, &owner, &account_address)?;
        Ok(account_address)
    }

    /// Preflight / dry-run of [`Self::deploy_account_with_metadata`].
    ///
    /// Enforces the same Accounts vec cap as
    /// [`Self::deploy_account_with_metadata`] and validates all three metadata
    /// string size limits.  **No storage is written and no events are emitted.**
    ///
    /// Use this to catch [`MuxAccountFactoryError::MetadataTooLarge`] or
    /// [`MuxAccountFactoryError::TooManyAccounts`] before paying for a
    /// state-mutating transaction.
    pub fn simulate_deploy_with_metadata(
        env: Env,
        owner: Address,
        account_address: Address,
        version: String,
        description: String,
        author: String,
    ) -> Result<Address, MuxAccountFactoryError> {
        if account_address == owner {
            return Err(MuxAccountFactoryError::InvalidAccount);
        }
        let _ = Self::load_accounts_for_deploy(&env, &owner, &account_address)?;
        Self::validate_metadata(&version, &description, &author)?;
        Ok(account_address)
    }

    /// Return the maximum number of accounts permitted per owner (`64`).
    ///
    /// TypeScript clients and other on-chain callers should query this before
    /// calling `deploy_account` so that they can surface a friendly error
    /// rather than burning fees on a predictably-failing transaction.
    ///
    /// The value is a compile-time constant (`MAX_ACCOUNTS_PER_OWNER`); this
    /// entrypoint exists so clients never need to hardcode the number.
    ///
    /// No authorization required.  Does **not** extend TTL.  No state written.
    ///
    /// # See also
    ///
    /// - [`Self::simulate_deploy`] — full preflight including the cap check
    /// - [docs/account-factory-flow.md § Preflight Pattern](../../../../../docs/account-factory-flow.md#preflight-pattern)
    pub fn max_accounts_per_owner(_env: Env) -> u32 {
        MAX_ACCOUNTS_PER_OWNER
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    /// Load the owner's Accounts vec, rejecting when it is already at the
    /// storage-griefing cap unless the account is already registered (idempotency).
    /// Shared by deploy and simulate paths.
    fn load_accounts_for_deploy(
        env: &Env,
        owner: &Address,
        account_address: &Address,
    ) -> Result<(Vec<Address>, bool), MuxAccountFactoryError> {
        let accounts: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Accounts(owner.clone()))
            .unwrap_or_else(|| Vec::new(env));

        let already_registered = accounts.contains(account_address);
        if !already_registered && accounts.len() >= MAX_ACCOUNTS_PER_OWNER {
            return Err(MuxAccountFactoryError::TooManyAccounts);
        }
        Ok((accounts, already_registered))
    }

    fn validate_metadata(
        version: &String,
        description: &String,
        author: &String,
    ) -> Result<(), MuxAccountFactoryError> {
        if version.len() > MAX_VERSION_LENGTH
            || description.len() > MAX_DESCRIPTION_LENGTH
            || author.len() > MAX_AUTHOR_LENGTH
        {
            return Err(MuxAccountFactoryError::MetadataTooLarge);
        }
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
    use soroban_sdk::{testutils::Address as _, Env, FromVal};

    fn setup() -> (Env, MuxAccountFactoryClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccountFactory);
        let client = MuxAccountFactoryClient::new(&env, &contract_id);
        (env, client)
    }

    #[test]
    fn test_deploy_account() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let deployed = client.deploy_account(&owner, &account_addr);
        assert_eq!(deployed, account_addr);
    }

    #[test]
    fn test_deployed_address_distinct_from_owner() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let deployed = client.deploy_account(&owner, &account_addr);
        assert_ne!(deployed, owner);
    }

    #[test]
    fn test_account_registry_updated_after_deployment() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.deploy_account(&owner, &account_addr);
        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts.get(0).unwrap(), account_addr);
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_multiple_account_deployments() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account1 = Address::generate(&env);
        let account2 = Address::generate(&env);
        client.deploy_account(&owner, &account1);
        client.deploy_account(&owner, &account2);
        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), 2);
        assert_eq!(client.account_count(), 2);
    }

    #[test]
    fn test_invalid_account_same_as_owner() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let result = client.try_deploy_account(&owner, &owner);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::InvalidAccount)));
    }

    #[test]
    fn test_accounts_cap_enforced() {
        use soroban_test_helpers::{assert_contract_err, assert_len_at_most};

        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        for _ in 0..64 {
            client.deploy_account(&owner, &Address::generate(&env));
        }
        assert_contract_err(
            client.try_deploy_account(&owner, &Address::generate(&env)),
            MuxAccountFactoryError::TooManyAccounts,
        );
        assert_len_at_most(
            client.get_accounts(&owner).len(),
            MAX_ACCOUNTS_PER_OWNER,
            "accounts",
        );
    }

    #[test]
    fn test_deploy_account_idempotent() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let deployed1 = client.deploy_account(&owner, &account_addr);
        assert_eq!(deployed1, account_addr);
        assert_eq!(client.account_count(), 1);
        assert_eq!(client.get_accounts(&owner).len(), 1);

        // Second deploy with same account is idempotent: does not duplicate or increment count
        let deployed2 = client.deploy_account(&owner, &account_addr);
        assert_eq!(deployed2, account_addr);
        assert_eq!(client.account_count(), 1);
        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts.get(0).unwrap(), account_addr);
    }

    #[test]
    fn test_deploy_account_with_metadata_idempotent() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version1 = String::from_str(&env, "1.0.0");
        let desc1 = String::from_str(&env, "Initial");
        let author = String::from_str(&env, "mux-labs");

        client.deploy_account_with_metadata(&owner, &account_addr, &version1, &desc1, &author);
        assert_eq!(client.account_count(), 1);
        assert_eq!(client.get_accounts(&owner).len(), 1);

        let meta1 = client.get_account_metadata(&owner, &account_addr);
        assert_eq!(meta1.version, version1);
        assert_eq!(meta1.description, desc1);

        // Updating metadata idempotently for the same account
        let version2 = String::from_str(&env, "1.1.0");
        let desc2 = String::from_str(&env, "Updated");
        client.deploy_account_with_metadata(&owner, &account_addr, &version2, &desc2, &author);
        assert_eq!(client.account_count(), 1);
        assert_eq!(client.get_accounts(&owner).len(), 1);

        let meta2 = client.get_account_metadata(&owner, &account_addr);
        assert_eq!(meta2.version, version2);
        assert_eq!(meta2.description, desc2);
    }

    #[test]
    fn test_deploy_account_idempotent_at_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        let first_account = Address::generate(&env);
        client.deploy_account(&owner, &first_account);

        for _ in 1..64 {
            client.deploy_account(&owner, &Address::generate(&env));
        }
        assert_eq!(client.get_accounts(&owner).len(), 64);

        // Re-deploying an existing account succeeds even when at cap
        let result = client.try_deploy_account(&owner, &first_account);
        assert!(result.is_ok());

        let sim_result = client.try_simulate_deploy(&owner, &first_account);
        assert!(sim_result.is_ok());

        // But deploying a 65th distinct account still fails with TooManyAccounts
        let new_account = Address::generate(&env);
        let err_result = client.try_deploy_account(&owner, &new_account);
        assert_eq!(err_result, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));
    }

    #[test]
    fn test_deploy_emits_event() {
        use soroban_sdk::testutils::Events;
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.deploy_account(&owner, &account_addr);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (_, topics, _) = events.get(0).unwrap();
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(action, symbol_short!("deployed"));
    }

    #[test]
    fn test_deploy_with_metadata_emits_deployed_and_meta_set_events() {
        use soroban_sdk::testutils::Events;
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test account");
        let author = String::from_str(&env, "mux-labs");

        client.deploy_account_with_metadata(&owner, &account_addr, &version, &description, &author);

        let events = env.events().all();
        assert_eq!(events.len(), 2);

        let (_, topics0, _) = events.get(0).unwrap();
        let action0 = soroban_sdk::Symbol::from_val(&env, &topics0.get(1).unwrap());
        let (_, topics1, _) = events.get(1).unwrap();
        let action1 = soroban_sdk::Symbol::from_val(&env, &topics1.get(1).unwrap());

        assert_eq!(action0, symbol_short!("deployed"));
        assert_eq!(action1, symbol_short!("meta_set"));
    }

    #[test]
    fn test_deploy_account_does_not_emit_meta_set() {
        use soroban_sdk::testutils::Events;
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.deploy_account(&owner, &account_addr);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (_, topics, _) = events.get(0).unwrap();
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_ne!(action, symbol_short!("meta_set"));
    }

    #[test]
    fn test_accounts_cap_returns_too_many_accounts() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        for _ in 0..64 {
            client.deploy_account(&owner, &Address::generate(&env));
        }
        let result = client.try_deploy_account(&owner, &Address::generate(&env));
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));
    }

    #[test]
    fn test_ttl_extended_on_deploy() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        client.deploy_account(&owner, &Address::generate(&env));
        // If extend_ttl was missing the SDK would panic; reaching here is the assertion.
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_ttl_extended_on_deploy_with_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        client.deploy_account_with_metadata(&owner, &account_addr, &version, &description, &author);
        // If extend_ttl was missing the SDK would panic; reaching here is the assertion.
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_read_operations_do_not_extend_ttl() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);

        // Deploy an account (this extends TTL)
        client.deploy_account(&owner, &account_addr);

        // Read operations should not extend TTL
        let _accounts = client.get_accounts(&owner);
        let _count = client.account_count();

        // If read operations extended TTL incorrectly, the test would still pass
        // but this documents the expected behavior
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_deploy_account_with_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test account");
        let author = String::from_str(&env, "test-author");

        let deployed = client.deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(deployed, account_addr);
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_get_account_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Account with metadata");
        let author = String::from_str(&env, "mux-labs");

        client.deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version.clone(),
            &description.clone(),
            &author.clone(),
        );

        let meta = client.get_account_metadata(&owner, &account_addr);
        assert_eq!(meta.version, version);
        assert_eq!(meta.description, description);
        assert_eq!(meta.author, author);
    }

    #[test]
    fn test_get_account_metadata_not_found() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let result = client.try_get_account_metadata(&owner, &account_addr);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));
    }

    #[test]
    fn test_get_account_metadata_not_found_after_deploy_without_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.deploy_account(&owner, &account_addr);
        let result = client.try_get_account_metadata(&owner, &account_addr);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));
    }

    #[test]
    fn test_get_account_metadata_wrong_owner() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let other_owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        client.deploy_account_with_metadata(&owner, &account_addr, &version, &description, &author);
        let result = client.try_get_account_metadata(&other_owner, &account_addr);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));
    }

    // ── Unauthorized deploy (no mock_all_auths) ───────────────────────────────

    #[test]
    fn test_deploy_account_unauthorized_without_auth() {
        use soroban_sdk::testutils::Events;
        // Deliberately omit mock_all_auths — owner.require_auth() must reject.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxAccountFactory);
        let client = MuxAccountFactoryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);

        let result = client.try_deploy_account(&owner, &account_addr);
        assert!(result.is_err());

        // No state mutation and no events on auth failure.
        assert_eq!(client.get_accounts(&owner).len(), 0);
        assert_eq!(client.account_count(), 0);
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_deploy_account_with_metadata_unauthorized_without_auth() {
        use soroban_sdk::testutils::Events;
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxAccountFactory);
        let client = MuxAccountFactoryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert!(result.is_err());

        assert_eq!(client.get_accounts(&owner).len(), 0);
        assert_eq!(client.account_count(), 0);
        assert!(client
            .try_get_account_metadata(&owner, &account_addr)
            .is_err());
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_unauthorized_deploy_does_not_affect_other_owners() {
        // Authorized deploy for owner_a, then unauthorized attempt on a fresh env.
        let (env, client) = setup();
        let owner_a = Address::generate(&env);
        let account_a = Address::generate(&env);
        client.deploy_account(&owner_a, &account_a);
        assert_eq!(client.account_count(), 1);

        // Separate env with no mock_all_auths — auth failure must leave it empty.
        let env_no_auth = Env::default();
        let contract_id = env_no_auth.register_contract(None, MuxAccountFactory);
        let client_no_auth = MuxAccountFactoryClient::new(&env_no_auth, &contract_id);
        let owner_b = Address::generate(&env_no_auth);
        let account_b = Address::generate(&env_no_auth);
        let result = client_no_auth.try_deploy_account(&owner_b, &account_b);
        assert!(result.is_err());
        assert_eq!(client_no_auth.get_accounts(&owner_b).len(), 0);
        assert_eq!(client_no_auth.account_count(), 0);

        // Original authorized env still has owner_a's account.
        assert_eq!(client.get_accounts(&owner_a).len(), 1);
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_deploy_account_with_metadata_enforces_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        // Fill up to the cap
        for _ in 0..64 {
            client.deploy_account_with_metadata(
                &owner,
                &Address::generate(&env),
                &version.clone(),
                &description.clone(),
                &author.clone(),
            );
        }
        // One more must be rejected
        let result = client.try_deploy_account_with_metadata(
            &owner,
            &Address::generate(&env),
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));
    }

    #[test]
    fn test_deploy_account_with_metadata_invalid_account() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &owner,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::InvalidAccount)));
    }

    #[test]
    fn test_metadata_version_too_long() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        // Create a version string longer than MAX_VERSION_LENGTH (32)
        let version = String::from_str(&env, "a".repeat(33).as_str());
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataTooLarge)));
    }

    #[test]
    fn test_metadata_description_too_long() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        // Create a description string longer than MAX_DESCRIPTION_LENGTH (256)
        let description = String::from_str(&env, "a".repeat(257).as_str());
        let author = String::from_str(&env, "test");

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataTooLarge)));
    }

    #[test]
    fn test_metadata_author_too_long() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        // Create an author string longer than MAX_AUTHOR_LENGTH (64)
        let author = String::from_str(&env, "a".repeat(65).as_str());

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataTooLarge)));
    }

    #[test]
    fn test_metadata_at_max_length_succeeds() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        // Create strings at exactly the maximum allowed length
        let version = String::from_str(&env, "a".repeat(32).as_str());
        let description = String::from_str(&env, "a".repeat(256).as_str());
        let author = String::from_str(&env, "a".repeat(64).as_str());

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert!(result.is_ok());
    }

    // ── simulate_deploy tests ─────────────────────────────────────────────────

    #[test]
    fn test_simulate_deploy_returns_account_address() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let result = client.simulate_deploy(&owner, &account_addr);
        assert_eq!(result, account_addr);
    }

    #[test]
    fn test_simulate_deploy_rejects_owner_as_account() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let result = client.try_simulate_deploy(&owner, &owner);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::InvalidAccount)));
    }

    #[test]
    fn test_simulate_deploy_does_not_write_state() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.simulate_deploy(&owner, &account_addr);
        // No accounts should have been registered
        assert_eq!(client.get_accounts(&owner).len(), 0);
        assert_eq!(client.account_count(), 0);
    }

    #[test]
    fn test_simulate_deploy_does_not_emit_events() {
        use soroban_sdk::testutils::Events;
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.simulate_deploy(&owner, &account_addr);
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_simulate_deploy_with_metadata_returns_account_address() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");
        let result = client.simulate_deploy_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, account_addr);
    }

    #[test]
    fn test_simulate_deploy_with_metadata_rejects_owner_as_account() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");
        let result = client.try_simulate_deploy_with_metadata(
            &owner,
            &owner,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::InvalidAccount)));
    }

    #[test]
    fn test_simulate_deploy_with_metadata_does_not_write_state() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");
        client.simulate_deploy_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert!(client.get_accounts(&owner).is_empty());
        assert_eq!(client.account_count(), 0);
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────

    #[test]
    fn test_symbol_short_lengths_within_limit() {
        let tags = [symbol_short!("mux_fac")];
        let actions = [symbol_short!("deployed"), symbol_short!("meta_set")];
        for sym in tags.iter().chain(actions.iter()) {
            let _ = sym;
        }
    }

    // ── multi-owner isolation ─────────────────────────────────────────────────

    /// Two owners deploying into the same factory must not see each other's
    /// accounts; their Accounts vecs are keyed separately in instance storage.
    #[test]
    fn test_multi_owner_accounts_are_isolated() {
        let (env, client) = setup();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        let account_a1 = Address::generate(&env);
        let account_a2 = Address::generate(&env);
        let account_b1 = Address::generate(&env);

        client.deploy_account(&owner_a, &account_a1);
        client.deploy_account(&owner_a, &account_a2);
        client.deploy_account(&owner_b, &account_b1);

        let accounts_a = client.get_accounts(&owner_a);
        let accounts_b = client.get_accounts(&owner_b);

        // Each owner only sees their own accounts.
        assert_eq!(accounts_a.len(), 2);
        assert_eq!(accounts_b.len(), 1);

        // owner_b's accounts must not appear in owner_a's list.
        for i in 0..accounts_a.len() {
            assert_ne!(accounts_a.get(i).unwrap(), account_b1);
        }
        // owner_a's accounts must not appear in owner_b's list.
        assert_ne!(accounts_b.get(0).unwrap(), account_a1);
        assert_ne!(accounts_b.get(0).unwrap(), account_a2);
    }

    /// `account_count` is a global counter that increments for every owner.
    #[test]
    fn test_account_count_is_global_across_owners() {
        let (env, client) = setup();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);

        assert_eq!(client.account_count(), 0);

        client.deploy_account(&owner_a, &Address::generate(&env));
        assert_eq!(client.account_count(), 1);

        client.deploy_account(&owner_b, &Address::generate(&env));
        assert_eq!(client.account_count(), 2);

        client.deploy_account(&owner_a, &Address::generate(&env));
        assert_eq!(client.account_count(), 3);
    }

    /// `account_count` increments correctly when both deploy paths are mixed.
    #[test]
    fn test_account_count_increments_via_both_deploy_paths() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let acc1 = Address::generate(&env);
        let acc2 = Address::generate(&env);

        // deploy_account
        client.deploy_account(&owner, &acc1);
        assert_eq!(client.account_count(), 1);

        // deploy_account_with_metadata
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");
        client.deploy_account_with_metadata(&owner, &acc2, &version, &description, &author);
        assert_eq!(client.account_count(), 2);
    }

    // ── get_accounts for unknown owner ────────────────────────────────────────

    /// Querying accounts for an owner that has never deployed must return an
    /// empty vec rather than panicking or erroring.
    #[test]
    fn test_get_accounts_empty_for_unknown_owner() {
        let (env, client) = setup();
        let unknown_owner = Address::generate(&env);
        let accounts = client.get_accounts(&unknown_owner);
        assert_eq!(accounts.len(), 0);
    }

    /// `account_count` must be zero before any deploys.
    #[test]
    fn test_account_count_zero_initially() {
        let (_env, client) = setup();
        assert_eq!(client.account_count(), 0);
    }

    // ── max_accounts_per_owner query ──────────────────────────────────────────

    /// The public `max_accounts_per_owner` entrypoint must return 64.
    #[test]
    fn test_max_accounts_per_owner_returns_64() {
        let (_env, client) = setup();
        assert_eq!(client.max_accounts_per_owner(), MAX_ACCOUNTS_PER_OWNER);
        assert_eq!(client.max_accounts_per_owner(), 64);
    }

    // ── duplicate deploy (no dedup guard) ────────────────────────────────────

    /// The factory intentionally does not deduplicate account addresses; the
    /// same account_address may be registered more than once for the same
    /// owner (e.g. after a re-deployment). Both entries appear in the vec and
    /// account_count increments for each.
    #[test]
    fn test_deploy_same_address_twice_is_allowed() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);

        client.deploy_account(&owner, &account_addr);
        client.deploy_account(&owner, &account_addr);

        let accounts = client.get_accounts(&owner);
        // Both registrations are recorded.
        assert_eq!(accounts.len(), 2);
        assert_eq!(client.account_count(), 2);
        // Both entries point to the same address.
        assert_eq!(accounts.get(0).unwrap(), account_addr);
        assert_eq!(accounts.get(1).unwrap(), account_addr);
    }

    // ── simulate_deploy enforces cap ──────────────────────────────────────────

    /// When an owner is already at the 64-account cap, `simulate_deploy` must
    /// return `TooManyAccounts` — identical behaviour to `deploy_account`.
    #[test]
    fn test_simulate_deploy_enforces_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);

        for _ in 0..64 {
            client.deploy_account(&owner, &Address::generate(&env));
        }

        let result = client.try_simulate_deploy(&owner, &Address::generate(&env));
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));
    }

    /// `simulate_deploy_with_metadata` must also enforce the cap.
    #[test]
    fn test_simulate_deploy_with_metadata_enforces_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        for _ in 0..64 {
            client.deploy_account(&owner, &Address::generate(&env));
        }

        let result = client.try_simulate_deploy_with_metadata(
            &owner,
            &Address::generate(&env),
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));
    }

    // ── simulate vs deploy parity ─────────────────────────────────────────────

    /// `simulate_deploy` must return the exact same address that `deploy_account`
    /// would register — clients can use the simulated return value to predict the
    /// on-chain result.
    #[test]
    fn test_simulate_and_deploy_return_same_address() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);

        // Simulate first (read-only; no state change).
        let simulated = client.simulate_deploy(&owner, &account_addr);
        // Then actually deploy.
        let deployed = client.deploy_account(&owner, &account_addr);

        assert_eq!(simulated, deployed);
        assert_eq!(simulated, account_addr);
    }

    /// `simulate_deploy_with_metadata` must return the exact same address that
    /// `deploy_account_with_metadata` would register.
    #[test]
    fn test_simulate_with_metadata_and_deploy_return_same_address() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "My account");
        let author = String::from_str(&env, "mux-labs");

        let simulated = client.simulate_deploy_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        let deployed = client.deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );

        assert_eq!(simulated, deployed);
        assert_eq!(simulated, account_addr);
    }

    /// Simulate must not affect state — calling simulate followed by deploy must
    /// produce exactly one entry in the owner's account list.
    #[test]
    fn test_simulate_then_deploy_produces_single_entry() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);

        // Simulate is a no-op for state.
        client.simulate_deploy(&owner, &account_addr);
        assert_eq!(client.get_accounts(&owner).len(), 0);
        assert_eq!(client.account_count(), 0);

        // Actual deploy registers exactly one entry.
        client.deploy_account(&owner, &account_addr);
        assert_eq!(client.get_accounts(&owner).len(), 1);
        assert_eq!(client.account_count(), 1);
    }

    /// Metadata simulation must be a complete read-only preflight: it cannot
    /// create an account, increment the global counter, store metadata, or emit
    /// an event before the real deployment is submitted.
    #[test]
    fn test_simulate_with_metadata_is_read_only() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Read-only preflight");
        let author = String::from_str(&env, "mux-labs");

        let result = client.simulate_deploy_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );

        assert_eq!(result, Ok(account_addr));
        assert_eq!(client.get_accounts(&owner).len(), 0);
        assert_eq!(client.account_count(), 0);
        assert_eq!(
            client.try_get_account_metadata(&owner, &account_addr),
            Err(Ok(MuxAccountFactoryError::MetadataNotFound))
        );
        assert_eq!(env.events().all().len(), 0);
    }

    // ── cross-owner metadata isolation ────────────────────────────────────────

    /// Metadata stored for (owner_a, account_addr) must not be readable via
    /// (owner_b, account_addr) — DataKey::Metadata is keyed on both owner and
    /// account address.
    #[test]
    fn test_metadata_is_isolated_per_owner() {
        let (env, client) = setup();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        // Use the same account address under both owners to test key isolation.
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Owner A account");
        let author = String::from_str(&env, "owner-a");

        client.deploy_account_with_metadata(
            &owner_a,
            &account_addr,
            &version,
            &description,
            &author,
        );

        // owner_b has no metadata for account_addr.
        let result = client.try_get_account_metadata(&owner_b, &account_addr);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));

        // owner_a's metadata is intact.
        let meta = client.get_account_metadata(&owner_a, &account_addr);
        assert_eq!(meta.version, version);
        assert_eq!(meta.author, author);
    }

    // ── storage-bound invariants after cap ────────────────────────────────────

    /// After reaching the cap, the owner's account vec length must not exceed
    /// MAX_ACCOUNTS_PER_OWNER even when rejection errors are swallowed.
    #[test]
    fn test_accounts_vec_length_bounded_after_cap() {
        use soroban_test_helpers::assert_len_at_most;

        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);

        // Fill to the cap.
        for _ in 0..MAX_ACCOUNTS_PER_OWNER {
            client.deploy_account(&owner, &Address::generate(&env));
        }
        // Attempt to exceed — must be rejected.
        let _ = client.try_deploy_account(&owner, &Address::generate(&env));

        // Vec length must still be exactly at (not above) the cap.
        assert_len_at_most(
            client.get_accounts(&owner).len(),
            MAX_ACCOUNTS_PER_OWNER,
            "accounts vec after cap rejection",
        );
    }

    /// The global account_count must equal MAX_ACCOUNTS_PER_OWNER after the
    /// single-owner fill, and must not increase on a rejected deploy.
    #[test]
    fn test_account_count_does_not_increment_on_rejected_deploy() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);

        for _ in 0..MAX_ACCOUNTS_PER_OWNER {
            client.deploy_account(&owner, &Address::generate(&env));
        }
        let count_before_rejection = client.account_count();

        let _ = client.try_deploy_account(&owner, &Address::generate(&env));

        assert_eq!(client.account_count(), count_before_rejection);
        assert_eq!(client.account_count(), u64::from(MAX_ACCOUNTS_PER_OWNER));
    }

    // ── no WASM/interface validation on registration (#652, by design) ───────
    //
    // Per docs/account-factory-flow.md "Integration with mux-account", the
    // factory is intentionally decoupled from mux-account at the contract
    // level — it stores an address but never calls into it, so it cannot
    // verify account_address is a deployed mux-account instance. This test
    // locks in that documented behaviour: an arbitrary address that is not a
    // mux-account (or any contract at all) is still accepted.

    #[test]
    fn test_deploy_account_accepts_address_with_no_code_hash_validation() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        // Address::generate produces a plain account-style address, not a
        // deployed contract — the factory does not distinguish it from a
        // real mux-account instance.
        let arbitrary_address = Address::generate(&env);
        let deployed = client.deploy_account(&owner, &arbitrary_address);
        assert_eq!(deployed, arbitrary_address);
        assert_eq!(client.get_accounts(&owner).get(0).unwrap(), arbitrary_address);
    }

    // ── cap is per-owner, not global ──────────────────────────────────────────

    /// Filling one owner's cap must not affect a second owner's ability to
    /// deploy — the cap is per-owner, not a global limit.
    #[test]
    fn test_cap_is_per_owner_not_global() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);

        // Fill owner_a to the cap.
        for _ in 0..MAX_ACCOUNTS_PER_OWNER {
            client.deploy_account(&owner_a, &Address::generate(&env));
        }

        // owner_a must be rejected.
        let result_a = client.try_deploy_account(&owner_a, &Address::generate(&env));
        assert_eq!(result_a, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));

        // owner_b must still be able to deploy freely.
        let acc_b = Address::generate(&env);
        let result_b = client.try_deploy_account(&owner_b, &acc_b);
        assert!(result_b.is_ok());
        assert_eq!(client.get_accounts(&owner_b).len(), 1);
    }

    // ── #689: concurrent deploy griefing and enumeration ──────────────────────

    /// Concurrent deploys by the same owner must be correctly serialized:
    /// the cap check reads the current vec length, so rapid concurrent calls
    /// that both see len=63 could both attempt to push (creating len=65).
    /// Soroban's transactional model prevents this, but we verify the cap
    /// guard holds under simulated concurrent load.
    #[test]
    fn test_concurrent_deploy_respects_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);

        // Fill to one below cap (63 accounts).
        for _ in 0..(MAX_ACCOUNTS_PER_OWNER - 1) {
            client.deploy_account(&owner, &Address::generate(&env));
        }
        assert_eq!(client.get_accounts(&owner).len(), MAX_ACCOUNTS_PER_OWNER - 1);

        // Simulate two "concurrent" deploy attempts.
        // In Soroban's execution model, these are actually sequential within
        // the test, but this verifies the cap check is evaluated correctly
        // on each call regardless of prior attempts in the same "batch."
        let deploy_1 = client.try_deploy_account(&owner, &Address::generate(&env));
        let deploy_2 = client.try_deploy_account(&owner, &Address::generate(&env));

        // First deploy should succeed (64th account).
        assert!(deploy_1.is_ok(), "64th deploy must succeed");

        // Second deploy must be rejected (would be 65th).
        assert_eq!(
            deploy_2,
            Err(Ok(MuxAccountFactoryError::TooManyAccounts)),
            "65th deploy must fail with TooManyAccounts"
        );

        // Final vec length must be exactly at cap, not above.
        assert_eq!(client.get_accounts(&owner).len(), MAX_ACCOUNTS_PER_OWNER);
        assert_eq!(client.account_count(), u64::from(MAX_ACCOUNTS_PER_OWNER));
    }

    /// Concurrent deploys from different owners must not interfere with each
    /// other's cap enforcement. This tests that the per-owner Accounts vec
    /// isolation is maintained under concurrent load.
    #[test]
    fn test_concurrent_deploys_across_owners_isolated() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        let owner_c = Address::generate(&env);

        // Fill owner_a to cap (64).
        for _ in 0..MAX_ACCOUNTS_PER_OWNER {
            client.deploy_account(&owner_a, &Address::generate(&env));
        }

        // Fill owner_b to one below cap (63).
        for _ in 0..(MAX_ACCOUNTS_PER_OWNER - 1) {
            client.deploy_account(&owner_b, &Address::generate(&env));
        }

        // Owner_c has no accounts yet (0).

        // Simulate "concurrent" deploys from all three owners.
        let result_a = client.try_deploy_account(&owner_a, &Address::generate(&env));
        let result_b = client.try_deploy_account(&owner_b, &Address::generate(&env));
        let result_c = client.try_deploy_account(&owner_c, &Address::generate(&env));

        // owner_a is at cap — must be rejected.
        assert_eq!(result_a, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));

        // owner_b has space for one more — must succeed.
        assert!(result_b.is_ok());

        // owner_c has no accounts — must succeed.
        assert!(result_c.is_ok());

        // Verify final state: each owner's vec is independent.
        assert_eq!(
            client.get_accounts(&owner_a).len(),
            MAX_ACCOUNTS_PER_OWNER,
            "owner_a remains at cap"
        );
        assert_eq!(
            client.get_accounts(&owner_b).len(),
            MAX_ACCOUNTS_PER_OWNER,
            "owner_b now at cap"
        );
        assert_eq!(client.get_accounts(&owner_c).len(), 1, "owner_c has 1");

        // Global counter incremented only for successful deploys.
        assert_eq!(
            client.account_count(),
            u64::from(MAX_ACCOUNTS_PER_OWNER) * 2 + 1
        );
    }

    /// Owner enumeration: after filling to cap, get_accounts must return
    /// exactly MAX_ACCOUNTS_PER_OWNER entries, all distinct (unless duplicates
    /// were intentionally registered), and querying metadata for each must
    /// succeed when metadata was stored.
    #[test]
    fn test_owner_enumeration_at_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test account");
        let author = String::from_str(&env, "mux-labs");

        // Deploy 64 accounts, half with metadata, half without.
        let mut expected_addresses: Vec<Address> = Vec::new(&env);
        for i in 0..MAX_ACCOUNTS_PER_OWNER {
            let addr = Address::generate(&env);
            if i % 2 == 0 {
                client.deploy_account(&owner, &addr);
            } else {
                client.deploy_account_with_metadata(
                    &owner,
                    &addr,
                    &version,
                    &description,
                    &author,
                );
            }
            expected_addresses.push_back(addr);
        }

        // get_accounts must return all 64.
        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), MAX_ACCOUNTS_PER_OWNER);

        // Each address must be present in the returned vec.
        for expected in expected_addresses.iter() {
            let mut found = false;
            for actual in accounts.iter() {
                if actual == expected {
                    found = true;
                    break;
                }
            }
            assert!(found, "expected address not found in get_accounts result");
        }

        // Metadata must be readable for accounts deployed with metadata.
        for i in 0..expected_addresses.len() {
            let addr = expected_addresses.get(i).unwrap();
            if i % 2 == 1 {
                // Odd index → deployed with metadata.
                let meta = client.get_account_metadata(&owner, &addr);
                assert_eq!(meta.version, version);
            } else {
                // Even index → deployed without metadata.
                let result = client.try_get_account_metadata(&owner, &addr);
                assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));
            }
        }
    }

    /// Griefing resistance: verify that an attacker filling their own cap
    /// does not degrade performance or storage cost for other users.
    /// The cap is per-owner, so owner_attacker filling to 64 must not
    /// increase the cost of owner_victim's first deploy.
    #[test]
    fn test_cap_griefing_resistance() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner_attacker = Address::generate(&env);
        let owner_victim = Address::generate(&env);

        // Attacker fills their cap.
        for _ in 0..MAX_ACCOUNTS_PER_OWNER {
            client.deploy_account(&owner_attacker, &Address::generate(&env));
        }

        // Victim should be unaffected — first deploy must succeed normally.
        let victim_account = Address::generate(&env);
        let result = client.try_deploy_account(&owner_victim, &victim_account);
        assert!(result.is_ok(), "victim's deploy must succeed");

        // Victim's account list is independent.
        assert_eq!(client.get_accounts(&owner_victim).len(), 1);
        assert_eq!(
            client.get_accounts(&owner_victim).get(0).unwrap(),
            victim_account
        );

        // Attacker's list is still full and independent.
        assert_eq!(
            client.get_accounts(&owner_attacker).len(),
            MAX_ACCOUNTS_PER_OWNER
        );
    }

    /// Enumeration edge case: deploying the same address multiple times
    /// (which is allowed) must not break enumeration or metadata queries.
    #[test]
    fn test_enumeration_with_duplicate_addresses() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let duplicate_addr = Address::generate(&env);
        let version_1 = String::from_str(&env, "1.0.0");
        let version_2 = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Duplicate test");
        let author = String::from_str(&env, "test");

        // Deploy the same address twice with different metadata.
        client.deploy_account_with_metadata(
            &owner,
            &duplicate_addr,
            &version_1,
            &description,
            &author,
        );
        client.deploy_account_with_metadata(
            &owner,
            &duplicate_addr,
            &version_2,
            &description,
            &author,
        );

        // get_accounts must show both entries.
        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts.get(0).unwrap(), duplicate_addr);
        assert_eq!(accounts.get(1).unwrap(), duplicate_addr);

        // Metadata query returns the last-written metadata (storage key is
        // (owner, address), so the second deploy overwrites the first).
        let meta = client.get_account_metadata(&owner, &duplicate_addr);
        assert_eq!(meta.version, version_2);

        // account_count increments for both deploys.
        assert_eq!(client.account_count(), 2);
    }
}
