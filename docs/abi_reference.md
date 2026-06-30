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
| `TTL_THRESHOLD` | 17,280 | ~1 day — TTL extension trigger (ledgers) |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — TTL extended to (ledgers) |

### Methods

| Method | Args | Returns | Auth | Description |
|---|---|---|---|---|
| `deploy_account` | `owner: Address, account_address: Address` | `Result<Address, MuxAccountFactoryError>` | `owner` | Register a new account. Returns the registered address. |
| `deploy_account_with_metadata` | `owner: Address, account_address: Address, version: String, description: String, author: String` | `Result<Address, MuxAccountFactoryError>` | `owner` | Register a new account and store metadata. |
| `get_accounts` | `owner: Address` | `Vec<Address>` | none | Return all accounts registered for `owner`. |
| `get_account_metadata` | `owner: Address, account_address: Address` | `Result<AccountMetadata, MuxAccountFactoryError>` | none | Return stored metadata for a specific account. |
| `account_count` | — | `u64` | none | Return the total number of accounts registered across all owners. |

### Events

| Topic | Data | Condition |
|---|---|---|
| `deployed` | `(owner: Address, account_address: Address)` | Every successful `deploy_account` or `deploy_account_with_metadata` call |

### Errors

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `Unauthorized` | 1 | 401 | Caller is not the `owner` |
| `InvalidAccount` | 2 | 400 | `account_address` equals `owner` |
| `TooManyAccounts` | 3 | 409 | Owner has reached `MAX_ACCOUNTS_PER_OWNER` (64) |
| `MetadataNotFound` | 4 | 404 | No metadata stored for the specified owner/account pair |

### Notes

- `deploy_account` and `deploy_account_with_metadata` require `owner.require_auth()`.
- Instance storage TTL is extended on every write (`deploy_account*`); read-only calls do not extend TTL.
- The per-owner cap of 64 accounts prevents unbounded growth of the `Accounts` storage vector (see `docs/storage-griefing.md`).

---

## mux-batcher

### Types

```rust
pub struct Operation {
    pub target: Address,
    pub fn_name: Symbol,
    pub args: Vec<Val>,
    pub require_success: bool,
}

pub struct BatchResult {
    pub success_count: u32,
    pub failure_count: u32,
    pub errors: Vec<Bytes>,
}
```

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `execute_batch` | `caller: Address, ops: Vec<Operation>` | `Result<BatchResult, MuxBatcherError>` | Execute a batch of cross-contract calls atomically |
| `simulate_batch` | `caller: Address, ops: Vec<Operation>` | `Result<BatchResult, MuxBatcherError>` | Preflight check — no state written |
| `max_batch_size` | — | `u32` | Returns the maximum allowed operations per batch (50) |

### Events

| Topic | Data | Condition |
|---|---|---|
| `executed` | `(caller, success_count, failure_count)` | Every successful `execute_batch` call |
| `bat_ok` | `(caller, success_count)` | All operations succeeded (`failure_count == 0`) |
| `bat_abort` | `caller` | A `require_success=true` operation failed |

### Errors

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `EmptyBatch` | 1 | 400 | `ops` vector is empty |
| `BatchTooLarge` | 2 | 400 | `ops.len() > 50` |
| `RequiredOperationFailed` | 3 | 500 | A `require_success=true` op failed |
| `Unauthorized` | 4 | 401 | Reserved for future per-op auth checks |
| `ReentrancyDetected` | 5 | 409 | A batched op re-entered `execute_batch` |

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
    pub expiry_ledger: u32,
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
| `set_delegate` | `delegate: Address, expiry_ledger: u32, can_spend: bool` | `Result<(), MuxAccountError>` | Add or update a delegate (max 64); owner-only |
| `remove_delegate` | `delegate: Address` | `Result<(), MuxAccountError>` | Remove a delegate; owner-only |
| `set_spend_limit` | `asset: Address, amount: i128, period_ledgers: u32` | `Result<(), MuxAccountError>` | Set per-asset spend limit; owner-only |
| `debit_spend` | `asset: Address, spend: i128` | `Result<(), MuxAccountError>` | Check and debit a spend against the limit; contract-only |
| `owner` | — | `Result<Address, MuxAccountError>` | Return current owner |
| `delegates` | — | `Result<Map<Address, DelegateInfo>, MuxAccountError>` | Return all active (non-expired) delegates |
| `get_delegate` | `delegate: Address` | `Result<DelegateInfo, MuxAccountError>` | Return delegate info if currently active |
| `guardians` | — | `Result<Vec<Address>, MuxAccountError>` | Return guardian set |
| `execute_with_session` | `session_key: Address, payload: Bytes` | `Result<Bytes, MuxAccountError>` | Execute a payload via an authorized session key (stub — registry integration pending) |

### Events

| Topic | Data | Condition |
|---|---|---|
| `init` | `owner: Address` | Contract initialized |
| `unpaused` | `()` | Contract unpaused |
| `dlg_set` | `(delegate: Address, expiry_ledger: u32, can_spend: bool)` | Delegate added or updated |
| `dlg_rm` | `delegate: Address` | Delegate removed |
| `lmt_set` | `(asset: Address, amount: i128, period_ledgers: u32)` | Spend limit set |
| `debited` | `(asset: Address, spend: i128)` | Spend debited |
| `ses_exe` | `(session_key: Address, payload: Bytes)` | Session key execution |

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
| `propose_admin` | `new_admin: Address` | `Result<(), MuxPermissionsError>` | Propose a new admin candidate (admin-only, idempotent) |
| `approve_admin` | `approver: Address, new_admin: Address` | `Result<(), MuxPermissionsError>` | Approve a pending admin; promotes when threshold reached (admin-only) |
| `get_pending_admins` | — | `Vec<Address>` | Return all pending admin candidates |

### Methods — TTL Management

| Method | Args | Returns | Description |
|---|---|---|---|
| `bump_ttl` | — | `()` | Extend instance storage TTL; callable by anyone (keepers, bots) |
| `ttl_config` | — | `(u32, u32)` | Return `(TTL_THRESHOLD, TTL_EXTEND_TO)` constants |

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

---

## mux-wallet-registry

Maps symbolic names (`Symbol`) to wallet addresses. One owner is set at deploy
time and is the only account permitted to write entries. Reads are open to any
caller.

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `owner: Address` | `Result<(), WalletRegistryError>` | Record the owner; must be called once before any other method. Owner auth required. |
| `register_wallet` | `name: Symbol, wallet: Address` | `Result<(), WalletRegistryError>` | Register or overwrite the address stored under `name`. Owner auth required. |
| `get_wallet` | `name: Symbol` | `Result<Address, WalletRegistryError>` | Return the address registered under `name`. No auth required. |

### Errors

| Variant | Code | Description |
|---|---|---|
| `NotInitialized` | 1 | `initialize` has not been called; owner is unknown. |
| `AlreadyInitialized` | 2 | `initialize` was called a second time on the same instance. |
| `Unauthorized` | 3 | Reserved. Auth failures are surfaced as host errors by `Address::require_auth`. |
| `WalletNotFound` | 4 | No wallet is registered under the requested name. |

---

For full source, see the `contracts/` directory. TypeScript clients are in `bindings/src/generated/`.
