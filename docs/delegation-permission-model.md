# Delegation Permission Model

**Contract:** `mux-delegation`  
**Version:** 0.1.0  
**Status:** Living document — update whenever the delegation contract changes.

---

## 1. Overview

`mux-delegation` provides a scoped, enumerable permission-grant system for the
Mux Protocol. An _owner_ (any Soroban `Address`) can grant a named set of
_permissions_ to a _delegate_ address. Delegates act on behalf of the owner
**only within the granted permission set** — they cannot self-escalate or grant
permissions to third parties.

The contract is `#![no_std]` and stores all state using Soroban persistent
storage with explicit per-entry TTL management.

---

## 2. Roles

| Role | Who | Capabilities |
|---|---|---|
| **Owner** | Any address that owns a delegation grant | Grant permissions to delegates; revoke grants; enumerate own delegates |
| **Delegate** | Address granted one or more permissions by an owner | Act within the granted permission set on behalf of the owner |
| **Caller** | Any address | Read-only queries (`get_delegate_permissions`, `is_delegate`, `get_delegates`, `check_delegate`) |

> **No super-owner over delegation grants.** There is no global admin for the
> owner/delegate permission model above. Each `(owner, delegate)` pair is
> independent. An owner can only grant or revoke their own delegates.
>
> This does not extend to `link_contract_id(admin, contract_id)` or
> `initialize(admin)`/`upgrade`, which accept a separate, unrelated `admin`
> concept scoped to contract self-registration and WASM upgrades — never to
> delegation grants. See [`docs/delegation-upgrade.md`](delegation-upgrade.md#admin--initialization)
> and [`docs/access-control-checklist.md`](access-control-checklist.md#17-mux-delegation)
> for the full breakdown.

---

## 3. Permission Model

### 3.1 What is a permission?

A permission is a `Symbol` — a compact Soroban string value of up to 9 bytes.
Permission names are chosen by the application layer; the contract treats them
as opaque identifiers. Common examples: `"transfer"`, `"read"`, `"swap"`,
`"vote"`, `"trade"`.

### 3.2 Grant semantics

Calling `grant_delegate(owner, delegate, permissions)`:

- **Replaces** any prior grant for the same `(owner, delegate)` pair — there is
  no append mode. The new `permissions` list becomes the authoritative grant.
- The operation is atomic: if validation fails (empty list, too many permissions,
  too many delegates) the existing grant is left unchanged.
- Requires `owner.require_auth()` — only the owner can grant.

```
grant_delegate(owner, delegate, ["transfer", "read"])
  → DelegatePerms(owner, delegate) = ["transfer", "read"]

grant_delegate(owner, delegate, ["swap"])   // overwrites
  → DelegatePerms(owner, delegate) = ["swap"]
```

### 3.3 Revoke semantics

Calling `revoke_delegate(owner, delegate)`:

- **Removes the entire grant** for the pair. There is no partial revocation.
- Removes the delegate from the owner's enumeration list (`OwnerDelegates`).
- Requires `owner.require_auth()`.
- Returns `Err(NotADelegate)` if no grant exists.

### 3.4 Permission checks

| Entrypoint | Auth required | Returns |
|---|---|---|
| `is_delegate(owner, delegate, permission)` | None | `bool` |
| `check_delegate(owner, delegate, permission)` | None | `Ok(())` or `Err(NotADelegate)` |
| `get_delegate_permissions(owner, delegate)` | None | `Vec<Symbol>` (empty if no grant) |
| `get_delegates(owner)` | None | `Vec<Address>` (empty if none) |

`is_delegate` and `check_delegate` are functionally equivalent; `check_delegate`
is useful when callers need an error value for chained authorization checks.

---

## 4. Storage Layout

| Key | Value | Kind | TTL |
|---|---|---|---|
| `DelegatePerms(owner, delegate)` | `Vec<Symbol>` | Persistent | Refreshed on every write |
| `OwnerDelegates(owner)` | `Vec<Address>` | Persistent | Refreshed on every write |

### 4.1 TTL management

Each write calls `extend_entry_ttl` on the affected storage key independently of
the contract instance TTL, using:

- **Threshold:** 17,280 ledgers (~1 day at 5-second close times)  
- **Extend to:** 518,400 ledgers (~30 days)

This ensures individual `DelegatePerms` and `OwnerDelegates` entries remain live
as long as they are actively used, even on long-running contracts. See
[`docs/storage-griefing.md`](storage-griefing.md) for the keeper runbook.

---

## 5. Bounds (Storage Griefing Guards)

| Constant | Value | Scope |
|---|---|---|
| `MAX_DELEGATE_PERMS` | 64 | Permissions per `(owner, delegate)` pair |
| `MAX_DELEGATES_PER_OWNER` | 128 | Delegate addresses per owner |

Both caps are enforced at `grant_delegate` call time:

- `permissions.len() > 64` → `Err(TooManyPermissions)` (error code 6002)
- `delegates.len() >= 128` (when adding a new delegate) → `Err(TooManyDelegates)` (error code 6004)

Re-granting an existing delegate does not count toward the delegate cap.

**Storage size estimate:**  
Each `DelegatePerms` entry holds up to 64 `Symbol` values (~9 bytes each) ≈ 576 bytes per pair.  
`OwnerDelegates` holds up to 128 `Address` values (~32 bytes each) ≈ 4 KB per owner.

---

## 6. Error Codes

Error codes 6001–6004 are **stable ABI**. Coordinate any change with a registry
version bump via `register_with_metadata`.

| Code | Variant | Description |
|---|---|---|
| 6001 | `NotADelegate` | No grant exists for the `(owner, delegate)` pair |
| 6002 | `TooManyPermissions` | `permissions` list exceeds the 64-entry cap |
| 6003 | `EmptyPermissions` | `permissions` list is empty; at least one required |
| 6004 | `TooManyDelegates` | Owner already has 128 delegates registered |

**HTTP status mapping** (for gateway/API use — see
[`docs/bindings-error-mapping.md`](bindings-error-mapping.md)):

| Code | HTTP status | Rationale |
|---|---|---|
| `NotADelegate` (6001) | 404 | Grant not found |
| `TooManyPermissions` (6002) | 400 | Bad request — input exceeds cap |
| `EmptyPermissions` (6003) | 400 | Bad request — empty input |
| `TooManyDelegates` (6004) | 409 | Conflict — cap reached |

---

## 7. Audit Events

Contract tag: `mux_dlg`  
Topic layout: `[topics[0]: "mux_dlg", topics[1]: <action>]`

| Action | Trigger | Data payload |
|---|---|---|
| `dlg_grant` | `grant_delegate` succeeds | `(owner: Address, delegate: Address)` |
| `dlg_rev` | `revoke_delegate` succeeds | `(owner: Address, delegate: Address)` |

Events are emitted **only on success**. Rejected calls (auth failure, validation
error) emit no events.

**TypeScript — subscribing to delegation events:**

```ts
const rawEvents = await server.getEvents({
  startLedger,
  filters: [{
    type: "contract",
    contractIds: [DELEGATION_CONTRACT_ID],
    topics: [["mux_dlg"]],
  }],
});

for (const event of rawEvents.records) {
  const action = event.topic[1]; // "dlg_grant" or "dlg_rev"
  // data: [owner: Address, delegate: Address]
}
```

---

## 8. TypeScript Binding Notes

The `MuxDelegationClient` in
[`bindings/src/generated/mux-delegation.ts`](../bindings/src/generated/mux-delegation.ts)
mirrors all on-chain entrypoints:

| TS method | On-chain entrypoint | Notes |
|---|---|---|
| `grantDelegate(kp, owner, delegate, permissions)` | `grant_delegate` | `permissions` is `string[]` |
| `revokeDelegate(kp, owner, delegate)` | `revoke_delegate` | — |
| `getDelegatePermissions(kp, owner, delegate, filters?)` | `get_delegate_permissions` | Supports `DelegationQueryFilters` |
| `isDelegate(kp, owner, delegate, permission)` | `is_delegate` | Returns `boolean` |
| `getDelegates(kp, owner, filters?)` | `get_delegates` | Supports `DelegationQueryFilters` |
| `checkDelegate(kp, owner, delegate, permission)` | `check_delegate` | Returns `boolean` (absorbs `NotADelegate` as `false`) |

`DelegationQueryFilters` supports client-side narrowing:
- `permission?: string` — filter permission list to a single entry.
- `hasAnyPermission?: boolean` — gate on whether the list is non-empty.

Error codes are resolved to human-readable strings via `muxDelegationErrorMessage(code)`
exported from `bindings/src/types.ts`.

---

## 9. `no_std` Constraints

This contract is `#![no_std]`. It does **not** use `std::vec`, `std::string`, or
`extern crate alloc`. All collections use `soroban_sdk::Vec<T>` backed by the
Soroban host environment. The restriction is enforced by the absence of `std` in
`Cargo.toml` features.

---

## 10. Security Notes

1. **No delegate-to-delegate grants.** A delegate cannot grant sub-permissions
   to a third party using this contract. Delegation is owner-initiated only.
2. **No expiry.** Grants are indefinite until explicitly revoked. Callers that
   need time-bounded delegation should track expiry off-chain or use
   `mux-account`'s session-key expiry feature.
3. **Overwrite on re-grant.** Re-calling `grant_delegate` replaces the full
   permission set. Callers must supply the complete desired permission list —
   partial additions are not supported.
4. **Owner-only mutations.** `grant_delegate` and `revoke_delegate` both call
   `owner.require_auth()` before any storage read or write. Unauthorized calls
   fail at the host level before touching state.
5. **Storage griefing.** The `MAX_DELEGATES_PER_OWNER = 128` cap prevents an
   attacker from forcing an owner's `OwnerDelegates` list to grow unboundedly.

---

## 11. Related Documents

- [`docs/audit-events.md`](audit-events.md) — full event schema reference
- [`docs/delegation-upgrade.md`](delegation-upgrade.md) — upgrade and migration notes
- [`docs/storage-griefing.md`](storage-griefing.md) — collection caps and keeper runbook
- [`docs/bindings-error-mapping.md`](bindings-error-mapping.md) — Rust error → TS union → HTTP status
- [`contracts/mux-delegation/src/lib.rs`](../contracts/mux-delegation/src/lib.rs) — contract source
- Issue [#410](https://github.com/mux-labs/mux-contracts/issues/410) — tracking issue
