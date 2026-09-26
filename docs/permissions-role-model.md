# Mux Permissions — Role Model

**Version:** 0.1.0  
**Status:** Living document — update when the permissions contract interface or data model changes.

---

## Overview

The `mux-permissions` contract implements a role-based access control (RBAC) registry
that other Mux contracts can call to verify caller permissions before executing
privileged operations. It is a standalone Soroban contract with its own storage,
admin, and audit event stream.

Design goals:

- **Fine-grained**: Permissions are symbolic labels (`Symbol`) assigned to roles,
  and roles are assigned to accounts. An account may hold multiple roles, and a
  role may hold multiple permissions.
- **Bounded storage**: All `Vec`-backed collections are capped to prevent
  storage-griefing attacks (see [storage-griefing.md](storage-griefing.md)).
- **Auditable**: Every state-mutating operation emits a structured Soroban event.
- **Multisig admin**: Admin transfer uses a threshold-based approval model
  (similar to a simple multisig) so no single key can unilaterally change the
  admin.

---

## Data Model

### Storage Keys (`DataKey`)

| Variant | Type | Purpose |
|---|---|---|
| `Admin` | `Address` | Current admin address |
| `RoleMembers(Symbol)` | `Vec<Address>` | Members of a role |
| `RolePermissions(Symbol)` | `Vec<Symbol>` | Permissions attached to a role |
| `AccountRoles(Address)` | `Vec<Symbol>` | Roles held by an account (index) |
| `PendingAdmins` | `Vec<Address>` | Pending admin candidates (multisig) |
| `AdminThreshold` | `u32` | Number of approvals required to promote a candidate |
| `AdminApprovals(Address)` | `Vec<Address>` | Approvers who have voted for a candidate |
| `Metadata` | `RegistryMeta` | Optional registry metadata (name, version, description) |

All keys use **instance storage**, which is preserved across WASM upgrades.

### Registry Metadata

```rust
pub struct RegistryMeta {
    pub name: String,        // e.g. "mux-mainnet-perm"
    pub version: String,     // e.g. "1.0.0"
    pub description: String, // free-form notes
}
```

### Cap Constants

| Constant | Value | Scope |
|---|---|---|
| `MAX_ROLE_MEMBERS` | 256 | Per role |
| `MAX_ROLES_PER_ACCOUNT` | 32 | Per account |
| `MAX_PENDING_ADMINS` | 16 | Global |

These caps prevent unbounded `Vec` growth in instance storage (STORAGE-GRIEFING T-21).

---

## Access Control Model

```
┌─────────────────────────────────────────────────────────────────┐
│                        Admin                                     │
│  (single Address, set on initialize)                             │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │  Can: create_role, grant_role, revoke_role,               │   │
│  │        set_admin_threshold, propose_admin, approve_admin,  │   │
│  │        set_metadata, upgrade                                │   │
│  └───────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
                    ┌──────────────────┐
                    │     Roles        │
                    │  (Symbol keys)   │
                    └──────────────────┘
                      │            │
                      ▼            ▼
              ┌────────────┐  ┌────────────┐
              │  Members   │  │Permissions │
              │ (Addresses)│  │ (Symbols)  │
              └────────────┘  └────────────┘
                      │
                      ▼
               ┌──────────────┐
               │   Accounts   │
               │ (many:many)  │
               └──────────────┘
```

**Key rules:**

1. **Admin-only mutations**: Only the current admin can create roles, grant/revoke
   roles, change the threshold, propose/approve admins, or set metadata.
2. **Reads are unauthenticated**: `has_permission`, `get_roles`, `get_role_members`,
   `get_admin_threshold`, `get_pending_admins`, `get_metadata` require no auth.
3. **Idempotent grant**: Granting a role the account already holds is a no-op
   (returns `Ok(())`, emits no event, writes no storage).
