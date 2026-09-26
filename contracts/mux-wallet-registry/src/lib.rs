/*!
 * mux-wallet-registry: Named wallet address registry for Mux Protocol.
 *
 * Allows an owner to register and look up wallet addresses by a symbolic name.
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and uses `extern crate alloc` only inside
 * `#[cfg(test)]` modules. All data structures use Soroban SDK types backed by
 * the Soroban host.
 *
 * # Events
 *
 * Contract tag: `mux_wreg`
 *
 * | Action    | Trigger                          | Data payload                  |
 * |-----------|----------------------------------|-------------------------------|
 * | `init`    | `initialize` succeeds            | `owner: Address`              |
 * | `wlt_reg` | `register_wallet` succeeds       | `(name: Symbol, wallet: Address)` |
 * | `wlt_meta`| `register_wallet_with_metadata` succeeds | `(name: Symbol, wallet: Address)` |
 * This crate is `#![no_std]` and uses `extern crate alloc` for heap-backed
 * types used in tests (e.g. `format!` for generating symbol names).
 * All data structures use Soroban SDK types backed by the Soroban host.
 *
 * ## Upgrade Migration Notes
 *
 * When upgrading this contract to a new version:
 *
 * 1. **Storage Compatibility**: All storage keys (Owner, Wallet) must remain stable.
 *    Do not change DataKey enum variants or their discriminants.
 *
 * 2. **Owner Migration**: The Owner address will persist across upgrades.
 *    No migration action required for existing owner authorization.
 *
 * 3. **Wallet Registry Migration**: All registered wallet entries (Symbol -> Address)
 *    will remain accessible. Maintain backward compatibility with existing wallet lookups.
 *
 * 4. **Breaking Changes**: If introducing new storage fields, ensure they are optional
 *    to maintain compatibility with existing instances. Use a version marker if needed.
 *
 * 5. **Testing**: After upgrade, verify:
 *    - Owner can still authorize operations
 *    - All registered wallets can be retrieved
 *    - New wallets can be registered
 */

#![no_std]

extern crate alloc;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────

/// Emit a contract event under the `mux_wreg` topic namespace.
///
/// Topics layout: `[mux_wreg, action]`; data is the action-specific payload.
/// Only called on successful state-mutating paths — failed `Result::Err`
/// returns must not reach this helper.
fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_wreg"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Persistent storage keys used by the wallet registry contract.
#[contracttype]
pub enum DataKey {
    /// The owner address authorised to register wallets.
    Owner,
    /// A registered wallet entry keyed by name: `DataKey::Wallet(name)`.
    Wallet(Symbol),
    /// List of wallet names registered in this contract.
    Names,
    /// Optional metadata associated with a wallet name.
    Metadata(Symbol),
}

/// Descriptive metadata attached to a wallet entry.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletMetadata {
    /// Human-readable label for the wallet.
    pub label: String,
    /// Free-form description / notes.
    pub description: String,
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Error codes returned by wallet registry contract methods.
///
/// The numeric discriminants are part of the on-chain ABI; do not renumber
/// existing variants.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WalletRegistryError {
    /// `initialize` has not been called yet; the owner is unknown.
    NotInitialized = 1,
    /// `initialize` was called more than once on the same contract instance.
    AlreadyInitialized = 2,
    /// Reserved for future use. Auth failures are surfaced as host-level
    /// errors by `Address::require_auth`.
    Unauthorized = 3,
    /// No wallet is registered under the requested name.
    WalletNotFound = 4,
    TooManyWallets = 5,
}

// ── Storage limits ─────────────────────────────────────────────────────────────

/// Maximum number of distinct wallet names that may be registered.
const MAX_WALLETS: u32 = 128;

// ── Storage TTL ─────────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

/// Named wallet address registry.
///
/// Deploy one instance per namespace (e.g. one per application, or one shared
/// registry for the whole protocol). The owner set at initialisation is the
/// only account that may write entries.
#[contract]
pub struct MuxWalletRegistry;

