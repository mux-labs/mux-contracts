/*!
 * mux-wallet-registry: Wallet registry contract for Mux Protocol.
 *
 * Allows an owner to register and look up wallet addresses by a symbolic name.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Symbol};

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Storage keys used by the wallet registry contract.
#[contracttype]
pub enum DataKey {
    /// The owner address authorised to register wallets.
    Owner,
    /// A registered wallet entry keyed by name: DataKey::Wallet(name).
    Wallet(Symbol),
    /// Optional metadata keyed by name: DataKey::Metadata(name).
    Metadata(Symbol),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Optional metadata associated with a registered wallet entry.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletMetadata {
    /// Short human-readable label for this wallet entry.
    pub label: String,
    /// Optional description providing more context.
    pub description: String,
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
