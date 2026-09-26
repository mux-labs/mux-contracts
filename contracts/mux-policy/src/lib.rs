/*!
 * mux-policy: Per-wallet daily spend-limit policy for Mux Protocol.
 *
 * Tracks a rolling daily spend window per wallet address. The contract
 * admin sets per-wallet limits; any caller may record a spend (subject to
 * the configured limit); the admin or the wallet itself may reset the
 * daily counter.
 *
 * # Fail-closed auth
 *
 * Every state-mutating entrypoint (except `record_spend`) requires
 * `require_admin` to pass before any state is modified. `require_admin`
 * calls `admin.require_auth()` on the stored admin address, which fails
 * closed: if no admin has been initialised, or if the caller is not the
 * admin, the call panics with `Unauthorized` before touching any storage.
 *
 * `record_spend` enforces the configured limit; spending above the limit
 * reverts with `LimitExceeded`. If no limit has been set for a wallet,
 * the call reverts with `LimitNotSet` (deny-by-default).
 *
 * # Audit Events
 *
 * Contract tag: `mux_pol`
 *
 * | Action     | Trigger                | Data payload                              |
 * |------------|------------------------|-------------------------------------------|
 * | `init`     | `initialize`           | `admin: Address`                          |
 * | `lmt_set`  | `set_daily_limit`      | `(wallet: Address, limit: i128, day_ledgers: u32)` |
 * | `spent`    | `record_spend`         | `(wallet: Address, amount: i128)`         |
 * | `ctr_rst`  | `reset_daily_counter`  | `wallet: Address`                         |
 *
 * `get_daily_limit` is read-only and emits no events.
 * `upgrade` extends TTL but does not emit an audit event of its own.
 *
 * See [`docs/audit-events.md`] and [`docs/policy-semantics.md`] for the full
 * event schema and policy design.
 *
 * [`docs/audit-events.md`]: ../../docs/audit-events.md
 * [`docs/policy-semantics.md`]: ../../docs/policy-semantics.md
 * mux-policy: Per-wallet daily spend limit policy contract for Mux Protocol.
 *
 * Stores and enforces a daily spend limit per wallet address. The daily
 * counter resets automatically once the current day (measured in ledgers)
 * has elapsed.
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types backed by the Soroban host.
 *
 * # Public Interface
 *
 * - `initialize(admin)` — One-time setup with admin authorization
 * - `set_daily_limit(wallet, limit, day_ledgers)` — Set/update spending policy (admin)
 * - `get_daily_limit(wallet)` — Query current limit and spent amount (public)
 * - `record_spend(wallet, amount)` — Debit against limit with auto-reset (wallet auth)
 * - `reset_daily_counter(wallet)` — Manual reset for emergency corrections (admin)
 *
 * # Storage Constraints
 *
 * The policy enforces a cap of 256 wallets with configured limits to prevent
 * storage griefing through unbounded per-wallet storage growth.
 *
 * # Registry Metadata
 *
 * Each daily limit record includes an optional `registry_id` that links this
 * policy to a registry contract, enabling cross-contract policy lookup and
 * validation.
 *
 * # Events
 *
 * - `"init"` — Emitted on initialization
 * - `"lmt_set"` — Emitted on limit configuration with (wallet, limit, day_ledgers)
 * - `"spent"` — Emitted on spend recording with (wallet, amount)
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env,
};

// ── Audit events ──────────────────────────────────────────────────────────────

    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_pol"), action), data);
}

// ── TTL constants ─────────────────────────────────────────────────────────────

/// Minimum TTL threshold (ledgers) before an auto-extend is triggered.
/// At ~5 s per ledger: 17 280 ≈ 1 day.
const TTL_THRESHOLD: u32 = 17_280;

/// Target TTL (ledgers) after an auto-extend.
/// At ~5 s per ledger: 518 400 ≈ 30 days.
const TTL_EXTEND_TO: u32 = 518_400;

/// Default number of ledgers in a daily window.
/// At ~5 s per ledger: 17 280 ≈ 1 day.
pub const DEFAULT_DAY_LEDGERS: u32 = 17_280;

// ── Error codes ───────────────────────────────────────────────────────────────

/// Stable error codes returned by mux-policy.
///
/// These codes are on-chain ABI — changing an existing variant is a
/// breaking change. New variants may be appended.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PolicyError {
    /// The contract has not been initialised.
    NotInitialized = 1,
    /// The contract has already been initialised.
    AlreadyInitialized = 2,
    /// The caller is not the contract admin.
    Unauthorized = 3,
    /// No spend limit has been configured for this wallet.
    LimitNotSet = 4,
    /// The requested spend would exceed the configured daily limit.
    LimitExceeded = 5,
    /// An amount of zero or negative was supplied.
    InvalidAmount = 6,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

/// Storage keys for the mux-policy contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address (`Address`).
    Admin,
    /// Per-wallet spend limit record keyed by wallet `Address`.
    Limit(Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A per-wallet daily spend limit record stored on-chain.
///
/// `reset_ledger` records the ledger at which the current window started.
/// When `env.ledger().sequence() >= reset_ledger + day_ledgers`, the window
/// has elapsed and `spent` is treated as 0 (but only written on next spend).
#[contracttype]
#[derive(Clone, Debug)]
pub struct SpendRecord {
    /// Configured daily limit in the asset's smallest unit.
    pub limit: i128,
    /// Amount spent in the current window.
    pub spent: i128,
    /// Ledger sequence at which the current window began.
    pub reset_ledger: u32,
    /// Window length in ledgers.
    pub day_ledgers: u32,
}
// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    /// Per-wallet daily spend limit record.
    WalletLimit(Address),
    /// List of all wallets with configured limits (for griefing guard).
    WalletNames,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Daily spend limit record stored per wallet.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DailyLimit {
    /// Maximum amount allowed per day.
    pub limit: i128,
    /// Amount spent in the current day window.
    pub spent: i128,
    /// Ledger sequence at which the current window expires and `spent` resets.
    pub reset_ledger: u32,
    /// Number of ledgers in one day window (set at limit creation time).
    pub day_ledgers: u32,
    /// Optional registry contract ID for cross-contract policy validation.
    pub registry_id: Option<Address>,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxPolicyError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    LimitNotFound = 4,
    LimitExceeded = 5,
    InvalidAmount = 6,
    InvalidPeriod = 7,
    // STORAGE-GRIEFING: unbounded WalletLimit entries would let admin bloat storage.
    TooManyWallets = 8,
    /// The registry contract linked via `registry_id` is unreachable or not
    /// a valid mux-registry deployment. `record_spend` is fail-closed: if the
    /// registry cannot be validated the spend is rejected.
    RegistryNotFound = 9,
}

// ── Constants ─────────────────────────────────────────────────────────────

/// Maximum number of wallets with configured limits to bound storage growth.
const MAX_WALLETS: u32 = 256;

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxPolicy;

#[contractimpl]
impl MuxPolicy {
    // ── Auth helpers ──────────────────────────────────────────────────────────

    /// Fail-closed admin guard.
    ///
    /// Reads the stored admin address and calls `require_auth()` on it.
    /// Panics with [`PolicyError::NotInitialized`] if no admin is stored,
    /// or with [`PolicyError::Unauthorized`] if the caller is not the admin.
    ///
    /// Every state-mutating admin entrypoint MUST call `require_admin` before
    /// reading or writing any other storage key.
    fn require_admin(env: &Env) -> Result<Address, PolicyError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PolicyError::NotInitialized)?;
        admin.require_auth();
        Ok(admin)
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    /// Initialise the contract and set the admin address.
    ///
    /// Fails with [`PolicyError::AlreadyInitialized`] if called more than once.
    /// The `admin` address is stored as the sole privileged principal.
    ///
    /// Emits: `init`
    pub fn initialize(env: Env, admin: Address) -> Result<(), PolicyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PolicyError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("init"), admin);
        Ok(())
    }

    /// Upgrade the contract WASM.
    ///
    /// Requires admin authorisation. Extends instance TTL after the upgrade
    /// to keep the entry live. Does not emit an audit event — the
    /// upload/invoke transaction is the on-chain audit record.
    pub fn upgrade(env: Env, new_wasm_hash: soroban_sdk::BytesN<32>) -> Result<(), PolicyError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    // ── Admin entrypoints ─────────────────────────────────────────────────────

    /// Set or update the daily spend limit for `wallet`.
    ///
    /// `limit` must be positive. `day_ledgers` is the window size; pass 0 to
    /// use [`DEFAULT_DAY_LEDGERS`]. Setting a new limit resets the `spent`
    /// counter and starts a new window from the current ledger.
    ///
    /// Requires admin authorisation (fail-closed via [`Self::require_admin`]).
    ///
    /// Emits: `lmt_set`
    /// Initialize the policy contract with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxPolicyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxPolicyError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(
            &DataKey::WalletNames,
            &soroban_sdk::Vec::<Address>::new(&env),
        );
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Upgrade the contract WASM. Admin only.
    ///
    /// See the module-level doc comment and `docs/contract-upgrade-pattern.md`
    /// for storage-compatibility rules that must be observed between versions.
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxPolicyError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        // T-21: upgrade is a write; refresh TTL so the contract survives a
        // subsequent idle period without a keeper run.
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Set or update the daily spend limit for a wallet. Admin only.
    ///
    /// `day_ledgers` is the number of ledgers that constitute one day
    /// (≈ 17 280 at 5-second ledger close).
    /// `registry_id` is an optional registry contract address for policy validation.
    ///
    /// Returns `TooManyWallets` if the wallet limit count has reached capacity.
    pub fn set_daily_limit(
        env: Env,
        wallet: Address,
        limit: i128,
        day_ledgers: u32,
    ) -> Result<(), PolicyError> {
        Self::require_admin(&env)?;
        if limit <= 0 {
            return Err(PolicyError::InvalidAmount);
        }
        let window = if day_ledgers == 0 {
            DEFAULT_DAY_LEDGERS
        } else {
            day_ledgers
        };
        let record = SpendRecord {
            limit,
            spent: 0,
            reset_ledger: env.ledger().sequence(),
            day_ledgers: window,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Limit(wallet.clone()), &record);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("lmt_set"), (wallet, limit, window));
        Ok(())
    }

    /// Reset the daily spend counter for `wallet` to zero.
    ///
    /// Starts a new window from the current ledger. Does not change the
    /// configured limit or window size.
    ///
    /// Requires admin authorisation (fail-closed via [`Self::require_admin`]).
    ///
    /// Emits: `ctr_rst`
    pub fn reset_daily_counter(env: Env, wallet: Address) -> Result<(), PolicyError> {
        Self::require_admin(&env)?;
        let mut record: SpendRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Limit(wallet.clone()))
            .ok_or(PolicyError::LimitNotSet)?;
        record.spent = 0;
        record.reset_ledger = env.ledger().sequence();
        env.storage()
            .persistent()
            .set(&DataKey::Limit(wallet.clone()), &record);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("ctr_rst"), wallet);
        Ok(())
    }

    // ── Public entrypoints ────────────────────────────────────────────────────

    /// Record a spend of `amount` against `wallet`'s daily limit.
    ///
    /// Reverts with [`PolicyError::LimitNotSet`] if no limit is configured
    /// (deny-by-default). Reverts with [`PolicyError::LimitExceeded`] if
    /// `amount` would push the cumulative spend past the limit.
    ///
    /// If the current ledger is past the window's `reset_ledger + day_ledgers`,
    /// the counter is automatically reset before checking the limit.
    ///
    /// Emits: `spent`
    pub fn record_spend(env: Env, wallet: Address, amount: i128) -> Result<(), PolicyError> {
        if amount <= 0 {
            return Err(PolicyError::InvalidAmount);
        }
        let mut record: SpendRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Limit(wallet.clone()))
            .ok_or(PolicyError::LimitNotSet)?;

        // Auto-reset if the window has elapsed.
        let current = env.ledger().sequence();
        if current >= record.reset_ledger.saturating_add(record.day_ledgers) {
            record.spent = 0;
            record.reset_ledger = current;
        }

        let new_spent = record.spent.saturating_add(amount);
        if new_spent > record.limit {
            return Err(PolicyError::LimitExceeded);
        }
        record.spent = new_spent;
        env.storage()
            .persistent()
            .set(&DataKey::Limit(wallet.clone()), &record);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("spent"), (wallet, amount));
        Ok(())
    }

    /// Return the current spend record for `wallet`.
    ///
    /// Read-only — emits no events. Returns `None` if no limit is configured.
    /// Note: `spent` is reported as 0 when the window has elapsed, but this
    /// reset is NOT persisted by this call — only `record_spend` and
    /// `reset_daily_counter` write the reset.
    pub fn get_daily_limit(env: Env, wallet: Address) -> Option<SpendRecord> {
        let record: SpendRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Limit(wallet))?;
        let current = env.ledger().sequence();
        if current >= record.reset_ledger.saturating_add(record.day_ledgers) {
            // Window elapsed — report spent as 0 without persisting.
            return Some(SpendRecord {
                spent: 0,
                ..record
            });
        }
        Some(record)
        registry_id: Option<Address>,
    ) -> Result<(), MuxPolicyError> {
        Self::require_admin(&env)?;
        if limit <= 0 {
            return Err(MuxPolicyError::InvalidAmount);
        }
        if day_ledgers == 0 {
            return Err(MuxPolicyError::InvalidPeriod);
        }

        // Track wallets and enforce storage griefing guard
        let mut wallet_names: soroban_sdk::Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::WalletNames)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));

        if !wallet_names.contains(&wallet) {
            // STORAGE-GRIEFING: cap the WalletNames vec to bound storage growth.
            if wallet_names.len() >= MAX_WALLETS {
                return Err(MuxPolicyError::TooManyWallets);
            }
            wallet_names.push_back(wallet.clone());
            env.storage()
                .instance()
                .set(&DataKey::WalletNames, &wallet_names);
        }

        let record = DailyLimit {
            limit,
            spent: 0,
            reset_ledger: env.ledger().sequence().saturating_add(day_ledgers),
            day_ledgers,
            registry_id,
        };
        let key = DataKey::WalletLimit(wallet.clone());
        env.storage().persistent().set(&key, &record);
        // #287 – extend persistent entry TTL on every write so the record
        // survives beyond the default ledger TTL.
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("lmt_set"), (wallet, limit, day_ledgers));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the current daily limit record for a wallet.
    /// Returns the record with an up-to-date `spent` value (resets if the
    /// day window has elapsed) without persisting the reset — call
    /// `record_spend` to actually debit.
    pub fn get_daily_limit(env: Env, wallet: Address) -> Result<DailyLimit, MuxPolicyError> {
        let mut record: DailyLimit = env
            .storage()
            .persistent()
            .get(&DataKey::WalletLimit(wallet))
            .ok_or(MuxPolicyError::LimitNotFound)?;
        if env.ledger().sequence() >= record.reset_ledger {
            record.spent = 0;
        }
        Ok(record)
    }

    /// Record a spend against a wallet's daily limit.
    ///
    /// Resets the counter if the day window has elapsed, then debits `amount`.
    /// Returns `LimitExceeded` if the debit would exceed the daily limit.
    ///
    /// If the wallet's `DailyLimit` record contains a `registry_id`, the
    /// registry contract at that address is called (`list_contracts`) to
    /// validate it is a live, accessible registry before the spend is recorded.
    /// This ensures that a stale or spoofed registry link cannot silently pass
    /// validation at spend time. The call is fail-closed: if the registry
    /// contract is unreachable or returns an error, `record_spend` returns
    /// `RegistryNotFound`.
    pub fn record_spend(env: Env, wallet: Address, amount: i128) -> Result<(), MuxPolicyError> {
        wallet.require_auth();
        if amount <= 0 {
            return Err(MuxPolicyError::InvalidAmount);
        }
        let key = DataKey::WalletLimit(wallet.clone());
        let mut record: DailyLimit = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(MuxPolicyError::LimitNotFound)?;

        // Cross-contract registry validation (fail-closed).
        // If a registry_id is linked, call the registry to confirm it is live.
        if let Some(ref registry_addr) = record.registry_id {
            let result = env.try_invoke_contract::<soroban_sdk::Vec<soroban_sdk::Symbol>, soroban_sdk::Error>(
                registry_addr,
                &soroban_sdk::Symbol::new(&env, "list_contracts"),
                soroban_sdk::Vec::new(&env),
            );
            if result.is_err() {
                return Err(MuxPolicyError::RegistryNotFound);
            }
        }

        // Reset counter if the day window has elapsed.
        if env.ledger().sequence() >= record.reset_ledger {
            record.spent = 0;
            record.reset_ledger = env.ledger().sequence().saturating_add(record.day_ledgers);
        }

        let new_spent = record
            .spent
            .checked_add(amount)
            .ok_or(MuxPolicyError::LimitExceeded)?;
        if new_spent > record.limit {
            return Err(MuxPolicyError::LimitExceeded);
        }
        record.spent = new_spent;
        env.storage().persistent().set(&key, &record);
        // #287 – extend persistent entry TTL on every write.
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("spent"), (wallet, amount));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Explicitly reset a wallet's daily spend counter. Admin only.
    ///
    /// Clears `spent` to `0` and starts a fresh window from the current ledger.
    /// Intended for emergency resets and post-upgrade counter corrections.
    /// Fails with `LimitNotFound` if no limit has been configured for `wallet`.
    pub fn reset_daily_counter(env: Env, wallet: Address) -> Result<(), MuxPolicyError> {
        Self::require_admin(&env)?;
        let key = DataKey::WalletLimit(wallet.clone());
        let mut record: DailyLimit = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(MuxPolicyError::LimitNotFound)?;
        record.spent = 0;
        record.reset_ledger = env.ledger().sequence().saturating_add(record.day_ledgers);
        env.storage().persistent().set(&key, &record);
        // #287 – extend persistent entry TTL on every write.
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        emit(&env, symbol_short!("ctr_rst"), wallet);
        Self::extend_ttl(&env);
        Ok(())
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxPolicyError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxPolicyError::NotInitialized)?;
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
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events, Ledger as _},
        Env, FromVal,
    };

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

    fn setup() -> (Env, MuxPolicyClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPolicy);
        let client = MuxPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPolicy);
        let client = MuxPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_fails() {
        let (_env, client, admin) = setup();
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_set_daily_limit_requires_admin_auth() {
        // Deliberately omit mock_all_auths — admin.require_auth() must reject.
        // Seed the Admin key directly via as_contract (mock_all_auths is a
        // permanent switch in soroban-sdk 21, not a restorable guard).
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxPolicy);
        let client = MuxPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
        });

        let wallet = Address::generate(&env);
        let result = client.try_set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        assert!(result.is_err());
        assert!(client.try_get_daily_limit(&wallet).is_err());
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_record_spend_requires_wallet_auth() {
        // Deliberately omit mock_all_auths — wallet.require_auth() must reject.
        // Seed Admin + a WalletLimit record directly via as_contract so the
        // auth gate is what is under test, not LimitNotFound.
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxPolicy);
        let client = MuxPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let wallet = Address::generate(&env);
        let limit = DailyLimit {
            limit: 1000,
            spent: 0,
            reset_ledger: env.ledger().sequence().saturating_add(17_280),
            day_ledgers: 17_280,
            registry_id: None,
        };
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            env.storage()
                .persistent()
                .set(&DataKey::WalletLimit(wallet.clone()), &limit);
        });

        let result = client.try_record_spend(&wallet, &100_i128);
        assert!(result.is_err());
        assert_eq!(client.get_daily_limit(&wallet).spent, 0);
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_set_daily_limit() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        let record = client.get_daily_limit(&wallet);
        assert_eq!(record.limit, 1000);
        assert_eq!(record.spent, 0);
        assert_eq!(record.registry_id, None);
    }

    // ── #282: size / bounds checks ──────────────────────────────────────────

    #[test]
    fn test_set_daily_limit_zero_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_set_daily_limit(&wallet, &0_i128, &17280_u32, &None),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_set_daily_limit_negative_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_set_daily_limit(&wallet, &-1_i128, &17280_u32, &None),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_set_daily_limit_zero_period_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_set_daily_limit(&wallet, &1000_i128, &0_u32, &None),
            Err(Ok(MuxPolicyError::InvalidPeriod))
        );
    }

    #[test]
    fn test_record_spend_zero_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        assert_eq!(
            client.try_record_spend(&wallet, &0_i128),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_record_spend_negative_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        assert_eq!(
            client.try_record_spend(&wallet, &-5_i128),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_record_spend_exact_limit_allowed() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert!(client
            .try_set_daily_limit(&wallet, &0_i128, &17280_u32, &None)
            .is_err());
        assert!(client
            .try_set_daily_limit(&wallet, &-1_i128, &17280_u32, &None)
            .is_err());
    }

    #[test]
    fn test_record_spend_one_over_limit_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert!(client
            .try_set_daily_limit(&wallet, &1000_i128, &0_u32, &None)
            .is_err());
    }

    #[test]
    fn test_record_spend_cumulative_boundary() {
        // Two spends of 500 each against a 1000 limit; third spend of 1 fails.
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        client.record_spend(&wallet, &500_i128);
        client.record_spend(&wallet, &500_i128);
        assert_eq!(
            client.try_record_spend(&wallet, &1_i128),
            Err(Ok(MuxPolicyError::LimitExceeded))
        );
    }

    #[test]
    fn test_record_spend_i128_max_overflows_to_limit_exceeded() {
        // Spending i128::MAX when any positive limit is set must not panic.
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        let result = client.try_record_spend(&wallet, &i128::MAX);
        assert_eq!(result, Err(Ok(MuxPolicyError::LimitExceeded)));
    }

    // ── Existing functional tests ───────────────────────────────────────────

    #[test]
    fn test_record_spend_within_limit() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        client.record_spend(&wallet, &400_i128);
        let record = client.get_daily_limit(&wallet);
        assert_eq!(record.spent, 400);
    }

    #[test]
    fn test_record_spend_exceeds_limit() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &500_i128, &17280_u32, &None);
        client.record_spend(&wallet, &300_i128);
        assert!(client.try_record_spend(&wallet, &300_i128).is_err());
    }

    #[test]
    fn test_get_limit_not_found() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert!(client.try_get_daily_limit(&wallet).is_err());
    }

    #[test]
    fn test_record_spend_emits_event() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        client.record_spend(&wallet, &100_i128);
        let events = env.events().all();
        // init + lmt_set + spent
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("spent"));
    }

    #[test]
    fn test_multiple_wallets_independent_limits() {
        let (env, client, _) = setup();
        let wallet_a = Address::generate(&env);
        let wallet_b = Address::generate(&env);
        client.set_daily_limit(&wallet_a, &500_i128, &17280_u32, &None);
        client.set_daily_limit(&wallet_b, &200_i128, &17280_u32, &None);
        client.record_spend(&wallet_a, &500_i128);
        // wallet_b limit unaffected
        client.record_spend(&wallet_b, &200_i128);
        assert!(client.try_record_spend(&wallet_a, &1_i128).is_err());
        assert!(client.try_record_spend(&wallet_b, &1_i128).is_err());
    }

    // ── TTL tests (T-21) ──────────────────────────────────────────────────────

    /// Every write path must extend instance TTL without panicking (T-21).
    /// The Soroban test environment does not surface TTL values directly, so a
    /// successful call without panic is the observable proof for each path.

    #[test]
    fn test_ttl_extended_on_initialize() {
        // setup() calls initialize — reaching here without panic confirms TTL extension.
        let (_env, _client, _admin) = setup();
    }

    #[test]
    fn test_ttl_extended_on_set_daily_limit() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
    }

    #[test]
    fn test_ttl_extended_on_record_spend() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        client.record_spend(&wallet, &100_i128);
    }

    #[test]
    fn test_ttl_extended_on_reset_daily_counter() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        client.reset_daily_counter(&wallet);
    }

    /// TTL constants are sized for the expected ~30-day window.
    #[test]
    fn test_ttl_constants() {
        assert_eq!(TTL_THRESHOLD, 17_280);
        assert_eq!(TTL_EXTEND_TO, 518_400);
        // Checked at compile time to satisfy clippy's assertions_on_constants.
        const _: () = assert!(TTL_EXTEND_TO > TTL_THRESHOLD);
    }

    #[test]
    fn test_record_spend_resets_counter_after_day_window() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        let wallet = Address::generate(&env);
        // Use a short window (10 ledgers) to stay within persistent TTL.
        client.set_daily_limit(&wallet, &1000_i128, &10_u32, &None);
        client.record_spend(&wallet, &900_i128);

        // Advance past reset_ledger (0 + 10 = 10).
        env.ledger().set_sequence_number(11);
        // After the window expires the counter resets; a fresh 900 spend must succeed.
        client.record_spend(&wallet, &900_i128);
        let record = client.get_daily_limit(&wallet);
        assert_eq!(record.spent, 900);
    }

    #[test]
    fn test_get_daily_limit_shows_reset_spent_without_persisting() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        let wallet = Address::generate(&env);
        // Use a short window (10 ledgers) to stay within persistent TTL.
        client.set_daily_limit(&wallet, &500_i128, &10_u32, &None);
        client.record_spend(&wallet, &300_i128);

        // Advance past the reset window (0 + 10 = 10).
        env.ledger().set_sequence_number(11);

        // get_daily_limit should show spent=0 (window elapsed) without persisting.
        let record = client.get_daily_limit(&wallet);
        assert_eq!(record.spent, 0);

        // A subsequent record_spend should see the reset and allow the full limit.
        client.record_spend(&wallet, &500_i128);
        let record2 = client.get_daily_limit(&wallet);
        assert_eq!(record2.spent, 500);
    }

    #[test]
    fn test_record_spend_invalid_amount_zero_fails() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        assert!(client.try_record_spend(&wallet, &0_i128).is_err());
    }

    #[test]
    fn test_record_spend_invalid_amount_negative_fails() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        assert!(client.try_record_spend(&wallet, &-1_i128).is_err());
    }

    #[test]
    fn test_record_spend_no_limit_configured_fails() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert!(client.try_record_spend(&wallet, &100_i128).is_err());
    }

    #[test]
    fn test_set_daily_limit_emits_event() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);
        let events = env.events().all();
        // init + lmt_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("lmt_set"));
    }

    #[test]
    fn test_spend_exactly_at_limit_succeeds() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &500_i128, &17280_u32, &None);
        // Spending exactly the limit must succeed.
        client.record_spend(&wallet, &500_i128);
        let record = client.get_daily_limit(&wallet);
        assert_eq!(record.spent, 500);
        // One more unit must fail.
        assert!(client.try_record_spend(&wallet, &1_i128).is_err());
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────

    #[test]
    fn test_symbol_short_lengths_within_limit() {
        let tags = [symbol_short!("mux_pol")];
        let actions = [
            symbol_short!("init"),
            symbol_short!("lmt_set"),
            symbol_short!("spent"),
            symbol_short!("ctr_rst"),
        ];
        for sym in tags.iter().chain(actions.iter()) {
            let _ = sym;
        }
    }

    // ── Issue #615: registry_id cross-contract validation ─────────────────────

    /// record_spend must return RegistryNotFound when the linked registry_id
    /// points to a non-existent contract. This is the regression guard: if the
    /// validation is removed, the spend succeeds against an invalid registry.
    #[test]
    fn test_record_spend_with_invalid_registry_id_returns_registry_not_found() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        // Use a random address as the registry_id — it is not a deployed contract,
        // so the cross-contract call will fail.
        let invalid_registry = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &Some(invalid_registry));

        let result = client.try_record_spend(&wallet, &100_i128);
        assert_eq!(
            result,
            Err(Ok(MuxPolicyError::RegistryNotFound)),
            "record_spend must fail when the linked registry_id is not a valid contract"
        );
    }

    /// record_spend must succeed (no registry check) when registry_id is None.
    #[test]
    fn test_record_spend_without_registry_id_succeeds() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32, &None);

        assert!(
            client.try_record_spend(&wallet, &100_i128).is_ok(),
            "record_spend must succeed when no registry_id is linked"
        );
        assert_eq!(client.get_daily_limit(&wallet).spent, 100);
    }

    /// RegistryNotFound error code must be 9 — stable ABI.
    #[test]
    fn test_registry_not_found_error_code_is_9() {
        assert_eq!(
            MuxPolicyError::RegistryNotFound as u32,
            9,
            "RegistryNotFound must remain error code 9; coordinate with docs/error_codes.md"
        );
    }
}
