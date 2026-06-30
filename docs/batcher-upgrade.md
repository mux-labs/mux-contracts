# mux-batcher Upgrade & Migration Notes

This document covers upgrade considerations specific to the `mux-batcher` contract.

## General Upgrade Pattern

Soroban contracts are upgraded by uploading new WASM to the ledger and calling
`upgrade()` on the live instance. See
[docs/contract-upgrade-pattern.md](./contract-upgrade-pattern.md) for the
general procedure.

## Storage Layout

`mux-batcher` uses **instance storage** only:

| Key               | Type   | Notes                                              |
|-------------------|--------|----------------------------------------------------|
| `DataKey::Executing` | `bool` | Reentrancy guard — always `false` between calls |

`DataKey::Executing` is set to `true` at the start of `execute_batch` and
removed before the function returns (including on error paths). It is never
`true` at rest between transactions. Upgrades performed between transactions
leave no guard state to clean up.

## Migration Steps

1. **Build and upload the new WASM** (see contract-upgrade-pattern.md).

2. **Call `upgrade()`** on the live instance with the new WASM hash.

3. **Verify the contract is reachable** by calling `max_batch_size`:
   ```bash
   stellar contract invoke --id $BATCHER_CONTRACT_ID \
     --network $NETWORK -- max_batch_size
   ```

4. **Re-run smoke tests** to confirm batches are accepted and the reentrancy
   guard clears correctly after execution.

## Breaking Changes to Watch For

### Changing `MAX_BATCH_SIZE`

`MAX_BATCH_SIZE` (currently `50`) is enforced at call time. Lowering it is a
**breaking change** for callers that construct batches up to the old limit —
they will receive `BatchTooLarge` after the upgrade. Raising it is safe.

### Changing `FEE_PER_OP`

`FEE_PER_OP` (currently `100` stroops) affects the `estimate_fees` return
value only. Clients that cache fee estimates should refresh after an upgrade
that changes this constant.

### Changing Error Code Values

`MuxBatcherError` discriminants (1–5) are part of the on-chain ABI. Clients
that match on numeric codes must be updated if codes change. Coordinate any
renumbering with a registry version bump and update `docs/error_codes.md`.

### Adding a New `DataKey` Variant

Adding a variant is **non-breaking** — existing keys are unaffected. Ensure
the new variant has a distinct discriminant.

### Removing or Renaming a `DataKey` Variant

This is a **breaking storage change**. Follow the standard migration pattern:
add a one-time migration function, call it in the same transaction as
`upgrade()`, and bump the major contract version.

## TTL Considerations

Instance storage TTL is extended on every successful `execute_batch` call
(`TTL_EXTEND_TO = 518_400` ledgers ≈ 30 days). An upgrade itself does **not**
extend the TTL. If the contract is near expiry, call `execute_batch` (or any
state-writing function) immediately after upgrading to reset the TTL.

## Pre-Upgrade Checklist

- [ ] Verify new WASM hash with `scripts/verify-wasm-hash.sh`
- [ ] Confirm `MAX_BATCH_SIZE` and `FEE_PER_OP` changes are intentional
- [ ] Confirm `DataKey` enum is backward-compatible
- [ ] Run all batcher unit and integration tests against the new WASM
- [ ] Update `docs/error_codes.md` if `MuxBatcherError` variants changed
- [ ] Retain prior WASM hash for rollback

## Rollback

Call `upgrade()` with the prior WASM hash. No storage migration is needed for
rollback — `DataKey::Executing` is transient and is never persisted across
transaction boundaries.
