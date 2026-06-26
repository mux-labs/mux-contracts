/*!
 * mux-wallet-registry: Named wallet address registry for Mux Protocol.
 *
 * Maintains a mapping from symbolic names (`Symbol`) to wallet addresses
 * (`Address`). A single owner is set at initialisation time; only that owner
 * may write to the registry. Reads are open to any caller.
 *
 * # Public interface
 *
 * | Method            | Mutating | Auth required |
 * |-------------------|----------|---------------|
 * | `initialize`      | yes      | owner         |
 * | `register_wallet` | yes      | owner         |
 * | `get_wallet`      | no       | —             |
 *
 * # Errors
 *
 * All methods return `Result<_, WalletRegistryError>`. See that type for the
 * full set of error codes and when each is produced.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Persistent storage keys used by the wallet registry contract.
#[contracttype]
pub enum DataKey {
    /// The owner address authorised to register wallets.
    Owner,
    /// A registered wallet entry keyed by name: `DataKey::Wallet(name)`.
    Wallet(Symbol),
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
}

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
        env.storage()
            .instance()
            .set(&DataKey::Wallet(name), &wallet);
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
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, MockAuth, MockAuthInvoke},
        vec, Env,
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

    /// `register_wallet` must return `NotInitialized` when called before
    /// `initialize`.
    #[test]
    fn test_register_wallet_not_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let name = symbol_short!("alice");
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_register_wallet(&name, &wallet),
            Err(Ok(WalletRegistryError::NotInitialized))
        );
    }

    /// `get_wallet` on an uninitialised or empty registry returns
    /// `WalletNotFound` (not a host error), so callers can handle the
    /// miss without catching panics.
    #[test]
    fn test_get_wallet_before_any_registration() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        // No initialize, no entries — must return WalletNotFound cleanly.
        assert_eq!(
            client.try_get_wallet(&symbol_short!("miss")),
            Err(Ok(WalletRegistryError::WalletNotFound))
        );
    }

    /// `register_wallet` invocation without the owner's authorisation must
    /// fail at the host level. Auth is enforced by `Address::require_auth`;
    /// the call is rejected before reaching contract logic.
    #[test]
    fn test_register_wallet_requires_owner_auth() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);

        // Authorise only the initialize call.
        env.mock_auths(&[MockAuth {
            address: &owner,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "initialize",
                args: vec![&env, owner.to_val()],
                sub_invokes: &[],
            },
        }]);
        client.initialize(&owner);

        // register_wallet has no auth mocked → host rejects it.
        let name = symbol_short!("hack");
        let wallet = Address::generate(&env);
        assert!(client.try_register_wallet(&name, &wallet).is_err());
    }
}
