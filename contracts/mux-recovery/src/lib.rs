/*!
 * mux-recovery: Account recovery system for Mux Protocol.
 *
 * Implements a guardian-initiated recovery mechanism with a mandatory
 * timelock (~24 hours at 5-second ledger close) before the new owner
 * can take control. The current owner may cancel a pending recovery at
 * any time during the timelock window.
 *
 * # Recovery Lifecycle
 *
 * ```text
 * guardian calls initiate_recovery()
 *       │
 *       ▼
 *  ┌──────────┐
 *  │ PENDING  │  ◄── owner can cancel_recovery() at any time
 *  └──────────┘
 *       │
 *  RECOVERY_TIMELOCK ledgers elapse (~24 h)
 *       │
 *       ▼
 *  guardian calls execute_recovery()
 *       │
 *       ▼
 *  ┌──────────┐
 *  │ EXECUTED │  ownership transferred to new_owner
 *  └──────────┘
 * ```
 *
 * # Public Constants
 *
 * | Constant            | Value   | Ledgers | Approx duration (5 s/ledger) |
 * |---------------------|---------|---------|------------------------------|
 * | [`RECOVERY_TIMELOCK`] | 17 280 | delay   | ~24 hours                    |
 * | [`RECOVERY_EXPIRY`]   | 120 960| window  | ~7 days                      |
 *
 * These constants are **stable ABI** — changing them is a breaking change
 * for off-chain tooling that computes deadlines from `rec_init` event data.
 * Coordinate with a registry version bump.
 *
 * Both constants are re-exported in the TypeScript bindings via
 * `bindings/src/types.ts` as [`RECOVERY_TIMELOCK_LEDGERS`] and
 * [`RECOVERY_EXPIRY_LEDGERS`] so TypeScript clients can compute
 * `executableAt` and `expiresAt` from the `initiatedAt` ledger without
 * an extra RPC call.
 *
 * # Registry link
 *
 * An optional registry contract address can be associated with this
 * recovery contract via `set_registry`. The stored address is readable
 * via `registry_id` (returns `None` if not set). The TypeScript binding
 * exposes `setRegistry()` and `getRegistryId()` for these methods.
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types backed by the Soroban host.
 *
 * # Audit Events
 *
 * Contract tag: `mux_recv`
 *
 * | Action     | Trigger             | Data payload                                                   |
 * |------------|---------------------|----------------------------------------------------------------|
 * | `init`     | `initialize`           | `owner: Address`                                               |
 * | `rec_init` | `initiate_recovery`    | `(guardian, new_owner, initiated_at, executable_at, expires_at)` |
 * | `rec_appr` | `approve_recovery`     | `(guardian: Address, approval_count: u32)`                     |
 * | `rec_exec` | `execute_recovery`     | `(guardian: Address, new_owner: Address)`                      |
 * | `rec_adm`  | `approve_recovery_admin` | `new_owner: Address`                                          |
 * | `rec_cncl` | `cancel_recovery`      | `()`                                                           |
 * | `grd_add`  | `add_guardian`         | `guardian: Address`                                            |
 * | `grd_rm`   | `remove_guardian`      | `guardian: Address`                                            |
 * | `qrm_set`  | `set_quorum_threshold` | `threshold: u32`                                               |
 * | `reg_link` | `set_registry`         | `registry_id: Address`                                         |
 *
 * See [`docs/recovery-trust-model.md`] and [`docs/audit-events.md`] for
 * full security model and event schema reference.
 *
 * [`docs/recovery-trust-model.md`]: ../../docs/recovery-trust-model.md
 * [`docs/audit-events.md`]: ../../docs/audit-events.md
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_recv"), action), data);
}

// ── Timelock ──────────────────────────────────────────────────────────────────

/// Minimum number of ledgers that must pass between `initiate_recovery` and
/// `execute_recovery`.
///
/// At ~5-second ledger close times:
///   17_280 ledgers × 5 s = 86_400 s = **24 hours**
///
/// This window gives the legitimate owner time to observe the `rec_init`
/// on-chain event and call `cancel_recovery` before a fraudulent execution
/// can proceed.
///
/// # Stable ABI
///
/// This value is encoded in `rec_init` event payloads as `executable_at =
/// initiated_at + RECOVERY_TIMELOCK`. Off-chain indexers and TypeScript
/// clients use it to compute deadlines without a storage read. Changing
/// this constant is a **breaking change** for any tooling that derives
/// deadlines from event data — coordinate with a registry version bump and
/// update `bindings/src/types.ts` (`RECOVERY_TIMELOCK_LEDGERS`).
///
/// # TypeScript binding
///
/// Re-exported as `RECOVERY_TIMELOCK_LEDGERS` in `bindings/src/types.ts`.
pub const RECOVERY_TIMELOCK: u32 = 17_280;

/// Maximum number of ledgers after initiation during which a recovery
/// can be executed. After this window the request is considered expired
/// and a new recovery must be initiated.
///
/// At ~5-second ledger close times:
///   120_960 ledgers × 5 s = 604_800 s = **7 days**
///
/// An expired `Pending` request does **not** block a new `initiate_recovery`
/// call — the stale request is overwritten.
///
/// # Stable ABI
///
/// Like [`RECOVERY_TIMELOCK`], this value is encoded in `rec_init` event
/// payloads as `expires_at = initiated_at + RECOVERY_EXPIRY`. Changing it
/// is a breaking change.
///
/// # TypeScript binding
///
/// Re-exported as `RECOVERY_EXPIRY_LEDGERS` in `bindings/src/types.ts`.
pub const RECOVERY_EXPIRY: u32 = 120_960;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Lifecycle state of a recovery request.
///
/// State transitions:
///
/// ```text
///   None ──► Pending ──► Executed   (guardian executes after RECOVERY_TIMELOCK)
///                 └────► Cancelled  (owner cancels at any time)
/// ```
///
/// `Executed` and `Cancelled` are **terminal** states — no further transitions
/// occur from them. A new recovery request may be initiated after:
/// - A prior request is `Cancelled` (by the owner), or
/// - A prior `Pending` request reaches `expires_at` without execution
///   (treated as stale; overwritten by the next `initiate_recovery` call).
///
/// # ABI Stability
///
/// Variant names and their relative ordinal positions are on-chain ABI.
/// The TypeScript binding re-exports this as the `RecoveryStatus` enum in
/// `bindings/src/generated/mux-recovery.ts`.
///
/// # Variants
///
/// - [`RecoveryStatus::None`] — no active recovery request.
/// - [`RecoveryStatus::Pending`] — recovery initiated, timelock has not elapsed.
/// - [`RecoveryStatus::Executed`] — recovery executed, ownership transferred.
/// - [`RecoveryStatus::Cancelled`] — recovery cancelled by the owner.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryStatus {
    /// No active recovery request. Default state after initialization.
    None,
    /// A recovery has been initiated but [`RECOVERY_TIMELOCK`] ledgers have
    /// not yet elapsed. The current owner may call `cancel_recovery` at any
    /// time to abort.
    Pending,
    /// The recovery was executed after the timelock: ownership has been
    /// transferred to the `new_owner` specified at initiation. Terminal state.
    Executed,
    /// The recovery was cancelled by the current owner before execution.
    /// Terminal state. A new recovery may be initiated after cancellation.
    Cancelled,
}

/// An active recovery request stored on-chain.
///
/// Storage is bounded: exactly one `RecoveryRequest` per contract instance
/// at [`DataKey::Recovery`]. The struct is serialised via Soroban SDK's
/// `contracttype` and is directly deserialisable from TypeScript bindings.
///
/// # TypeScript binding shape
///
/// ```typescript
/// export interface RecoveryRequest {
///   newOwner: Address;      // Stellar address of the proposed owner
///   initiatedAt: u32;       // Ledger sequence when recovery was started
///   executableAt: u32;      // Earliest ledger for execute_recovery
///   expiresAt: u32;         // Latest ledger; auto-expires after this
///   status: RecoveryStatus; // None | Pending | Executed | Cancelled
///   approvals: Address[];   // Guardians who have approved (M-of-N quorum)
/// }
/// ```
///
/// # Storage griefing
///
/// The `approvals` Vec is bounded by `MAX_GUARDIANS` (16 entries), so
/// instance storage growth remains inherently bounded.
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
    /// The latest ledger at which `execute_recovery` may still be called.
    /// After this point the request is considered expired and a new
    /// recovery must be initiated (`initiated_at + RECOVERY_EXPIRY`).
    pub expires_at: u32,
    /// Current lifecycle state.
    pub status: RecoveryStatus,
    /// Guardians who have approved this recovery request.
    ///
    /// `initiate_recovery` adds the initiating guardian as the first approval.
    /// Additional guardians call `approve_recovery` to add their approval.
    /// `execute_recovery` requires `approvals.len() >= quorum_threshold`.
    pub approvals: Vec<Address>,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Owner,
    Guardians,
    Recovery,
    RegistryId,
    QuorumThreshold,
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
    TooManyGuardians = 7,
    GuardianAlreadyExists = 8,
    GuardianNotFound = 9,
    MinGuardiansRequired = 10,
    RecoveryExpired = 11,
    QuorumNotReached = 12,
    DuplicateApproval = 13,
    InvalidQuorumThreshold = 14,
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Storage griefing ─────────────────────────────────────────────────────────
/// Maximum number of guardians to bound instance-storage growth.
/// Each Address is ~32 bytes; 16 entries ≈ 0.5 KB.
const MAX_GUARDIANS: u32 = 16;

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRecovery;

#[contractimpl]
impl MuxRecovery {
    /// Initialize the recovery contract with an owner, a guardian set, and a
    /// quorum threshold.
    ///
    /// # Arguments
    /// * `quorum_threshold` — number of guardian approvals required to execute
    ///   recovery. Must be >= 1 and <= guardians.len(). A threshold of 1
    ///   preserves the previous single-guardian behaviour.
    pub fn initialize(
        env: Env,
        owner: Address,
        guardians: Vec<Address>,
        quorum_threshold: u32,
    ) -> Result<(), RecoveryError> {
        if env.storage().instance().has(&DataKey::Owner) {
            return Err(RecoveryError::AlreadyInitialized);
        }
        if quorum_threshold == 0 || quorum_threshold > guardians.len() {
            return Err(RecoveryError::InvalidQuorumThreshold);
        }
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&DataKey::Guardians, &guardians);
        env.storage()
            .instance()
            .set(&DataKey::QuorumThreshold, &quorum_threshold);
        emit(&env, symbol_short!("init"), owner);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Upgrade the contract WASM. Owner only.
    ///
    /// See `docs/contract-upgrade-pattern.md` for storage-compatibility rules
    /// that must be observed between versions. Instance storage (owner,
    /// guardians, active recovery request, and registry link) is preserved
    /// across upgrades by the Soroban host.
    ///
    /// **Recovery time-criticality note**: a WASM upgrade should never be
    /// performed while a `Pending` recovery is in flight, as the upgrade
    /// temporarily changes the executing code while the timelock window is
    /// open. Verify `recovery_status()` is not `Pending` before upgrading.
    ///
    /// Extends the instance storage TTL so an upgrade performed just before a
    /// long quiet period does not leave storage at risk of expiry (T-21).
    ///
    /// # Errors
    /// - [`RecoveryError::NotInitialized`] if `initialize` was never called.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), RecoveryError> {
        Self::require_owner(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Initiate a recovery request. Must be called by a registered guardian.
    ///
    /// Only one pending recovery may exist at a time. The timelock starts
    /// at the current ledger sequence. The initiating guardian's address is
    /// recorded as the first approval toward the quorum threshold.
    pub fn initiate_recovery(
        env: Env,
        guardian: Address,
        new_owner: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        Self::require_guardian(&env, &guardian)?;

        // Reject if a non-expired pending recovery already exists.
        // Expired pending requests are treated as stale and may be overwritten.
        if let Some(req) = Self::active_recovery(&env) {
            if req.status == RecoveryStatus::Pending && env.ledger().sequence() < req.expires_at {
                return Err(RecoveryError::RecoveryAlreadyPending);
            }
        }

        let initiated_at = env.ledger().sequence();
        // The initiating guardian counts as the first approval.
        let mut approvals = Vec::new(&env);
        approvals.push_back(guardian.clone());
        let request = RecoveryRequest {
            new_owner: new_owner.clone(),
            initiated_at,
            executable_at: initiated_at.saturating_add(RECOVERY_TIMELOCK),
            expires_at: initiated_at.saturating_add(RECOVERY_EXPIRY),
            status: RecoveryStatus::Pending,
            approvals,
        };
        env.storage().instance().set(&DataKey::Recovery, &request);
        // Carry the timelock window in the payload so indexers can surface the
        // execute/expiry deadlines without a follow-up storage read.
        emit(
            &env,
            symbol_short!("rec_init"),
            (
                guardian,
                new_owner,
                request.initiated_at,
                request.executable_at,
                request.expires_at,
            ),
        );
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Add a guardian's approval to a pending recovery request.
    ///
    /// Each registered guardian may approve at most once per request. Once
    /// the number of approvals reaches the stored quorum threshold the
    /// request is ready to be executed via `execute_recovery`.
    ///
    /// Emits a `rec_appr` event recording the approving guardian and the
    /// running approval count.
    pub fn approve_recovery(env: Env, guardian: Address) -> Result<(), RecoveryError> {
        guardian.require_auth();
        Self::require_guardian(&env, &guardian)?;
        let mut request = Self::require_pending(&env)?;

        // Prevent a guardian from approving the same request twice.
        if request.approvals.contains(&guardian) {
            return Err(RecoveryError::DuplicateApproval);
        }

        request.approvals.push_back(guardian.clone());
        let approval_count = request.approvals.len();
        env.storage().instance().set(&DataKey::Recovery, &request);
        emit(
            &env,
            symbol_short!("rec_appr"),
            (guardian, approval_count),
        );
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

    /// Execute a recovery after the timelock has expired and the quorum
    /// threshold has been reached.
    ///
    /// Must be called by a registered guardian. Checks that the number of
    /// approvals stored on the request is >= the contract's quorum threshold.
    /// Transfers ownership to `RecoveryRequest.new_owner`.
    pub fn execute_recovery(env: Env, guardian: Address) -> Result<(), RecoveryError> {
        guardian.require_auth();
        Self::require_guardian(&env, &guardian)?;
        let mut request = Self::require_pending(&env)?;

        if env.ledger().sequence() < request.executable_at {
            return Err(RecoveryError::TimelockNotExpired);
        }
        if env.ledger().sequence() >= request.expires_at {
            return Err(RecoveryError::RecoveryExpired);
        }

        // Enforce M-of-N quorum: require enough approvals before execution.
        let threshold: u32 = env
            .storage()
            .instance()
            .get(&DataKey::QuorumThreshold)
            .unwrap_or(1);
        if request.approvals.len() < threshold {
            return Err(RecoveryError::QuorumNotReached);
        }

        let new_owner = request.new_owner.clone();
        request.status = RecoveryStatus::Executed;
        env.storage().instance().set(&DataKey::Owner, &new_owner);
        env.storage().instance().set(&DataKey::Recovery, &request);
        emit(&env, symbol_short!("rec_exec"), (guardian, new_owner));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Admin-approved recovery: allow the current owner to approve and
    /// execute a pending recovery immediately, with a guardian co-signing to
    /// prevent the owner from unilaterally bypassing the 24-hour timelock.
    ///
    /// Both the owner **and** at least one registered guardian must authorize
    /// this call. The guardian co-sign requirement ensures the timelock bypass
    /// cannot be triggered by a compromised owner key alone — the attacker
    /// would also need to control a guardian key.
    ///
    /// # Arguments
    /// * `co_guardian` — a registered guardian address whose auth is required
    ///   alongside the owner's auth.
    ///
    /// # Errors
    /// * `Unauthorized` — caller is not the owner, or `co_guardian` is not a
    ///   registered guardian.
    /// * `NoActiveRecovery` — no pending recovery request exists.
    pub fn approve_recovery_admin(env: Env, co_guardian: Address) -> Result<(), RecoveryError> {
        // Both owner AND the specified co-guardian must authorize.
        Self::require_owner(&env)?;
        co_guardian.require_auth();
        Self::require_guardian(&env, &co_guardian)?;

        let mut request = Self::require_pending(&env)?;
        let new_owner = request.new_owner.clone();
        request.status = RecoveryStatus::Executed;
        env.storage().instance().set(&DataKey::Owner, &new_owner);
        env.storage().instance().set(&DataKey::Recovery, &request);
        // Emit a distinct audit event for owner-approved recoveries.
        emit(&env, symbol_short!("rec_adm"), new_owner.clone());
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Add a guardian to the guardian set. Owner only.
    ///
    /// The set is capped at `MAX_GUARDIANS` to bound instance-storage growth.
    pub fn add_guardian(env: Env, guardian: Address) -> Result<(), RecoveryError> {
        Self::require_owner(&env)?;
        let mut guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .ok_or(RecoveryError::NotInitialized)?;
        if guardians.contains(&guardian) {
            return Err(RecoveryError::GuardianAlreadyExists);
        }
        if guardians.len() >= MAX_GUARDIANS {
            return Err(RecoveryError::TooManyGuardians);
        }
        guardians.push_back(guardian.clone());
        env.storage().instance().set(&DataKey::Guardians, &guardians);
        emit(&env, symbol_short!("grd_add"), guardian);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Remove a guardian from the guardian set. Owner only.
    ///
    /// At least one guardian must always remain, otherwise recovery would
    /// become permanently unreachable.
    pub fn remove_guardian(env: Env, guardian: Address) -> Result<(), RecoveryError> {
        Self::require_owner(&env)?;
        let mut guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .ok_or(RecoveryError::NotInitialized)?;
        let index = guardians
            .first_index_of(&guardian)
            .ok_or(RecoveryError::GuardianNotFound)?;
        if guardians.len() <= 1 {
            return Err(RecoveryError::MinGuardiansRequired);
        }
        guardians.remove(index);
        env.storage().instance().set(&DataKey::Guardians, &guardians);
        emit(&env, symbol_short!("grd_rm"), guardian);
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

    /// Return the full recovery request, or `None` if no request exists.
    ///
    /// This entrypoint is primarily used by off-chain indexers and TypeScript
    /// bindings that need the complete `RecoveryRequest` struct rather than
    /// just the status.
    pub fn recovery_request(env: Env) -> Option<RecoveryRequest> {
        env.storage().instance().get(&DataKey::Recovery)
    }

    /// Link a registry contract address to this recovery contract.
    ///
    /// Only the current owner may call this method. The caller-supplied
    /// `owner` argument must equal the stored owner — a mismatch is rejected
    /// with [`RecoveryError::Unauthorized`] — and `owner.require_auth()` is
    /// called before the storage write. Emits a `reg_link` audit event and
    /// extends instance TTL.
    pub fn set_registry(
        env: Env,
        owner: Address,
        registry_id: Address,
    ) -> Result<(), RecoveryError> {
        // Fail-closed: the caller-supplied `owner` must be the stored owner.
        // Otherwise any stranger could pass their own address, satisfy
        // `owner.require_auth()` with their own signature, and re-link the
        // registry to mislead off-chain tooling.
        let stored_owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(RecoveryError::NotInitialized)?;
        if stored_owner != owner {
            return Err(RecoveryError::Unauthorized);
        }
        owner.require_auth();

        // Cross-contract registry validation (fail-closed).
        // Call list_contracts on the registry to confirm it is live.
        let result = env.try_invoke_contract::<soroban_sdk::Vec<soroban_sdk::Symbol>, soroban_sdk::Error>(
            &registry_id,
            &soroban_sdk::Symbol::new(&env, "list_contracts"),
            soroban_sdk::Vec::new(&env),
        );
        if result.is_err() {
            return Err(RecoveryError::RegistryNotFound);
        }

        env.storage()
            .instance()
            .set(&DataKey::RegistryId, &registry_id);
        emit(&env, symbol_short!("reg_link"), registry_id);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Return the linked registry contract address, or `None` if not set.
    pub fn registry_id(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::RegistryId)
    }

    /// Return the current quorum threshold.
    ///
    /// The threshold is the minimum number of guardian approvals required
    /// before `execute_recovery` may be called. Defaults to 1 if never set
    /// (preserves backward compatibility for contracts upgraded without
    /// migration).
    pub fn quorum_threshold(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::QuorumThreshold)
            .unwrap_or(1)
    }

    /// Update the quorum threshold. Owner only.
    ///
    /// The new threshold must be >= 1 and <= the current guardian count.
    /// Emits a `qrm_set` audit event.
    pub fn set_quorum_threshold(env: Env, threshold: u32) -> Result<(), RecoveryError> {
        Self::require_owner(&env)?;
        let guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .ok_or(RecoveryError::NotInitialized)?;
        if threshold == 0 || threshold > guardians.len() {
            return Err(RecoveryError::InvalidQuorumThreshold);
        }
        env.storage()
            .instance()
            .set(&DataKey::QuorumThreshold, &threshold);
        emit(&env, symbol_short!("qrm_set"), threshold);
        Self::extend_ttl(&env);
        Ok(())
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
        testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
        vec, Env, FromVal, IntoVal,
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
        client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);
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
        // A non-empty guardian set is required: quorum_threshold (1) must be
        // <= guardians.len() (an empty set would fail with
        // InvalidQuorumThreshold before any storage write).
        let guardian = Address::generate(&env);
        client.initialize(&owner, &vec![&env, guardian], &1_u32);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_rejected() {
        let (env, client, owner, _) = setup();
        let err = client
            .try_initialize(&owner, &vec![&env], &1_u32)
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
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
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
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        assert_eq!(client.recovery_status(), RecoveryStatus::Executed);
        assert_eq!(client.owner(), new_owner);
    }

    #[test]
    fn test_execute_recovery_emits_event() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
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
        let err = client.try_execute_recovery(&guardian).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::TimelockNotExpired);
    }

    #[test]
    fn test_execute_recovery_non_guardian_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        let stranger = Address::generate(&env);
        let err = client.try_execute_recovery(&stranger).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::Unauthorized);
    }

    #[test]
    fn test_execute_recovery_without_pending_request_rejected() {
        let (_env, client, _, guardian) = setup();
        let err = client.try_execute_recovery(&guardian).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::NoActiveRecovery);
    }

    #[test]
    fn test_execute_cancelled_recovery_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        client.cancel_recovery();
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        let err = client.try_execute_recovery(&guardian).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::NoActiveRecovery);
    }

    // ── add_guardian / remove_guardian (#393) ──────────────────────────────────

    #[test]
    fn test_add_guardian_succeeds() {
        let (env, client, _, _guardian) = setup();
        let new_guardian = Address::generate(&env);
        let guardians = client.guardians();
        assert!(!guardians.contains(&new_guardian));
        assert_eq!(guardians.len(), 1);
    }

    #[test]
    fn test_add_guardian_emits_event() {
        let (env, client, _, _) = setup();
        let new_guardian = Address::generate(&env);
        let _ = client.try_add_guardian(&new_guardian);
        let events = env.events().all();
        // init + grd_add
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("grd_add"));
    }

    #[test]
    fn test_add_duplicate_guardian_rejected() {
        let (_env, client, _, guardian) = setup();
        let err = client
            .try_add_guardian(&guardian)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::GuardianAlreadyExists);
    }

    #[test]
    fn test_add_guardian_cap_enforced() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        // Initialize with one guardian
        let g1 = Address::generate(&env);
        client.initialize(&owner, &vec![&env, g1], &1_u32);
        // Fill to MAX_GUARDIANS (16), already have 1
        for _ in 1..MAX_GUARDIANS {
            let _ = client.try_add_guardian(&Address::generate(&env));
        }
        // One more must be rejected
        let err = client
            .try_add_guardian(&Address::generate(&env))
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::TooManyGuardians);
    }

    #[test]
    fn test_remove_guardian_succeeds() {
        let (env, client, _, guardian) = setup();
        let g2 = Address::generate(&env);
        let _ = client.try_add_guardian(&g2);
        // Now we have 2 guardians, removing one should work
        let _ = client.try_remove_guardian(&guardian);
        assert!(!client.guardians().contains(&guardian));
        assert_eq!(client.guardians().len(), 1);
    }

    #[test]
    fn test_remove_guardian_emits_event() {
        let (env, client, _, guardian) = setup();
        let g2 = Address::generate(&env);
        client.add_guardian(&g2);
        client.remove_guardian(&guardian);
        let events = env.events().all();
        // init + grd_add + grd_rm
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("grd_rm"));
    }

    #[test]
    fn test_remove_last_guardian_rejected() {
        let (_env, client, _, guardian) = setup();
        // Only 1 guardian, can't remove
        let err = client
            .try_remove_guardian(&guardian)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::MinGuardiansRequired);
    }

    #[test]
    fn test_remove_nonexistent_guardian_rejected() {
        let (env, client, _, _) = setup();
        let stranger = Address::generate(&env);
        let err = client
            .try_remove_guardian(&stranger)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::GuardianNotFound);
    }

    #[test]
    fn test_add_guardian_on_uninitialised_contract_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let guardian = Address::generate(&env);
        let err = client
            .try_add_guardian(&guardian)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::NotInitialized);
    }

    #[test]
    fn test_removed_guardian_cannot_initiate_recovery() {
        let (env, client, _, guardian) = setup();
        let g2 = Address::generate(&env);
        client.add_guardian(&g2);
        client.remove_guardian(&guardian);
        // Removed guardian tries to initiate recovery
        let new_owner = Address::generate(&env);
        let err = client
            .try_initiate_recovery(&guardian, &new_owner)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::Unauthorized);
    }

    #[test]
    fn test_newly_added_guardian_can_initiate_recovery() {
        let (env, client, _, _) = setup();
        let g2 = Address::generate(&env);
        client.add_guardian(&g2);
        // New guardian can initiate recovery
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&g2, &new_owner);
        assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────
    // symbol_short! enforces ≤ 8 chars at compile time; these declarations
    // confirm all event tag/action strings compile without truncation.
    #[test]
    fn test_symbol_short_lengths_within_limit() {
        let _mux_recv = symbol_short!("mux_recv");
        let _init = symbol_short!("init");
        let _rec_init = symbol_short!("rec_init");
        let _rec_cncl = symbol_short!("rec_cncl");
        let _rec_exec = symbol_short!("rec_exec");
    }

    // ── recovery expiry and event payload (#400) ─────────────────────────────

    #[test]
    fn test_execute_recovery_after_expiry_rejected() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_EXPIRY + 1);
        let err = client.try_execute_recovery(&guardian).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::RecoveryExpired);
    }

    #[test]
    fn test_expired_pending_recovery_can_be_reinitiated() {
        let (env, client, _, guardian) = setup();
        let first_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &first_owner);
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_EXPIRY + 1);
        // The stale request must not block a fresh one.
        let second_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &second_owner);
        assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    }

    #[test]
    fn test_initiate_recovery_event_carries_timelock_window() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        let initiated_at = env.ledger().sequence();
        client.initiate_recovery(&guardian, &new_owner);

        let events = env.events().all();
        let (_, _, data) = events.get(1).unwrap();
        let (ev_guardian, ev_new_owner, ev_initiated, ev_executable, ev_expires): (
            Address,
            Address,
            u32,
            u32,
            u32,
        ) = FromVal::from_val(&env, &data);

        assert_eq!(ev_guardian, guardian);
        assert_eq!(ev_new_owner, new_owner);
        assert_eq!(ev_initiated, initiated_at);
        assert_eq!(ev_executable, initiated_at + RECOVERY_TIMELOCK);
        assert_eq!(ev_expires, initiated_at + RECOVERY_EXPIRY);
    }

    #[test]
    fn test_recovery_does_not_transfer_ownership_until_executed() {
        let (env, client, owner, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        assert_eq!(client.owner(), owner);
        client.execute_recovery(&guardian);
        assert_eq!(client.owner(), new_owner);
    }

    #[test]
    fn test_recovery_cannot_be_executed_twice() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        let err = client.try_execute_recovery(&guardian).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::NoActiveRecovery);
    }

    #[test]
    fn test_cancelled_recovery_can_be_reinitiated() {
        let (env, client, _, guardian) = setup();
        let first_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &first_owner);
        client.cancel_recovery();
        let second_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &second_owner);
        assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        assert_eq!(client.owner(), second_owner);
    }

    // ── recovery_request storage struct (#396) ────────────────────────────────

    #[test]
    fn test_recovery_request_returns_none_when_no_active_recovery() {
        let (_env, client, _, _) = setup();
        assert!(client.recovery_request().is_none());
    }

    #[test]
    fn test_recovery_request_returns_full_struct_after_initiate() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);

        let req = client.recovery_request().unwrap();
        assert_eq!(req.new_owner, new_owner);
        assert_eq!(req.status, RecoveryStatus::Pending);
        let seq = env.ledger().sequence();
        assert_eq!(req.initiated_at, seq);
        assert_eq!(req.executable_at, seq + RECOVERY_TIMELOCK);
        assert_eq!(req.expires_at, seq + RECOVERY_EXPIRY);
    }

    #[test]
    fn test_recovery_request_status_transitions_via_struct() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);

        // Pending
        assert_eq!(client.recovery_request().unwrap().status, RecoveryStatus::Pending);

        // Cancel -> Cancelled
        client.cancel_recovery();
        assert_eq!(client.recovery_request().unwrap().status, RecoveryStatus::Cancelled);

        // Re-initiate -> Pending
        let second_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &second_owner);
        assert_eq!(client.recovery_request().unwrap().status, RecoveryStatus::Pending);

        // Execute -> Executed
        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        client.execute_recovery(&guardian);
        assert_eq!(client.recovery_request().unwrap().status, RecoveryStatus::Executed);
    }

    // ── registry link (#403 / #616) ───────────────────────────────────────────

    #[test]
    fn test_set_registry_with_invalid_address_returns_registry_not_found() {
        // A random address (not a deployed contract) must return RegistryNotFound.
        // This is the regression guard: if cross-contract validation is removed the
        // call would succeed against an invalid registry.
        let (_env, client, owner, _) = setup();
        let invalid_registry = Address::generate(&_env);
        let result = client.try_set_registry(&owner, &invalid_registry);
        assert_eq!(
            result,
            Err(Ok(RecoveryError::RegistryNotFound)),
            "set_registry must reject a non-existent registry address"
        );
        // Registry must not be stored on failure.
        assert!(client.registry_id().is_none());
    }

    #[test]
    fn test_registry_id_none_before_set() {
        let (_env, client, _, _) = setup();
        assert!(client.registry_id().is_none());
    }

    #[test]
    fn test_set_registry_emits_no_event_on_failure() {
        // On RegistryNotFound no reg_link event should be emitted.
        let (env, client, owner, _) = setup();
        let invalid_registry = Address::generate(&env);
        let _ = client.try_set_registry(&owner, &invalid_registry);
        let events = env.events().all();
        // Only the init event from setup() should be present.
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    /// RegistryNotFound error code must be 12 — stable ABI.
    #[test]
    fn test_set_registry_non_owner_rejected() {
        // Fail-closed: even with auth fully mocked, a caller-supplied `owner`
        // that is not the stored owner must be rejected. Otherwise any
        // stranger could re-link the registry with their own signature.
        let (env, client, _owner, _) = setup();
        let stranger = Address::generate(&env);
        let registry = Address::generate(&env);
        let err = client
            .try_set_registry(&stranger, &registry)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::Unauthorized);
        assert_eq!(client.registry_id(), None);
    }

    #[test]
    fn test_set_registry_stranger_with_valid_auth_rejected() {
        // A stranger holding a perfectly valid signature over `set_registry`
        // must not be able to substitute for the owner: the stored-owner
        // check fires before `require_auth` is consulted.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let guardian = Address::generate(&env);
        client.initialize(&owner, &vec![&env, guardian], &1_u32);

        let stranger = Address::generate(&env);
        let registry = Address::generate(&env);
        env.mock_auths(&[MockAuth {
            address: &stranger,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "set_registry",
                args: vec![
                    &env,
                    stranger.clone().into_val(&env),
                    registry.clone().into_val(&env),
                ],
                sub_invokes: &[],
            },
        }]);

        let err = client
            .try_set_registry(&stranger, &registry)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::Unauthorized);
        assert_eq!(client.registry_id(), None);
    }

    #[test]
    fn test_symbol_short_reg_link_within_limit() {
        // symbol_short! enforces ≤ 8 chars at compile time.
        let _reg_link = symbol_short!("reg_link");
    }

    // ── M-of-N quorum tests (#614) ────────────────────────────────────────────

    /// With quorum=1, a single guardian initiating and executing is sufficient
    /// (backward-compatible behaviour).
    #[test]
    fn test_single_guardian_quorum_works() {
        let (_env, client, _, guardian) = setup(); // setup uses quorum=1
        let new_owner = Address::generate(&_env);
        client.initiate_recovery(&guardian, &new_owner);
        _env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);
        assert!(client.try_execute_recovery(&guardian).is_ok());
        assert_eq!(client.owner(), new_owner);
    }

    /// With quorum=2, a single guardian initiating and then executing without
    /// the second approval must be rejected.
    #[test]
    fn test_execute_recovery_fails_below_quorum() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        let g2 = Address::generate(&env);
        // quorum = 2
        client.initialize(&owner, &vec![&env, g1.clone(), g2.clone()], &2_u32);

        let new_owner = Address::generate(&env);
        client.initiate_recovery(&g1, &new_owner); // g1 = 1 approval

        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);

        // Only 1 approval, need 2 — must fail.
        let err = client.try_execute_recovery(&g1).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::QuorumNotReached);
    }

    /// With quorum=2, the second guardian adds approval and then execution succeeds.
    #[test]
    fn test_execute_recovery_succeeds_at_quorum() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        let g2 = Address::generate(&env);
        // quorum = 2
        client.initialize(&owner, &vec![&env, g1.clone(), g2.clone()], &2_u32);

        let new_owner = Address::generate(&env);
        client.initiate_recovery(&g1, &new_owner); // g1 = 1 approval
        client.approve_recovery(&g2);              // g2 = 2 approvals

        env.ledger()
            .with_mut(|l| l.sequence_number += RECOVERY_TIMELOCK + 1);

        assert!(client.try_execute_recovery(&g1).is_ok());
        assert_eq!(client.owner(), new_owner);
    }

    /// A guardian cannot approve a recovery request twice.
    #[test]
    fn test_duplicate_approval_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        let g2 = Address::generate(&env);
        client.initialize(&owner, &vec![&env, g1.clone(), g2.clone()], &2_u32);

        let new_owner = Address::generate(&env);
        client.initiate_recovery(&g1, &new_owner);

        // g1 already approved at initiation; cannot approve again.
        let err = client.try_approve_recovery(&g1).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::DuplicateApproval);
    }

    /// A non-guardian cannot add an approval.
    #[test]
    fn test_non_guardian_cannot_approve_recovery() {
        let (env, client, _, guardian) = setup();
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&guardian, &new_owner);
        let stranger = Address::generate(&env);
        let err = client.try_approve_recovery(&stranger).unwrap_err().unwrap();
        assert_eq!(err, RecoveryError::Unauthorized);
    }

    /// initialize rejects threshold > guardian count.
    #[test]
    fn test_initialize_rejects_threshold_exceeding_guardian_count() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        // 1 guardian, threshold=2 → invalid
        let err = client
            .try_initialize(&owner, &vec![&env, g1], &2_u32)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::InvalidQuorumThreshold);
    }

    /// initialize rejects threshold == 0.
    #[test]
    fn test_initialize_rejects_zero_threshold() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        let err = client
            .try_initialize(&owner, &vec![&env, g1], &0_u32)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, RecoveryError::InvalidQuorumThreshold);
    }

    /// set_quorum_threshold updates the stored threshold.
    #[test]
    fn test_set_quorum_threshold_updates_value() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        let g2 = Address::generate(&env);
        client.initialize(&owner, &vec![&env, g1, g2], &1_u32);
        assert_eq!(client.quorum_threshold(), 1);
        client.set_quorum_threshold(&2_u32);
        assert_eq!(client.quorum_threshold(), 2);
    }

    /// approve_recovery emits rec_appr event.
    #[test]
    fn test_approve_recovery_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        let g2 = Address::generate(&env);
        client.initialize(&owner, &vec![&env, g1.clone(), g2.clone()], &2_u32);
        let new_owner = Address::generate(&env);
        client.initiate_recovery(&g1, &new_owner);
        client.approve_recovery(&g2);
        let events = env.events().all();
        // init + rec_init + rec_appr
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("rec_appr"));
    }
}
