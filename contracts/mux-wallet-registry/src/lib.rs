/*!
 * mux-wallet-registry: Wallet registration registry for Mux Protocol.
 *
 * Tracks registered wallet addresses and their associated metadata.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: soroban_sdk::Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_wreg"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Wallet(Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WalletRecord {
    pub owner: Address,
    pub registered_at: u32,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxWalletRegistryError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    AlreadyRegistered = 4,
    WalletNotFound = 5,
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280;
const TTL_EXTEND_TO: u32 = 518_400;

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxWalletRegistry;

#[contractimpl]
impl MuxWalletRegistry {
    /// Initialize the registry with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxWalletRegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxWalletRegistryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register a wallet address. Admin only.
    pub fn register(env: Env, wallet: Address) -> Result<(), MuxWalletRegistryError> {
        Self::require_admin(&env)?;
        if env.storage().persistent().has(&DataKey::Wallet(wallet.clone())) {
            return Err(MuxWalletRegistryError::AlreadyRegistered);
        }
        let record = WalletRecord {
            owner: wallet.clone(),
            registered_at: env.ledger().sequence(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Wallet(wallet.clone()), &record);
        emit(&env, symbol_short!("register"), wallet);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Look up a registered wallet record.
    pub fn get_wallet(env: Env, wallet: Address) -> Result<WalletRecord, MuxWalletRegistryError> {
        env.storage()
            .persistent()
            .get(&DataKey::Wallet(wallet))
            .ok_or(MuxWalletRegistryError::WalletNotFound)
    }

    /// Check whether a wallet is registered.
    pub fn is_registered(env: Env, wallet: Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Wallet(wallet))
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxWalletRegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxWalletRegistryError::NotInitialized)?;
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
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, MuxWalletRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_register_and_get() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.register(&wallet);
        assert!(client.is_registered(&wallet));
        let record = client.get_wallet(&wallet);
        assert_eq!(record.owner, wallet);
    }

    #[test]
    fn test_double_register_fails() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.register(&wallet);
        assert!(client.try_register(&wallet).is_err());
    }

    #[test]
    fn test_get_unregistered_fails() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert!(client.try_get_wallet(&wallet).is_err());
    }

    #[test]
    fn test_is_registered_false_for_unknown() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert!(!client.is_registered(&wallet));
    }
}
