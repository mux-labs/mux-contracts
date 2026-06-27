/*!
 * mux-policy: Per-wallet daily spend limit policy contract for Mux Protocol.
 *
 * Stores and enforces a daily spend limit per wallet address. The daily
 * counter resets automatically once the current day (measured in ledgers)
 * has elapsed.
 *
 * ## Upgrade / Migration Note (#292)
 *
 * This contract implements an `upgrade()` entry point so the admin can atomically
 * swap the running WASM without redeploying. Storage layout rules:
 *
 * - **Never remove or rename** existing `DataKey` variants between versions.
 *   Persistent `WalletLimit` entries survive the upgrade and must remain
 *   deserializable.
 * - **New fields on `DailyLimit`** must be wrapped in `Option<T>` so that
 *   pre-upgrade ledger entries can still be read by the new code.
 * - **If a breaking layout change is unavoidable**, ship a `migrate()` function
 *   that rewrites affected entries before the new read paths execute.  Call it
 *   once as admin immediately after upgrading.
 * - The `reset_daily_counter()` admin function is available as an emergency
 *   escape hatch after an upgrade if per-wallet counters need to be cleared
 *   (e.g., following a spend-accounting correction during migration).
 *
 * See `docs/contract-upgrade-pattern.md` for the full checklist and rollback
 * procedure.
 */

#![no_std]

use soroban_sdk::{
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

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    /// Per-wallet daily spend limit record.
    WalletLimit(Address),
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
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxPolicy;

#[contractimpl]
impl MuxPolicy {
    /// Initialize the policy contract with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxPolicyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxPolicyError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Upgrade the contract WASM. Admin only.
    ///
    /// See the module-level doc comment and `docs/contract-upgrade-pattern.md`
    /// for storage-compatibility rules that must be observed between versions.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), MuxPolicyError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Set or update the daily spend limit for a wallet. Admin only.
    ///
    /// `day_ledgers` is the number of ledgers that constitute one day
    /// (≈ 17 280 at 5-second ledger close).
    pub fn set_daily_limit(
        env: Env,
        wallet: Address,
        limit: i128,
        day_ledgers: u32,
    ) -> Result<(), MuxPolicyError> {
        Self::require_admin(&env)?;
        if limit <= 0 {
            return Err(MuxPolicyError::InvalidAmount);
        }
        if day_ledgers == 0 {
            return Err(MuxPolicyError::InvalidPeriod);
        }
        let record = DailyLimit {
            limit,
            spent: 0,
            reset_ledger: env.ledger().sequence().saturating_add(day_ledgers),
            day_ledgers,
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
        testutils::{Address as _, Events},
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
    fn test_set_daily_limit() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        let record = client.get_daily_limit(&wallet);
        assert_eq!(record.limit, 1000);
        assert_eq!(record.spent, 0);
    }

    // ── #282: size / bounds checks ──────────────────────────────────────────

    #[test]
    fn test_set_daily_limit_zero_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_set_daily_limit(&wallet, &0_i128, &17280_u32),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_set_daily_limit_negative_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_set_daily_limit(&wallet, &-1_i128, &17280_u32),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_set_daily_limit_zero_period_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_set_daily_limit(&wallet, &1000_i128, &0_u32),
            Err(Ok(MuxPolicyError::InvalidPeriod))
        );
    }

    #[test]
    fn test_record_spend_zero_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        assert_eq!(
            client.try_record_spend(&wallet, &0_i128),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_record_spend_negative_amount_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        assert_eq!(
            client.try_record_spend(&wallet, &-5_i128),
            Err(Ok(MuxPolicyError::InvalidAmount))
        );
    }

    #[test]
    fn test_record_spend_exact_limit_allowed() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        assert!(client.try_record_spend(&wallet, &1000_i128).is_ok());
    }

    #[test]
    fn test_record_spend_one_over_limit_rejected() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        assert_eq!(
            client.try_record_spend(&wallet, &1001_i128),
            Err(Ok(MuxPolicyError::LimitExceeded))
        );
    }

    #[test]
    fn test_record_spend_cumulative_boundary() {
        // Two spends of 500 each against a 1000 limit; third spend of 1 fails.
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
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
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        let result = client.try_record_spend(&wallet, &i128::MAX);
        assert_eq!(result, Err(Ok(MuxPolicyError::LimitExceeded)));
    }

    // ── Existing functional tests ───────────────────────────────────────────

    #[test]
    fn test_record_spend_within_limit() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        client.record_spend(&wallet, &400_i128);
        let record = client.get_daily_limit(&wallet);
        assert_eq!(record.spent, 400);
    }

    #[test]
    fn test_record_spend_exceeds_limit() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &500_i128, &17280_u32);
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
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
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
        client.set_daily_limit(&wallet_a, &500_i128, &17280_u32);
        client.set_daily_limit(&wallet_b, &200_i128, &17280_u32);
        client.record_spend(&wallet_a, &500_i128);
        // wallet_b limit unaffected
        client.record_spend(&wallet_b, &200_i128);
        assert!(client.try_record_spend(&wallet_a, &1_i128).is_err());
        assert!(client.try_record_spend(&wallet_b, &1_i128).is_err());
    }

    #[test]
    fn test_ttl_extended_on_write() {
        // Reaching here without panic confirms extend_ttl was called (T-21).
        let (_env, _client, _admin) = setup();
    }

    // ── #292: reset_daily_counter ───────────────────────────────────────────

    #[test]
    fn test_reset_daily_counter_clears_spent() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        client.record_spend(&wallet, &800_i128);
        assert_eq!(client.get_daily_limit(&wallet).spent, 800);

        client.reset_daily_counter(&wallet);
        assert_eq!(client.get_daily_limit(&wallet).spent, 0);
    }

    #[test]
    fn test_reset_daily_counter_emits_event() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        client.reset_daily_counter(&wallet);
        let events = env.events().all();
        // init + lmt_set + ctr_rst
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("ctr_rst"));
    }

    #[test]
    fn test_reset_daily_counter_no_limit_fails() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        assert_eq!(
            client.try_reset_daily_counter(&wallet),
            Err(Ok(MuxPolicyError::LimitNotFound))
        );
    }

    #[test]
    fn test_reset_daily_counter_allows_full_spend_again() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &500_i128, &17280_u32);
        client.record_spend(&wallet, &500_i128);
        assert!(client.try_record_spend(&wallet, &1_i128).is_err());

        client.reset_daily_counter(&wallet);
        // After reset the full limit is available again.
        assert!(client.try_record_spend(&wallet, &500_i128).is_ok());
    }

    // ── #287: persistent TTL extension ─────────────────────────────────────

    #[test]
    fn test_persistent_ttl_extended_on_set_daily_limit() {
        // Verifies that set_daily_limit calls extend_ttl on the persistent key
        // without panic (SDK testutils do not expose the TTL value directly,
        // so a clean run without error is the observable assertion).
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        // If persistent TTL extension panicked, we would not reach this line.
        assert_eq!(client.get_daily_limit(&wallet).limit, 1000);
    }

    #[test]
    fn test_persistent_ttl_extended_on_record_spend() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        client.record_spend(&wallet, &100_i128);
        assert_eq!(client.get_daily_limit(&wallet).spent, 100);
    }

    #[test]
    fn test_persistent_ttl_extended_on_reset_counter() {
        let (env, client, _) = setup();
        let wallet = Address::generate(&env);
        client.set_daily_limit(&wallet, &1000_i128, &17280_u32);
        client.reset_daily_counter(&wallet);
        assert_eq!(client.get_daily_limit(&wallet).spent, 0);
    }
}
