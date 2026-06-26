/*!
 * mux-wallet-registry: Wallet registry contract for Mux Protocol.
 *
 * Allows an owner to register and look up wallet addresses by a symbolic name.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

// ── Audit events ──────────────────────────────────────────────────────────────

fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_wreg"), action), data);
}

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
    ///
    /// Emits: `("mux_wreg", "init")` with data `owner: Address`.
    pub fn initialize(env: Env, owner: Address) -> Result<(), WalletRegistryError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(WalletRegistryError::AlreadyInitialized);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        emit(&env, symbol_short!("init"), owner);
        Ok(())
    }

    /// Register (or update) a wallet address under `name`. Owner only.
    ///
    /// Emits: `("mux_wreg", "wlt_reg")` with data `(name: Symbol, wallet: Address)`.
    pub fn register_wallet(
        env: Env,
        name: Symbol,
        wallet: Address,
    ) -> Result<(), WalletRegistryError> {
        Self::require_owner(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::Wallet(name.clone()), &wallet);
        emit(&env, symbol_short!("wlt_reg"), (name, wallet));
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
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events},
        Env, FromVal,
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

    // ── Event tests ───────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_emits_init_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        client.initialize(&owner);

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));

        // Verify data payload is the owner address.
        let (_, _, data) = events.get(0).unwrap();
        let emitted_owner = Address::from_val(&env, &data);
        assert_eq!(emitted_owner, owner);
    }

    #[test]
    fn test_register_wallet_emits_wlt_reg_event() {
        let (env, client, _) = setup();
        let name = symbol_short!("vault");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);

        let events = env.events().all();
        // events[0] = init from setup, events[1] = wlt_reg
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("wlt_reg"));
    }

    #[test]
    fn test_failed_register_emits_no_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxWalletRegistry);
        let client = MuxWalletRegistryClient::new(&env, &contract_id);

        // register_wallet before initialize — must fail with NotInitialized
        let name = symbol_short!("x");
        let wallet = Address::generate(&env);
        let result = client.try_register_wallet(&name, &wallet);
        assert_eq!(result, Err(Ok(WalletRegistryError::NotInitialized)));

        // No events emitted for the failed call.
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_get_wallet_emits_no_event() {
        let (env, client, _) = setup();
        let name = symbol_short!("probe");
        let wallet = Address::generate(&env);
        client.register_wallet(&name, &wallet);

        let before = env.events().all().len();
        client.get_wallet(&name);
        // Read-only call must not append any new events.
        assert_eq!(env.events().all().len(), before);
    }
}
