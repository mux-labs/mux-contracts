# ABI Reference

Soroban contract interfaces for Mux Protocol. All contracts are written in Rust using the Soroban SDK v21.

---

## mux-account-factory

Factory contract for deploying and registering `MuxAccount` instances. Maintains
a per-owner index of deployed accounts and an optional metadata store for each
registered account.

### Types

```rust
pub struct AccountMetadata {
    pub version: String,     // Semantic version, e.g. "1.2.0"
    pub description: String, // Short human-readable description
    pub author: String,      // Author or team identifier
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `MAX_ACCOUNTS_PER_OWNER` | 64 | Maximum accounts per owner (storage griefing cap) |
| `MAX_VERSION_LENGTH` | 32 | Maximum byte length of the `version` metadata field |
| `MAX_DESCRIPTION_LENGTH` | 256 | Maximum byte length of the `description` metadata field |
| `MAX_AUTHOR_LENGTH` | 64 | Maximum byte length of the `author` metadata field |
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger (ledgers) |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to (ledgers) |

### Methods

| Method | Args | Returns | Auth | Description |
|---|---|---|---|---|
| `deploy_account` | `owner: Address, account_address: Address` | `Result<Address, MuxAccountFactoryError>` | `owner` | Register a new account. Returns the registered address. Rejects at `MAX_ACCOUNTS_PER_OWNER`. |
| `deploy_account_with_metadata` | `owner: Address, account_address: Address, version: String, description: String, author: String` | `Result<Address, MuxAccountFactoryError>` | `owner` | Register a new account and store metadata. Rejects at `MAX_ACCOUNTS_PER_OWNER`. |
| `get_accounts` | `owner: Address` | `Vec<Address>` | none | Return all accounts registered for `owner`. |
| `get_account_metadata` | `owner: Address, account_address: Address` | `Result<AccountMetadata, MuxAccountFactoryError>` | none | Return stored metadata for a specific account. |
| `account_count` | — | `u64` | none | Return the total number of accounts registered across all owners. |
| `max_accounts_per_owner` | — | `u32` | none | Returns the maximum accounts permitted per owner (64). |
| `simulate_deploy` | `owner: Address, account_address: Address` | `Result<Address, MuxAccountFactoryError>` | none | Dry-run of `deploy_account` (includes Accounts vec bound). |
| `simulate_deploy_with_metadata` | `owner: Address, account_address: Address, version: String, description: String, author: String` | `Result<Address, MuxAccountFactoryError>` | none | Dry-run of `deploy_account_with_metadata` (includes Accounts vec bound). |

### Events

| Topic | Data | Condition |
|---|---|---|
| `deployed` | `(owner: Address, account_address: Address)` | Every successful `deploy_account` or `deploy_account_with_metadata` call |
| `meta_set` | `(owner: Address, account_address: Address, version: String)` | Every successful `deploy_account_with_metadata` call |

### Errors

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `Unauthorized` | 1 | 401 | Caller is not the `owner` |
| `InvalidAccount` | 2 | 400 | `account_address` equals `owner` |
| `TooManyAccounts` | 3 | 409 | Owner has reached `MAX_ACCOUNTS_PER_OWNER` (64) |
| `MetadataNotFound` | 4 | 404 | No metadata stored for the specified owner/account pair |
| `MetadataTooLarge` | 5 | 400 | A metadata field exceeds its size limit (`version` > 32, `description` > 256, or `author` > 64 bytes) |

### Notes

- `deploy_account` and `deploy_account_with_metadata` require `owner.require_auth()`.
- Instance storage TTL is extended on every write (`deploy_account*`); read-only calls do not extend TTL.
- The per-owner cap of 64 accounts prevents unbounded growth of the `Accounts` storage vector (see `docs/storage-griefing.md`).
- Metadata string size limits (`MAX_VERSION_LENGTH`, `MAX_DESCRIPTION_LENGTH`, `MAX_AUTHOR_LENGTH`) prevent storage bloat through oversized strings; violations return `MetadataTooLarge` (code 5, HTTP 400).
- `simulate_deploy*` enforces the same Accounts vec bound (and metadata size checks) without writing state.
- Clients should query `max_accounts_per_owner` before deploy to avoid a `TooManyAccounts` error at execution time.
- The cap is per-owner: filling one owner's quota does not affect any other owner.

---

## mux-batcher

### Types

```rust
/// Classifies the intent of a batched operation.
/// Informational only — the batcher does not gate execution on the kind.
pub enum BatchOperationKind {
    Invoke,   // Generic cross-contract function call (default)
    Transfer, // Asset transfer (e.g. SAC `transfer`)
    Approve,  // Allowance / approval (e.g. SAC `approve`)
}

