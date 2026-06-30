# mux-account Upgrade Migration Notes

This document covers storage-layout and owner-state considerations when
upgrading the `mux-account` contract to a new WASM build.

## General Upgrade Pattern

Soroban contracts are upgraded by uploading new WASM to the ledger and calling
`upgrade()` on the live instance. See
[docs/contract-upgrade-pattern.md](./contract-upgrade-pattern.md) for the
general procedure.

> **Note:** `mux-account` does not yet expose an on-chain `upgrade()` entry
> point. Deployments are currently immutable. This document describes the
> migration rules to follow when upgrade support is added.

## Storage Layout

`mux-account` uses **instance storage** for all state:

| Key | Type | Notes |
|-----|------|-------|
| `DataKey::Owner` | `Address` | Account owner — preserved across upgrades |
| `DataKey::Delegates` | `Map<Address, DelegateInfo>` | Active delegate set |
| `DataKey::SpendLimit(Address)` | `SpendLimit` | Per-asset spend limits |
| `DataKey::GuardianSet` | `Vec<Address>` | Recovery guardians |
| `DataKey::Nonce` | `u64` | Transaction counter |
| `DataKey::SessionKey(Address, Address)` | `SessionKeyRecord` | Session key records |
| `DataKey::SessionKeyIndex(Address)` | `Vec<Address>` | Session key index per owner |
| `DataKey::Paused` | `bool` | Pause flag |
| `DataKey::Executing` | `bool` | Reentrancy guard |
| `DataKey::Metadata` | `RegistryMeta` | Optional registry metadata (additive) |

**Instance storage is preserved across WASM upgrades** — existing owner,
delegates, spend limits, and guardians are not affected by the upgrade itself.

## Migration Steps

1. **Build and upload the new WASM** (see contract-upgrade-pattern.md).

2. **Call `upgrade()`** on the live instance with the new WASM hash.
   The account owner must authorise this call once upgrade support is added.

3. **No `migrate()` call is required** for additive changes such as the
   optional `DataKey::Metadata` field. Reads fall back to `None` when metadata
   has not been set.

4. **Verify storage is intact** by reading key state after the upgrade:
   ```bash
   stellar contract invoke --id $ACCOUNT_CONTRACT_ID \
     --network $NETWORK -- owner
   stellar contract invoke --id $ACCOUNT_CONTRACT_ID \
     --network $NETWORK -- delegates
   ```

5. **Re-run smoke tests** to confirm delegate lookup, spend limits, and pause
   state behave as expected.

## Breaking Changes to Watch For

### Adding a New `DataKey` Variant

Adding a variant to `DataKey` is **non-breaking** — existing keys are
unaffected. Ensure the new variant has a distinct discriminant value.

### Removing or Renaming a `DataKey` Variant

Removing or renaming a variant is a **breaking storage change**: existing
on-chain values stored under the old key become unreachable. If this is
necessary:

1. Bump the major contract version.
2. Add a one-time migration function that reads the old key and writes
   to the new key.
3. Call the migration function in the same transaction as `upgrade()`.

### Changing `MAX_DELEGATES`

Lowering this constant is a **breaking change** if existing data already
exceeds the new cap. Raising it is safe.

### Owner and Delegate State During Upgrade

The `Owner` key and all delegate entries are preserved. Pending session keys
and spend-limit counters remain readable. If an upgrade happens while the
contract is paused, the pause flag persists until the owner calls `unpause()`.

## TTL Considerations

Instance storage TTL is extended on every write (`TTL_EXTEND_TO = 518_400`
ledgers ≈ 30 days). An upgrade does **not** automatically extend the TTL.
If the contract is close to expiry, call any write function (e.g.,
`set_delegate`) immediately after upgrading to reset the TTL.

## Rollback

Call `upgrade()` with the prior WASM hash. No storage migration is needed for
rollback unless a `migrate()` function was executed post-upgrade — in that case,
prepare and test a reverse migration before deploying.