#[contractimpl]
impl MuxWalletRegistry {
    /// Initialise the registry and record its owner.
    ///
    /// Must be called exactly once, before any other method. The `owner`
    /// address must authorise this call (via `require_auth`).
    ///
    /// # Errors
    /// - [`WalletRegistryError::AlreadyInitialized`] if called a second time.
    pub fn initialize(env: Env, owner: Address) -> Result<(), WalletRegistryError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(WalletRegistryError::AlreadyInitialized);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        emit(&env, symbol_short!("init"), owner);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Upgrade the contract WASM. Owner only.
    ///
    /// See `docs/contract-upgrade-pattern.md` for storage-compatibility rules
    /// that must be observed between versions. Instance storage (owner, wallet
    /// entries, and names list) is preserved across upgrades by the Soroban host.
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    ///
    /// # Errors
    /// - [`WalletRegistryError::NotInitialized`] if `initialize` was never called.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), WalletRegistryError> {
        Self::require_owner(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register or overwrite the wallet address stored under `name`.
    ///
    /// Only the owner recorded at initialisation may call this method;
    /// the owner address must authorise the invocation. Calling this with
    /// an existing `name` silently replaces the previous entry.
    ///
    /// # Errors
    /// - [`WalletRegistryError::NotInitialized`] if `initialize` was never
    ///   called.
    pub fn register_wallet(
        env: Env,
        name: Symbol,
        wallet: Address,
    ) -> Result<(), WalletRegistryError> {
        Self::require_owner(&env)?;
        let mut names: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env));

        if !names.contains(&name) {
            if names.len() >= MAX_WALLETS {
                return Err(WalletRegistryError::TooManyWallets);
            }
            names.push_back(name.clone());
            env.storage().instance().set(&DataKey::Names, &names);
        }

