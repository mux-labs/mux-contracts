# mux-permissions Upgrade Migration Notes

This document covers storage-layout and admin-state considerations when
upgrading the `mux-permissions` contract to a new WASM build.

## Status

`mux-permissions` **does** expose an `upgrade()` entry point — see
`contracts/mux-permissions/src/lib.rs`. The function is admin-gated via
`require_admin()` (the same helper used by role and multisig-rotation
entrypoints) and returns `NotInitialized` (fail-closed) if called before
`initialize`. This document describes the full upgrade procedure.

Unit tests in the same file verify the auth gate:
- `test_upgrade_requires_admin_auth` — calling `upgrade()` without admin auth is rejected
- `test_upgrade_before_initialize_returns_not_initialized` — calling before `initialize`
  returns `NotInitialized`

## General Upgrade Pattern

Soroban contracts are upgraded by uploading new WASM to the ledger and calling
`upgrade()` on the live instance. See
[docs/contract-upgrade-pattern.md](./contract-upgrade-pattern.md) for the
general procedure.

## Storage Layout

`mux-permissions` uses **instance storage** for all state:

| Key | Type | Notes |
|-----|------|-------|
| `DataKey::Admin` | `Address` | Active admin — preserved across upgrades |
| `DataKey::RoleMembers(Symbol)` | `Vec<Address>` | Members per role |
| `DataKey::RolePermissions(Symbol)` | `Vec<Symbol>` | Permissions per role |
| `DataKey::AccountRoles(Address)` | `Vec<Symbol>` | Roles held per account |
| `DataKey::PendingAdmins` | `Vec<Address>` | Pending multisig candidates |
| `DataKey::AdminThreshold` | `u32` | Required approval count |
| `DataKey::AdminApprovals(Address)` | `Vec<Address>` | Approvals per candidate |
| `DataKey::Metadata` | `RegistryMeta` | Optional registry metadata |

**Instance storage is preserved across WASM upgrades** — existing roles,
members, and the current admin are not affected by the upgrade itself.

## Migration Steps

1. **Build and upload the new WASM** (see contract-upgrade-pattern.md).

2. **Call `upgrade()`** on the live instance with the new WASM hash.
   The active admin must authorise this call.

3. **Verify storage is intact** by reading key state after the upgrade:
   ```bash
   # Confirm admin is unchanged
   stellar contract invoke --id $PERMISSIONS_CONTRACT_ID \
     --network $NETWORK -- get_roles --account $KNOWN_ACCOUNT
   ```

4. **Re-run smoke tests** to confirm role lookup and permission checks work.

## Production Dry-Run Procedure

Before executing an upgrade on Stellar mainnet or long-lived testnets, operators must perform a full **dry-run** in a local sandbox or testnet environment.

### 1. Pre-Upgrade State Snapshot & Dry-Run Setup

Capture existing storage state for critical entities before initiating any upgrade transaction:

```bash
# Export current admin
stellar contract invoke \
  --id $PERMISSIONS_CONTRACT_ID \
  --network $NETWORK \
  --source $OPERATOR_KEY \
  -- get_metadata

# Check roles assigned to test/operational accounts
stellar contract invoke \
  --id $PERMISSIONS_CONTRACT_ID \
  --network $NETWORK \
  --source $OPERATOR_KEY \
  -- get_roles --account $TARGET_ACCOUNT

# Check members of critical system roles (e.g. symbol "Admin", "Operator")
stellar contract invoke \
  --id $PERMISSIONS_CONTRACT_ID \
  --network $NETWORK \
  --source $OPERATOR_KEY \
  -- get_role_members --role Operator
```

Record the outputs and the current ledger sequence for state comparison post-dry-run.

### 2. Transaction Simulation (Offline Dry-Run)

Simulate the upgrade transaction using Soroban RPC simulation without broadcasting on-chain:

```bash
# Perform simulation dry-run
stellar contract invoke \
  --id $PERMISSIONS_CONTRACT_ID \
  --source $ADMIN_ACCOUNT \
  --network $NETWORK \
  --simulate \
  -- upgrade \
  --new_wasm_hash $NEW_WASM_HASH
```

Verify:
- **Simulation status**: returns success (HTTP 200 / execution success).
- **Resource consumption**: CPU instructions and memory limits are within acceptable thresholds (under 80% maximum budget).
- **Footprint**: Read/write storage footprint accurately targets contract instance storage and TTL extensions without unexpected persistent keys.
- **Fail-closed checks**: If simulated without the admin signature or with an invalid key, the simulation MUST fail with authorization error / fail-closed rejection.

### 3. Dry-Run Execution on Staging/Testnet

1. Deploy the new WASM hash to the network:
   ```bash
   NEW_WASM_HASH=$(stellar contract upload \
     --wasm target/wasm32-unknown-unknown/release/mux_permissions.wasm \
     --source $DEPLOYER_ACCOUNT \
     --network $NETWORK)
   echo "Uploaded WASM hash: $NEW_WASM_HASH"
   ```

2. Execute the `upgrade` entrypoint:
   ```bash
   stellar contract invoke \
     --id $PERMISSIONS_CONTRACT_ID \
     --source $ADMIN_ACCOUNT \
     --network $NETWORK \
     -- upgrade \
     --new_wasm_hash $NEW_WASM_HASH
   ```

### 4. Post-Upgrade Invariant Verification

Run the automated verification assertions to ensure state integrity:

- **Admin Preservation**: Stored admin address matches pre-upgrade snapshot.
- **Role Continuity**: `get_roles(account)` returns identical Symbol vectors.
- **Permission Grants**: `has_permission(account, perm)` preserves true/false evaluations.
- **Pending Multi-sig Continuity**: `get_pending_admins()` preserves active threshold and pending candidates.
- **TTL Extension Verified**: Confirm instance storage TTL has been extended to `518_400` ledgers.

Verification script template:
```bash
# Check admin continuity
PRE_ADMIN="G..."
POST_ADMIN=$(stellar contract invoke --id $PERMISSIONS_CONTRACT_ID --network $NETWORK --source $OPERATOR_KEY -- get_metadata | jq -r '.admin')
if [ "$PRE_ADMIN" != "$POST_ADMIN" ]; then
  echo "CRITICAL: Admin mismatch post-upgrade!"
  exit 1
fi

# Verify permission resolution
stellar contract invoke \
  --id $PERMISSIONS_CONTRACT_ID \
  --network $NETWORK \
  --source $OPERATOR_KEY \
  -- has_permission --account $KNOWN_ACCOUNT --perm execute_batch
```

### 5. Rollback Dry-Run Procedure

If verification fails during the dry-run:
1. Re-invoke `upgrade` passing the prior WASM hash (`$PREV_WASM_HASH`).
2. Verify all reads and permission evaluations succeed on the reverted bytecode.
3. Document root cause and simulation diff before re-attempting.

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

### Changing `MAX_ROLE_MEMBERS` or `MAX_ROLES_PER_ACCOUNT`

Lowering these constants is a **breaking change** if existing data already
exceeds the new cap. Raising them is safe.

### Admin State During Upgrade

The `Admin` key in instance storage is always preserved. Pending multisig
candidates (`PendingAdmins`) and their partial approvals are also preserved.
If an upgrade happens while an admin promotion is in flight, the promotion can
still be completed after the upgrade.

## TTL Considerations

Instance storage TTL is extended on every write (`TTL_EXTEND_TO = 518_400`
ledgers ≈ 30 days), including `upgrade()` itself (T-21 mitigation) — an
upgrade performed just before a long quiet period does not leave storage at
risk of expiry on its own.

