# ABI Reference

Soroban contract interfaces for Mux Protocol. All contracts are written in Rust using the Soroban SDK v21.

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

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `owner: Address, guardians: Vec<Address>` | `Result<(), MuxAccountError>` | Set owner and guardian set |
| `set_delegate` | `delegate: Address, expiry_ledger: u32, can_spend: bool` | `Result<(), MuxAccountError>` | Add or update a delegate |
| `remove_delegate` | `delegate: Address` | `Result<(), MuxAccountError>` | Remove a delegate |
| `set_spend_limit` | `asset: Address, amount: i128, period_ledgers: u32` | `Result<(), MuxAccountError>` | Set per-asset spend limit |
| `debit_spend` | `asset: Address, spend: i128` | `Result<(), MuxAccountError>` | Check and debit a spend |
| `owner` | — | `Result<Address, MuxAccountError>` | Return current owner |
| `delegates` | — | `Result<Map<Address, DelegateInfo>, MuxAccountError>` | Return all delegates |
| `guardians` | — | `Result<Vec<Address>, MuxAccountError>` | Return guardian set |

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

For full source, see the `contracts/` directory. TypeScript clients are in `bindings/src/generated/`.
