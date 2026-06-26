/*!
 * mux-wallet-registry: Wallet registry contract for Mux Protocol.
 *
 * Allows an owner to register and look up wallet addresses by a symbolic name.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Storage keys used by the wallet registry contract.
#[contracttype]
pub enum DataKey {
    /// The owner address authorised to register wallets.
    Owner,
    /// A registered wallet entry keyed by name: DataKey::Wallet(name).
    Wallet(Symbol),
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WalletRegistryError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    WalletNotFound = 4,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxWalletRegistry;

#[contractimpl]
impl MuxWalletRegistry {
    /// Initialise the registry with an owner address.
    pub fn initialize(env: Env, owner: Address) -> Result<(), WalletRegistryError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(WalletRegistryError::AlreadyInitialized);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        Ok(())
    }

    /// Register (or update) a wallet address under `name`. Owner only.
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
    pub fn get_wallet(env: Env, name: Symbol) -> Result<Address, WalletRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Wallet(name))
            .ok_or(WalletRegistryError::WalletNotFound)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

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

    // ── #308 unit tests (happy path) ──────────────────────────────────────────

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
    fn test_register_wallet_overwrites_existing() {
        let (env, client, _) = setup();
        let name = symbol_short!("bob");
        let w1 = Address::generate(&env);
        let w2 = Address::generate(&env);
        client.register_wallet(&name, &w1);
        client.register_wallet(&name, &w2);
        assert_eq!(client.get_wallet(&name), w2);
    }

    #[test]
    fn test_multiple_distinct_wallets() {
        let (env, client, _) = setup();
        let alice = symbol_short!("alice");
        let bob = symbol_short!("bob");
        let carol = symbol_short!("carol");
        let wa = Address::generate(&env);
        let wb = Address::generate(&env);
        let wc = Address::generate(&env);
        client.register_wallet(&alice, &wa);
        client.register_wallet(&bob, &wb);
        client.register_wallet(&carol, &wc);
        assert_eq!(client.get_wallet(&alice), wa);
        assert_eq!(client.get_wallet(&bob), wb);
        assert_eq!(client.get_wallet(&carol), wc);
    }

    #[test]
    fn test_register_wallet_records_owner_auth() {
        let (env, client, owner) = setup();
        let name = symbol_short!("treasury");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);
        // Soroban mock_all_auths records every auth call; verify owner was required.
        assert!(env.auths().iter().any(|(addr, _)| addr == &owner));
    }

    // ── #309 negative-path tests ──────────────────────────────────────────────

    #[test]
    fn test_double_initialize_returns_already_initialized() {
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
}
