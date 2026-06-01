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

### Methods

| Method | Args | Returns | Description |
|---|---|---|---|
| `initialize` | `admin: Address` | `Result<(), MuxPermissionsError>` | Set admin |
| `create_role` | `role: Symbol, permissions: Vec<Symbol>` | `Result<(), MuxPermissionsError>` | Create a role |
| `grant_role` | `account: Address, role: Symbol` | `Result<(), MuxPermissionsError>` | Grant role to account |
| `revoke_role` | `account: Address, role: Symbol` | `Result<(), MuxPermissionsError>` | Revoke role from account |
| `has_permission` | `account: Address, permission: Symbol` | `bool` | Check if account holds a permission |
| `get_roles` | `account: Address` | `Vec<Symbol>` | Return all roles for an account |
| `get_role_members` | `role: Symbol` | `Result<Vec<Address>, MuxPermissionsError>` | Return all members of a role |

---

For full source, see the `contracts/` directory. TypeScript clients are in `bindings/src/generated/`.
