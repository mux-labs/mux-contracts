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
