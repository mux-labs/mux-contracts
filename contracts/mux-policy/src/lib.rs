/*!
 * mux-policy: Spend-limit policy contract for Mux Protocol.
 *
 * Allows an admin to configure per-asset spend limits that other
 * contracts can query before authorising a transfer.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: soroban_sdk::Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_pol"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Limit(Address), // keyed by asset address
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyLimit {
    pub asset: Address,
    pub amount: i128,
    pub period_ledgers: u32,
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
    InvalidAmount = 5,
    InvalidPeriod = 6,
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280;
const TTL_EXTEND_TO: u32 = 518_400;

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

    /// Set or update the spend limit for an asset. Admin only.
    pub fn set_limit(
        env: Env,
        asset: Address,
        amount: i128,
        period_ledgers: u32,
    ) -> Result<(), MuxPolicyError> {
        Self::require_admin(&env)?;
        if amount <= 0 {
            return Err(MuxPolicyError::InvalidAmount);
        }
        if period_ledgers == 0 {
            return Err(MuxPolicyError::InvalidPeriod);
        }
        let limit = PolicyLimit {
            asset: asset.clone(),
            amount,
            period_ledgers,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Limit(asset.clone()), &limit);
        emit(&env, symbol_short!("lmt_set"), (asset, amount, period_ledgers));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Retrieve the policy limit for an asset.
    pub fn get_limit(env: Env, asset: Address) -> Result<PolicyLimit, MuxPolicyError> {
        env.storage()
            .persistent()
            .get(&DataKey::Limit(asset))
            .ok_or(MuxPolicyError::LimitNotFound)
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
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxPolicy);
        let client = MuxPolicyClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_set_limit_and_get() {
        let (env, client, _) = setup();
        let asset = Address::generate(&env);
        client.set_limit(&asset, &1000_i128, &100_u32);
        let limit = client.get_limit(&asset);
        assert_eq!(limit.amount, 1000);
        assert_eq!(limit.period_ledgers, 100);
        assert_eq!(limit.asset, asset);
    }

    #[test]
    fn test_set_limit_emits_event() {
        let (env, client, _) = setup();
        let asset = Address::generate(&env);
        client.set_limit(&asset, &500_i128, &50_u32);
        let events = env.events().all();
        // init + lmt_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("lmt_set"));
    }

    #[test]
    fn test_set_limit_invalid_amount() {
        let (env, client, _) = setup();
        let asset = Address::generate(&env);
        assert!(client.try_set_limit(&asset, &0_i128, &100_u32).is_err());
        assert!(client.try_set_limit(&asset, &-1_i128, &100_u32).is_err());
    }

    #[test]
    fn test_set_limit_invalid_period() {
        let (env, client, _) = setup();
        let asset = Address::generate(&env);
        assert!(client.try_set_limit(&asset, &100_i128, &0_u32).is_err());
    }

    #[test]
    fn test_get_limit_not_found() {
        let (env, client, _) = setup();
        let asset = Address::generate(&env);
        assert!(client.try_get_limit(&asset).is_err());
    }

    #[test]
    fn test_set_limit_update() {
        let (env, client, _) = setup();
        let asset = Address::generate(&env);
        client.set_limit(&asset, &1000_i128, &100_u32);
        client.set_limit(&asset, &2000_i128, &200_u32);
        let limit = client.get_limit(&asset);
        assert_eq!(limit.amount, 2000);
        assert_eq!(limit.period_ledgers, 200);
    }
}
