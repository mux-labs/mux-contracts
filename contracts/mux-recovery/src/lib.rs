/*!
 * mux-recovery: Account recovery system for Mux Protocol.
 *
 * Implements a guardian-initiated recovery mechanism with a mandatory
 * timelock (~24 hours at 5-second ledger close) before the new owner
 * can take control. The current owner may cancel a pending recovery at
 * any time during the timelock window.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: soroban_sdk::Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_recv"), action), data);
}

// ── Timelock ──────────────────────────────────────────────────────────────────

/// Minimum number of ledgers that must pass between `initiate_recovery` and
/// `execute_recovery`.
///
/// At ~5-second ledger close times:
///   17_280 ledgers ≈ 24 hours
///
/// This gives the legitimate owner a window to cancel a fraudulent recovery
/// before it can be executed.
pub const RECOVERY_TIMELOCK: u32 = 17_280;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Lifecycle state of a recovery request.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryStatus {
    /// No active recovery request.
    None,
    /// A recovery has been initiated but the timelock has not expired.
    Pending,
    /// The recovery was executed and ownership transferred.
    Executed,
    /// The recovery was cancelled by the current owner.
    Cancelled,
}

/// An active recovery request stored on-chain.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryRequest {
    /// The proposed new owner address.
    pub new_owner: Address,
    /// The ledger sequence at which the request was initiated.
    pub initiated_at: u32,
    /// The earliest ledger at which `execute_recovery` may be called
    /// (`initiated_at + RECOVERY_TIMELOCK`).
    pub executable_at: u32,
    /// Current lifecycle state.
    pub status: RecoveryStatus,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Owner,
    Guardians,
    Recovery,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RecoveryError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    RecoveryAlreadyPending = 4,
    NoActiveRecovery = 5,
    TimelockNotExpired = 6,
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRecovery;

#[contractimpl]
impl MuxRecovery {
    /// Initialize the recovery contract with an owner and a guardian set.
    pub fn initialize(
        env: Env,
        owner: Address,
        guardians: Vec<Address>,
    ) -> Result<(), RecoveryError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(RecoveryError::AlreadyInitialized);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage().instance().set(&DataKey::Guardians, &guardians);
        emit(&env, symbol_short!("init"), owner);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Initiate a recovery request. Must be called by a registered guardian.
    ///
    /// Only one pending recovery may exist at a time. The timelock starts
    /// at the current ledger sequence.
    pub fn initiate_recovery(
        env: Env,
        guardian: Address,
        new_owner: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        Self::require_guardian(&env, &guardian)?;

        // Reject if a pending recovery already exists.
        if let Some(req) = Self::active_recovery(&env) {
            if req.status == RecoveryStatus::Pending {
                return Err(RecoveryError::RecoveryAlreadyPending);
            }
        }

        let initiated_at = env.ledger().sequence();
        let request = RecoveryRequest {
            new_owner: new_owner.clone(),
            initiated_at,
            executable_at: initiated_at.saturating_add(RECOVERY_TIMELOCK),
            status: RecoveryStatus::Pending,
        };
        env.storage().instance().set(&DataKey::Recovery, &request);
        emit(&env, symbol_short!("rec_init"), (guardian, new_owner));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Cancel a pending recovery. May be called by the current owner at any
    /// time before the recovery is executed.
    pub fn cancel_recovery(env: Env) -> Result<(), RecoveryError> {
        Self::require_owner(&env)?;
        let mut request = Self::require_pending(&env)?;
        request.status = RecoveryStatus::Cancelled;
        env.storage().instance().set(&DataKey::Recovery, &request);
        emit(&env, symbol_short!("rec_cncl"), ());
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Execute a recovery after the timelock has expired.
    ///
    /// Must be called by a registered guardian. Transfers ownership to
    /// `RecoveryRequest.new_owner`.
    pub fn execute_recovery(env: Env, guardian: Address) -> Result<(), RecoveryError> {
        guardian.require_auth();
        Self::require_guardian(&env, &guardian)?;
        let mut request = Self::require_pending(&env)?;

        if env.ledger().sequence() < request.executable_at {
            return Err(RecoveryError::TimelockNotExpired);
        }

        let new_owner = request.new_owner.clone();
        request.status = RecoveryStatus::Executed;
        env.storage().instance().set(&DataKey::Owner, &new_owner);
        env.storage().instance().set(&DataKey::Recovery, &request);
        emit(&env, symbol_short!("rec_exec"), (guardian, new_owner));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the current owner address.
    pub fn owner(env: Env) -> Result<Address, RecoveryError> {
        env.storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(RecoveryError::NotInitialized)
    }

    /// Return the registered guardian set.
    pub fn guardians(env: Env) -> Result<Vec<Address>, RecoveryError> {
        env.storage()
            .instance()
            .get(&DataKey::Guardians)
            .ok_or(RecoveryError::NotInitialized)
    }

    /// Return the current recovery status.
    pub fn recovery_status(env: Env) -> RecoveryStatus {
        env.storage()
            .instance()
            .get::<DataKey, RecoveryRequest>(&DataKey::Recovery)
            .map(|r| r.status)
            .unwrap_or(RecoveryStatus::None)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_owner(env: &Env) -> Result<(), RecoveryError> {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(RecoveryError::NotInitialized)?;
        owner.require_auth();
        Ok(())
    }

    fn require_guardian(env: &Env, guardian: &Address) -> Result<(), RecoveryError> {
        let guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .ok_or(RecoveryError::NotInitialized)?;
        if !guardians.contains(guardian) {
            return Err(RecoveryError::Unauthorized);
        }
        Ok(())
    }

    fn active_recovery(env: &Env) -> Option<RecoveryRequest> {
        env.storage().instance().get(&DataKey::Recovery)
    }

    fn require_pending(env: &Env) -> Result<RecoveryRequest, RecoveryError> {
        let req = Self::active_recovery(env).ok_or(RecoveryError::NoActiveRecovery)?;
        if req.status != RecoveryStatus::Pending {
            return Err(RecoveryError::NoActiveRecovery);
        }
        Ok(req)
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
        testutils::{Address as _, Events, Ledger},
        vec, Env, FromVal,
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

    fn setup() -> (Env, MuxRecoveryClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let guardian = Address::generate(&env);
        client.initialize(&owner, &vec![&env, guardian.clone()]);
        (env, client, owner, guardian)
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_sets_owner_and_guardians() {
        let (_env, client, owner, guardian) = setup();
        assert_eq!(client.owner(), owner);
        assert!(client.guardians().contains(&guardian));
    }

    #[test]
    fn test_initialize_emits_init_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        client.initialize(&owner, &vec![&env]);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_rejected() {
        let (env, client, owner, _) = setup();
        let err = client
            .try_initialize(&owner, &vec![&env])
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::AlreadyInitialized);
    }

    // ── recovery_status default ───────────────────────────────────────────────

    #[test]
    fn test_recovery_status_none_by_default() {
        let (_env, client, _, _) = setup();
        assert_eq!(client.recovery_status(), RecoveryStatus::None);
    }

    // ── initiate_recovery ─────────────────────────────────────────────────────

    #[test]
    fn test_initiate_recovery_sets_pending() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    }

    #[test]
    fn test_initiate_recovery_emits_event() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        let events = env.events().all();
        // init + rec_init
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("rec_init"));
    }

    #[test]
    fn test_initiate_recovery_non_guardian_rejected() {
        let (env, client, _, _) = setup();
        let stranger = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let err = client
            .try_initiate_recovery(&stranger, &new_owner)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::Unauthorized);
    }

    #[test]
    fn test_initiate_recovery_duplicate_pending_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        let err = client
            .try_initiate_recovery(&guardian, &new_owner)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::RecoveryAlreadyPending);
    }

    #[test]
    fn test_initiate_recovery_on_uninitialised_contract_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let guardian = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let err = client
            .try_initiate_recovery(&guardian, &new_owner)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::NotInitialized);
    }

    // ── cancel_recovery ───────────────────────────────────────────────────────

    #[test]
    fn test_cancel_recovery_sets_cancelled() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        client.cancel_recovery();
        assert_eq!(client.recovery_status(), RecoveryStatus::Cancelled);
    }

    #[test]
    fn test_cancel_recovery_emits_event() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        client.cancel_recovery();
        let events = env.events().all();
        // init + rec_init + rec_cncl
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("rec_cncl"));
    }

    #[test]
    fn test_cancel_recovery_without_pending_request_rejected() {
        let (_env, client, _, _) = setup();
        let err = client.try_cancel_recovery().unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::NoActiveRecovery);
    }

    #[test]
    fn test_cancel_already_executed_recovery_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger().with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        let err = client.try_cancel_recovery().unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::NoActiveRecovery);
    }

    // ── execute_recovery ──────────────────────────────────────────────────────

    #[test]
    fn test_execute_recovery_after_timelock_transfers_ownership() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger().with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        assert_eq!(client.recovery_status(), RecoveryStatus::Executed);
        assert_eq!(client.owner(), new_owner);
    }

    #[test]
    fn test_execute_recovery_emits_event() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger().with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        let events = env.events().all();
        // init + rec_init + rec_exec
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("rec_exec"));
    }

    #[test]
    fn test_execute_recovery_before_timelock_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        // Do NOT advance ledger — timelock not expired.
        let err = client
            .try_execute_recovery(&guardian)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::TimelockNotExpired);
    }

    #[test]
    fn test_execute_recovery_non_guardian_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger().with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        let stranger = Address::generate(&env);
        let err = client
            .try_execute_recovery(&stranger)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::Unauthorized);
    }

    #[test]
    fn test_execute_recovery_without_pending_request_rejected() {
        let (_env, client, _, guardian) = setup();
        let err = client
            .try_execute_recovery(&guardian)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::NoActiveRecovery);
    }

    #[test]
    fn test_execute_cancelled_recovery_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        client.cancel_recovery();
        env.ledger().with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        let err = client
            .try_execute_recovery(&guardian)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::NoActiveRecovery);
    }
}
