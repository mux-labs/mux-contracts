# mux-batcher — Gas & Fee Helper

**Version:** 0.1.0
**Date:** 2026-07-27
**Related:** [Batching Limits](batching-limits.md) · [Simulate Batch](simulate-batch.md) · [Threat Model](threat-model.md)

---

## Overview

`estimate_fees` is a pure, read-only function that returns a conservative
fee estimate (in stroops) for a batch of a given size.  It lets TypeScript
clients and off-chain orchestrators compute expected costs **before**
constructing or submitting a real transaction. It is exposed in the TypeScript
bindings as `MuxBatcherClient.estimateFees`.

Combining `estimate_fees` with `simulate_batch` gives a complete preflight
picture: shape validation + cost projection, with no on-chain side effects.

---

## Fee model

| Constant | Value | Meaning |
|---|---|---|
| `FEE_PER_OP` | **100 stroops** | Base fee charged per operation |
| `MAX_BATCH_SIZE` | **50** | Maximum operations per batch (also the upper bound for `estimate_fees`) |

The fee is calculated as:

```
estimated_fee = op_count × FEE_PER_OP
```

This is a **conservative lower bound**.  The actual on-chain fee depends on
Soroban's resource model (CPU instructions, memory, ledger writes) and the
current fee market.  Use the Soroban RPC `simulateTransaction` endpoint for a
precise fee quote when precision matters.

### Why a constant multiplier?

The per-operation cost is dominated by the `try_invoke_contract` call overhead,
which is roughly proportional to the number of operations.  A fixed multiplier
keeps the estimate simple, deterministic, and auditable, while giving callers a
safe floor for displaying estimated costs in UIs.

---

## Function signature

```rust
/// Estimate the fee (in stroops) for a batch of the given size.
///
/// Returns `Err(EmptyBatch)` when `op_count` is zero.
/// Returns `Err(BatchTooLarge)` when `op_count` exceeds `max_batch_size`.
pub fn estimate_fees(_env: Env, op_count: u32) -> Result<u32, MuxBatcherError>
```

### Parameters

| Parameter | Type | Description |
|---|---|---|
| `op_count` | `u32` | Number of operations in the planned batch |

### Returns

| Variant | Description |
|---|---|
| `Ok(u32)` | Estimated fee in stroops: `op_count × FEE_PER_OP` |
| `Err(EmptyBatch)` | `op_count` is zero |
| `Err(BatchTooLarge)` | `op_count > MAX_BATCH_SIZE` (50) |

### Properties

- **Pure computation** — no state is read or written; `_env` is unused.
- **No auth required** — callable by any account without authorization.
- **No events emitted** — does not publish to the event log.
- **No TTL extension** — since no storage is touched, the contract TTL is not extended.

---

## Error codes

| Code | Value | HTTP | Condition |
|---|---|---|---|
| `EmptyBatch` | 1 | 400 | `op_count == 0` |
| `BatchTooLarge` | 2 | 400 | `op_count > MAX_BATCH_SIZE` (50) |

---

## Usage patterns

### Rust

```rust
use soroban_sdk::Env;

// Estimate fees for a 5-op batch
let fee = batcher_client.estimate_fees(&5_u32)?;
// fee == 500 stroops (5 × 100)

// Validate the batch size before constructing operations
let op_count: u32 = 10;
let fee = batcher_client.estimate_fees(&op_count)?;
// fee == 1_000 stroops
```

Full preflight flow (estimate → simulate → execute):

```rust
// 1. Estimate fees
let fee = batcher_client.estimate_fees(&(ops.len() as u32))?;
log::info!("Estimated fee: {} stroops", fee);

// 2. Simulate shape
let sim = batcher_client.simulate_batch(&caller, &ops)?;
// sim.success_count == ops.len(), sim.failure_count == 0

// 3. Execute
let result = batcher_client.execute_batch(&caller, &ops)?;
```

### TypeScript

```typescript
import { MuxBatcherClient } from "@mux-protocol/contracts";

const client = new MuxBatcherClient({ contractId, networkPassphrase, rpcUrl });

// Estimate fees — pure read, no transaction submitted
const stroops = await client.estimateFees(signer, operations.length);
console.log(`Estimated fee: ${stroops} stroops`);
// → "Estimated fee: 500 stroops" for 5 operations

// Full preflight before executing
async function preflight(ops: Operation[]): Promise<void> {
  // Throws if ops.length === 0 or > 50
  const fee = await client.estimateFees(signer, ops.length);
  console.log(`Fee estimate: ${fee} stroops`);

  // Validate shape (no contracts called, no state written)
  const sim = await client.simulateBatch(signer, callerAddress, ops);
  console.log(`Simulated ${sim.successCount} ops`);

  // All clear — execute
  const result = await client.executeBatch(signer, callerAddress, ops);
  console.log(`Batch done: ${result.successCount} succeeded, ${result.failureCount} failed`);
}
```

---

## Combining estimate_fees with max_batch_size

Always query `max_batch_size()` at runtime rather than hard-coding 50.  This
ensures your client adapts automatically if the constant is raised in a future
upgrade:

```typescript
const limit = await client.maxBatchSize(signer);
const opCount = Math.min(planned.length, limit);

// Safe: opCount is always ≤ limit
const fee = await client.estimateFees(signer, opCount);
```

---

## Limitations

1. **Conservative estimate only.** The actual Soroban fee is determined by
   resource consumption (CPU instructions, memory, byte reads/writes) and the
   current ledger fee market.  `estimate_fees` returns `op_count × FEE_PER_OP`
   as a lower bound only.  Use `simulateTransaction` from the Soroban RPC for
   a precise quote.

2. **Does not account for argument complexity.** Operations with large `args`
   vectors cost more on-chain than operations with no arguments.  The estimate
   assumes a flat per-operation cost and does not scale with argument size.

3. **FEE_PER_OP is a protocol constant, not a fee-market price.** Changes to
   `FEE_PER_OP` require a contract upgrade.  Clients that cache fee estimates
   should refresh after any upgrade that bumps this constant (see
   [batcher-upgrade.md](batcher-upgrade.md#changing-fee_per_op)).

4. **No auth required, no rate limiting.** Any account can call `estimate_fees`
   without signing a transaction.  This is by design (it is purely informational),
   but callers should not use it as a substitute for on-chain validation.

---

## Threat cross-reference

| Threat ID | Description | Mitigation |
|---|---|---|
| T-BATCH-01 | Caller uses `estimate_fees` to probe the size gate before submitting an oversized batch | `estimate_fees` enforces the same `MAX_BATCH_SIZE = 50` cap, returning `Err(BatchTooLarge)` for `op_count > 50` |
| T-21 | Instance storage TTL expiry | `estimate_fees` does not touch storage, so TTL is unaffected; operators should not rely on estimation calls to keep the contract alive |
