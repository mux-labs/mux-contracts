/*!
 * mux-wallet-registry: Wallet registry contract for Mux Protocol.
 *
 * Allows an owner to register and look up wallet addresses by a symbolic name.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Storage keys used by the wallet registry contract.
#[contracttype]
pub enum DataKey {
    /// The owner address authorised to register wallets.
    Owner,
    /// A registered wallet entry keyed by name: DataKey::Wallet(name).
    Wallet(Symbol),
    /// List of wallet names registered in this contract.
    Names,
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
    TooManyWallets = 5,
}

// ── Storage limits ─────────────────────────────────────────────────────────────

/// Maximum number of distinct wallet names that may be registered.
const MAX_WALLETS: u32 = 128;

// ── Storage TTL ─────────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

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
        env.storage()
            .instance()
            .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register (or update) a wallet address under `name`. Owner only.
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
    fn test_register_and_get() {
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
}
