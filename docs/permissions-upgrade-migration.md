# mux-permissions Upgrade Migration Notes

This document covers storage-layout and admin-state considerations when
upgrading the `mux-permissions` contract to a new WASM build.

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
ledgers ≈ 30 days). An upgrade does **not** automatically extend the TTL.
If the contract is close to expiry, call any write function (e.g.,
`set_admin_threshold`) immediately after upgrading to reset the TTL.
