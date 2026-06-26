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
    /// Running count of distinct wallet names registered so far.
    // STORAGE-GRIEFING: used to enforce MAX_WALLETS on new insertions.
    WalletCount,
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
    // STORAGE-GRIEFING: registering more than MAX_WALLETS distinct names would
    // bloat instance storage and raise rent for every caller.
    TooManyWallets = 5,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of distinct wallet names that may be registered.
///
/// Each entry occupies roughly 42–50 bytes of instance storage
/// (10-byte Symbol key + 32-byte Address). At this cap the wallet
/// table consumes at most ~12 KB, well within Soroban's limits.
pub const MAX_WALLETS: u32 = 256;

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): extend instance TTL on every write so the registry
// stays live as long as it is actively used.  See docs/storage-griefing.md.
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
        env.storage().instance().set(&DataKey::WalletCount, &0u32);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register (or update) a wallet address under `name`. Owner only.
    ///
    /// New names are counted against `MAX_WALLETS`. Updating an existing
    /// name is always allowed regardless of the current count.
    ///
    /// # Errors
    /// - [`WalletRegistryError::NotInitialized`] if `initialize` was never called.
    /// - [`WalletRegistryError::TooManyWallets`] if adding a new name would
    ///   exceed `MAX_WALLETS`.
    pub fn register_wallet(
        env: Env,
        name: Symbol,
        wallet: Address,
    ) -> Result<(), WalletRegistryError> {
        Self::require_owner(&env)?;

        let is_new = !env.storage().instance().has(&DataKey::Wallet(name.clone()));
        if is_new {
            // STORAGE-GRIEFING: only count new keys; updates don't grow storage.
            let count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::WalletCount)
                .unwrap_or(0);
            if count >= MAX_WALLETS {
                return Err(WalletRegistryError::TooManyWallets);
            }
            env.storage()
                .instance()
                .set(&DataKey::WalletCount, &(count + 1));
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

    /// Produce a unique Symbol for index n without heap allocation.
    /// Generates "w0".."w256" — all ≤5 chars, valid Symbol characters.
    fn idx_sym(n: u32) -> Symbol {
        let h = (n / 100) as u8;
        let t = ((n % 100) / 10) as u8;
        let o = (n % 10) as u8;
        let mut buf = [0u8; 5];
        buf[0] = b'w';
        let len = if h > 0 {
            buf[1] = b'0' + h;
            buf[2] = b'0' + t;
            buf[3] = b'0' + o;
            4
        } else if t > 0 {
            buf[1] = b'0' + t;
            buf[2] = b'0' + o;
            3
        } else {
            buf[1] = b'0' + o;
            2
        };
        let s = core::str::from_utf8(&buf[..len]).expect("ascii digits are valid utf-8");
        Symbol::short(s)
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

    // ── Size-check tests ──────────────────────────────────────────────────────

    /// Filling to exactly MAX_WALLETS succeeds; the next distinct name fails.
    #[test]
    fn test_wallet_cap_enforced() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();

        for i in 0..MAX_WALLETS {
            let key = idx_sym(i);
            assert!(
                client.try_register_wallet(&key, &Address::generate(&env)).is_ok(),
                "insertion {i} should succeed"
            );
        }

        let overflow = idx_sym(MAX_WALLETS);
        assert_eq!(
            client.try_register_wallet(&overflow, &Address::generate(&env)),
            Err(Ok(WalletRegistryError::TooManyWallets))
        );
    }

    /// Updating an already-registered name must succeed even when the registry
    /// is at capacity (updates don't grow storage).
    #[test]
    fn test_update_allowed_at_cap() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();

        let first = symbol_short!("first");
        client.register_wallet(&first, &Address::generate(&env));

        // Fill the remaining MAX_WALLETS - 1 slots.
        for i in 1..MAX_WALLETS {
            client.register_wallet(&idx_sym(i), &Address::generate(&env));
        }

        // At capacity — updating `first` must still succeed.
        let updated = Address::generate(&env);
        assert!(
            client.try_register_wallet(&first, &updated).is_ok(),
            "update at cap must succeed"
        );
        assert_eq!(client.get_wallet(&first), updated);
    }

    /// A failed register_wallet call (cap exceeded) must not increment the
    /// count, so a subsequent update to an existing name still works.
    #[test]
    fn test_cap_rejection_does_not_corrupt_count() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();

        let known = symbol_short!("known");
        client.register_wallet(&known, &Address::generate(&env));

        for i in 1..MAX_WALLETS {
            client.register_wallet(&idx_sym(i), &Address::generate(&env));
        }

        // Overflow attempt must fail.
        assert_eq!(
            client.try_register_wallet(&idx_sym(MAX_WALLETS), &Address::generate(&env)),
            Err(Ok(WalletRegistryError::TooManyWallets))
        );

        // Count must be unchanged — updating `known` still works.
        let refreshed = Address::generate(&env);
        assert!(client.try_register_wallet(&known, &refreshed).is_ok());
        assert_eq!(client.get_wallet(&known), refreshed);
    }
}
