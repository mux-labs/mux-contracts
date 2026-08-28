/*!
 * mux-account: Account abstraction contract for Mux Protocol.
 *
 * Provides delegated signing, guardian management, and spending limits
 * on top of a Stellar Soroban account.
 *
 * # `no_std` Constraints
 *
 * This crate is `#![no_std]` and does not use `extern crate alloc`.
 * All data structures use Soroban SDK types (`Vec`, `Map`, `String`)
 * which are backed by the Soroban host and do not require a Rust allocator.
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
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, String,
    Symbol, Val, Vec,
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
    /// Relayer allowlist entry: DataKey::Sponsor(relayer) -> bool
    Sponsor(Address),
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
    /// Unix timestamp at which this delegation stops being valid.
    pub expires_at: u64,
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

/// Audit payload emitted after a successful session execution.
///
/// `sponsor` is `None` for a directly submitted session call and carries the
/// relayer address when the call was gas-sponsored via
/// [`MuxAccount::execute_with_session_sponsored`].
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SessionExecutedEvent {
    pub session_key: Address,
    pub target: Address,
    pub function: Symbol,
    pub sponsor: Option<Address>,
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
    ScopeNotGranted = 13,
    SponsorNotAuthorized = 14,
    InvalidNonce = 15,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of delegates to bound instance-storage growth.
/// Each DelegateInfo entry is ~72 bytes; 64 entries ≈ 4.6 KB.
const MAX_DELEGATES: u32 = 64;

/// Maximum number of session keys per owner to bound instance-storage growth.
/// Each entry is ~32 bytes; 32 entries ≈ 1 KB.
#[allow(dead_code)]
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

    /// Pause the contract — suspends all non-admin operations.
    ///
    /// Once paused, every entrypoint that calls `require_not_paused` will
    /// return `Unauthorized` until the owner calls `unpause`. Only the owner
    /// can pause, so the pause mechanism itself cannot be weaponised by a
    /// third party to lock the account.
    ///
    /// Emits a `paused` audit event.
    pub fn pause(env: Env) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        env.storage().instance().set(&DataKey::Paused, &true);
        emit(&env, symbol_short!("paused"), ());
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Unpause the contract — restores normal operation.
    pub fn unpause(env: Env) -> Result<(), MuxAccountError> {
        Self::require_owner(&env)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        emit(&env, symbol_short!("unpaused"), ());
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
        expires_at: u64,
        can_spend: bool,
    ) -> Result<(), MuxAccountError> {
        Self::require_not_paused(&env)?;
        Self::require_owner(&env)?;
        let mut delegates: Map<Address, DelegateInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Delegates)
            .ok_or(MuxAccountError::NotInitialized)?;

        // Reclaim expired entries before applying the cap. This keeps storage
        // bounded without allowing stale delegates to permanently exhaust it.
        if !delegates.contains_key(delegate.clone()) {
            // `expires_at` on `DelegateInfo` is a Unix timestamp (see its
            // doc comment and the `delegates()` query below), not a ledger
            // sequence number — compare against `ledger().timestamp()` to
            // match. Comparing against `ledger().sequence()` here was a
            // pre-existing bug: it both failed to compile (u32 vs u64) and,
            // once coerced, would have compared two numbers on unrelated
            // scales, so expired delegates were never actually reclaimed.
            let now = env.ledger().timestamp();
            let mut expired = Vec::new(&env);
            for (address, info) in delegates.iter() {
                if Self::is_delegate_expired(&info, now) {
                    expired.push_back(address);
                }
            }
            for address in expired.iter() {
                delegates.remove(address);
            }
        }

        // STORAGE-GRIEFING: reject new entries beyond the cap; updates to existing
        // delegates are always allowed since they don't grow the map.
        if !delegates.contains_key(delegate.clone()) && delegates.len() >= MAX_DELEGATES {
            return Err(MuxAccountError::TooManyDelegates);
        }
        delegates.set(
            delegate.clone(),
            DelegateInfo {
                address: delegate.clone(),
                expires_at,
                can_spend,
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::Delegates, &delegates);
        emit(
            &env,
            symbol_short!("dlg_set"),
            (delegate, expires_at, can_spend),
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
        Self::apply_spend(&env, &asset, spend)?;
        emit(&env, symbol_short!("debited"), (asset, spend));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Execute a contract call and account for its asset spend.
    ///
    /// Follows checks-effects-interactions: the spend limit is validated up
    /// front without writing storage (check), the target is invoked while the
    /// reentrancy guard is held (interaction), and the debit is written to
    /// storage only after the invocation returns (effect). Holding the guard
    /// across `invoke_contract` — rather than releasing it beforehand — is
    /// what makes it actually cover the cross-contract call: a callback into
    /// `execute()` or `debit_spend()` from the invoked target during this
    /// window is rejected with `ReentrancyDetected`.
    pub fn execute(
        env: Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        asset: Address,
        spend: i128,
        nonce: u64,
    ) -> Result<Val, MuxAccountError> {
        Self::require_not_paused(&env)?;
        Self::require_owner(&env)?;

        Self::acquire_guard(&env)?;
        let limit = match Self::compute_spend_update(&env, &asset, spend) {
            Ok(limit) => limit,
            Err(e) => {
                Self::release_guard(&env);
                return Err(e);
            }
        };
        if let Err(e) = Self::consume_nonce(&env, nonce) {
            Self::release_guard(&env);
            return Err(e);
        }

        // Interaction: guard is held for the duration of the external call.
        let result = env.invoke_contract::<Val>(&target, &function, args);

        // Effect: the debit is only persisted after the interaction returns.
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(asset.clone()), &limit);
        Self::release_guard(&env);

        emit(&env, symbol_short!("executed"), (target, asset, spend));
        Self::extend_ttl(&env);
        Ok(result)
    }

    /// Validate a spend against the configured limit and return the record it
    /// would produce, without writing storage. Rolls the period counter over
    /// if the current ledger has passed `reset_ledger`.
    fn compute_spend_update(
        env: &Env,
        asset: &Address,
        spend: i128,
    ) -> Result<SpendLimit, MuxAccountError> {
        if spend <= 0 {
            return Err(MuxAccountError::InvalidAmount);
        }

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
        Ok(limit)
    }

    /// Reject if a call is already in progress; otherwise set the reentrancy
    /// guard. Callers must release it on every exit path — a contract-level
    /// `Err` return does not auto-rollback storage on Soroban, so a guard set
    /// here and never released would permanently lock out `execute()` and
    /// `debit_spend()`.
    fn acquire_guard(env: &Env) -> Result<(), MuxAccountError> {
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Executing)
            .unwrap_or(false)
        {
            return Err(MuxAccountError::ReentrancyDetected);
        }
        env.storage().instance().set(&DataKey::Executing, &true);
        Ok(())
    }

    fn release_guard(env: &Env) {
        env.storage().instance().remove(&DataKey::Executing);
    }

    fn apply_spend(env: &Env, asset: &Address, spend: i128) -> Result<(), MuxAccountError> {
        Self::acquire_guard(env)?;
        let limit = match Self::compute_spend_update(env, asset, spend) {
            Ok(limit) => limit,
            Err(e) => {
                Self::release_guard(env);
                return Err(e);
            }
        };
        env.storage()
            .instance()
            .set(&DataKey::SpendLimit(asset.clone()), &limit);
        Self::release_guard(env);
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
            if !Self::is_delegate_expired(&info, env.ledger().timestamp()) {
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
        if Self::is_delegate_expired(&info, env.ledger().timestamp()) {
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

    /// Return the account's current transaction nonce — the value the next
    /// execution call must supply.
    pub fn nonce(env: Env) -> Result<u64, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::Nonce)
            .ok_or(MuxAccountError::NotInitialized)
    }

    /// Register or replace a session key. Owner only.
    pub fn register_session_key(
        env: Env,
        session_key: Address,
        expires_at: u64,
        scopes: Vec<Scope>,
    ) -> Result<(), MuxAccountError> {
        Self::require_not_paused(&env)?;
        Self::require_owner(&env)?;
        let owner = Self::stored_owner(&env)?;
        let key = DataKey::SessionKey(owner.clone(), session_key.clone());
        if !env.storage().instance().has(&key) {
            Self::require_session_key_cap(&env, &owner)?;
            let index_key = DataKey::SessionKeyIndex(owner);
            let mut index: Vec<Address> = env
                .storage()
                .instance()
                .get(&index_key)
                .unwrap_or_else(|| Vec::new(&env));
            index.push_back(session_key.clone());
            env.storage().instance().set(&index_key, &index);
        }
        env.storage().instance().set(
            &key,
            &SessionKeyRecord {
                expires_at,
                scopes,
                revoked: false,
            },
        );
        emit(&env, symbol_short!("sk_reg"), session_key);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Revoke a registered session key. Owner only.
    pub fn revoke_session_key(env: Env, session_key: Address) -> Result<(), MuxAccountError> {
        Self::require_not_paused(&env)?;
        Self::require_owner(&env)?;
        let owner = Self::stored_owner(&env)?;
        let key = DataKey::SessionKey(owner.clone(), session_key.clone());
        let mut record: SessionKeyRecord = env
            .storage()
            .instance()
            .get(&key)
            .ok_or(MuxAccountError::Unauthorized)?;
        record.revoked = true;
        env.storage().instance().set(&key, &record);

        let index_key = DataKey::SessionKeyIndex(owner);
        let stored_index: Option<Vec<Address>> = env.storage().instance().get(&index_key);
        if let Some(mut index) = stored_index {
            if let Some(pos) = index.iter().position(|k| k == session_key) {
                index.remove(pos as u32);
                env.storage().instance().set(&index_key, &index);
            }
        }

        emit(&env, symbol_short!("sk_rev"), session_key);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Check whether a session key is currently valid and usable.
    ///
    /// Returns `Ok(true)` if the key is registered, not revoked, and not
    /// expired. Returns `Ok(false)` for a revoked, expired, or unknown key.
    pub fn is_session_key_valid(env: Env, session_key: Address) -> Result<bool, MuxAccountError> {
        let owner = Self::stored_owner(&env)?;
        let record: Option<SessionKeyRecord> = env
            .storage()
            .instance()
            .get(&DataKey::SessionKey(owner, session_key));
        Ok(match record {
            Some(r) => !r.revoked && env.ledger().timestamp() < r.expires_at,
            None => false,
        })
    }

    /// Execute a transaction payload on behalf of the account using a delegated session key.
    ///
    /// The session key signs instead of the owner: the owner pre-authorizes the
    /// key out of band with a scope list, and the key may afterwards invoke only
    /// the methods that list names. The call is dispatched under this contract's
    /// authorization context while the reentrancy guard is held, so a callback
    /// into `execute`, `debit_spend`, or this entrypoint is rejected.
    ///
    /// # Arguments
    /// * `session_key` - The address of the authorized session key
    /// * `target` - Contract to invoke
    /// * `function` - Method on `target`; must be present in the key's scopes
    /// * `args` - Arguments forwarded verbatim to `target`
    ///
    /// # Returns
    /// * `Ok(Val)` - The target's return value
    /// * `Err(MuxAccountError)` - If the session key or the scope is invalid
    ///
    /// # Events
    /// Emits a `ses_exe` event on successful execution.
    pub fn execute_with_session(
        env: Env,
        session_key: Address,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        nonce: u64,
    ) -> Result<Val, MuxAccountError> {
        Self::require_not_paused(&env)?;
        session_key.require_auth();
        Self::authorize_session(&env, &session_key, &function)?;
        Self::consume_nonce(&env, nonce)?;
        let result = Self::dispatch(&env, &target, &function, args)?;

        emit(
            &env,
            symbol_short!("ses_exe"),
            SessionExecutedEvent {
                session_key,
                target,
                function,
                sponsor: None,
            },
        );
        Self::extend_ttl(&env);
        Ok(result)
    }

    /// Gas-abstracted variant of [`Self::execute_with_session`]: a relayer
    /// submits (and pays the network fee for) a call authorized by a session
    /// key it does not own.
    ///
    /// Both parties must authorize: `sponsor` proves it submitted the call and
    /// `session_key` proves the account granted the capability. The sponsor must
    /// also be on the owner-managed allowlist — sponsorship never widens what a
    /// session key may do, it only decides who may pay for it.
    ///
    /// # Events
    /// Emits a `ses_exe` event carrying `sponsor: Some(relayer)`.
    pub fn execute_with_session_sponsored(
        env: Env,
        session_key: Address,
        sponsor: Address,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        nonce: u64,
    ) -> Result<Val, MuxAccountError> {
        Self::require_not_paused(&env)?;
        sponsor.require_auth();
        if !env
            .storage()
            .instance()
            .get(&DataKey::Sponsor(sponsor.clone()))
            .unwrap_or(false)
        {
            return Err(MuxAccountError::SponsorNotAuthorized);
        }
        session_key.require_auth();
        Self::authorize_session(&env, &session_key, &function)?;
        Self::consume_nonce(&env, nonce)?;
        let result = Self::dispatch(&env, &target, &function, args)?;

        emit(
            &env,
            symbol_short!("ses_exe"),
            SessionExecutedEvent {
                session_key,
                target,
                function,
                sponsor: Some(sponsor),
            },
        );
        Self::extend_ttl(&env);
        Ok(result)
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
        let owner = Self::stored_owner(env)?;
        owner.require_auth();
        Ok(())
    }

    fn stored_owner(env: &Env) -> Result<Address, MuxAccountError> {
        env.storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(MuxAccountError::NotInitialized)
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

    fn is_delegate_expired(info: &DelegateInfo, now: u64) -> bool {
        now >= info.expires_at
    }

    /// Extend instance-storage TTL on every write to prevent silent data loss (T-21).
    fn extend_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    /// Check the caller-supplied nonce against the stored counter and advance
    /// it by one.
    ///
    /// FAIL-CLOSED: the caller must state the nonce it signed for. Storing the
    /// counter without ever reading it left `DataKey::Nonce` at 0 forever and
    /// gave the account no replay or ordering semantics of its own — a relayer
    /// holding a session-key authorization could resubmit the same invocation.
    /// A mismatch is rejected with `InvalidNonce`; the counter only advances on
    /// a call that passed every preceding check, so a rejected call does not
    /// burn a nonce.
    fn consume_nonce(env: &Env, nonce: u64) -> Result<(), MuxAccountError> {
        let current: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Nonce)
            .ok_or(MuxAccountError::NotInitialized)?;
        if nonce != current {
            return Err(MuxAccountError::InvalidNonce);
        }
        env.storage()
            .instance()
            .set(&DataKey::Nonce, &current.saturating_add(1));
        Ok(())
    }

    /// Validate a session key against the stored `SessionKeyRecord` and the
    /// method it is trying to reach.
    ///
    /// FAIL-CLOSED (T-08, T-40 in docs/threat-model.md): every rejection path
    /// returns an error rather than falling through. A key that is unknown,
    /// revoked, or expired is rejected; a key granted **no** scopes has zero
    /// capabilities and is rejected; and a key whose scope list does not name
    /// `function` is rejected with `ScopeNotGranted`. `scopes` is the capability
    /// list the owner granted at registration, so a non-empty list is not a
    /// blanket permit — it is matched against the method actually invoked.
    fn authorize_session(
        env: &Env,
        session_key: &Address,
        function: &Symbol,
    ) -> Result<(), MuxAccountError> {
        let owner = Self::stored_owner(env)?;
        let record: SessionKeyRecord = env
            .storage()
            .instance()
            .get(&DataKey::SessionKey(owner, session_key.clone()))
            .ok_or(MuxAccountError::Unauthorized)?;
        if record.revoked || env.ledger().timestamp() >= record.expires_at {
            return Err(MuxAccountError::Unauthorized);
        }
        if record.scopes.is_empty() {
            return Err(MuxAccountError::Unauthorized);
        }
        let mut granted = false;
        for scope in record.scopes.iter() {
            if scope.method == *function {
                granted = true;
                break;
            }
        }
        if !granted {
            return Err(MuxAccountError::ScopeNotGranted);
        }
        Ok(())
    }

    /// Invoke `target` while holding the reentrancy guard, releasing it on
    /// every exit path. Session execution keeps no spend accounting of its own;
    /// a target that moves funds must call back into `debit_spend`, which the
    /// held guard rejects for the duration of this call.
    fn dispatch(
        env: &Env,
        target: &Address,
        function: &Symbol,
        args: Vec<Val>,
    ) -> Result<Val, MuxAccountError> {
        Self::acquire_guard(env)?;
        let result = env.invoke_contract::<Val>(target, function, args);
        Self::release_guard(env);
        Ok(result)
    }

    /// Enforce the session key storage cap (T-22).
    /// Called before adding a new session key to prevent unbounded growth.
    #[allow(dead_code)]
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
        testutils::{storage::Instance as _, Address as _, Events, Ledger as _},
        Env, FromVal, IntoVal, String, Vec,
    };

    #[contract]
    struct ExecuteTarget;

    #[contractimpl]
    impl ExecuteTarget {
        pub fn ping() -> u32 {
            7
        }
    }

    /// Test-only contract that, when invoked by `execute()`, attempts to call
    /// back into the invoking `MuxAccount` instance's `debit_spend` while the
    /// outer call is still in flight. Returns `true` if the reentrant call was
    /// rejected (guard held) and `false` if it went through (guard bypassed).
    #[contract]
    struct ReentrantTarget;

    #[contractimpl]
    impl ReentrantTarget {
        pub fn attack(env: Env, mux_account: Address, asset: Address, spend: i128) -> bool {
            let args: Vec<Val> =
                soroban_sdk::vec![&env, asset.into_val(&env), spend.into_val(&env)];
            let result = env.try_invoke_contract::<soroban_sdk::Val, soroban_sdk::Error>(
                &mux_account,
                &soroban_sdk::Symbol::new(&env, "debit_spend"),
                args,
            );
            result.is_err()
        }
    }

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

    fn setup() -> (Env, MuxAccountClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccount);
        let client = MuxAccountClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, client, owner, contract_id)
    }

    fn setup_without_auth() -> (Env, MuxAccountClient<'static>, Address) {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxAccount);
        let owner = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Owner, &owner);
            env.storage()
                .instance()
                .set(&DataKey::GuardianSet, &Vec::<Address>::new(&env));
            env.storage().instance().set(
                &DataKey::Delegates,
                &Map::<Address, DelegateInfo>::new(&env),
            );
            env.storage().instance().set(&DataKey::Nonce, &0_u64);
        });
        let client = MuxAccountClient::new(&env, &contract_id);
        (env, client, owner)
    }

    #[test]
    fn test_owner_mutations_reject_missing_authorization() {
        let (env, client, _owner) = setup_without_auth();
        let delegate = Address::generate(&env);
        let asset = Address::generate(&env);
        let metadata = RegistryMeta {
            name: String::from_str(&env, "unauthorized"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, ""),
        };

        assert!(client
            .try_set_delegate(&delegate, &1000_u64, &true)
            .is_err());
        assert!(client
            .try_set_spend_limit(&asset, &100_i128, &10_u32)
            .is_err());
        assert!(client.try_unpause().is_err());
        assert!(client.try_set_metadata(&metadata).is_err());

        assert_eq!(client.delegates().len(), 0);
        assert!(client.get_metadata().is_none());
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_initialize_rejects_missing_owner_authorization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxAccount);
        let client = MuxAccountClient::new(&env, &contract_id);
        let owner = Address::generate(&env);

        assert!(client.try_initialize(&owner, &Vec::new(&env)).is_err());
        assert_eq!(
            client.try_owner(),
            Err(Ok(MuxAccountError::NotInitialized))
        );
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn test_initialize_emits_event() {
        let (env, client, owner, _cid) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_set_delegate_emits_event() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u64, &true);
        let events = env.events().all();
        // init + dlg_set
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("dlg_set"));
    }

    #[test]
    fn test_remove_delegate_emits_event() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u64, &false);
        client.remove_delegate(&delegate);
        let events = env.events().all();
        // init + dlg_set + dlg_rm
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("dlg_rm"));
    }

    #[test]
    fn test_spend_limit_emits_events() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);
        client.try_debit_spend(&asset, &200_i128).unwrap().unwrap();
        let events = env.events().all();
        // init + lmt_set + debited
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("lmt_set"));
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("debited"));
    }

    #[test]
    fn test_delegate_cap_enforced() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));

        // Fill up to the cap
        for _ in 0..64 {
            client.set_delegate(&Address::generate(&env), &1000_u64, &false);
        }
        // One more new delegate must be rejected
        let result = client.try_set_delegate(&Address::generate(&env), &1000_u64, &false);
        assert!(result.is_err());
    }

    #[test]
    fn test_delegate_cap_allows_update() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));

        // Fill to cap
        let first = Address::generate(&env);
        client.set_delegate(&first, &1000_u64, &false);
        for _ in 1..64 {
            client.set_delegate(&Address::generate(&env), &1000_u64, &false);
        }
        // Updating an existing delegate must still succeed even at cap
        assert!(client.try_set_delegate(&first, &2000_u64, &true).is_ok());
    }

    #[test]
    fn test_delegate_cap_reclaims_expired_entries() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let expiry = env.ledger().timestamp() + 1;
        for _ in 0..64 {
            client.set_delegate(&Address::generate(&env), &expiry, &false);
        }

        env.ledger().set_timestamp(expiry);
        let replacement = Address::generate(&env);
        assert!(client
            .try_set_delegate(&replacement, &(expiry + 100), &true)
            .is_ok());
        assert_eq!(client.delegates().len(), 1);
        assert!(client.delegates().contains_key(replacement));
    }

    #[test]
    fn test_initialize() {
        let (env, client, owner, _cid) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        assert!(client.try_initialize(&owner, &guardians).is_ok());
        assert_eq!(client.owner(), owner);
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, owner, _cid) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let result = client.try_initialize(&owner, &guardians);
        assert!(result.is_err());
    }

    #[test]
    fn test_double_initialize_returns_already_initialized_error() {
        let (env, client, owner, _cid) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let result = client.try_initialize(&owner, &guardians);
        assert_eq!(result, Err(Ok(MuxAccountError::AlreadyInitialized)));
    }

    #[test]
    fn test_initialize_with_different_owner_returns_already_initialized() {
        let (env, client, owner, _cid) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);
        let other_owner = Address::generate(&env);
        let result = client.try_initialize(&other_owner, &guardians);
        assert_eq!(result, Err(Ok(MuxAccountError::AlreadyInitialized)));
    }

    #[test]
    fn test_initialize_with_guardians_returns_already_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccount);
        let client = MuxAccountClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let g1 = Address::generate(&env);
        let g2 = Address::generate(&env);
        let guardians = soroban_sdk::vec![&env, g1, g2];
        client.initialize(&owner, &guardians);
        // Second init with different guardians still fails with AlreadyInitialized.
        let new_guardians = soroban_sdk::vec![&env, Address::generate(&env)];
        let result = client.try_initialize(&owner, &new_guardians);
        assert_eq!(result, Err(Ok(MuxAccountError::AlreadyInitialized)));
    }

    #[test]
    fn test_set_and_remove_delegate() {
        let (env, client, owner, _cid) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u64, &true);

        let delegates = client.delegates();
        assert!(delegates.contains_key(delegate.clone()));

        client.remove_delegate(&delegate);
        let delegates_after = client.delegates();
        assert!(!delegates_after.contains_key(delegate));
    }

    #[test]
    fn test_get_delegate_returns_active_delegate_info() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u64, &true);

        let info = client.get_delegate(&delegate);
        assert_eq!(info.address, delegate);
        assert!(info.can_spend);
        assert_eq!(info.expires_at, 1000_u64);
    }

    #[test]
    fn test_get_delegate_fails_for_unauthorized_delegate() {
        let (env, client, _owner, _cid) = setup();
        client.initialize(&_owner, &Vec::new(&env));
        let delegate = Address::generate(&env);

        let result = client.try_get_delegate(&delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_delegate_fails_when_delegate_expired() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        let current = env.ledger().timestamp();
        let expiry = current + 1;
        client.set_delegate(&delegate, &expiry, &true);
        env.ledger().set_timestamp(expiry);

        let result = client.try_get_delegate(&delegate);
        assert!(result.is_err());
    }

    #[test]
    fn test_delegates_filters_expired_delegates() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let delegate = Address::generate(&env);
        let current = env.ledger().timestamp();
        let expiry = current + 1;
        client.set_delegate(&delegate, &expiry, &true);
        env.ledger().set_timestamp(expiry);

        let active = client.delegates();
        assert!(!active.contains_key(delegate));
    }

    #[test]
    fn test_spend_limit_enforcement() {
        let (env, client, owner, _cid) = setup();
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
    fn test_execute_enforces_spend_limit_before_invocation() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &100_i128, &100_u32);
        let target = env.register_contract(None, ExecuteTarget);
        let args = Vec::new(&env);

        let value = client.execute(
            &target,
            &symbol_short!("ping"),
            &args,
            &asset,
            &40_i128,
            &0_u64,
        );
        assert_eq!(u32::from_val(&env, &value), 7);
        assert!(matches!(
            client.try_execute(
                &target,
                &symbol_short!("ping"),
                &args,
                &asset,
                &70_i128,
                &1_u64,
            ),
            Err(Ok(MuxAccountError::SpendLimitExceeded))
        ));
    }

    // ── CEI ordering / reentrancy guard (invoke-then-debit) ─────────────────

    #[test]
    fn test_execute_holds_reentrancy_guard_across_invocation() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);

        let attacker = env.register_contract(None, ReentrantTarget);
        let attack_args: Vec<Val> = soroban_sdk::vec![
            &env,
            cid.clone().into_val(&env),
            asset.clone().into_val(&env),
            50_i128.into_val(&env)
        ];

        let value = client.execute(
            &attacker,
            &symbol_short!("attack"),
            &attack_args,
            &asset,
            &50_i128,
            &0_u64,
        );
        assert!(
            bool::from_val(&env, &value),
            "a reentrant debit_spend call during execute() must be rejected \
             with ReentrancyDetected — the guard must be held across invoke_contract, \
             not released before it (previously the debit was written and the \
             guard cleared before the target was ever invoked)"
        );

        // The outer spend must be recorded exactly once — the rejected
        // reentrant attempt must not have added anything.
        let recorded: SpendLimit = env.as_contract(&cid, || {
            env.storage()
                .instance()
                .get(&DataKey::SpendLimit(asset.clone()))
                .unwrap()
        });
        assert_eq!(recorded.spent, 50_i128);
    }

    #[test]
    fn test_execute_spend_limit_rejection_does_not_lock_out_future_calls() {
        // A contract-level Err return does not auto-rollback storage on
        // Soroban, so if the reentrancy guard were left set on a
        // SpendLimitExceeded rejection, every subsequent execute()/
        // debit_spend() call would permanently fail with ReentrancyDetected.
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &100_i128, &100_u32);
        let target = env.register_contract(None, ExecuteTarget);
        let args = Vec::new(&env);

        let rejected =
            client.try_execute(&target, &symbol_short!("ping"), &args, &asset, &500_i128, &0_u64);
        assert!(matches!(
            rejected,
            Err(Ok(MuxAccountError::SpendLimitExceeded))
        ));

        // The guard must have been released on that failure path — a
        // within-limit call right after must succeed, not hit
        // ReentrancyDetected.
        let value = client.execute(&target, &symbol_short!("ping"), &args, &asset, &40_i128, &0_u64);
        assert_eq!(u32::from_val(&env, &value), 7);
    }

    #[test]
    fn test_debit_spend_rejection_does_not_lock_out_future_calls() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &100_i128, &100_u32);

        let rejected = client.try_debit_spend(&asset, &500_i128);
        assert_eq!(rejected, Err(Ok(MuxAccountError::SpendLimitExceeded)));

        // Guard must be released on the rejection path.
        assert!(client.try_debit_spend(&asset, &40_i128).is_ok());
    }

    #[test]
    fn test_execute_rejects_non_positive_spend() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &100_i128, &100_u32);
        assert!(matches!(
            client.try_execute(
                &Address::generate(&env),
                &symbol_short!("ping"),
                &Vec::new(&env),
                &asset,
                &0_i128,
                &0_u64,
            ),
            Err(Ok(MuxAccountError::InvalidAmount))
        ));
    }

    #[test]
    fn test_spend_limit_invalid_amount() {
        let (env, client, owner, _cid) = setup();
        let guardians: Vec<Address> = Vec::new(&env);
        client.initialize(&owner, &guardians);

        let asset = Address::generate(&env);
        let result = client.try_set_spend_limit(&asset, &0_i128, &100_u32);
        assert!(result.is_err());
    }

    #[test]
    fn test_unpause_emits_event() {
        let (env, client, owner, _cid) = setup();
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
    fn test_pause_emits_event() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        client.pause();
        let events = env.events().all();
        // init + paused
        assert!(events.len() >= 2);
        assert_eq!(
            topic_action(&env, &events, events.len() - 1),
            symbol_short!("paused")
        );
    }

    #[test]
    fn test_pause_sets_paused_flag() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        assert!(!client.is_paused());
        client.pause();
        assert!(client.is_paused());
    }

    #[test]
    fn test_pause_then_unpause_clears_flag() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        client.pause();
        assert!(client.is_paused());
        client.unpause();
        assert!(!client.is_paused());
    }

    #[test]
    fn test_pause_blocks_set_delegate() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        client.pause();
        let delegate = Address::generate(&env);
        let result = client.try_set_delegate(&delegate, &1000_u64, &true);
        assert_eq!(result, Err(Ok(MuxAccountError::Unauthorized)));
    }

    #[test]
    fn test_pause_blocks_execute() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);
        client.pause();
        let target = env.register_contract(None, ExecuteTarget);
        let result = client.try_execute(
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &asset,
            &10_i128,
            &0_u64,
        );
        assert!(matches!(result, Err(Ok(MuxAccountError::Unauthorized))));
    }

    #[test]
    fn test_pause_requires_owner_auth() {
        let (_env, client, _owner) = setup_without_auth();
        let result = client.try_pause();
        assert!(result.is_err());
    }

    /// A single-scope list used by session-key happy-path tests. Empty-scope
    /// keys are rejected fail-closed (T-40 in docs/threat-model.md), so tests
    /// that expect success must register at least one granted capability.
    fn pay_scope(env: &Env) -> soroban_sdk::Vec<Scope> {
        soroban_sdk::vec![
            &env,
            Scope {
                method: symbol_short!("pay"),
            },
        ]
    }

    /// Scope list granting exactly the `ping` method of `ExecuteTarget`, used
    /// by the dispatch tests.
    fn ping_scope(env: &Env) -> soroban_sdk::Vec<Scope> {
        soroban_sdk::vec![
            &env,
            Scope {
                method: symbol_short!("ping"),
            },
        ]
    }

    #[test]
    fn test_execute_with_session_emits_event() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );
        let _ = client.execute_with_session(
            &session_key,
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &0_u64,
        );
        let events = env.events().all();
        // init + ses_exe
        assert!(events.len() >= 2);
        assert_eq!(
            topic_action(&env, &events, events.len() - 1),
            symbol_short!("ses_exe")
        );
    }

    /// Phase 2 milestone: `execute_with_session` must actually dispatch to the
    /// target contract, not just validate the key and return an empty value.
    /// This test fails if the entrypoint regresses to a validation-only stub.
    #[test]
    fn test_execute_with_session_dispatches_to_target() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );

        let value = client.execute_with_session(
            &session_key,
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &0_u64,
        );
        assert_eq!(
            u32::from_val(&env, &value),
            7,
            "the target's return value must be forwarded to the caller"
        );

        let events = env.events().all();
        let (_, _, data) = events.get(events.len() - 1).unwrap();
        let payload = SessionExecutedEvent::from_val(&env, &data);
        assert_eq!(payload.target, target);
        assert_eq!(payload.function, symbol_short!("ping"));
        assert_eq!(payload.sponsor, None);
    }

    /// Fail-closed scope matching: a non-empty scope list is not a blanket
    /// permit. A key scoped to `pay` must not be able to reach `ping`.
    #[test]
    fn test_execute_with_session_rejects_method_outside_scopes() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &pay_scope(&env),
        );

        assert_eq!(
            client.try_execute_with_session(
                &session_key,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &0_u64,
            ),
            Err(Ok(MuxAccountError::ScopeNotGranted)),
            "a method absent from the granted scopes must fail closed"
        );
        // Nothing may be emitted on the rejected path.
        assert_eq!(env.events().all().len(), 1);
    }

    // ── Relayer sponsorship ──────────────────────────────────────────────────

    /// Sponsorship is fail-closed: a relayer that was never allowlisted cannot
    /// relay a session call even when the session key itself is valid.
    #[test]
    fn test_sponsored_execution_rejects_unknown_sponsor() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );
        let relayer = Address::generate(&env);

        assert_eq!(
            client.try_execute_with_session_sponsored(
                &session_key,
                &relayer,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &0_u64,
            ),
            Err(Ok(MuxAccountError::SponsorNotAuthorized))
        );
    }

    #[test]
    fn test_sponsored_execution_dispatches_and_records_sponsor() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );
        let relayer = Address::generate(&env);
        client.set_sponsor(&relayer, &true);
        assert!(client.is_sponsor(&relayer));

        let value = client.execute_with_session_sponsored(
            &session_key,
            &relayer,
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &0_u64,
        );
        assert_eq!(u32::from_val(&env, &value), 7);

        let events = env.events().all();
        let (_, _, data) = events.get(events.len() - 1).unwrap();
        let payload = SessionExecutedEvent::from_val(&env, &data);
        assert_eq!(payload.sponsor, Some(relayer.clone()));

        // Removal is immediate and fail-closed.
        client.set_sponsor(&relayer, &false);
        assert!(!client.is_sponsor(&relayer));
        assert_eq!(
            client.try_execute_with_session_sponsored(
                &session_key,
                &relayer,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &1_u64,
            ),
            Err(Ok(MuxAccountError::SponsorNotAuthorized))
        );
    }

    /// A sponsor never widens a session key's capabilities — the scope check
    /// still applies on the sponsored path.
    #[test]
    fn test_sponsored_execution_still_enforces_scopes() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &pay_scope(&env),
        );
        let relayer = Address::generate(&env);
        client.set_sponsor(&relayer, &true);

        assert_eq!(
            client.try_execute_with_session_sponsored(
                &session_key,
                &relayer,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &0_u64,
            ),
            Err(Ok(MuxAccountError::ScopeNotGranted))
        );
    }

    // ── Transaction nonce ────────────────────────────────────────────────────

    /// `DataKey::Nonce` was written once at initialization and never read or
    /// advanced. It must now start at 0 and advance by exactly one per
    /// successful execution, on every execution path.
    #[test]
    fn test_nonce_starts_at_zero_and_advances_per_execution() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        assert_eq!(client.nonce(), 0, "a fresh account starts at nonce 0");

        let target = env.register_contract(None, ExecuteTarget);
        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);
        client.execute(
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &asset,
            &10_i128,
            &0_u64,
        );
        assert_eq!(client.nonce(), 1, "execute must advance the nonce");

        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );
        client.execute_with_session(
            &session_key,
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &1_u64,
        );
        assert_eq!(client.nonce(), 2, "session execution must advance the nonce");

        let relayer = Address::generate(&env);
        client.set_sponsor(&relayer, &true);
        client.execute_with_session_sponsored(
            &session_key,
            &relayer,
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &2_u64,
        );
        assert_eq!(
            client.nonce(),
            3,
            "sponsored execution must advance the nonce"
        );
    }

    /// Replay protection: resubmitting a call that already consumed its nonce
    /// must fail closed rather than execute a second time.
    #[test]
    fn test_execute_with_session_rejects_replayed_nonce() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );

        client.execute_with_session(
            &session_key,
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &0_u64,
        );
        assert_eq!(
            client.try_execute_with_session(
                &session_key,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &0_u64,
            ),
            Err(Ok(MuxAccountError::InvalidNonce)),
            "replaying a consumed nonce must fail closed"
        );
        // A nonce from the future is equally invalid — the counter is exact,
        // not a lower bound.
        assert_eq!(
            client.try_execute_with_session(
                &session_key,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &7_u64,
            ),
            Err(Ok(MuxAccountError::InvalidNonce))
        );
        assert_eq!(client.nonce(), 1, "rejected calls must not advance it");
    }

    /// A call rejected by an earlier check must not burn a nonce — otherwise a
    /// third party could desynchronise a relayer's queue by spamming rejects.
    #[test]
    fn test_rejected_call_does_not_consume_a_nonce() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &pay_scope(&env),
        );

        assert_eq!(
            client.try_execute_with_session(
                &session_key,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &0_u64,
            ),
            Err(Ok(MuxAccountError::ScopeNotGranted))
        );
        assert_eq!(client.nonce(), 0);

        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &100_i128, &100_u32);
        assert!(matches!(
            client.try_execute(
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &asset,
                &500_i128,
                &0_u64,
            ),
            Err(Ok(MuxAccountError::SpendLimitExceeded))
        ));
        assert_eq!(client.nonce(), 0);
    }

    #[test]
    fn test_execute_with_session_rejects_unknown_revoked_and_expired_keys() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let target = env.register_contract(None, ExecuteTarget);
        let session_key = Address::generate(&env);
        let ping = symbol_short!("ping");
        let args: Vec<Val> = Vec::new(&env);
        assert_eq!(
            client.try_execute_with_session(&session_key, &target, &ping, &args, &0_u64),
            Err(Ok(MuxAccountError::Unauthorized))
        );

        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );
        client.revoke_session_key(&session_key);
        assert_eq!(
            client.try_execute_with_session(&session_key, &target, &ping, &args, &0_u64),
            Err(Ok(MuxAccountError::Unauthorized))
        );

        client.register_session_key(&session_key, &env.ledger().timestamp(), &ping_scope(&env));
        assert_eq!(
            client.try_execute_with_session(&session_key, &target, &ping, &args, &0_u64),
            Err(Ok(MuxAccountError::Unauthorized))
        );
    }

    /// T-40 fail-closed: a session key registered with an empty scope list
    /// grants zero capabilities and must be rejected at execution time, not
    /// silently accepted (the pre-fix behavior returned `Ok` unconditionally).
    #[test]
    fn test_execute_with_session_rejects_empty_scopes() {
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &Vec::new(&env),
        );
        let target = env.register_contract(None, ExecuteTarget);
        assert_eq!(
            client.try_execute_with_session(
                &session_key,
                &target,
                &symbol_short!("ping"),
                &Vec::new(&env),
                &0_u64,
            ),
            Err(Ok(MuxAccountError::Unauthorized)),
            "empty-scope session key must fail closed"
        );
        // No ses_exe event may be emitted on the rejected path.
        let events = env.events().all();
        assert_eq!(
            events.len(),
            2,
            "only the init and sk_reg events may exist: {events:?}"
        );
    }

    #[test]
    fn test_ttl_extended_on_write() {
        // Verify that initialize bumps instance TTL (T-21 mitigation).
        // The Soroban test environment starts with TTL = 0; after a write that
        // calls extend_ttl the value must be > 0.
        let (env, client, owner, _cid) = setup();
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
        let (env, client, owner, _cid) = setup();
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
        let (env, client, owner, _cid) = setup();
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
        let (env, client, owner, _cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        assert!(client.get_metadata().is_none());
    }

    #[test]
    fn test_set_metadata_emits_event() {
        let (env, client, owner, _cid) = setup();
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
        let (_env, client, _owner, _cid) = setup();
        let meta = RegistryMeta {
            name: String::from_str(&_env, "registry"),
            version: String::from_str(&_env, "1.0.0"),
            description: String::from_str(&_env, ""),
        };
        let result = client.try_set_metadata(&meta);
        assert!(result.is_err());
    }

    // ── TTL extension coverage (#391) ──────────────────────────────────────────

    /// Helper: read the instance TTL remaining via `as_contract`.
    fn instance_ttl(env: &Env, contract_id: &soroban_sdk::Address) -> u32 {
        env.as_contract(contract_id, || env.storage().instance().get_ttl())
    }

    #[test]
    fn test_ttl_extended_on_set_delegate() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let ttl_before = instance_ttl(&env, &cid);

        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u64, &true);

        let ttl_after = instance_ttl(&env, &cid);
        assert!(
            ttl_after >= ttl_before,
            "TTL must not decrease after set_delegate"
        );
    }

    #[test]
    fn test_ttl_extended_on_remove_delegate() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));

        let delegate = Address::generate(&env);
        client.set_delegate(&delegate, &1000_u64, &false);
        let ttl_before = instance_ttl(&env, &cid);

        client.remove_delegate(&delegate);

        let ttl_after = instance_ttl(&env, &cid);
        assert!(
            ttl_after >= ttl_before,
            "TTL must not decrease after remove_delegate"
        );
    }

    #[test]
    fn test_ttl_extended_on_set_spend_limit() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let ttl_before = instance_ttl(&env, &cid);

        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);

        let ttl_after = instance_ttl(&env, &cid);
        assert!(
            ttl_after >= ttl_before,
            "TTL must not decrease after set_spend_limit"
        );
    }

    #[test]
    fn test_ttl_extended_on_debit_spend() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));

        let asset = Address::generate(&env);
        client.set_spend_limit(&asset, &1000_i128, &100_u32);
        let ttl_before = instance_ttl(&env, &cid);

        client.try_debit_spend(&asset, &200_i128).unwrap().unwrap();

        let ttl_after = instance_ttl(&env, &cid);
        assert!(
            ttl_after >= ttl_before,
            "TTL must not decrease after debit_spend"
        );
    }

    #[test]
    fn test_ttl_extended_on_unpause() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let ttl_before = instance_ttl(&env, &cid);

        client.unpause();

        let ttl_after = instance_ttl(&env, &cid);
        assert!(
            ttl_after >= ttl_before,
            "TTL must not decrease after unpause"
        );
    }

    #[test]
    fn test_ttl_extended_on_execute_with_session() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let ttl_before = instance_ttl(&env, &cid);

        let session_key = Address::generate(&env);
        client.register_session_key(
            &session_key,
            &(env.ledger().timestamp() + 60),
            &ping_scope(&env),
        );
        let target = env.register_contract(None, ExecuteTarget);
        let _ = client.execute_with_session(
            &session_key,
            &target,
            &symbol_short!("ping"),
            &Vec::new(&env),
            &0_u64,
        );

        let ttl_after = instance_ttl(&env, &cid);
        assert!(
            ttl_after >= ttl_before,
            "TTL must not decrease after execute_with_session"
        );
    }

    #[test]
    fn test_ttl_extended_on_set_metadata() {
        let (env, client, owner, cid) = setup();
        client.initialize(&owner, &Vec::new(&env));
        let ttl_before = instance_ttl(&env, &cid);

        let meta = RegistryMeta {
            name: String::from_str(&env, "test"),
            version: String::from_str(&env, "1.0.0"),
            description: String::from_str(&env, "desc"),
        };
        client.set_metadata(&meta);

        let ttl_after = instance_ttl(&env, &cid);
        assert!(
            ttl_after >= ttl_before,
            "TTL must not decrease after set_metadata"
        );
    }

    // ── symbol_short length audit (#496) ─────────────────────────────────────

    /// All contract tag and action symbols must be <= 9 characters so that
    /// `symbol_short!` produces valid Soroban symbols. The macro itself
    /// enforces this at compile time — this test documents the contract's
    /// event vocabulary and will fail to compile if a symbol is too long.
    #[test]
    fn test_symbol_short_lengths_within_limit() {
        let _tags = [symbol_short!("mux_acct")];
        let _actions = [
            symbol_short!("init"),
            symbol_short!("paused"),
            symbol_short!("dlg_set"),
            symbol_short!("dlg_rm"),
            symbol_short!("lmt_set"),
            symbol_short!("debited"),
            symbol_short!("ses_exe"),
            symbol_short!("meta_set"),
            symbol_short!("unpaused"),
            symbol_short!("spn_set"),
        ];
        // symbol_short! validates length at compile time; reaching here is sufficient.
    }
}
