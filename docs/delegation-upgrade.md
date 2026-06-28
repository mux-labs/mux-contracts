# Delegation Contract Upgrade & Migration Notes

This document covers upgrade considerations specific to the `mux-delegation` contract.

## Storage Layout

The delegation contract uses two persistent storage keys:

| DataKey variant | Value type | Purpose |
|---|---|---|
| `DelegatePerms(owner, delegate)` | `Vec<Symbol>` | Granted permission set |
| `OwnerDelegates(owner)` | `Vec<Address>` | All delegates for an owner |

## Migration Considerations

### Adding new permission types

New permission symbols can be granted without any migration — `Vec<Symbol>` is
open-ended. No WASM upgrade is required to introduce new permission names.

### Changing error codes

Error code values (e.g. 6001–6003) are part of the ABI. Clients that match on
numeric codes must be updated when codes change. Coordinate error code changes
with a registry version bump via `register_with_metadata`.

### Adding new DataKey variants

Follow the general [contract upgrade pattern](contract-upgrade-pattern.md):

1. Add the new variant to the `DataKey` enum — never remove or rename existing
   variants.
2. Use `Option<T>` if existing entries must deserialise without the new field.
3. Upload new WASM and call `upgrade()` on the live instance.

### Changing MAX_DELEGATE_PERMS

The `MAX_DELEGATE_PERMS` constant (currently 64) is enforced at grant time only.
Lowering it does not invalidate existing grants that exceed the new limit — they
remain readable and revocable. Raising it requires no migration.

## Pre-Upgrade Checklist

- [ ] Verify new WASM hash with `scripts/verify-wasm-hash.sh`
- [ ] Run all delegation tests against the new WASM
- [ ] Confirm `DataKey` enum is backward-compatible
- [ ] Bump version in registry via `register_with_metadata`
- [ ] Retain prior WASM hash for rollback
- [ ] Update `docs/error_codes.md` if error variants changed

## Rollback

Call `upgrade()` with the prior WASM hash. No storage migration is needed for
rollback unless a `migrate()` function was executed post-upgrade — in that case,
prepare and test a reverse migration before deploying.
