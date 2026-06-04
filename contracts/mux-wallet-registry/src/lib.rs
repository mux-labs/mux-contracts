/*!
 * mux-wallet-registry: Wallet registry contract for Mux Protocol.
 *
 * Allows an owner to register and look up wallet addresses by a symbolic name.
 * Only the initialised owner may write to the registry; reads are public.
 *
 * # Lifecycle
 * 1. Deploy the contract.
 * 2. Call [`MuxWalletRegistry::initialize`] once to set the owner.
 * 3. The owner calls [`MuxWalletRegistry::register_wallet`] to map symbolic
 *    names to wallet addresses.
 * 4. Any caller may resolve a name via [`MuxWalletRegistry::get_wallet`].
 */

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Storage keys used by the wallet registry contract.
///
/// All data is stored in **instance storage**, which shares the contract's TTL
/// and is archived together with it.
#[contracttype]
pub enum DataKey {
    /// The owner address authorised to register wallets.
    ///
    /// Set once during [`MuxWalletRegistry::initialize`] and never changed.
    Owner,
    /// A registered wallet entry keyed by name: `DataKey::Wallet(name)`.
    ///
    /// The value is the [`Address`] mapped to the given symbolic `name`.
    Wallet(Symbol),
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Errors returned by [`MuxWalletRegistry`] contract methods.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WalletRegistryError {
    /// The registry has not yet been initialised; call
    /// [`MuxWalletRegistry::initialize`] first.
    NotInitialized = 1,
    /// [`MuxWalletRegistry::initialize`] has already been called.
    /// The owner cannot be changed after initialisation.
    AlreadyInitialized = 2,
    /// The caller is not the registered owner.
    /// Only the owner may call [`MuxWalletRegistry::register_wallet`].
    Unauthorized = 3,
    /// No wallet address is registered under the requested name.
    /// Use [`MuxWalletRegistry::register_wallet`] to add an entry first.
    WalletNotFound = 4,
}

// ── Contract ──────────────────────────────────────────────────────────────────

/// On-chain wallet registry for Mux Protocol.
///
/// Associates symbolic [`Symbol`] names with [`Address`] values so that other
/// contracts and off-chain clients can resolve wallet addresses by name rather
/// than hard-coding addresses.
///
/// # Access control
/// A single *owner* address is set at initialisation time. All write
/// operations require the owner's authorisation; reads are unrestricted.
#[contract]
pub struct MuxWalletRegistry;

#[contractimpl]
impl MuxWalletRegistry {
    /// Initialise the registry with an owner address.
    ///
    /// Must be called exactly once after deployment. The `owner` address must
    /// authorise this call (i.e. sign the transaction).
    ///
    /// # Errors
    /// * [`WalletRegistryError::AlreadyInitialized`] — if called more than once.
    pub fn initialize(env: Env, owner: Address) -> Result<(), WalletRegistryError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(WalletRegistryError::AlreadyInitialized);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        Ok(())
    }

    /// Register (or update) a wallet address under `name`. Owner only.
    ///
    /// If an entry for `name` already exists it is overwritten with the new
    /// `wallet` address. The owner's authorisation is required.
    ///
    /// # Arguments
    /// * `name`   — Symbolic key for the wallet (max 32 bytes UTF-8).
    /// * `wallet` — The [`Address`] to associate with `name`.
    ///
    /// # Errors
    /// * [`WalletRegistryError::NotInitialized`] — registry not yet initialised.
    /// * [`WalletRegistryError::Unauthorized`]   — caller is not the owner.
    pub fn register_wallet(
        env: Env,
        name: Symbol,
        wallet: Address,
    ) -> Result<(), WalletRegistryError> {
        Self::require_owner(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::Wallet(name), &wallet);
        Ok(())
    }

    /// Return the wallet address registered under `name`.
    ///
    /// This is a read-only operation and requires no authorisation.
    ///
    /// # Arguments
    /// * `name` — Symbolic key to look up.
    ///
    /// # Errors
    /// * [`WalletRegistryError::WalletNotFound`] — no entry exists for `name`.
    pub fn get_wallet(env: Env, name: Symbol) -> Result<Address, WalletRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Wallet(name))
            .ok_or(WalletRegistryError::WalletNotFound)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    /// Assert that the stored owner has authorised the current invocation.
    ///
    /// Returns [`WalletRegistryError::NotInitialized`] if the owner has not
    /// been set, otherwise delegates to [`Address::require_auth`] which panics
    /// if the authorisation is absent.
    fn require_owner(env: &Env) -> Result<(), WalletRegistryError> {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(WalletRegistryError::NotInitialized)?;
        owner.require_auth();
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Env};

    fn setup() -> (Env, MuxWalletRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        client.initialize(&owner);
        (env, client, owner)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        assert!(client.try_initialize(&owner).is_ok());
        assert_eq!(
            client.try_initialize(&owner),
            Err(Ok(WalletRegistryError::AlreadyInitialized))
        );
    }

    #[test]
    fn test_register_emits_event() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.register(&wallet);
        let events = env.events().all();
        // init + register
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("register"));
    }

    #[test]
    fn test_register_and_get() {
        let (env, client, _) = setup();
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);
        assert_eq!(client.get_wallet(&name), wallet);
    }

    #[test]
    fn test_get_wallet_not_found() {
        let (_, client, _) = setup();
        let result = client.try_get_wallet(&symbol_short!("ghost"));
        assert_eq!(result, Err(Ok(WalletRegistryError::WalletNotFound)));
    }

    #[test]
    fn test_register_wallet_updates_existing() {
        let (env, client, _) = setup();
        let name = symbol_short!("bob");
        let wallet1 = Address::generate(&env);
        let wallet2 = Address::generate(&env);
        client.register_wallet(&name, &wallet1);
        client.register_wallet(&name, &wallet2);
        assert_eq!(client.get_wallet(&name), wallet2);
    }

    /// A non-owner caller must not be able to register a wallet.
    /// `register_wallet` internally calls `owner.require_auth()`; without
    /// a mock the Soroban test runtime panics, confirming the access guard.
    #[test]
    #[should_panic]
    fn test_register_wallet_unauthorized() {
        // No mock_all_auths — any require_auth() call will panic.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        // initialize itself calls owner.require_auth() → panics.
        client.initialize(&owner);
    }
}