pub struct Operation {
    pub target: Address,
    pub fn_name: Symbol,
    pub args: Vec<Val>,
    pub require_success: bool,
    /// Operation intent — surfaced in events and TypeScript clients.
    pub kind: BatchOperationKind,
}

pub struct BatchResult {
    pub success_count: u32,
    pub failure_count: u32,
    pub errors: Vec<Bytes>,
}

/// Contract-level metadata stored once at deployment for registry discovery.
pub struct BatcherMeta {
    pub description: String,
    pub author: String,
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `MAX_BATCH_SIZE` | 50 | Maximum operations per batch; enforced on every entry point |
| `FEE_PER_OP` | 100 stroops | Base fee per operation used by `estimate_fees` |
| `TTL_THRESHOLD` | 17,280 ledgers | ~1 day — TTL extension trigger |
| `TTL_EXTEND_TO` | 518,400 ledgers | ~30 days — TTL extended to |

### Methods

| Method | Args | Returns | Auth | Description |
|---|---|---|---|---|
| `execute_batch` | `caller: Address, ops: Vec<Operation>` | `Result<BatchResult, MuxBatcherError>` | `caller` | Execute a batch of cross-contract calls atomically |
| `submit_batch` | `ops: Vec<Operation>` | `Result<BatchResult, MuxBatcherError>` | invoker | Convenience wrapper — derives caller from the invoking address |
| `simulate_batch` | `caller: Address, ops: Vec<Operation>` | `Result<BatchResult, MuxBatcherError>` | `caller` | Preflight check — no state written, no contracts called |
| `estimate_fees` | `op_count: u32` | `Result<u32, MuxBatcherError>` | none | Returns `op_count × FEE_PER_OP` stroops; pure computation, no state touched |
| `max_batch_size` | — | `u32` | none | Returns `MAX_BATCH_SIZE` (50) |
| `set_registry_metadata` | `description: String, author: String` | `Result<(), MuxBatcherError>` | none | Store deployment metadata once; returns `Err(MetadataAlreadySet)` on repeat calls |
| `get_registry_metadata` | — | `Option<BatcherMeta>` | none | Return stored metadata, or `None` if not set |

### Events

| Topic | Data | Condition |
|---|---|---|
| `bat_start` | `(caller, op_count)` | Emitted at the start of every `execute_batch` call, before any operations run |
| `executed` | `(caller, success_count, failure_count)` | Every successful `execute_batch` call |
| `bat_ok` | `(caller, success_count)` | All operations succeeded (`failure_count == 0`) |
| `bat_abort` | `caller` | A `require_success=true` operation failed |
| `sim_done` | `(caller, success_count)` | Successful `simulate_batch` call |

### Errors

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `EmptyBatch` | 1 | 400 | `ops` vector is empty, or `op_count == 0` for `estimate_fees` |
| `BatchTooLarge` | 2 | 400 | `ops.len() > 50`, or `op_count > 50` for `estimate_fees` |
| `RequiredOperationFailed` | 3 | 500 | A `require_success=true` op failed |
| `Unauthorized` | 4 | 401 | Reserved for future per-op auth checks |
| `ReentrancyDetected` | 5 | 409 | A batched op re-entered `execute_batch` |
| `MetadataAlreadySet` | 6 | 409 | `set_registry_metadata` called after metadata was already stored |

### Notes

- `execute_batch` and `submit_batch` enforce `require_auth()` on the caller; `simulate_batch` does too, even though no state is written.
- `estimate_fees` and `max_batch_size` are pure reads requiring no authorization.
- `set_registry_metadata` is write-once with no auth check — it is expected to be called by the deployer immediately after deployment.
- Instance storage TTL is extended on every successful `execute_batch` call and on `set_registry_metadata`. See [batcher-fees.md](batcher-fees.md) and [batching-limits.md](batching-limits.md) for details.

---

## mux-account

### Types

```rust
pub struct SpendLimit {
    pub asset: Address,
    pub amount: i128,
    pub period_ledgers: u32,
    pub spent: i128,
    pub reset_ledger: u32,
}

pub struct DelegateInfo {
    pub address: Address,
    pub expires_at: u64,
    pub can_spend: bool,
}

/// Scope of a session key capability.
pub struct Scope {
    pub method: Symbol,
}

/// Session key record with expiration, scopes, and revocation status.
pub struct SessionKeyRecord {
    pub expires_at: u64,
    pub scopes: Vec<Scope>,
    pub revoked: bool,
}

/// Audit payload emitted after a successful session execution.
pub struct SessionExecutedEvent {
    pub session_key: Address,
    pub target: Address,
    pub function: Symbol,
    pub sponsor: Option<Address>,
}

/// Registry-level metadata for this account instance.
pub struct RegistryMeta {
    pub name: String,
    pub version: String,
    pub description: String,
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `MAX_DELEGATES` | 64 | Maximum delegates to bound instance-storage growth |
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to |

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `owner: Address, guardians: Vec<Address>` | `Result<(), MuxAccountError>` | Set owner and guardian set; can only be called once |
| `unpause` | — | `Result<(), MuxAccountError>` | Unpause the contract; owner-only |
| `is_paused` | — | `bool` | Return whether the contract is currently paused |
| `set_delegate` | `delegate: Address, expires_at: u64, can_spend: bool` | `Result<(), MuxAccountError>` | Add or update a delegate with a Unix timestamp expiry (max 64); owner-only |
| `remove_delegate` | `delegate: Address` | `Result<(), MuxAccountError>` | Remove a delegate; owner-only |
| `set_spend_limit` | `asset: Address, amount: i128, period_ledgers: u32` | `Result<(), MuxAccountError>` | Set per-asset spend limit; owner-only |
| `debit_spend` | `asset: Address, spend: i128` | `Result<(), MuxAccountError>` | Check and debit a spend against the limit; contract-only |
| `execute` | `target: Address, function: Symbol, args: Vec<Val>, asset: Address, spend: i128, nonce: u64` | `Result<Val, MuxAccountError>` | Owner-authorized contract call with atomic on-chain spend-limit enforcement; consumes the account nonce |
| `owner` | — | `Result<Address, MuxAccountError>` | Return current owner |
| `delegates` | — | `Result<Map<Address, DelegateInfo>, MuxAccountError>` | Return all active (non-expired) delegates |
| `get_delegate` | `delegate: Address` | `Result<DelegateInfo, MuxAccountError>` | Return delegate info if currently active |
| `guardians` | — | `Result<Vec<Address>, MuxAccountError>` | Return guardian set |
| `nonce` | — | `Result<u64, MuxAccountError>` | Return the account's current transaction nonce — the value the next execution call must supply |
| `register_session_key` | `session_key: Address, expires_at: u64, scopes: Vec<Scope>` | `Result<(), MuxAccountError>` | Register or replace a session key (max `MAX_SESSION_KEYS` per owner); owner-only |
| `revoke_session_key` | `session_key: Address` | `Result<(), MuxAccountError>` | Revoke a registered session key and remove it from `SessionKeyIndex`; owner-only |
| `is_session_key_valid` | `session_key: Address` | `Result<bool, MuxAccountError>` | Return `true` if the key is registered, not revoked, and not expired; `false` for a revoked, expired, or unknown key |
| `execute_with_session` | `session_key: Address, payload: Bytes` | `Result<Bytes, MuxAccountError>` | Validate an authorized, non-expired, non-revoked session key; **fail-closed scope check (T-40)** — a key with an empty `scopes` list is rejected with `Unauthorized`. Does not decode or execute `payload`; returns empty `Bytes` on success. See [`docs/aa_sequence_diagram.md`](aa_sequence_diagram.md) for the remaining gap (non-empty scopes are not matched against the payload's target method) |
| `set_metadata` | `meta: RegistryMeta` | `Result<(), MuxAccountError>` | Store registry-level metadata for this account instance; owner-only |
| `get_metadata` | — | `Option<RegistryMeta>` | Return stored registry metadata, or `None` if not set |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `owner: Address` | Contract initialized |
| `unpaused` | `()` | Contract unpaused |
| `dlg_set` | `(delegate: Address, expires_at: u64, can_spend: bool)` | Delegate added or updated |
| `dlg_rm` | `delegate: Address` | Delegate removed |
| `lmt_set` | `(asset: Address, amount: i128, period_ledgers: u32)` | Spend limit set |
| `debited` | `(asset: Address, spend: i128)` | Spend debited |
| `ses_exe` | `SessionExecutedEvent { session_key: Address, payload_len: u32 }` | Session key execution without duplicating payload data |
| `sk_reg` | `session_key: Address` | `register_session_key` succeeds |
| `sk_rev` | `session_key: Address` | `revoke_session_key` succeeds |
| `meta_set` | `name: String` | `set_metadata` succeeds |

### Errors

| Variant | Code | Description |
|---|---|---|
| `NotInitialized` | 1 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | `initialize` called more than once |
| `Unauthorized` | 3 | Caller is not the owner or contract is paused |
| `DelegateNotFound` | 4 | Delegate does not exist |
| `DelegateExpired` | 5 | Delegate has expired |
| `SpendLimitExceeded` | 6 | Spend would exceed limit |
| `InvalidAmount` | 7 | Spend limit amount is zero or negative |
| `InvalidPeriod` | 8 | Spend limit period is zero |
| `TooManyDelegates` | 9 | Delegate map has reached `MAX_DELEGATES` (64) |
| `ReentrancyDetected` | 10 | Reentrant `debit_spend` call detected |
| `ArithmeticOverflow` | 11 | Arithmetic overflow in spend tracking |
| `TooManySessionKeys` | 12 | Owner has reached `MAX_SESSION_KEYS` (32) |
| `ScopeNotGranted` | 13 | Invoked method is not named in the session key's `scopes` |
| `SponsorNotAuthorized` | 14 | Relayer is not on the account's sponsor allowlist |
| `InvalidNonce` | 15 | Supplied nonce does not match the account's current nonce |

---

## mux-permissions

### Types

```rust
pub enum DataKey {
    Admin,
    RoleMembers(Symbol),
    RolePermissions(Symbol),
    AccountRoles(Address),
    PendingAdmins,
    AdminThreshold,
    AdminApprovals(Address),
}

pub struct RoleInfo {
    pub name: Symbol,
    pub members: Vec<Address>,
    pub permissions: Vec<Symbol>,
}

/// Registry-level metadata for this permissions registry instance.
pub struct RegistryMeta {
    pub name: String,
    pub version: String,
    pub description: String,
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `MAX_ROLE_MEMBERS` | 256 | Maximum members per role |
| `MAX_ROLES_PER_ACCOUNT` | 32 | Maximum roles per account |
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to |

### Methods — Role Management

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `admin: Address` | `Result<(), MuxPermissionsError>` | Set admin; can only be called once |
| `create_role` | `role: Symbol, permissions: Vec<Symbol>` | `Result<(), MuxPermissionsError>` | Create a role with permissions (admin-only) |
| `grant_role` | `account: Address, role: Symbol` | `Result<(), MuxPermissionsError>` | Grant role to account (admin-only) |
| `revoke_role` | `account: Address, role: Symbol` | `Result<(), MuxPermissionsError>` | Revoke role from account (admin-only) |
| `has_permission` | `account: Address, permission: Symbol` | `bool` | Check if account holds a permission via any role |
| `get_roles` | `account: Address` | `Vec<Symbol>` | Return all roles for an account |
| `get_role_members` | `role: Symbol` | `Result<Vec<Address>, MuxPermissionsError>` | Return all members of a role |

### Methods — Multisig Admin

| Method | Args | Returns | Description |
|---|---|---|---|
| `set_admin_threshold` | `threshold: u32` | `Result<(), MuxPermissionsError>` | Set approval count required to promote a pending admin (admin-only) |
| `get_admin_threshold` | — | `u32` | Return the current admin threshold, or `1` if never explicitly set |
| `propose_admin` | `new_admin: Address` | `Result<(), MuxPermissionsError>` | Propose a new admin candidate (admin-only, idempotent, capped at `MAX_PENDING_ADMINS`) |
| `approve_admin` | `approver: Address, new_admin: Address` | `Result<(), MuxPermissionsError>` | Approve a pending admin; promotes when threshold reached (admin-only) |
| `get_pending_admins` | — | `Vec<Address>` | Return all pending admin candidates |

### Methods — Registry Metadata

| Method | Args | Returns | Description |
|---|---|---|---|
| `set_metadata` | `meta: RegistryMeta` | `Result<(), MuxPermissionsError>` | Store registry-level metadata for this instance; admin-only |
| `get_metadata` | — | `Option<RegistryMeta>` | Return stored registry metadata, or `None` if not set |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `admin: Address` | Contract initialized |
| `role_crt` | `role: Symbol` | Role created |
| `role_grt` | `(account: Address, role: Symbol)` | Role granted |
| `role_rev` | `(account: Address, role: Symbol)` | Role revoked |
| `adm_thr` | `threshold: u32` | Admin threshold updated |
| `adm_prp` | `new_admin: Address` | Admin candidate proposed |
| `adm_apr` | `(approver: Address, new_admin: Address)` | Admin approval recorded (below threshold) |
| `adm_prm` | `new_admin: Address` | Admin promoted (threshold reached) |
| `meta_set` | `name: String` | `set_metadata` succeeds |

### Errors

| Variant | Code | Description |
|---|---|---|
| `NotInitialized` | 1 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | `initialize` called more than once |
| `Unauthorized` | 3 | Caller is not the admin |
| `RoleNotFound` | 4 | Role does not exist |
| `AccountNotInRole` | 5 | Account is not a member of the role |
| `PermissionNotFound` | 6 | Permission does not exist |
| `TooManyMembers` | 7 | Role has reached `MAX_ROLE_MEMBERS` (256) |
| `TooManyRoles` | 8 | Account has reached `MAX_ROLES_PER_ACCOUNT` (32) |
| `AdminNotFound` | 9 | Candidate is not in the pending admin list |
| `AlreadyApproved` | 10 | Approver already voted for this candidate |
| `TooManyPendingAdmins` | 11 | Pending admin list has reached `MAX_PENDING_ADMINS` (16) |

---

## mux-spending-policy

Spending-policy enforcement contract. Stores per-account spend limits and validates spend requests.

### Types

```rust
pub struct SpendLimit {
    pub asset: Address,
    pub limit: i128,
    pub spent: i128,
    pub reset_ledger: u32,
    pub period_ledgers: u32,
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger (ledgers) |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to (ledgers) |

### Methods

| Method | Args | Returns | Auth | Description |
|---|---|---|---|---|
| `initialize` | `admin: Address` | `Result<(), SpendingPolicyError>` | `admin` | One-time setup; stores the admin address |
| `set_policy` | `account: Address, asset: Address, limit: i128, period_ledgers: u32` | `Result<(), SpendingPolicyError>` | admin | Set or replace a spend limit for account/asset with a rolling `period_ledgers` window; resets `spent` to 0 |
| `get_policy` | `account: Address, asset: Address` | `Result<SpendLimit, SpendingPolicyError>` | none | Return the spend limit for account/asset |
| `check_spend` | `account: Address, asset: Address, amount: i128` | `Result<(), SpendingPolicyError>` | none | Check if amount is within the policy limit for the current rolling window (resets `spent` if the window has elapsed) |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `admin: Address` | Contract initialized |
| `lmt_set` | `(account: Address, asset: Address, limit: i128)` | Spend limit set |
| `chk_ok` | `(account: Address, asset: Address, amount: i128)` | Spend within limit |
| `chk_ex` | `(account: Address, asset: Address, amount: i128, limit_or_reason: i128 \| Symbol)` | Spend exceeds limit or no policy |

### Errors

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not the admin |
| `PolicyNotFound` | 4 | 404 | No spend policy for the account/asset pair |
| `SpendLimitExceeded` | 5 | 400 | Requested spend exceeds the configured limit |
| `InvalidInput` | 6 | 400 | Limit is not positive or spend amount is negative |
| `InvalidPeriod` | 7 | 400 | `period_ledgers` is zero |

---

## mux-wallet-registry

Maps symbolic names (`Symbol`) to wallet addresses. One owner is set at deploy
time and is the only account permitted to write entries. Reads are open to any
caller.

### Types

```rust
pub struct WalletMetadata {
    pub label: String,
    pub description: String,
}
```

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `owner: Address` | `Result<(), WalletRegistryError>` | Record the owner; must be called once before any other method. Owner auth required. |
| `register_wallet` | `name: Symbol, wallet: Address` | `Result<(), WalletRegistryError>` | Register or overwrite the address stored under `name` (capped at `MAX_WALLETS`). Owner auth required. |
| `register_wallet_with_metadata` | `name: Symbol, wallet: Address, label: String, description: String` | `Result<(), WalletRegistryError>` | Register or overwrite the address and attach descriptive metadata (capped at `MAX_WALLETS`). Owner auth required. |
| `get_wallet` | `name: Symbol` | `Result<Address, WalletRegistryError>` | Return the address registered under `name`. No auth required. |
| `get_metadata` | `name: Symbol` | `Result<WalletMetadata, WalletRegistryError>` | Return the metadata for a wallet registered via `register_wallet_with_metadata`. No auth required. |
| `list_wallets` | — | `Vec<Symbol>` | Return all registered wallet names. No auth required. |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `owner: Address` | Contract initialized |
| `wlt_reg` | `(name: Symbol, wallet: Address)` | `register_wallet` succeeds |
| `wlt_meta` | `(name: Symbol, wallet: Address)` | `register_wallet_with_metadata` succeeds |

### Errors

| Variant | Code | Description |
|---|---|---|
| `NotInitialized` | 1 | `initialize` has not been called; owner is unknown. |
| `AlreadyInitialized` | 2 | `initialize` was called a second time on the same instance. |
| `Unauthorized` | 3 | Reserved. Auth failures are surfaced as host errors by `Address::require_auth`. |
| `WalletNotFound` | 4 | No wallet (or no metadata) is registered under the requested name. |
| `TooManyWallets` | 5 | Registry has reached `MAX_WALLETS` (128) |

---

## mux-registry

Contract version registry. Tracks registered component names, their version
strings, and optional full metadata. Capped at `MAX_CONTRACTS` entries.

### Types

```rust
pub struct ContractMetadata {
    pub version: String,
    pub description: String,
    pub author: String,
    pub repository: String,
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `MAX_CONTRACTS` | 128 | Maximum registered contract names |
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to |

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `admin: Address` | `Result<(), MuxRegistryError>` | Set admin; can only be called once |
| `register` | `name: Symbol, version: String` | `Result<(), MuxRegistryError>` | Register or update a version (admin-only, capped at `MAX_CONTRACTS`) |
| `register_with_metadata` | `name: Symbol, version: String, description: String, author: String, repository: String` | `Result<(), MuxRegistryError>` | Register or update with full metadata (admin-only, capped at `MAX_CONTRACTS`) |
| `get_version` | `name: Symbol` | `Result<String, MuxRegistryError>` | Return the registered version string |
| `check_version` | `name: Symbol` | `Result<String, MuxRegistryError>` | Dry-run version lookup; identical to `get_version` (no state mutation either way) |
| `get_metadata` | `name: Symbol` | `Result<ContractMetadata, MuxRegistryError>` | Return the full metadata for a registered name |
| `list_contracts` | — | `Vec<Symbol>` | Return all registered contract names |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `admin: Address` | Contract initialized |
| `reg` | `(name: Symbol, version: String)` | `register` succeeds |
| `regmeta` | `(name: Symbol, version: String)` | `register_with_metadata` succeeds |

### Errors

| Variant | Code | Description |
|---|---|---|
| `NotInitialized` | 1 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | `initialize` called more than once |
| `Unauthorized` | 3 | Caller is not the admin |
| `ContractNotFound` | 4 | No entry registered under the requested name |
| `TooManyContracts` | 5 | Registry has reached `MAX_CONTRACTS` (128) |

---

## mux-recovery

Guardian-initiated account recovery with a mandatory timelock. A guardian
initiates recovery for a new owner; after `RECOVERY_TIMELOCK` ledgers elapse
(and before `RECOVERY_EXPIRY`), any guardian may execute the transfer. The
current owner may cancel at any time before execution, or approve immediately
via `approve_recovery_admin` without waiting out the timelock.

### Types

```rust
pub enum RecoveryStatus {
    None,
    Pending,
    Executed,
    Cancelled,
}

pub struct RecoveryRequest {
    pub new_owner: Address,
    pub initiated_at: u32,
    pub executable_at: u32,
    pub expires_at: u32,
    pub status: RecoveryStatus,
    pub approvals: Vec<Address>,
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `RECOVERY_TIMELOCK` | 17,280 ledgers (~24h) | Minimum delay between `initiate_recovery` and `execute_recovery`. Stable ABI — encoded in `rec_init` event payloads. |
| `RECOVERY_EXPIRY` | 120,960 ledgers (~7d) | Window after which a pending request is stale and may be overwritten by a new `initiate_recovery` call. Stable ABI. |
| `MAX_GUARDIANS` | 16 | Maximum guardians per instance |
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to |

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `owner: Address, guardians: Vec<Address>, quorum_threshold: u32` | `Result<(), RecoveryError>` | Set owner, guardian set, and quorum threshold; can only be called once; `quorum_threshold` must be >= 1 and <= guardians.len() |
| `initiate_recovery` | `guardian: Address, new_owner: Address` | `Result<(), RecoveryError>` | Guardian-authorized; records the initiating guardian as the first approval; rejects if a non-expired recovery is already pending |
| `approve_recovery` | `guardian: Address` | `Result<(), RecoveryError>` | Guardian-authorized; adds approval to the pending request; rejects duplicates |
| `cancel_recovery` | — | `Result<(), RecoveryError>` | Owner-authorized; cancels a pending request at any time |
| `execute_recovery` | `guardian: Address` | `Result<(), RecoveryError>` | Guardian-authorized; transfers ownership once `executable_at` has passed and before `expires_at` |
| `approve_recovery_admin` | — | `Result<(), RecoveryError>` | Owner-authorized; immediately executes a pending recovery without waiting for the timelock |
| `add_guardian` | `guardian: Address` | `Result<(), RecoveryError>` | Owner-authorized; capped at `MAX_GUARDIANS` |
| `remove_guardian` | `guardian: Address` | `Result<(), RecoveryError>` | Owner-authorized; rejects if it would leave zero guardians |
| `set_quorum_threshold` | `threshold: u32` | `Result<(), RecoveryError>` | Owner-authorized; must be >= 1 and <= guardian count; emits `qrm_set` |
| `owner` | — | `Result<Address, RecoveryError>` | Return current owner |
| `guardians` | — | `Result<Vec<Address>, RecoveryError>` | Return the guardian set |
| `recovery_status` | — | `RecoveryStatus` | Return the current recovery lifecycle state (`None` if no request has ever been made) |
| `recovery_request` | — | `Option<RecoveryRequest>` | Return the full recovery request record, or `None` |
| `set_registry` | `owner: Address, registry_id: Address` | `Result<(), RecoveryError>` | Owner-authorized; the passed `owner` must equal the stored owner (`Unauthorized` otherwise); link a registry contract address for off-chain discovery |
| `registry_id` | — | `Option<Address>` | Return the linked registry address, or `None` if not set |
| `quorum_threshold` | — | `u32` | Return the current M-of-N quorum threshold |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `owner: Address` | Contract initialized |
| `rec_init` | `(guardian, new_owner, initiated_at, executable_at, expires_at)` | `initiate_recovery` succeeds |
| `rec_appr` | `(guardian: Address, approval_count: u32)` | `approve_recovery` succeeds |
| `rec_exec` | `(guardian: Address, new_owner: Address)` | `execute_recovery` succeeds |
| `rec_adm` | `new_owner: Address` | `approve_recovery_admin` succeeds |
| `qrm_set` | `threshold: u32` | `set_quorum_threshold` succeeds |
| `rec_cncl` | `()` | `cancel_recovery` succeeds |
| `grd_add` | `guardian: Address` | `add_guardian` succeeds |
| `grd_rm` | `guardian: Address` | `remove_guardian` succeeds |
| `reg_link` | `registry_id: Address` | `set_registry` succeeds |

### Errors

| Variant | Code | Description |
|---|---|---|
| `NotInitialized` | 1 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | `initialize` called more than once |
| `Unauthorized` | 3 | Caller is not a registered guardian |
| `RecoveryAlreadyPending` | 4 | A non-expired recovery request already exists |
| `NoActiveRecovery` | 5 | No pending recovery request exists |
| `TimelockNotExpired` | 6 | `execute_recovery` called before `executable_at` |
| `TooManyGuardians` | 7 | Guardian set has reached `MAX_GUARDIANS` (16) |
| `GuardianAlreadyExists` | 8 | `add_guardian` called with an existing guardian |
| `GuardianNotFound` | 9 | `remove_guardian` called with an address not in the set |
| `MinGuardiansRequired` | 10 | `remove_guardian` would leave zero guardians |
| `RecoveryExpired` | 11 | `execute_recovery` called after `expires_at` |
| `QuorumNotReached` | 12 | `execute_recovery` called with fewer approvals than `quorum_threshold` |
| `DuplicateApproval` | 13 | A guardian approved the same request twice |
| `InvalidQuorumThreshold` | 14 | `initialize`/`set_quorum_threshold` given 0 or a threshold exceeding the guardian count |

---

## mux-policy

Per-wallet daily spend limit policy contract. The daily counter resets
automatically once the configured `day_ledgers` window has elapsed.

### Types

```rust
pub struct DailyLimit {
    pub limit: i128,
    pub spent: i128,
    pub reset_ledger: u32,
    pub day_ledgers: u32,
    pub registry_id: Option<Address>,
}
```

### Constants

| Constant | Value | Description |
|---|---|---|
| `MAX_WALLETS` | 256 | Maximum wallets with a configured limit |
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to |

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `admin: Address` | `Result<(), MuxPolicyError>` | Set admin; can only be called once |
| `upgrade` | `new_wasm_hash: BytesN<32>` | `Result<(), MuxPolicyError>` | Admin-authorized; upgrades the contract WASM via `update_current_contract_wasm` |
| `set_daily_limit` | `wallet: Address, limit: i128, day_ledgers: u32, registry_id: Option<Address>` | `Result<(), MuxPolicyError>` | Admin-authorized; set/replace a wallet's daily limit (capped at `MAX_WALLETS`) |
| `get_daily_limit` | `wallet: Address` | `Result<DailyLimit, MuxPolicyError>` | Return the current record, with `spent` reported as `0` if the day window has elapsed (does not persist the reset) |
| `record_spend` | `wallet: Address, amount: i128` | `Result<(), MuxPolicyError>` | Wallet-authorized; resets the window if elapsed, then debits `amount` against the limit |
| `reset_daily_counter` | `wallet: Address` | `Result<(), MuxPolicyError>` | Admin-authorized; manually resets `spent` to 0 and starts a fresh window |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `admin: Address` | Contract initialized |
| `lmt_set` | `(wallet: Address, limit: i128, day_ledgers: u32)` | `set_daily_limit` succeeds |
| `spent` | `(wallet: Address, amount: i128)` | `record_spend` succeeds |
| `ctr_rst` | `wallet: Address` | `reset_daily_counter` succeeds |

### Errors

| Variant | Code | Description |
|---|---|---|
| `NotInitialized` | 1 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | `initialize` called more than once |
| `Unauthorized` | 3 | Caller is not the admin |
| `LimitNotFound` | 4 | No daily limit configured for the wallet |
| `LimitExceeded` | 5 | Spend would exceed the daily limit |
| `InvalidAmount` | 6 | Limit or spend amount is zero or negative |
| `InvalidPeriod` | 7 | `day_ledgers` is zero |
| `TooManyWallets` | 8 | Registry has reached `MAX_WALLETS` (256) |

---

## mux-delegation

Scoped delegate permission management. An owner grants a named permission set
(`Vec<Symbol>`) to a delegate address; granting again for the same
`(owner, delegate)` pair fully replaces the prior set (no append mode).

### Constants

| Constant | Value | Description |
|---|---|---|
| `MAX_DELEGATE_PERMS` | 64 | Maximum permissions per `(owner, delegate)` pair |
| `MAX_DELEGATES_PER_OWNER` | 128 | Maximum delegate addresses per owner |
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to |

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `grant_delegate` | `owner: Address, delegate: Address, permissions: Vec<Symbol>` | `Result<(), MuxDelegationError>` | Owner-authorized; replaces any prior grant for the pair |
| `revoke_delegate` | `owner: Address, delegate: Address` | `Result<(), MuxDelegationError>` | Owner-authorized; removes the grant and the delegate from the owner's list |
| `get_delegate_permissions` | `owner: Address, delegate: Address` | `Vec<Symbol>` | Return granted permissions, or an empty vec if no grant exists |
| `is_delegate` | `owner: Address, delegate: Address, permission: Symbol` | `bool` | Return whether `permission` is granted to `delegate` |
| `get_delegates` | `owner: Address` | `Vec<Address>` | Return all delegate addresses registered under `owner` |
| `check_delegate` | `owner: Address, delegate: Address, permission: Symbol` | `Result<(), MuxDelegationError>` | Read-only; `Ok(())` if granted, `Err(NotADelegate)` otherwise |
| `link_contract_id` | `admin: Address, contract_id: Address` | `Result<(), MuxDelegationError>` | Admin-authorized; write-once storage of this contract's own address for registry discovery |
| `get_contract_id` | — | `Option<Address>` | Return the linked contract address, or `None` if not yet set |

### Events

| Topic | Data | Condition |
|---|---|---|
| `dlg_grant` | `(owner: Address, delegate: Address)` | `grant_delegate` succeeds |
| `dlg_rev` | `(owner: Address, delegate: Address)` | `revoke_delegate` succeeds |
| `dlg_link` | `(admin: Address, contract_id: Address)` | `link_contract_id` succeeds |

### Errors

Error codes 6001–6005 are stable ABI — coordinate changes with a registry version bump.

| Variant | Code | Description |
|---|---|---|
| `NotADelegate` | 6001 | No grant exists for the `(owner, delegate)` pair |
| `TooManyPermissions` | 6002 | `permissions` exceeds `MAX_DELEGATE_PERMS` (64) |
| `EmptyPermissions` | 6003 | `permissions` is empty |
| `TooManyDelegates` | 6004 | Owner has reached `MAX_DELEGATES_PER_OWNER` (128) |
| `ContractIdAlreadySet` | 6005 | `link_contract_id` called after the address was already set |

---

For full source, see the `contracts/` directory. TypeScript clients are in `bindings/src/generated/`.