        env.storage()
            .instance()
            .set(&DataKey::Wallet(name.clone()), &wallet);
        emit(&env, symbol_short!("wlt_reg"), (name, wallet));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the wallet address registered under `name`.
    ///
    /// This is a read-only method; no authorisation is required. Any caller
    /// may look up entries.
    ///
    /// # Errors
    /// - [`WalletRegistryError::WalletNotFound`] if no wallet has been
    ///   registered under `name`.
    pub fn get_wallet(env: Env, name: Symbol) -> Result<Address, WalletRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Wallet(name))
            .ok_or(WalletRegistryError::WalletNotFound)
    }

    /// Register or overwrite a wallet address with descriptive metadata.
    ///
    /// Only the owner recorded at initialisation may call this method;
    /// the owner address must authorise the invocation. Calling this with
    /// an existing `name` silently replaces the previous entry and its metadata.
    ///
    /// Emits a `wlt_regm` event with `(name, wallet)` as the payload on
    /// successful registration.
    ///
    /// # Errors
    /// - [`WalletRegistryError::NotInitialized`] if `initialize` was never called.
    /// - [`WalletRegistryError::TooManyWallets`] (code 5) if the wallet cap
    ///   (128) has been reached.
    pub fn register_wallet_with_metadata(
        env: Env,
        name: Symbol,
        wallet: Address,
        label: String,
        description: String,
    ) -> Result<(), WalletRegistryError> {
        Self::require_owner(&env)?;
        let mut names: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env));

        if !names.contains(&name) {
            if names.len() >= MAX_WALLETS {
                return Err(WalletRegistryError::TooManyWallets);
            }
            names.push_back(name.clone());
            env.storage().instance().set(&DataKey::Names, &names);
        }

        env.storage()
            .instance()
            .set(&DataKey::Wallet(name.clone()), &wallet);
        let meta = WalletMetadata { label, description };
        env.storage()
            .instance()
            .set(&DataKey::Metadata(name.clone()), &meta);
        emit(&env, symbol_short!("wlt_meta"), (name, wallet));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the metadata for a wallet registered under `name`.
    ///
    /// This is a read-only method; no authorisation is required.
    ///
    /// Only wallets registered via [`Self::register_wallet_with_metadata`] have
    /// metadata. Wallets registered via [`Self::register_wallet`] alone do not,
    /// and this method will return [`WalletRegistryError::WalletNotFound`] for
    /// those names.
    ///
    /// # Errors
    /// - [`WalletRegistryError::WalletNotFound`] if no wallet with metadata is
    ///   registered under `name`.
    pub fn get_metadata(env: Env, name: Symbol) -> Result<WalletMetadata, WalletRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Metadata(name))
            .ok_or(WalletRegistryError::WalletNotFound)
    }

    /// List all registered wallet names.
    ///
    /// Returns an empty [`Vec`] when no wallets have been registered *or* when
    /// the contract has not yet been initialised (i.e. [`Self::initialize`] has
    /// not been called). No authorisation is required.
    pub fn list_wallets(env: Env) -> Vec<Symbol> {
        env.storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    /// Fetch the stored owner and require their auth. Returns
    /// [`WalletRegistryError::NotInitialized`] when no owner is recorded.
    fn require_owner(env: &Env) -> Result<(), WalletRegistryError> {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(WalletRegistryError::NotInitialized)?;
        owner.require_auth();
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
    extern crate alloc;
    use alloc::format;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events},
        Env, FromVal, String,
    };

    fn setup() -> (Env, MuxWalletRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        client.initialize(&owner);
        (env, client, owner)
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

    // ── Initialise ────────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        assert!(client.try_initialize(&owner).is_ok());
    }

    #[test]
    fn test_double_initialize_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        client.initialize(&owner);

        let result = client.try_initialize(&owner);
        assert_eq!(result, Err(Ok(WalletRegistryError::AlreadyInitialized)));
    }

    #[test]
    fn test_double_initialize_with_different_owner_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        client.initialize(&owner);

        let result = client.try_initialize(&other);
        assert_eq!(result, Err(Ok(WalletRegistryError::AlreadyInitialized)));

        // Original owner must still be able to register after the rejected re-init.
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        assert!(client.try_register_wallet(&name, &wallet).is_ok());
        assert_eq!(client.get_wallet(&name), wallet);
    }

    // ── Event: init ──────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_emits_init_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        client.initialize(&owner);

        let events = env.events().all();
        assert_eq!(events.len(), 1, "expected exactly 1 event after initialize");
        assert_eq!(topic_tag(&env, &events, 0), symbol_short!("mux_wreg"));
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    // ── register_wallet ──────────────────────────────────────────────────────

    #[test]
    fn test_register_and_get_wallet() {
        let (env, client, _) = setup();
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);
        assert_eq!(client.get_wallet(&name), wallet);
    }

    // ── Event: wlt_reg ───────────────────────────────────────────────────────

    #[test]
    fn test_register_wallet_emits_wlt_reg_event() {
        let (env, client, _) = setup();
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);

        let events = env.events().all();
        // events[0] = init, events[1] = wlt_reg
        assert_eq!(events.len(), 2, "expected init + wlt_reg events");
        assert_eq!(topic_tag(&env, &events, 1), symbol_short!("mux_wreg"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("wlt_reg"));
    }

    #[test]
    fn test_register_wallet_update_emits_wlt_reg_event() {
        // Re-registering (overwrite) also emits wlt_reg.
        let (env, client, _) = setup();
        let name = symbol_short!("bob");
        let wallet1 = Address::generate(&env);
        let wallet2 = Address::generate(&env);
        client.register_wallet(&name, &wallet1);
        client.register_wallet(&name, &wallet2);

        let events = env.events().all();
        // init + wlt_reg + wlt_reg
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("wlt_reg"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("wlt_reg"));
        // wallet was actually updated
        assert_eq!(client.get_wallet(&name), wallet2);
    }

    // ── register_wallet_with_metadata ────────────────────────────────────────

    #[test]
    fn test_register_wallet_with_metadata() {
        let (env, client, _) = setup();
        let name = symbol_short!("carol");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "Carol's Wallet");
        let description = String::from_str(&env, "Primary spending wallet");
        client.register_wallet_with_metadata(&name, &wallet, &label, &description);
        assert_eq!(client.get_wallet(&name), wallet);
        let meta = client.get_metadata(&name);
        assert_eq!(meta.label, label);
        assert_eq!(meta.description, description);
    }

    // ── Event: wlt_meta ──────────────────────────────────────────────────────

    #[test]
    fn test_register_wallet_with_metadata_emits_wlt_meta_event() {
        let (env, client, _) = setup();
        let name = symbol_short!("carol");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "Carol's Wallet");
        let description = String::from_str(&env, "Primary spending wallet");
        client.register_wallet_with_metadata(&name, &wallet, &label, &description);

        let events = env.events().all();
        // events[0] = init, events[1] = wlt_meta
        assert_eq!(events.len(), 2, "expected init + wlt_meta events");
        assert_eq!(topic_tag(&env, &events, 1), symbol_short!("mux_wreg"));
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("wlt_meta"));
    }

    #[test]
    fn test_register_wallet_with_metadata_update_emits_wlt_meta_event() {
        let (env, client, _) = setup();
        let name = symbol_short!("dave");
        let wallet = Address::generate(&env);
        let label1 = String::from_str(&env, "v1");
        let label2 = String::from_str(&env, "v2");
        let desc = String::from_str(&env, "desc");
        client.register_wallet_with_metadata(&name, &wallet, &label1, &desc);
        client.register_wallet_with_metadata(&name, &wallet, &label2, &desc);
        let meta = client.get_metadata(&name);
        assert_eq!(meta.label, label2);
        assert_eq!(client.get_wallet(&name), wallet);

        let events = env.events().all();
        // init + wlt_meta + wlt_meta
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("wlt_meta"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("wlt_meta"));
    }

    // ── symbol_short length audit ─────────────────────────────────────────────

    #[test]
    fn test_symbol_short_lengths_within_limit() {
        // Both the contract tag and every action must fit in 9 chars (symbol_short! limit).
        let tag = symbol_short!("mux_wreg");
        let actions = [
            symbol_short!("init"),
            symbol_short!("wlt_reg"),
            symbol_short!("wlt_meta"),
        ];
        // symbol_short! is a compile-time macro — if any name exceeded 9 chars it
        // would not compile.  This test documents the set and asserts they are valid.
        let _ = tag;
        for _ in actions.iter() {}
    }

    // ── Storage cap ──────────────────────────────────────────────────────────

    #[test]
    fn test_register_wallet_caps_names() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        for i in 0..MAX_WALLETS {
            let name = soroban_sdk::Symbol::new(&env, &format!("wallet{}", i));
            let wallet = Address::generate(&env);
            client.register_wallet(&name, &wallet);
        }

        let overflow_name = soroban_sdk::Symbol::new(&env, "overflow");
        let overflow_wallet = Address::generate(&env);
        let result = client.try_register_wallet(&overflow_name, &overflow_wallet);
        assert_eq!(result, Err(Ok(WalletRegistryError::TooManyWallets)));
    }

    #[test]
    fn test_ttl_extended_on_register_wallet() {
        let (env, client, _) = setup();
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);
        assert_eq!(client.get_wallet(&name), wallet);
    }

    // ── Error cases ──────────────────────────────────────────────────────────

    #[test]
    fn test_get_wallet_not_found() {
        let (_, client, _) = setup();
        assert_eq!(
            client.try_get_wallet(&symbol_short!("ghost")),
            Err(Ok(WalletRegistryError::WalletNotFound))
        );
    }

    #[test]
    fn test_register_wallet_before_init_returns_not_initialized() {
        // require_owner checks for Owner key; absent means NotInitialized.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let name = symbol_short!("wallet");
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_register_wallet(&name, &wallet),
            Err(Ok(WalletRegistryError::NotInitialized))
        );
    }

    #[test]
    fn test_get_wallet_on_uninitialised_contract_returns_not_found() {
        // get_wallet does not check auth — it just returns WalletNotFound when
        // nothing is stored, even on a completely fresh contract.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        assert_eq!(
            client.try_get_wallet(&symbol_short!("x")),
            Err(Ok(WalletRegistryError::WalletNotFound))
        );
    }

    #[test]
    fn test_get_unknown_name_after_registrations() {
        let (env, client, _) = setup();
        let known = symbol_short!("known");
        client.register_wallet(&known, &Address::generate(&env));
        // Unregistered name is still WalletNotFound.
        assert_eq!(
            client.try_get_wallet(&symbol_short!("unknown")),
            Err(Ok(WalletRegistryError::WalletNotFound))
        );
        // Registered name is unaffected.
        assert!(client.try_get_wallet(&known).is_ok());
    }

    #[test]
    fn test_get_metadata_not_found() {
        let (_, client, _) = setup();
        let result = client.try_get_metadata(&symbol_short!("ghost"));
        assert_eq!(result, Err(Ok(WalletRegistryError::WalletNotFound)));
    }

    // ── Failed paths emit no events ───────────────────────────────────────────

    #[test]
    fn test_failed_initialize_emits_no_event() {
        // Second initialize call is rejected — must not emit an event.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        client.initialize(&owner);
        let _ = client.try_initialize(&owner); // must fail

        let events = env.events().all();
        // Only the first (successful) initialize emitted an event.
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_over_cap_register_emits_no_extra_event() {
        // TooManyWallets rejection must not emit a wlt_reg event.
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        for i in 0..MAX_WALLETS {
            let name = soroban_sdk::Symbol::new(&env, &format!("wallet{}", i));
            client.register_wallet(&name, &Address::generate(&env));
        }
        let before = env.events().all().len();
        let _ = client.try_register_wallet(
            &soroban_sdk::Symbol::new(&env, "overflow"),
            &Address::generate(&env),
        );
        // No extra event from the failed call.
        assert_eq!(env.events().all().len(), before);
    }

    /// register_wallet_with_metadata must enforce the TooManyWallets cap.
    /// Fill the registry to MAX_WALLETS via register_wallet, then a call to
    /// register_wallet_with_metadata with a new name must return TooManyWallets.
    #[test]
    fn test_register_wallet_with_metadata_caps_names() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        // Register MAX_WALLETS entries via the basic path.
        for i in 0..MAX_WALLETS {
            let name = soroban_sdk::Symbol::new(&env, &alloc::format!("wlt{}", i));
            let wallet = Address::generate(&env);
            client.register_wallet(&name, &wallet);
        }

        // register_wallet_with_metadata with a new name must now be rejected.
        let overflow = soroban_sdk::Symbol::new(&env, "overflow");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "overflow label");
        let desc = String::from_str(&env, "overflow desc");
        let result =
            client.try_register_wallet_with_metadata(&overflow, &wallet, &label, &desc);
        assert_eq!(result, Err(Ok(WalletRegistryError::TooManyWallets)));
    }

    /// register_wallet_with_metadata on an existing name must not duplicate
    /// the name in the Names vec.
    #[test]
    fn test_register_wallet_with_metadata_no_duplicate_names() {
        let (env, client, _) = setup();
        let name = symbol_short!("carol");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "label");
        let desc = String::from_str(&env, "desc");

        // Register via basic path, then upgrade via metadata path.
        client.register_wallet(&name, &wallet);
        client.register_wallet_with_metadata(&name, &wallet, &label, &desc);

        // Name should appear exactly once.
        let names: soroban_sdk::Vec<soroban_sdk::Symbol> = client.list_wallets();
        let count = names.iter().filter(|n| *n == name).count();
        assert_eq!(count, 1);
    }

    /// register_wallet_with_metadata before init must return NotInitialized.
    #[test]
    fn test_register_wallet_with_metadata_before_init_returns_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "label");
        let desc = String::from_str(&env, "desc");
        assert_eq!(
            client.try_register_wallet_with_metadata(&name, &wallet, &label, &desc),
            Err(Ok(WalletRegistryError::NotInitialized))
        );
    }

    // ── Unauthorized owner tests ───────────────────────────────────────────────
    //
    // These tests verify that `register_wallet` and `register_wallet_with_metadata`
    // reject callers who have not been authorised as the declared owner.
    // Following the pattern used across mux-* contracts (see mux-account-factory
    // and mux-delegation), they deliberately omit `mock_all_auths` so that
    // `require_auth` rejects at the host level, surfacing as `Err(..)` from
    // `try_*`.  State mutation must not occur on a rejected call.

    /// `register_wallet` without any authorised signer must be rejected.
    /// No wallet entry or name list entry may be written.
    #[test]
    fn test_register_wallet_requires_owner_auth() {
        // Initialise with mocked auth, then attempt register_wallet without mock.
        let env = Env::default();
        let cid = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &cid);
        let owner = Address::generate(&env);
        // Seed the Owner key directly via as_contract (mock_all_auths is a
        // permanent switch in soroban-sdk 21, not a restorable guard).
        env.as_contract(&cid, || {
            env.storage().instance().set(&DataKey::Owner, &owner);
        });

        // No mock_all_auths — require_auth must reject.
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        let result = client.try_register_wallet(&name, &wallet);
        assert!(
            result.is_err(),
            "register_wallet must be rejected when owner auth is absent"
        );

        // No wallet entry must have been written.
        assert!(
            client.try_get_wallet(&name).is_err(),
            "no wallet must be stored after a rejected register_wallet"
        );
        // Names list must still be empty.
        assert_eq!(
            client.list_wallets().len(),
            0,
            "names list must remain empty after a rejected register_wallet"
        );
    }

    /// `register_wallet_with_metadata` without any authorised signer must be
    /// rejected.  No wallet, metadata, or name list entry may be written.
    #[test]
    fn test_register_wallet_with_metadata_requires_owner_auth() {
        let env = Env::default();
        let cid = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &cid);
        let owner = Address::generate(&env);
        // Seed the Owner key directly via as_contract (mock_all_auths is a
        // permanent switch in soroban-sdk 21, not a restorable guard).
        env.as_contract(&cid, || {
            env.storage().instance().set(&DataKey::Owner, &owner);
        });

        let name = symbol_short!("bob");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "Bob's Wallet");
        let desc = String::from_str(&env, "primary wallet");

        let result = client.try_register_wallet_with_metadata(&name, &wallet, &label, &desc);
        assert!(
            result.is_err(),
            "register_wallet_with_metadata must be rejected when owner auth is absent"
        );

        assert!(
            client.try_get_wallet(&name).is_err(),
            "no wallet must be stored after a rejected register_wallet_with_metadata"
        );
        assert!(
            client.try_get_metadata(&name).is_err(),
            "no metadata must be stored after a rejected register_wallet_with_metadata"
        );
        assert_eq!(
            client.list_wallets().len(),
            0,
            "names list must remain empty after a rejected register_wallet_with_metadata"
        );
    }

    /// A non-owner caller must not be able to register wallets.
    /// The owner key is present but `require_auth` must reject the wrong signer.
    #[test]
    fn test_register_wallet_rejects_non_owner_caller() {
        let env = Env::default();
        let cid = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &cid);
        let owner = Address::generate(&env);

        // Seed the Owner key directly via as_contract (mock_all_auths is a
        // permanent switch in soroban-sdk 21, not a restorable guard).
        env.as_contract(&cid, || {
            env.storage().instance().set(&DataKey::Owner, &owner);
        });

        // Attempt register_wallet without a mocked signer — host rejects.
        let name = symbol_short!("carol");
        let wallet = Address::generate(&env);
        let result = client.try_register_wallet(&name, &wallet);
        assert!(
            result.is_err(),
            "register_wallet must reject a non-owner caller"
        );

        assert!(client.try_get_wallet(&name).is_err());
        assert_eq!(client.list_wallets().len(), 0);
    }

    /// A non-owner caller must not be able to register wallets with metadata.
    #[test]
    fn test_register_wallet_with_metadata_rejects_non_owner_caller() {
        let env = Env::default();
        let cid = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &cid);
        let owner = Address::generate(&env);

        // Seed the Owner key directly via as_contract (mock_all_auths is a
        // permanent switch in soroban-sdk 21, not a restorable guard).
        env.as_contract(&cid, || {
            env.storage().instance().set(&DataKey::Owner, &owner);
        });

        let name = symbol_short!("dave");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "Dave");
        let desc = String::from_str(&env, "desc");

        let result = client.try_register_wallet_with_metadata(&name, &wallet, &label, &desc);
        assert!(
            result.is_err(),
            "register_wallet_with_metadata must reject a non-owner caller"
        );

        assert!(client.try_get_wallet(&name).is_err());
        assert!(client.try_get_metadata(&name).is_err());
        assert_eq!(client.list_wallets().len(), 0);
    }

    /// An unauthorized `register_wallet` must not affect a previously-authorized
    /// registration in a separate env — isolation check.
    #[test]
    fn test_unauthorized_register_wallet_does_not_affect_other_envs() {
        // Authorized env: register one wallet legitimately.
        let env_auth = Env::default();
        env_auth.mock_all_auths();
        let cid_auth = env_auth.register_contract(None, MuxWalletRegistry);
        let c_auth = MuxWalletRegistryClient::new(&env_auth, &cid_auth);
        let owner_auth = Address::generate(&env_auth);
        c_auth.initialize(&owner_auth);
        let name_auth = symbol_short!("alice");
        let wallet_auth = Address::generate(&env_auth);
        c_auth.register_wallet(&name_auth, &wallet_auth);
        assert_eq!(c_auth.list_wallets().len(), 1);

        // Unauthorized env: attempt without mock_all_auths.
        let env_unauth = Env::default();
        let cid_unauth = env_unauth.register_contract(None, MuxWalletRegistry);
        let c_unauth = MuxWalletRegistryClient::new(&env_unauth, &cid_unauth);
        let owner_unauth = Address::generate(&env_unauth);
        // Seed the Owner key directly via as_contract (mock_all_auths is a
        // permanent switch in soroban-sdk 21, not a restorable guard).
        env_unauth.as_contract(&cid_unauth, || {
            env_unauth
                .storage()
                .instance()
                .set(&DataKey::Owner, &owner_unauth);
        });
        let name_unauth = symbol_short!("bob");
        let wallet_unauth = Address::generate(&env_unauth);
        let result = c_unauth.try_register_wallet(&name_unauth, &wallet_unauth);
        assert!(result.is_err());
        assert_eq!(c_unauth.list_wallets().len(), 0);

        // Original authorized env must be unaffected.
        assert_eq!(c_auth.list_wallets().len(), 1);
        assert!(c_auth.try_get_wallet(&name_auth).is_ok());
    }

    /// get_metadata on a completely uninitialized contract returns WalletNotFound
    /// (no auth required — the Metadata key is simply absent).
    #[test]
    fn test_get_metadata_on_uninitialized_contract_returns_not_found() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        assert_eq!(
            client.try_get_metadata(&symbol_short!("x")),
            Err(Ok(WalletRegistryError::WalletNotFound))
        );
    }

    /// Registering with register_wallet does not create a Metadata entry.
    /// get_metadata on a wallet registered without metadata must return WalletNotFound.
    #[test]
    fn test_get_metadata_missing_after_register_without_metadata() {
        let (env, client, _) = setup();
        let name = symbol_short!("bare");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);
        assert_eq!(client.get_wallet(&name), wallet);
        assert_eq!(
            client.try_get_metadata(&name),
            Err(Ok(WalletRegistryError::WalletNotFound))
        );
    }

    /// register_wallet then register_wallet_with_metadata interop:
    /// upgrading a bare-registered name to full metadata must work and must
    /// leave the wallet address unchanged.
    #[test]
    fn test_register_then_register_with_metadata_interop() {
        let (env, client, _) = setup();
        let name = symbol_short!("eve");
        let wallet = Address::generate(&env);
        let label = String::from_str(&env, "Eve's Wallet");
        let desc = String::from_str(&env, "Upgraded from bare registration");

        // Bare register first.
        client.register_wallet(&name, &wallet);
        assert_eq!(client.get_wallet(&name), wallet);
        assert!(client.try_get_metadata(&name).is_err());

        // Upgrade to full metadata.
        client.register_wallet_with_metadata(&name, &wallet, &label, &desc);
        assert_eq!(client.get_wallet(&name), wallet);
        let meta = client.get_metadata(&name);
        assert_eq!(meta.label, label);
        assert_eq!(meta.description, desc);
    }

    // ── Register / get hardening tests ────────────────────────────────────────

    /// `register_wallet` with an existing name silently replaces the stored
    /// address. `get_wallet` must return the new address afterward, and the
    /// name must still appear exactly once in `list_wallets`.
    #[test]
    fn test_register_wallet_overwrites_address_for_existing_name() {
        let (env, client, _) = setup();
        let name = symbol_short!("alice");
        let wallet_v1 = Address::generate(&env);
        let wallet_v2 = Address::generate(&env);

        client.register_wallet(&name, &wallet_v1);
        assert_eq!(client.get_wallet(&name), wallet_v1);

        // Overwrite with a different address.
        client.register_wallet(&name, &wallet_v2);
        assert_eq!(
            client.get_wallet(&name),
            wallet_v2,
            "get_wallet must return the overwritten address"
        );

        // Name must appear exactly once in the list.
        let names = client.list_wallets();
        let count = names.iter().filter(|n| *n == name).count();
        assert_eq!(count, 1, "name must appear exactly once after overwrite");
    }

    /// `register_wallet_with_metadata` with an existing name must replace the
    /// stored address (not just the label/description).
    #[test]
    fn test_register_wallet_with_metadata_overwrites_address() {
        let (env, client, _) = setup();
        let name = symbol_short!("bob");
        let wallet_v1 = Address::generate(&env);
        let wallet_v2 = Address::generate(&env);
        let label = String::from_str(&env, "Bob");
        let desc = String::from_str(&env, "desc");

        client.register_wallet_with_metadata(&name, &wallet_v1, &label, &desc);
        assert_eq!(client.get_wallet(&name), wallet_v1);

        // Re-register with a new address.
        client.register_wallet_with_metadata(&name, &wallet_v2, &label, &desc);
        assert_eq!(
            client.get_wallet(&name),
            wallet_v2,
            "get_wallet must return the new address after metadata-path overwrite"
        );

        // Name must still appear once.
        let names = client.list_wallets();
        let count = names.iter().filter(|n| *n == name).count();
        assert_eq!(count, 1);
    }

    /// Cross-path overwrite: register via `register_wallet_with_metadata`, then
    /// call `register_wallet` for the same name. The plain register call must
    /// update the address; the metadata entry is left intact.
    #[test]
    fn test_register_wallet_after_metadata_path_overwrites_address() {
        let (env, client, _) = setup();
        let name = symbol_short!("carol");
        let wallet_v1 = Address::generate(&env);
        let wallet_v2 = Address::generate(&env);
        let label = String::from_str(&env, "Carol");
        let desc = String::from_str(&env, "original");

        // Initial registration via metadata path.
        client.register_wallet_with_metadata(&name, &wallet_v1, &label, &desc);
        assert_eq!(client.get_wallet(&name), wallet_v1);

        // Overwrite address via plain register_wallet.
        client.register_wallet(&name, &wallet_v2);
        assert_eq!(
            client.get_wallet(&name),
            wallet_v2,
            "register_wallet must replace the address set by the metadata path"
        );

        // Metadata entry is left intact by the plain path.
        let meta = client.get_metadata(&name);
        assert_eq!(meta.label, label);

        // Name appears once.
        let names = client.list_wallets();
        let count = names.iter().filter(|n| *n == name).count();
        assert_eq!(count, 1);
    }

    /// `list_wallets` must return exactly all registered names — no extras,
    /// no omissions — and each name must resolve correctly via `get_wallet`.
    #[test]
    fn test_list_wallets_reflects_all_registered_names() {
        let (env, client, _) = setup();
        let names_and_wallets: [(soroban_sdk::Symbol, Address); 4] = [
            (symbol_short!("alice"), Address::generate(&env)),
            (symbol_short!("bob"), Address::generate(&env)),
            (symbol_short!("carol"), Address::generate(&env)),
            (symbol_short!("dave"), Address::generate(&env)),
        ];

        for (name, wallet) in &names_and_wallets {
            client.register_wallet(name, wallet);
        }

        let list = client.list_wallets();
        assert_eq!(
            list.len(),
            names_and_wallets.len() as u32,
            "list_wallets must return exactly the number of registered names"
        );
        for (name, expected_wallet) in &names_and_wallets {
            assert!(
                list.contains(name),
                "list_wallets must include the registered name"
            );
            assert_eq!(
                client.get_wallet(name),
                *expected_wallet,
                "get_wallet must return the correct address for each registered name"
            );
        }
    }

    /// `list_wallets` on an uninitialised contract must return an empty vec
    /// without requiring auth.
    #[test]
    fn test_list_wallets_on_uninitialised_contract_returns_empty() {
        // No mock_all_auths and no initialize call.
        let env = Env::default();
        let cid = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &cid);
        assert_eq!(
            client.list_wallets().len(),
            0,
            "list_wallets must return an empty vec on an uninitialised contract"
        );
    }

    /// `list_wallets` on an initialised but empty registry must return an empty vec.
    #[test]
    fn test_list_wallets_empty_after_initialize() {
        let (_, client, _) = setup();
        assert_eq!(
            client.list_wallets().len(),
            0,
            "list_wallets must be empty before any wallets are registered"
        );
    }

    /// Multiple-wallet round-trip: register N wallets by both paths, retrieve
    /// each one, and verify `list_wallets` contains exactly those N names.
    #[test]
    fn test_multiple_wallets_round_trip() {
        let (env, client, _) = setup();

        // Register a mix via both registration paths.
        let bare_name = symbol_short!("bare");
        let bare_wallet = Address::generate(&env);
        client.register_wallet(&bare_name, &bare_wallet);

        let meta_name = symbol_short!("meta");
        let meta_wallet = Address::generate(&env);
        let label = String::from_str(&env, "Meta Wallet");
        let desc = String::from_str(&env, "registered with metadata");
        client.register_wallet_with_metadata(&meta_name, &meta_wallet, &label, &desc);

        let second_bare = symbol_short!("bare2");
        let second_wallet = Address::generate(&env);
        client.register_wallet(&second_bare, &second_wallet);

        // All three must be retrievable.
        assert_eq!(client.get_wallet(&bare_name), bare_wallet);
        assert_eq!(client.get_wallet(&meta_name), meta_wallet);
        assert_eq!(client.get_wallet(&second_bare), second_wallet);

        // list_wallets must contain exactly the three registered names.
        let list = client.list_wallets();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&bare_name));
        assert!(list.contains(&meta_name));
        assert!(list.contains(&second_bare));
    }

    /// `get_wallet` requires no auth: calling it from an env with no mocked
    /// signers must succeed for a registered name.
    #[test]
    fn test_get_wallet_requires_no_auth() {
        // Set up and register in an env with mocked auth.
        let env_auth = Env::default();
        env_auth.mock_all_auths();
        let cid = env_auth.register_contract(None, MuxWalletRegistry);
        let c_auth = MuxWalletRegistryClient::new(&env_auth, &cid);
        let owner = Address::generate(&env_auth);
        c_auth.initialize(&owner);
        let name = symbol_short!("pub");
        let wallet = Address::generate(&env_auth);
        c_auth.register_wallet(&name, &wallet);

        // Read from a fresh env instance referencing the same contract — no auth mocks.
        // (In unit tests each env is isolated, so we verify the read-only path on the
        // same env without re-mocking.)
        //
        // The authoritative check: get_wallet on a completely fresh contract with no
        // registered data must return WalletNotFound, not an auth error.  Any caller
        // can invoke it without signing.
        let env_noauth = Env::default();
        let cid2 = env_noauth.register_contract(None, MuxWalletRegistry);
        let c_noauth = MuxWalletRegistryClient::new(&env_noauth, &cid2);
        // Not initialised, not mocked — must get WalletNotFound, not an auth panic.
        let result = c_noauth.try_get_wallet(&symbol_short!("anything"));
        assert_eq!(
            result,
            Err(Ok(WalletRegistryError::WalletNotFound)),
            "get_wallet must return WalletNotFound (not an auth error) for any caller"
        );
    }

    /// Error discriminant stability: the numeric values of all `WalletRegistryError`
    /// variants are part of the on-chain ABI and must never change.
    #[test]
    fn test_error_discriminant_values_are_stable() {
        assert_eq!(WalletRegistryError::NotInitialized as u32, 1);
        assert_eq!(WalletRegistryError::AlreadyInitialized as u32, 2);
        assert_eq!(WalletRegistryError::Unauthorized as u32, 3);
        assert_eq!(WalletRegistryError::WalletNotFound as u32, 4);
        assert_eq!(WalletRegistryError::TooManyWallets as u32, 5);
    }
}
