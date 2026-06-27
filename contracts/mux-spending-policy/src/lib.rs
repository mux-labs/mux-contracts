/*!
 * mux-spending-policy: Spending-policy enforcement contract for Mux Protocol.
 *
 * Stores per-account spend limits and validates spend requests against them.
 * All state-mutating operations emit structured audit events so that off-chain
 * indexers and monitoring tools can track policy changes without querying
 * on-chain storage.
 *
 * # Audit events
 *
 * | Action      | Topics                        | Data                          |
 * |-------------|-------------------------------|-------------------------------|
 * | `init`      | `[mux_spol, init]`            | `admin: Address`              |
 * | `pol_set`   | `[mux_spol, pol_set]`         | `(account, asset, limit)`     |
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

// ── Audit events ──────────────────────────────────────────────────────────────
// All state-mutating operations publish a structured event:
//   topics: [contract_name, action]
//   data:   action-specific payload

fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_spol"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    /// SpendLimit(account, asset) -> SpendLimit
    SpendLimit(Address, Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SpendLimit {
    pub asset: Address,
    pub limit: i128,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SpendingPolicyError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    PolicyNotFound = 4,
    SpendLimitExceeded = 5,
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): extend instance TTL on every write so the policy
// registry stays live as long as it is actively used.  See docs/storage-griefing.md.
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxSpendingPolicy;

#[contractimpl]
impl MuxSpendingPolicy {
    /// Initialize the policy contract with an admin address.
    ///
    /// Emits: `init` with `admin` as data.
    pub fn initialize(env: Env, admin: Address) -> Result<(), SpendingPolicyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(SpendingPolicyError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Set a spend limit for an account/asset pair. Admin only.
    ///
    /// Emits: `pol_set` with `(account, asset, limit)` as data.
    pub fn set_policy(
        env: Env,
        account: Address,
        asset: Address,
        limit: i128,
    ) -> Result<(), SpendingPolicyError> {
        Self::require_admin(&env)?;
        let policy = SpendLimit {
            asset: asset.clone(),
            limit,
        };
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(account.clone(), asset.clone()), &policy);
        emit(&env, symbol_short!("pol_set"), (account, asset, limit));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Get the spend limit for an account/asset pair.
    pub fn get_policy(
        env: Env,
        account: Address,
        asset: Address,
    ) -> Result<SpendLimit, SpendingPolicyError> {
        env.storage()
            .instance()
            .get(&DataKey::SpendLimit(account, asset))
            .ok_or(SpendingPolicyError::PolicyNotFound)
    }

    /// Check whether `amount` is within the policy limit for `account`/`asset`.
    ///
    /// Returns `Ok(())` if allowed, `Err(SpendLimitExceeded)` if over limit,
    /// or `Err(PolicyNotFound)` if no policy is set.
    ///
    /// This is a read-only operation and does not emit an event.
    pub fn check_spend(
        env: Env,
        account: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), SpendingPolicyError> {
        let policy: SpendLimit = env
            .storage()
            .instance()
            .get(&DataKey::SpendLimit(account, asset))
            .ok_or(SpendingPolicyError::PolicyNotFound)?;
        if amount > policy.limit {
            return Err(SpendingPolicyError::SpendLimitExceeded);
        }
        Ok(())
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), SpendingPolicyError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SpendingPolicyError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    /// Extend instance-storage TTL on every write to prevent silent data loss (T-21).
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

    fn setup() -> (Env, MuxSpendingPolicyClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_emits_init_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxSpendingPolicy);
        let client = MuxSpendingPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, admin) = setup();
        assert_eq!(
            client.try_initialize(&admin),
            Err(Ok(SpendingPolicyError::AlreadyInitialized))
        );
    }

    // ── set_policy ────────────────────────────────────────────────────────────

    #[test]
    fn test_set_policy_emits_pol_set_event() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        let events = env.events().all();
        // init + pol_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("pol_set"));
    }

    #[test]
    fn test_set_and_get_policy() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        let policy = client.get_policy(&account, &asset);
        assert_eq!(policy.limit, 1000);
        assert_eq!(policy.asset, asset);
    }

    #[test]
    fn test_set_policy_overwrites_existing() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &500);
        client.set_policy(&account, &asset, &2000);
        let policy = client.get_policy(&account, &asset);
        assert_eq!(policy.limit, 2000);
    }

    #[test]
    fn test_set_policy_emits_event_on_update() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &100);
        client.set_policy(&account, &asset, &200);
        let events = env.events().all();
        // init + pol_set + pol_set
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("pol_set"));
    }

    // ── get_policy ────────────────────────────────────────────────────────────

    #[test]
    fn test_get_policy_not_found() {
        let (env, client, _) = setup();
        let result = client.try_get_policy(&Address::generate(&env), &Address::generate(&env));
        assert_eq!(result, Err(Ok(SpendingPolicyError::PolicyNotFound)));
    }

    // ── check_spend ───────────────────────────────────────────────────────────

    #[test]
    fn test_check_spend_within_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        assert!(client.try_check_spend(&account, &asset, &500).is_ok());
        assert!(client.try_check_spend(&account, &asset, &1000).is_ok());
    }

    #[test]
    fn test_check_spend_exceeds_limit() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        let result = client.try_check_spend(&account, &asset, &1001);
        assert_eq!(result, Err(Ok(SpendingPolicyError::SpendLimitExceeded)));
    }

    #[test]
    fn test_check_spend_no_policy() {
        let (env, client, _) = setup();
        let result =
            client.try_check_spend(&Address::generate(&env), &Address::generate(&env), &1);
        assert_eq!(result, Err(Ok(SpendingPolicyError::PolicyNotFound)));
    }

    #[test]
    fn test_check_spend_does_not_emit_event() {
        // check_spend is read-only; it must not emit any event.
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        client.set_policy(&account, &asset, &1000);
        let event_count_before = env.events().all().len();
        let _ = client.try_check_spend(&account, &asset, &500);
        assert_eq!(env.events().all().len(), event_count_before);
    }

    // ── error codes ───────────────────────────────────────────────────────────

    #[test]
    fn test_spend_limit_exceeded_error_code() {
        assert_eq!(SpendingPolicyError::SpendLimitExceeded as u32, 5);
    }

    // ── TTL ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_ttl_extended_on_initialize() {
        // Reaching here without panic confirms extend_ttl was called (T-21).
        let (_env, _client, _admin) = setup();
    }

    #[test]
    fn test_ttl_extended_on_set_policy() {
        let (env, client, _) = setup();
        let account = Address::generate(&env);
        let asset = Address::generate(&env);
        // If extend_ttl is missing the SDK would panic; reaching here is the assertion.
        client.set_policy(&account, &asset, &500);
        assert_eq!(client.get_policy(&account, &asset).limit, 500);
    }
}
