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
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env,
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
    }
}
