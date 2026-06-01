# Mux Protocol — Batching Limits

**Version:** 0.1.0
**Date:** 2026-05-31
**Related:** [Storage Griefing Notes](storage-griefing.md) · [Threat Model](threat-model.md)

---

## Overview

`mux-batcher` executes a sequence of cross-contract calls as a single Soroban
transaction.  To prevent resource exhaustion and protect ledger capacity for all
users, the contract enforces hard limits on the number and shape of operations
that may be submitted in one batch.

---

## Batch size limit

| Constant | Value | Reason |
|---|---|---|
| `MAX_BATCH_SIZE` | **50** | Bounds CPU instructions, memory, and fee usage per transaction |

### Enforcement

Both `execute_batch` and `simulate_batch` check the limit before any operations
are invoked:

```rust
if ops.len() > MAX_BATCH_SIZE {
    return Err(MuxBatcherError::BatchTooLarge);
}
```

Callers can query the current limit at runtime without needing to hard-code it:

```rust
let limit: u32 = batcher_client.max_batch_size();
```

### Why 50?

Soroban imposes per-transaction CPU instruction and memory budgets.  A batch of
cross-contract calls is among the most expensive transaction types because each
`try_invoke_contract` creates a new execution frame.  The cap of 50 was chosen
as a conservative upper bound that:

- Stays well within mainnet resource limits under worst-case argument sizes.
- Prevents a single caller from monopolising ledger throughput.
- Leaves headroom for future protocol-level instruction budget increases.

---

## Operation kind

Each `Operation` carries a `kind: BatchOperationKind` field that classifies its
intent.  The batcher does **not** gate execution on the kind — it is purely
informational metadata surfaced in events and available to off-chain indexers,
analytic pipelines, and TypeScript clients.

| Variant | Description |
|---|---|
| `Invoke` | Generic cross-contract function call (default / catch-all) |
| `Transfer` | Asset transfer (e.g. SAC `transfer` call) |
| `Approve` | Allowance / approval (e.g. SAC `approve` call) |

**Rust usage:**

```rust
Operation {
    target,
    fn_name: symbol_short!("transfer"),
    args,
    require_success: true,
    kind: BatchOperationKind::Transfer,
}
```

**TypeScript usage:**

```typescript
import type { BatchOperationKind, Operation } from "@mux-protocol/contracts";

const op: Operation = {
  target: addr,
  fnName: "transfer",
  args: [],
  requireSuccess: true,
  kind: "Transfer",
};
```

---

## Error codes

| Code | Value | Condition |
|---|---|---|
| `EmptyBatch` | 1 | `ops` vector is empty |
| `BatchTooLarge` | 2 | `ops.len() > MAX_BATCH_SIZE` (> 50) |
| `RequiredOperationFailed` | 3 | An op with `require_success = true` failed |
| `Unauthorized` | 4 | Reserved for future per-op authorization checks |
| `ReentrancyDetected` | 5 | A batched op attempted to call back into `mux-batcher` |

---

## Rollback behaviour

Soroban provides two rollback paths.  Understanding which path fires is
important for callers that depend on atomic all-or-nothing semantics.

### Host-level trap (panic)

If the Soroban host encounters an unrecoverable error (e.g. the contract itself
panics), **all** storage writes made during the invocation are discarded and
the transaction is marked as failed.  No events are committed to the ledger.

### Contract-level error (return `Err(...)`)

When `execute_batch` returns `Err(RequiredOperationFailed)`, the contract
function has returned normally with an error value.  The Soroban host does
**not** automatically roll back instance-storage writes for contract-level
errors.  `mux-batcher` therefore:

1. Emits a `bat_abort` event before returning so callers can observe the abort.
2. Explicitly removes the reentrancy guard (`DataKey::Executing`) to leave the
   contract in a clean state for subsequent calls.

No `executed` event is emitted when the batch aborts.

### All-or-nothing usage pattern

To get atomic semantics set `require_success = true` on **every** operation:

```rust
Operation {
    target,
    fn_name,
    args,
    require_success: true,   // abort entire batch if this op fails
}
```

A single failure will return `Err(RequiredOperationFailed)` and no partial
results are observable via events.

---

## Instance-storage TTL

`execute_batch` calls `extend_ttl` on every successful invocation (see
[Storage Griefing Notes](storage-griefing.md#ttl-auto-extension)):

| Constant | Value | Approximate duration |
|---|---|---|
| `TTL_THRESHOLD` | 17,280 ledgers | ~1 day |
| `TTL_EXTEND_TO` | 518,400 ledgers | ~30 days |

When a batch **fails** (returns an error) the TTL is not extended.  Operators
should ensure a keeper job extends the TTL externally if the contract goes idle
for extended periods.

---

## Reentrancy protection

`mux-batcher` maintains a boolean `Executing` flag in instance storage.  If a
batched cross-contract call attempts to re-enter `execute_batch` on the same
contract instance, the second call returns `Err(ReentrancyDetected)` immediately
without processing any operations.

The flag is cleared:

- After the batch loop completes successfully.
- Before returning `Err(RequiredOperationFailed)` on the abort path.

This ensures the flag never remains set after a call returns, regardless of
outcome.

---

## Threat cross-reference

| Threat ID | Description | Mitigation |
|---|---|---|
| T-BATCH-01 | Caller submits oversized batch to exhaust ledger resources | `MAX_BATCH_SIZE = 50` enforced on every entry point |
| T-BATCH-02 | Batched op re-enters `execute_batch` | `DataKey::Executing` reentrancy guard |
| T-21 | Instance storage TTL expiry | `extend_ttl` on every successful `execute_batch` call |
