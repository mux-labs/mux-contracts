# mux-batcher — Simulate Batch

**Version:** 0.1.0
**Date:** 2026-07-25
**Related:** [Batching Limits](batching-limits.md) · [Storage Griefing Notes](storage-griefing.md)

---

## Overview

`simulate_batch` provides a read-only preflight check for batch operations. It validates input constraints and returns a conservative estimate of batch results **without invoking any target contracts** or writing any state.

This is useful for:

- **Preflight estimation:** TypeScript clients and off-chain orchestrators can validate batch shape and estimate outcomes before submitting the real `execute_batch` call.
- **Fee estimation:** Combine with `estimate_fees` to show users the expected cost before execution.
- **Dry-run analytics:** Indexers and dashboards can observe simulated batches via the `sim_done` event to track preflight activity separately from real execution.

---

## Function Signature

```rust
pub fn simulate_batch(
    env: Env,
    caller: Address,
    ops: Vec<Operation>,
) -> Result<BatchResult, MuxBatcherError>
```

### Parameters

| Parameter | Type | Description |
|---|---|---|
| `caller` | `Address` | Address of the account authorising the simulation. `caller.require_auth()` is enforced. |
| `ops` | `Vec<Operation>` | Operations to simulate. Same `Operation` type used by `execute_batch`. |

### Returns

| Variant | Description |
|---|---|
| `Ok(BatchResult)` | Conservative estimate: `success_count = ops.len()`, `failure_count = 0`, `errors = []` |
| `Err(EmptyBatch)` | `ops` is empty |
| `Err(BatchTooLarge)` | `ops.len() > MAX_BATCH_SIZE` (50) |

---

## Behaviour

1. **Auth check:** `caller.require_auth()` — the caller must authorise the simulation, same as `execute_batch`.
2. **Input validation:** Checks for empty batch and batch size limit (same as `execute_batch`).
3. **No contract invocations:** Target contracts are **not** called. The result assumes all operations succeed.
4. **No state writes:** No instance or persistent storage is modified.
5. **Event emission:** Emits a `sim_done` event with `(caller, success_count)` so off-chain tooling can observe simulations.

### Key difference from `execute_batch`

| Aspect | `execute_batch` | `simulate_batch` |
|---|---|---|
| Invokes target contracts | Yes | **No** |
| Writes state | Yes (reentrancy guard, TTL) | **No** |
| Reports actual success/failure | Yes | **No** (assumes all succeed) |
| Emits events | `bat_start`, `executed`, `bat_ok`/`bat_abort` | `sim_done` only |
| Extends TTL | Yes (on success) | **No** |

---

## Usage Patterns

### Rust

```rust
use soroban_sdk::{symbol_short, vec, Address, Env};

let ops: Vec<Operation> = vec![
    &env,
    Operation {
        target: token_address.clone(),
        fn_name: symbol_short!("transfer"),
        args: vec![&env, from.into_val(&env), to.into_val(&env), amount.into_val(&env)],
        require_success: true,
        kind: BatchOperationKind::Transfer,
    },
];

// Preflight check
let sim = batcher_client.simulate_batch(&caller, &ops);
match sim {
    Ok(result) => {
        // result.success_count == 1, result.failure_count == 0
        // Proceed with execute_batch if the shape looks correct
    }
    Err(e) => {
        // Handle EmptyBatch or BatchTooLarge
    }
}
```

### TypeScript

```typescript
import { contract } from "@mux-protocol/contracts";

const ops = [
  {
    target: tokenAddress,
    fnName: "transfer",
    args: [from, to, amount],
    requireSuccess: true,
    kind: "Transfer",
  },
];

// Preflight check
const sim = await contract.simulateBatch(caller, ops);
if (sim.result) {
  console.log(`Simulated ${sim.result.successCount} ops`);
  // Proceed with executeBatch
}
```

### Combining with fee estimation

```rust
// Estimate fees for a 3-op batch
let fee = batcher_client.estimate_fees(&3)?;
// Simulate to validate shape
let sim = batcher_client.simulate_batch(&caller, &ops)?;
// Both passed — safe to execute
let result = batcher_client.execute_batch(&caller, &ops)?;
```

---

## Events

| Event topic | Data | Description |
|---|---|---|
| `sim_done` | `(caller, success_count)` | Emitted after successful simulation |

The `sim_done` event is distinct from `executed` (emitted by `execute_batch`), allowing indexers to separate simulated batches from real executions.

---

## Error Codes

| Code | Value | Condition |
|---|---|---|
| `EmptyBatch` | 1 | `ops` vector is empty |
| `BatchTooLarge` | 2 | `ops.len() > 50` |

---

## Limitations

1. **No actual execution:** `simulate_batch` does not call target contracts, so it cannot detect runtime errors (e.g., insufficient balance, unauthorized access on the target). For true dry-run semantics, use Soroban's `Simulation` RPC endpoint.

2. **Conservative estimate:** The returned `BatchResult` always reports all operations as successful. Callers should not rely on the result to predict actual success/failure rates.

3. **No TTL extension:** Since no state is written, the contract TTL is not extended by `simulate_batch`. Operators should not rely on simulation calls to keep the contract alive.

4. **Auth required:** Even though no state is written, `caller.require_auth()` is enforced. This prevents unauthorized callers from spamming simulations and polluting the event log.

---

## Threat Cross-Reference

| Threat ID | Description | Mitigation |
|---|---|---|
| T-BATCH-01 | Caller submits oversized batch to exhaust ledger resources | `MAX_BATCH_SIZE = 50` enforced |
| T-SIM-01 | Attacker spams simulate to pollute event log | `caller.require_auth()` enforced |