4. **Cap enforcement**: Adding a member to a full role returns `TooManyMembers`.
   Adding a role to an account that already holds `MAX_ROLES_PER_ACCOUNT` roles
   returns `TooManyRoles`.

---

## Admin Transfer (Multisig)

Admin transfer uses a threshold-based approval model:

```
       ┌──────────┐
       │  Admin   │
       └────┬─────┘
            │ propose_admin(candidate)      ← Admin-only
            ▼
    ┌───────────────┐
    │  PendingAdmins│  (Vec<Address>)
    └───────┬───────┘
            │
            ▼
    ┌───────────────┐
    │  Approvals    │  Each existing admin calls approve_admin(approver, candidate)
    │  (per cand.)  │  When approvals.len() >= threshold, candidate is promoted.
    └───────────────┘
```

1. Admin calls `set_admin_threshold(n)` to set the required approval count (default: 1).
2. Admin calls `propose_admin(candidate)` — adds candidate to `PendingAdmins` vec.
3. Existing admins call `approve_admin(approver, candidate)` — counts an approval.
4. When `approvals.len() >= threshold`, the candidate becomes the new admin
   (stored in `DataKey::Admin`) and is removed from `PendingAdmins`.

**Duplicate approval guard**: An approver cannot approve the same candidate twice
(`AlreadyApproved` error).

**Fail-closed enforcement**: `propose_admin` and `approve_admin` both call
`require_admin()`, and `approve_admin` additionally requires the named
`approver`'s own signature. With no auth mocked at all, both calls are
rejected and pending-admin state is left unchanged — see
`test_admin_rotation_calls_require_admin_auth` and
`test_multisig_admin_promotion_transfers_control` in
`contracts/mux-permissions/src/lib.rs`, which also asserts that the stored
admin does not change until approvals reach the configured threshold.

**Edge case — threshold = 1 (default)**: The proposing admin's own `approve_admin`
call immediately promotes the candidate, making this behave like a single-key
transfer.

---

## Authorization Flow Examples

### Create and grant a role

```
Admin calls create_role("operator", ["transfer", "burn"])
  ├─ require_admin() → admin.require_auth() ✓
  ├─ Storage: RoleMembers("operator") = [], RolePermissions("operator") = [transfer, burn]
  └─ Event: action = role_crt, data = "operator"

Admin calls grant_role("0xAlice", "operator")
  ├─ require_admin() → admin.require_auth() ✓
  ├─ Storage: RoleMembers("operator") += [0xAlice], AccountRoles(0xAlice) += ["operator"]
  └─ Event: action = role_grt, data = (0xAlice, "operator")
```

### Permission check

```
Anyone calls has_permission("0xAlice", "transfer")
  ├─ (no auth required — read-only)
  ├─ Reads AccountRoles(0xAlice) → finds "operator"
  ├─ Reads RolePermissions("operator") → finds "transfer"
  ├─ Event: action = perm_ok, data = (0xAlice, "transfer")  ← only on grant
  └─ Returns true
```

### Revoke a role

```
Admin calls revoke_role("0xAlice", "operator")
  ├─ require_admin() → admin.require_auth() ✓
  ├─ Storage: RoleMembers("operator") -= [0xAlice], AccountRoles(0xAlice) -= ["operator"]
  └─ Event: action = role_rev, data = (0xAlice, "operator")

Anyone calls has_permission("0xAlice", "transfer")
  └─ Returns false (no roles → no permissions)
```

### Multisig admin promotion (threshold = 2)

```
1. Admin1 calls set_admin_threshold(2)
   └─ Event: action = adm_thr, data = 2

2. Admin1 calls propose_admin("0xBob")
   └─ Event: action = adm_prp, data = "0xBob"

3. Admin1 calls approve_admin(admin1_addr, "0xBob")
   └─ approvals = [admin1_addr], 1 < 2, no promotion yet
   └─ Event: action = adm_apr, data = (admin1_addr, "0xBob")

4. Admin2 calls approve_admin(admin2_addr, "0xBob")
   └─ approvals = [admin1_addr, admin2_addr], 2 >= 2, promotion!
   └─ Storage: Admin = "0xBob", PendingAdmins -= ["0xBob"]
   └─ Event: action = adm_prm, data = "0xBob"
```

