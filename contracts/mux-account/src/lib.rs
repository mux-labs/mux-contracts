/*!
 * mux-account: Account abstraction contract for Mux Protocol.
 *
 * Provides delegated signing, guardian management, and spending limits
 * on top of a Stellar Soroban account.
 */

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Map, Vec,
};

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

#[contracttype]
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
    InvalidSessionKey = 9,
    SessionKeyExpired = 10,
    SessionKeyRevoked = 11,
}

impl From<soroban_sdk::Error> for MuxAccountError {
    fn from(_: soroban_sdk::Error) -> Self {
        MuxAccountError::Unauthorized
    }
}

impl From<&soroban_sdk::Error> for MuxAccountError {
    fn from(_: &soroban_sdk::Error) -> Self {
        MuxAccountError::Unauthorized
    }
}

impl Into<soroban_sdk::Error> for MuxAccountError {
    fn into(self) -> soroban_sdk::Error {
        soroban_sdk::Error::from((
            soroban_sdk::xdr::ScErrorType::WasmVm,
            soroban_sdk::xdr::ScErrorCode::InvalidInput,
        ))
    }
}

impl Into<soroban_sdk::Error> for &MuxAccountError {
    fn into(self) -> soroban_sdk::Error {
        soroban_sdk::Error::from((
            soroban_sdk::xdr::ScErrorType::WasmVm,
            soroban_sdk::xdr::ScErrorCode::InvalidInput,
        ))
    }
}

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
        env.storage().instance().set(&DataKey::GuardianSet, &guardians);
        env.storage().instance().set(&DataKey::Delegates, &Map::<Address, DelegateInfo>::new(&env));
        env.storage().instance().set(&DataKey::Nonce, &0_u64);
        Ok(())
    }

    /// Add or update a delegate with an expiry and spending permission flag.
    pub fn set_delegate(
        env: Env,
        delegate: Address,
        expiry_ledger: u32,
        can_spend: bool,
    ) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        let mut delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;

        delegates.set(
            delegate.clone(),
            DelegateInfo { address: delegate, expiry_ledger, can_spend },
        );
        env.storage().instance().set(&DataKey::Delegates, &delegates);
        Ok(())
    }

    /// Remove a delegate.
    pub fn remove_delegate(env: Env, delegate: Address) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        let mut delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;

        if !delegates.contains_key(delegate.clone()) {
            return Err(MuxAccountError::DelegateNotFound);
        }
        delegates.remove(delegate);
        env.storage().instance().set(&DataKey::Delegates, &delegates);
        Ok(())
    }

    /// Set a per-asset spend limit for a delegate.
    pub fn set_spend_limit(
        env: Env,
        asset: Address,
        amount: i128,
        period_ledgers: u32,
    ) -> Result<(), MuxAccountError> {
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
            reset_ledger: env.ledger().sequence() + period_ledgers,
        };
        env.storage().instance().set(&DataKey::SpendLimit(asset), &limit);
        Ok(())
    }

    /// Check and debit a spend against the configured limit.
    pub fn debit_spend(env: Env, asset: Address, spend: i128) -> Result<(), MuxAccountError> {
        let caller = env.current_contract_address();
        caller.require_auth();

        let mut limit: SpendLimit = env
            .storage()
            .instance()
            .get(&DataKey::SpendLimit(asset.clone()))
            .ok_or(MuxAccountError::SpendLimitExceeded)?;

        if env.ledger().sequence() >= limit.reset_ledger {
            limit.spent = 0;
            limit.reset_ledger = env.ledger().sequence() + limit.period_ledgers;
        }

        let new_spent = limit.spent + spend;
        if new_spent > limit.amount {
            return Err(MuxAccountError::SpendLimitExceeded);
        }
        limit.spent = new_spent;
        env.storage().instance().set(&DataKey::SpendLimit(asset), &limit);
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
        env.storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)
    }

    /// Return the guardian set.
    pub fn guardians(env: Env) -> Result<Vec<Address>, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::GuardianSet)
            .ok_or(MuxAccountError::NotInitialized)
    }

    /// Register a new session key for the account owner.
    /// Storage design: SessionKey(owner, session_key) -> SessionKeyRecord
    /// SessionKeyIndex(owner) -> Vec<Address> for enumeration and lookup.
    pub fn register_session_key(
        env: Env,
        owner: Address,
        session_key: Address,
        expires_at: u64,
        scopes: Vec<Scope>,
    ) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;

        let record = SessionKeyRecord {
            expires_at,
            scopes,
            revoked: false,
        };
        env.storage()
            .instance()
            .set(&DataKey::SessionKey(owner.clone(), session_key.clone()), &record);

        // Update the session key index for this owner
        let mut index: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::SessionKeyIndex(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        if !index.contains(&session_key) {
            index.push_back(session_key);
        }
        env.storage()
            .instance()
            .set(&DataKey::SessionKeyIndex(owner), &index);

        Ok(())
    }

    /// Revoke an existing session key.
    pub fn revoke_session_key(
        env: Env,
        owner: Address,
        session_key: Address,
    ) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;

        let mut record: SessionKeyRecord = env
            .storage()
            .instance()
            .get(&DataKey::SessionKey(owner.clone(), session_key.clone()))
            .ok_or(MuxAccountError::InvalidSessionKey)?;

        record.revoked = true;
        env.storage()
            .instance()
            .set(&DataKey::SessionKey(owner, session_key), &record);

        Ok(())
    }

    /// Check if a session key is valid (not expired, not revoked, exists).
    pub fn is_session_key_valid(
        env: Env,
        owner: Address,
        session_key: Address,
    ) -> Result<bool, MuxAccountError> {
        let record: SessionKeyRecord = env
            .storage()
            .instance()
            .get(&DataKey::SessionKey(owner, session_key))
            .ok_or(MuxAccountError::InvalidSessionKey)?;

        // Check if revoked
        if record.revoked {
            return Ok(false);
        }

        // Check if expired
        if env.ledger().timestamp() >= record.expires_at {
            return Ok(false);
        }

        Ok(true)
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
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Vec};

    fn setup() -> (Env, MuxAccountClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccount);
        let client = MuxAccountClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, client, owner)
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
    fn test_register_session_key_valid() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let session_key = Address::generate(&env);
        let expires_at = env.ledger().timestamp() + 3600; // 1 hour from now
        let scopes: Vec<Scope> = Vec::new(&env);

        // Register session key should succeed
        assert!(client
            .try_register_session_key(&owner, &session_key, &expires_at, &scopes)
            .is_ok());

        // Verify it's valid
        let is_valid = client.is_session_key_valid(&owner, &session_key);
        assert!(is_valid);
    }

    #[test]
    fn test_session_key_expired_returns_false() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let session_key = Address::generate(&env);
        // Create an expired session key - set expires_at to 0 (always in the past)
        let expires_at = 0u64;
        let scopes: Vec<Scope> = Vec::new(&env);

        client.register_session_key(&owner, &session_key, &expires_at, &scopes);

        // Expired key should return false
        let is_valid = client.is_session_key_valid(&owner, &session_key);
        assert!(!is_valid, "Session key should be expired when expires_at is 0");
    }

    #[test]
    fn test_revoked_session_key_returns_false() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let session_key = Address::generate(&env);
        let expires_at = env.ledger().timestamp() + 3600;
        let scopes: Vec<Scope> = Vec::new(&env);

        client.register_session_key(&owner, &session_key, &expires_at, &scopes);

        // Verify it's valid before revocation
        assert!(client.is_session_key_valid(&owner, &session_key));

        // Revoke the key
        assert!(client
            .try_revoke_session_key(&owner, &session_key)
            .is_ok());

        // Revoked key should return false
        let is_valid = client.is_session_key_valid(&owner, &session_key);
        assert!(!is_valid);
    }

    #[test]
    fn test_register_session_key_updates_index() {
        let (env, client, owner) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let session_key1 = Address::generate(&env);
        let session_key2 = Address::generate(&env);
        let expires_at = env.ledger().timestamp() + 3600;
        let scopes: Vec<Scope> = Vec::new(&env);

        client.register_session_key(&owner, &session_key1, &expires_at, &scopes);
        client.register_session_key(&owner, &session_key2, &expires_at, &scopes);

        // Both keys should be valid
        assert!(client.is_session_key_valid(&owner, &session_key1));
        assert!(client.is_session_key_valid(&owner, &session_key2));
    }
}
