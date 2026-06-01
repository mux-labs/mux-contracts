/*!
 * mux-recovery: Social recovery contract for Mux Protocol.
 *
 * Allows a guardian set to initiate and execute account recovery,
 * transferring ownership to a new address after a timelock period.
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
    NotEnoughGuardians = 7,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRecovery;

fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_rec"), action), data);
}

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
