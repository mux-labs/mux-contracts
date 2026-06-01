/*!
 * mux-recovery: Social recovery contract for Mux Protocol.
 *
 * Allows a guardian set to initiate and execute account recovery,
 * transferring ownership to a new address after a timelock period.
 *
 * # Trust model
 *
 * See `docs/recovery-trust-model.md` for the full security analysis.
 * In brief:
 * - Only guardians may initiate or execute recovery.
 * - Only the current owner may cancel a pending recovery.
 * - A `RECOVERY_TIMELOCK` ledger delay between initiation and execution
 *   gives the legitimate owner time to cancel a fraudulent request.
 */

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Vec};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Owner,
    GuardianSet,
    RecoveryRequest,
}

// ── Recovery status ───────────────────────────────────────────────────────────

/// Lifecycle state of a recovery request.
///
/// - `None`      – no active recovery request exists.
/// - `Pending`   – a recovery has been initiated; timelock is counting down.
/// - `Executed`  – recovery completed; ownership transferred to the new address.
/// - `Cancelled` – recovery was cancelled by the owner before execution.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryStatus {
    None,
    Pending,
    Executed,
    Cancelled,
}

// ── Timelock ──────────────────────────────────────────────────────────────────

/// Minimum ledgers between `initiate_recovery` and `execute_recovery`.
///
/// At ~5-second ledger close times: 17_280 ledgers ≈ 24 hours.
/// This window lets the legitimate owner cancel a fraudulent recovery.
pub const RECOVERY_TIMELOCK: u32 = 17_280;

// ── Recovery request ──────────────────────────────────────────────────────────

/// An active recovery request stored on-chain.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryRequest {
    /// The proposed new owner address.
    pub new_owner: Address,
    /// Ledger sequence at which the request was initiated.
    pub initiated_at: u32,
    /// Earliest ledger at which `execute_recovery` may be called.
    pub executable_at: u32,
    /// Current lifecycle state.
    pub status: RecoveryStatus,
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

// ── Event helpers ─────────────────────────────────────────────────────────────

fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_rec"), action), data);
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRecovery;

