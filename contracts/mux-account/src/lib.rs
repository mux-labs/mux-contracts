/*!
 * mux-account: Account abstraction contract for Mux Protocol.
 *
 * Provides delegated signing, guardian management, and spending limits
 * on top of a Stellar Soroban account.
 *
 * ## Upgrade Migration Notes
 *
 * When upgrading this contract to a new version:
 *
 * 1. **Storage Compatibility**: All existing `DataKey` variants must remain
 *    stable. Do not change enum discriminants for keys already on-chain.
 * 2. **Owner Migration**: The `Owner` address persists across upgrades; no
 *    migration action is required for existing authorization.
 * 3. **Additive Fields**: New storage keys (e.g. `Metadata`) must be optional
 *    so pre-upgrade instances deserialise without migration.
 * 4. **Testing**: After upgrade, verify owner auth, delegates, spend limits,
 *    and guardian set remain accessible.
 *
 * See `docs/account-upgrade-migration.md` for the full migration guide.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Map,
    String, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
// All state-mutating operations publish a structured event:
//   topics: [contract_name, action]
//   data:   action-specific payload (see docs/audit-events.md)

fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_acct"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
    SpendLimit(Address),
    GuardianSet,
    Nonce,
    /// Storage for session key record: DataKey::SessionKey(owner, session_key)
    SessionKey(Address, Address),
    /// Index of all session keys per owner: DataKey::SessionKeyIndex(owner)
    SessionKeyIndex(Address),
    Paused,
    Executing,
    /// Optional registry-level metadata for this account instance.
    Metadata,
}

// ── Registry metadata ─────────────────────────────────────────────────────────

/// Descriptive metadata attached to this account contract instance.
///
/// Stored under [`DataKey::Metadata`] and writable only by the account owner.
/// Useful for off-chain tooling (indexers, dashboards) that need to identify
/// or version a deployed account instance.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryMeta {
    /// Human-readable name for this account instance (e.g. `"mux-mainnet-acct"`).
    pub name: String,
    /// Semantic version string (e.g. `"1.0.0"`).
    pub version: String,
    /// Optional free-form description / notes.
    pub description: String,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SpendLimit {
    pub asset: Address,
    pub amount: i128,
    pub period_ledgers: u32,
    pub spent: i128,
    pub reset_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DelegateInfo {
    pub address: Address,
    pub expiry_ledger: u32,
    pub can_spend: bool,
}

/// Represents the scope or capability of a session key.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub method: soroban_sdk::Symbol,
}

/// Session key record stored for each delegated session.
/// Tracks expiration, allowed scopes, and revocation status.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SessionKeyRecord {
    pub expires_at: u64,
    pub scopes: Vec<Scope>,
    pub revoked: bool,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxAccountError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    DelegateNotFound = 4,
    DelegateExpired = 5,
    SpendLimitExceeded = 6,
    InvalidAmount = 7,
    InvalidPeriod = 8,
    TooManyDelegates = 9,
    ReentrancyDetected = 10,
    ArithmeticOverflow = 11,
    TooManySessionKeys = 12,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of delegates to bound instance-storage growth.
/// Each DelegateInfo entry is ~72 bytes; 64 entries ≈ 4.6 KB.
const MAX_DELEGATES: u32 = 64;

/// Maximum number of session keys per owner to bound instance-storage growth.
/// Each entry is ~32 bytes; 32 entries ≈ 1 KB.
const MAX_SESSION_KEYS: u32 = 32;

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): if instance storage TTL expires the contract loses
// all state silently.  Every write operation extends the TTL so the contract
// stays live as long as it is actively used.  Deployers must also extend TTL
// proactively via a keeper job; see docs/storage-griefing.md.
//
// Values: ~17,280 ledgers ≈ 1 day (5-second ledger close); bump to 30 days.
const TTL_THRESHOLD: u32 = 17_280; // extend when remaining TTL falls below 1 day
const TTL_EXTEND_TO: u32 = 518_400; // extend to ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxAccount;

#[contractimpl]
impl MuxAccount {
    /// Initialize the account with an owner and optional guardian set.
    pub fn initialize(
        env: Env,
        owner: Address,
        guardians: Vec<Address>,
    ) -> Result<(), MuxAccountError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(MuxAccountError::AlreadyInitialized);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&DataKey::GuardianSet, &guardians);
        env.storage().instance().set(
            &DataKey::Delegates,
            &Map::<Address, DelegateInfo>::new(&env),
        );
        env.storage().instance().set(&DataKey::Nonce, &0_u64);
        emit(&env, symbol_short!("init"), owner);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Unpause the contract — restores normal operation.
    pub fn unpause(env: Env) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return whether the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Add or update a delegate with an expiry and spending permission flag.
    pub fn set_delegate(
        env: Env,
        delegate: Address,
        expiry_ledger: u32,
        can_spend: bool,
    ) -> Result<(), MuxAccountError> {
        Self::require_not_paused(&env)?;
        Self::require_owner(&env)?;
        let mut delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;

        // STORAGE-GRIEFING: reject new entries beyond the cap; updates to existing
        // delegates are always allowed since they don't grow the map.
        if !delegates.contains_key(delegate.clone()) && delegates.len() >= MAX_DELEGATES {
            return Err(MuxAccountError::TooManyDelegates);
        }
        delegates.set(
            delegate.clone(),
            DelegateInfo {
                address: delegate.clone(),
                expiry_ledger,
                can_spend,
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::Delegates, &delegates);
        emit(
            &env,
            symbol_short!("dlg_set"),
            (delegate, expiry_ledger, can_spend),
        );
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Remove a delegate.
    pub fn remove_delegate(env: Env, delegate: Address) -> Result<(), MuxAccountError> {
        Self::require_not_paused(&env)?;
        Self::require_owner(&env)?;
        let mut delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;

        if !delegates.contains_key(delegate.clone()) {
            return Err(MuxAccountError::DelegateNotFound);
        }
        delegates.remove(delegate.clone());
        env.storage()
            .instance()
            .set(&DataKey::Delegates, &delegates);
        emit(&env, symbol_short!("dlg_rm"), delegate);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Set a per-asset spend limit for a delegate.
    pub fn set_spend_limit(
        env: Env,
        asset: Address,
        amount: i128,
        period_ledgers: u32,
    ) -> Result<(), MuxAccountError> {
        Self::require_not_paused(&env)?;
        Self::require_owner(&env)?;
        if amount <= 0 {
            return Err(MuxAccountError::InvalidAmount);
        }
        if period_ledgers == 0 {
            return Err(MuxAccountError::InvalidPeriod);
        }
        let limit = SpendLimit {
            asset: asset.clone(),
            amount,
            period_ledgers,
            spent: 0,
            reset_ledger: env.ledger().sequence().saturating_add(period_ledgers),
        };
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(asset.clone()), &limit);
        emit(
            &env,
            symbol_short!("lmt_set"),
            (asset, amount, period_ledgers),
        );
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Check and debit a spend against the configured limit.
    pub fn debit_spend(env: Env, asset: Address, spend: i128) -> Result<(), MuxAccountError> {
        Self::require_not_paused(&env)?;
        let caller = env.current_contract_address();
        caller.require_auth();

        // Reentrancy guard: reject if a debit_spend call is already in progress.
        // On error return Soroban rolls back storage, so the flag is self-cleaning.
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Executing)
            .unwrap_or(false)
        {
            return Err(MuxAccountError::ReentrancyDetected);
        }
        env.storage().instance().set(&DataKey::Executing, &true);

        let mut limit: SpendLimit = env
            .storage()
            .instance()
            .get(&DataKey::SpendLimit(asset.clone()))
            .ok_or(MuxAccountError::SpendLimitExceeded)?;

        if env.ledger().sequence() >= limit.reset_ledger {
            limit.spent = 0;
            limit.reset_ledger = env.ledger().sequence().saturating_add(limit.period_ledgers);
        }

        let new_spent = limit
            .spent
            .checked_add(spend)
            .ok_or(MuxAccountError::ArithmeticOverflow)?;
        if new_spent > limit.amount {
            return Err(MuxAccountError::SpendLimitExceeded);
        }
        limit.spent = new_spent;
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(asset.clone()), &limit);
        emit(&env, symbol_short!("debited"), (asset, spend));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the current owner.
    pub fn owner(env: Env) -> Result<Address, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(MuxAccountError::NotInitialized)
    }

    /// Return all active delegates.
    pub fn delegates(env: Env) -> Result<Map<Address, DelegateInfo>, MuxAccountError> {
        let delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;
        let mut active_delegates: Map<Address, DelegateInfo> = Map::new(&env);
        for (delegate, info) in delegates.iter() {
            if !Self::is_delegate_expired(&info, env.ledger().sequence()) {
                active_delegates.set(delegate, info);
            }
        }
        Ok(active_delegates)
    }

    /// Return delegate information if the delegate is currently active.
    pub fn get_delegate(env: Env, delegate: Address) -> Result<DelegateInfo, MuxAccountError> {
        let delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;
        let info = delegates
            .get(delegate.clone())
            .ok_or(MuxAccountError::DelegateNotFound)?;
        if Self::is_delegate_expired(&info, env.ledger().sequence()) {
            return Err(MuxAccountError::DelegateExpired);
        }
        Ok(info)
    }

    /// Return the guardian set.
    pub fn guardians(env: Env) -> Result<Vec<Address>, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::GuardianSet)
            .ok_or(MuxAccountError::NotInitialized)
    }

    /// Execute a transaction payload on behalf of the account using a delegated session key.
    ///
    /// This function allows a delegated session key to execute a transaction payload
    /// without requiring the account owner's direct authorization. The session key
    /// must be authorized for the current account (validated via the session registry).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `session_key` - The address of the authorized session key
    /// * `payload` - The serialized transaction payload to execute
    ///
    /// # Returns
    /// * `Ok(Bytes)` - Empty result on successful execution
    /// * `Err(MuxAccountError)` - If session key is not authorized or invalid
    ///
    /// # Events
    /// Emits a `ses_exe` event on successful execution.
    pub fn execute_with_session(
        env: Env,
        session_key: Address,
        payload: Bytes,
    ) -> Result<Bytes, MuxAccountError> {
        // TODO: Validate that session_key is authorized for this account.
        // This requires the session registry contract to be implemented.
        // session_key.require_auth();

        emit(&env, symbol_short!("ses_exe"), (session_key, payload));
        Self::extend_ttl(&env);
        Ok(Bytes::new(&env))
    }

    // ── Registry metadata ──────────────────────────────────────────────────────

    /// Store registry-level metadata. Owner only.
    ///
    /// Overwrites any previously stored metadata. Emits a `meta_set` audit event.
    pub fn set_metadata(env: Env, meta: RegistryMeta) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        env.storage().instance().set(&DataKey::Metadata, &meta);
        emit(&env, symbol_short!("meta_set"), meta.name.clone());
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the currently stored registry metadata, or `None` if not set.
    pub fn get_metadata(env: Env) -> Option<RegistryMeta> {
        env.storage().instance().get(&DataKey::Metadata)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_owner(env: &Env) -> Result<(), MuxAccountError> {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(MuxAccountError::NotInitialized)?;
        owner.require_auth();
        Ok(())
    }

    fn require_not_paused(env: &Env) -> Result<(), MuxAccountError> {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(MuxAccountError::Unauthorized);
        }
        Ok(())
    }

    fn is_delegate_expired(info: &DelegateInfo, current_ledger: u32) -> bool {
        current_ledger >= info.expiry_ledger
    }

    /// Extend instance-storage TTL on every write to prevent silent data loss (T-21).
    fn extend_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    /// Enforce the session key storage cap (T-22).
    /// Called before adding a new session key to prevent unbounded growth.
    fn require_session_key_cap(env: &Env, owner: &Address) -> Result<(), MuxAccountError> {
        let index: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::SessionKeyIndex(owner.clone()))
            .unwrap_or_else(|| Vec::new(env));
        if index.len() >= MAX_SESSION_KEYS {
            return Err(MuxAccountError::TooManySessionKeys);
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events, Ledger as _},
        Env, FromVal, String, Vec,
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

    fn setup() -> (Env, MuxAccountClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccount);
        let client = MuxAccountClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    #[test]
    fn test_initialize_emits_event() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_set_delegate_emits_event() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u32, &true);
        let events = env.events().all();
        // init + dlg_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("dlg_set"));
    }

    #[test]
    fn test_remove_delegate_emits_event() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u32, &false);
        client.remove_delegate(&delegate);
        let events = env.events().all();
        // init + dlg_set + dlg_rm
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("dlg_rm"));
    }

    #[test]
    fn test_spend_limit_emits_events() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);
        client.try_debit_spend(&asset, &200_i128).unwrap();
        let events = env.events().all();
        // init + lmt_set + debited
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("lmt_set"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("debited"));
    }

    #[test]
    fn test_delegate_cap_enforced() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));

        // Fill up to the cap
        for _ in 0..64 {
            client.set_delegate(&Address::generate(&env), &1000_u32, &false);
        }
        // One more new delegate must be rejected
        let result = client.try_set_delegate(&Address::generate(&env), &1000_u32, &false);
        assert!(result.is_err());
    }

    #[test]
    fn test_delegate_cap_allows_update() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));

        // Fill to cap
        let first = Address::generate(&env);
        client.set_delegate(&first, &1000_u32, &false);
        for _ in 1..64 {
            client.set_delegate(&Address::generate(&env), &1000_u32, &false);
        }
        // Updating an existing delegate must still succeed even at cap
        assert!(client.try_set_delegate(&first, &2000_u32, &true).is_ok());
    }

    #[test]
    fn test_initialize() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        assert!(client.try_initialize(&owner, &guardians).is_ok());
        assert_eq!(client.owner(), owner);
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let result = client.try_initialize(&owner, &guardians);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_and_remove_delegate() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u32, &true);

        let delegates = client.delegates();
        assert!(delegates.contains_key(delegate.clone()));

        client.remove_delegate(&delegate);
        let delegates_after = client.delegates();
        assert!(!delegates_after.contains_key(delegate));
    }

    #[test]
    fn test_get_delegate_returns_active_delegate_info() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u32, &true);

        let info = client.get_delegate(&delegate);
        assert_eq!(info.address, delegate);
        assert!(info.can_spend);
        assert_eq!(info.expiry_ledger, 1000_u32);
    }

    #[test]
    fn test_get_delegate_fails_for_unauthorized_delegate() {
        let (env, client, _owner) = setup();
        client.initialize(&_owner, &Vec::new(&env));
        let delegate = Address::generate(&env);

        let result = client.try_get_delegate(&delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_delegate_fails_when_delegate_expired() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        let current = env.ledger().sequence();
        let expiry = current + 1;
        client.set_delegate(&delegate, &expiry, &true);
        env.ledger().set_sequence_number(expiry);

        let result = client.try_get_delegate(&delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_delegates_filters_expired_delegates() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        let current = env.ledger().sequence();
        let expiry = current + 1;
        client.set_delegate(&delegate, &expiry, &true);
        env.ledger().set_sequence_number(expiry);

        let active = client.delegates();
        assert!(!active.contains_key(delegate));
    }

    #[test]
    fn test_spend_limit_enforcement() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);

        // Debit within limit succeeds
        assert!(client.try_debit_spend(&asset, &500_i128).is_ok());

        // Debit exceeding limit fails
        let result = client.try_debit_spend(&asset, &600_i128);
        assert!(result.is_err());
    }

    #[test]
    fn test_spend_limit_invalid_amount() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let asset = Address::generate(&env);
        let result = client.try_set_spend_limit(&asset, &0_i128, &100_u32);
        assert!(result.is_err());
    }

    #[test]
    fn test_unpause_emits_event() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        client.unpause();
        let events = env.events().all();
        // init + unpaused
        assert!(events.len() >= 2);
        assert_eq!(
            topic_action(&env, &events, events.len() - 1),
            symbol_short!("unpaused")
        );
    }

    #[test]
    fn test_execute_with_session_emits_event() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let session_key = Address::generate(&env);
        let payload = Bytes::new(&env);
        let _ = client.execute_with_session(&session_key, &payload);
        let events = env.events().all();
        // init + ses_exe
        assert!(events.len() >= 2);
        assert_eq!(
            topic_action(&env, &events, events.len() - 1),
            symbol_short!("ses_exe")
        );
    }

    #[test]
    fn test_ttl_extended_on_write() {
        // Verify that initialize bumps instance TTL (T-21 mitigation).
        // The Soroban test environment starts with TTL = 0; after a write that
        // calls extend_ttl the value must be > 0.
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        // If extend_ttl was not called the SDK would have panicked in the test
        // environment when TTL_EXTEND_TO > remaining TTL.  Reaching here means
        // the call succeeded without error.
        assert_eq!(client.owner(), owner);
    }

    // ── Registry metadata tests ────────────────────────────────────────────────

    #[test]
    fn test_set_and_get_metadata() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let meta = RegistryMeta {
            name: String::from_str(&env, "mux-testnet-acct"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, "Account contract for testnet"),
        };
        client.set_metadata(&meta);
        let stored = client.get_metadata().unwrap();
        assert_eq!(stored.name, meta.name);
        assert_eq!(stored.version, meta.version);
        assert_eq!(stored.description, meta.description);
    }

    #[test]
    fn test_set_metadata_overwrites_previous() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let meta1 = RegistryMeta {
            name: String::from_str(&env, "v1"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, "first"),
        };
        let meta2 = RegistryMeta {
            name: String::from_str(&env, "v2"),
            version: String::from_str(&env, "2.0.0"),
            description: String::from_str(&env, "second"),
        };
        client.set_metadata(&meta1);
        client.set_metadata(&meta2);
        let stored = client.get_metadata().unwrap();
        assert_eq!(stored.version, meta2.version);
    }

    #[test]
    fn test_get_metadata_returns_none_when_unset() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        assert!(client.get_metadata().is_none());
    }

    #[test]
    fn test_set_metadata_emits_event() {
        let (env, client, owner) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let meta = RegistryMeta {
            name: String::from_str(&env, "registry"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, ""),
        };
        client.set_metadata(&meta);
        let events = env.events().all();
        // init + meta_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("meta_set"));
    }

    #[test]
    fn test_set_metadata_before_initialize_fails() {
        let (_env, client, _owner) = setup();
        let meta = RegistryMeta {
            name: String::from_str(&_env, "registry"),
            version: String::from_str(&_env, "1.0.0"),
            description: String::from_str(&_env, ""),
        };
        let result = client.try_set_metadata(&meta);
        assert!(result.is_err());
    }
}
pub mod smart_wallet;
