# Delegation Contract Upgrade & Migration Notes

This document covers upgrade considerations specific to the `mux-delegation` contract.

## Admin & Initialization

`upgrade()` is admin-gated (`require_admin()` → `admin.require_auth()`), the
same pattern used by `mux-policy`, `mux-permissions`, and `mux-batcher`. The
admin is **optional**: `grant_delegate`, `revoke_delegate`, and
`link_contract_id` never required one and still do not. The admin only
exists to authorise `upgrade()`.

- A delegation contract deployed and never `initialize`d has no `upgrade()`
  path — calling `upgrade()` returns `NotInitialized`. Fail-closed: there is
  no admin to authorise a WASM replace.
- Call `initialize(admin)` once to set the admin. A second call returns
  `AlreadyInitialized`.

**Important — two independent "admin" concepts.** `link_contract_id(admin,
contract_id)` accepts an `admin: Address` *parameter* and only checks that
this specific address signed the call (`admin.require_auth()`) — it is
**not** checked against any stored identity, so any address can call it by
naming itself as `admin`. This is unrelated to the `DataKey::Admin` set by
`initialize` and used by `upgrade`. Do not assume that authorising
`link_contract_id` implies control over `upgrade`, or vice versa. See
[docs/access-control-checklist.md](access-control-checklist.md#17-mux-delegation)
for the full breakdown; unifying the two is tracked as a follow-up, not part
of this upgrade-entrypoint change.

## Storage Layout

The delegation contract uses two persistent storage keys and two instance keys:

| DataKey variant | Value type | Storage | Purpose |
|---|---|---|---|
| `DelegatePerms(owner, delegate)` | `Vec<Symbol>` | Persistent | Granted permission set |
| `OwnerDelegates(owner)` | `Vec<Address>` | Persistent | All delegates for an owner |
| `ContractId` | `Address` | Instance | Write-once self-registration address (see `link_contract_id`) |
| `Admin` | `Address` | Instance | Optional upgrade authority, set once by `initialize`. Absent unless `initialize` was called — `upgrade()` returns `NotInitialized` in that case. |

## Migration Considerations

### Adding new permission types

New permission symbols can be granted without any migration — `Vec<Symbol>` is
open-ended. No WASM upgrade is required to introduce new permission names.

### Changing error codes

Error code values (6001–6007, see `docs/error_codes.md`) are part of the ABI.
Clients that match on numeric codes must be updated when codes change.
Coordinate error code changes with a registry version bump via
`register_with_metadata`.

### Adding new DataKey variants

Follow the general [contract upgrade pattern](contract-upgrade-pattern.md):

1. Add the new variant to the `DataKey` enum — never remove or rename existing
   variants.
2. Use `Option<T>` if existing entries must deserialise without the new field.
3. Upload new WASM and call `upgrade()` on the live instance (requires the
   instance to have been `initialize`d — see "Admin & Initialization" above).

### Changing MAX_DELEGATE_PERMS

The `MAX_DELEGATE_PERMS` constant (currently 64) is enforced at grant time only.
Lowering it does not invalidate existing grants that exceed the new limit — they
remain readable and revocable. Raising it requires no migration.

## Pre-Upgrade Checklist

- [ ] Confirm the instance was `initialize`d (has a stored `DataKey::Admin`) — `upgrade()` returns `NotInitialized` otherwise
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