#[contractimpl]
impl MuxRecovery {
    /// Initialize the recovery contract with an owner and guardian set.
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
        env.storage().instance().set(&DataKey::GuardianSet, &guardians);
        emit(&env, symbol_short!("init"), owner);
        Ok(())
    }

    /// Initiate a recovery request proposing `new_owner` as the replacement owner.
    ///
    /// The caller must be a member of the guardian set. Only one active (Pending)
    /// recovery request may exist at a time. The request cannot be executed until
    /// `RECOVERY_TIMELOCK` ledgers have elapsed.
    ///
    /// Emits a `recovery_initiated` event with `(guardian, new_owner, executable_at)`.
    pub fn initiate_recovery(
        env: Env,
        guardian: Address,
        new_owner: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();

        let guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::GuardianSet)
            .ok_or(RecoveryError::NotInitialized)?;

        if !guardians.contains(&guardian) {
            return Err(RecoveryError::Unauthorized);
        }

        // Reject if a Pending request already exists.
        if let Some(req) = env
            .storage()
            .instance()
            .get::<DataKey, RecoveryRequest>(&DataKey::RecoveryRequest)
        {
            if req.status == RecoveryStatus::Pending {
                return Err(RecoveryError::RecoveryAlreadyPending);
            }
        }

        let initiated_at = env.ledger().sequence();
        let executable_at = initiated_at.saturating_add(RECOVERY_TIMELOCK);

        env.storage().instance().set(
            &DataKey::RecoveryRequest,
            &RecoveryRequest {
                new_owner: new_owner.clone(),
                initiated_at,
                executable_at,
                status: RecoveryStatus::Pending,
            },
        );

        // Emit recovery_initiated event: topics=[mux_rec, recovery_initiated]
        // data=(guardian, new_owner, executable_at)
        emit(
            &env,
            symbol_short!("rec_init"),
            (guardian, new_owner, executable_at),
        );

        Ok(())
    }

    /// Execute a pending recovery after the timelock has expired.
    /// Transfers ownership to `new_owner` and marks the request `Executed`.
    pub fn execute_recovery(env: Env, guardian: Address) -> Result<(), RecoveryError> {
        guardian.require_auth();

        let guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::GuardianSet)
            .ok_or(RecoveryError::NotInitialized)?;

        if !guardians.contains(&guardian) {
            return Err(RecoveryError::Unauthorized);
        }

        let mut req: RecoveryRequest = env
            .storage()
            .instance()
            .get(&DataKey::RecoveryRequest)
            .ok_or(RecoveryError::NoActiveRecovery)?;

        if req.status != RecoveryStatus::Pending {
            return Err(RecoveryError::NoActiveRecovery);
        }
        if env.ledger().sequence() < req.executable_at {
            return Err(RecoveryError::TimelockNotExpired);
        }

        let new_owner = req.new_owner.clone();
        req.status = RecoveryStatus::Executed;
        env.storage().instance().set(&DataKey::RecoveryRequest, &req);
        env.storage().instance().set(&DataKey::Owner, &new_owner);
        emit(&env, symbol_short!("rec_exec"), new_owner);
        Ok(())
    }

    /// Cancel a pending recovery. Only the current owner may cancel.
    pub fn cancel_recovery(env: Env) -> Result<(), RecoveryError> {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(RecoveryError::NotInitialized)?;
        owner.require_auth();

        let mut req: RecoveryRequest = env
            .storage()
            .instance()
            .get(&DataKey::RecoveryRequest)
            .ok_or(RecoveryError::NoActiveRecovery)?;

        if req.status != RecoveryStatus::Pending {
            return Err(RecoveryError::NoActiveRecovery);
        }

        req.status = RecoveryStatus::Cancelled;
        env.storage().instance().set(&DataKey::RecoveryRequest, &req);
        emit(&env, symbol_short!("rec_cncl"), owner);
        Ok(())
    }

    /// Return the current recovery status (`None` if no request exists).
    pub fn recovery_status(env: Env) -> RecoveryStatus {
        env.storage()
            .instance()
            .get::<DataKey, RecoveryRequest>(&DataKey::RecoveryRequest)
            .map(|r| r.status)
            .unwrap_or(RecoveryStatus::None)
    }

    /// Return the current owner.
    pub fn owner(env: Env) -> Result<Address, RecoveryError> {
        env.storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(RecoveryError::NotInitialized)
    }

    /// Return the guardian set.
    pub fn guardians(env: Env) -> Result<Vec<Address>, RecoveryError> {
        env.storage()
            .instance()
            .get(&DataKey::GuardianSet)
            .ok_or(RecoveryError::NotInitialized)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        vec, Env,
    };

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

    // ── Positive baseline ─────────────────────────────────────────────────────

    #[test]
    fn test_initiate_recovery_sets_pending() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    }

    #[test]
    fn test_execute_after_timelock_transfers_ownership() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger().with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        assert_eq!(client.owner(), new_owner);
        assert_eq!(client.recovery_status(), RecoveryStatus::Executed);
    }

    // ── Negative: initiate_recovery ───────────────────────────────────────────

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

    // ── Negative: execute_recovery ────────────────────────────────────────────

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

    // ── Negative: cancel_recovery ─────────────────────────────────────────────

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

    // ── Negative: double initialize ───────────────────────────────────────────

    #[test]
    fn test_double_initialize_rejected() {
        let (env, client, owner, _) = setup();
        let err = client
            .try_initialize(&owner, &vec![&env])
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::AlreadyInitialized);
    }

    // ── Event: recovery_initiated ─────────────────────────────────────────────

    #[test]
    fn test_initiate_recovery_emits_event() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        let events = env.events().all();
        // init event + rec_init event
        assert_eq!(events.len(), 2);
        let (_, topics, _) = events.get(1).unwrap();
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(action, symbol_short!("rec_init"));
    }
}