---

## Error Codes

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not an authorized admin |
| `RoleNotFound` | 4 | 404 | The specified role does not exist |
| `AccountNotInRole` | 5 | 404 | Account is not a member of the role |
| `PermissionNotFound` | 6 | 404 | Permission does not exist (reserved) |
| `TooManyMembers` | 7 | 409 | Role has reached `MAX_ROLE_MEMBERS` (256) |
| `TooManyRoles` | 8 | 409 | Account holds `MAX_ROLES_PER_ACCOUNT` (32) roles |
| `AdminNotFound` | 9 | 404 | Pending admin candidate not found |
| `AlreadyApproved` | 10 | 409 | Approver has already approved this candidate |
| `TooManyPendingAdmins` | 11 | 409 | Too many pending admin proposals |

---

## Events

Contract tag: `mux_perm`

| Action | Trigger | Data |
|---|---|---|
| `init` | `initialize` succeeds | `admin: Address` |
| `role_crt` | `create_role` succeeds | `role: Symbol` |
| `role_grt` | `grant_role` succeeds (new grant) | `(account: Address, role: Symbol)` |
| `role_rev` | `revoke_role` succeeds | `(account: Address, role: Symbol)` |
| `perm_ok` | `has_permission` returns `true` | `(account: Address, permission: Symbol)` |
| `adm_thr` | `set_admin_threshold` succeeds | `threshold: u32` |
| `adm_prp` | `propose_admin` adds a new candidate | `new_admin: Address` |
| `adm_apr` | `approve_admin` records an approval (threshold not yet reached) | `(approver: Address, new_admin: Address)` |
| `adm_prm` | `approve_admin` promotes a candidate (threshold reached) | `new_admin: Address` |
| `meta_set` | `set_metadata` succeeds | `name: String` |

Events are **success-only**: failed operations (returning `Err`) emit no events.
Read-only functions (`get_roles`, `get_role_members`, `get_admin_threshold`, etc.)
emit no events. `has_permission` is the one deliberate exception, and only in
one direction: a granted check emits `perm_ok`; a denied check emits nothing.

See [event-topic-conventions.md](event-topic-conventions.md) for topic layout rules
and [audit-events.md](audit-events.md) for the full per-contract catalog.

---

## Storage TTL

Instance storage TTL is extended on every write (`TTL_THRESHOLD = 17_280`,
`TTL_EXTEND_TO = 518_400` ledgers ≈ 30 days). Deployers should run a keeper
job to extend TTL proactively; see [storage-griefing.md](storage-griefing.md).

---

## Security Considerations

1. **Admin key custody**: The admin address is a single point of trust. For
   production deployments, the admin key should be a Stellar multisig account
   with threshold ≥ 2 stored on a hardware wallet or HSM.
2. **Storage griefing**: Role-member and account-role vecs are capped at
   `MAX_ROLE_MEMBERS` (256) and `MAX_ROLES_PER_ACCOUNT` (32) respectively.
   These caps prevent an admin from bloating instance storage.
3. **Idempotent grants**: `grant_role` on an already-held role is a no-op,
   preventing duplicate event spam and unnecessary storage writes.
4. **Event data minimisation**: Event payloads contain only public identifiers
   (addresses, symbols, u32 threshold values). No secrets or sensitive data
   are published.
5. **Read-only queries**: Permission checks (`has_permission`) do not require
   authentication. A granted check emits `perm_ok` for the audit trail; a
   denied check emits nothing, since `has_permission` takes no auth and an
   event on every denial would let any caller spam an arbitrary account's
   audit log with `perm_den` entries for permissions it never held. Off-chain
   indexers can stream `perm_ok` events.

