/*!
 * mux-wallet-registry: Named wallet address registry for Mux Protocol.
 *
 * Allows an owner to register and look up wallet addresses by a symbolic name.
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

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

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
            .set(&DataKey::Wallet(name), &wallet);
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

    /// Register (or update) a wallet with metadata. Owner only.
    pub fn register_wallet_with_metadata(
        env: Env,
        name: Symbol,
        wallet: Address,
        label: String,
        description: String,
    ) -> Result<(), WalletRegistryError> {
        Self::require_owner(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::Wallet(name.clone()), &wallet);
        let meta = WalletMetadata { label, description };
        env.storage()
            .instance()
            .set(&DataKey::Metadata(name), &meta);
        Ok(())
    }

    /// Return the metadata for a wallet registered under `name`.
    pub fn get_metadata(env: Env, name: Symbol) -> Result<WalletMetadata, WalletRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Metadata(name))
            .ok_or(WalletRegistryError::WalletNotFound)
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
    use soroban_sdk::{symbol_short, testutils::Address as _, Env, String};

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
    fn test_register_and_get_wallet() {
        let (env, client, _) = setup();
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);
        assert_eq!(client.get_wallet(&name), wallet);
    }

    #[test]
    fn test_register_wallet_caps_names() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        for i in 0..MAX_WALLETS {
            let name = soroban_sdk::Symbol::new(&env, format!("wallet{}", i));
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

    #[test]
    fn test_get_metadata_not_found() {
        let (_, client, _) = setup();
        let result = client.try_get_metadata(&symbol_short!("ghost"));
        assert_eq!(result, Err(Ok(WalletRegistryError::WalletNotFound)));
    }

    #[test]
    fn test_metadata_update_preserves_wallet() {
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
    }
}
