# Mux Protocol — Instance vs Persistent Storage Choices

**Version:** 1.1.0  
**Date:** 2026-08-25  
**Status:** Complete (Audit Ready)  
**Issue:** #684, #810  
**Related:** [Storage Griefing Notes](storage-griefing.md) · [Threat Model](threat-model.md)

---

## Overview

Soroban provides three storage types for contract data:

| Type | Access | TTL | Billing |
|---|---|---|---|
| `instance()` | Shared across all callers within the contract | Single rent unit | Rent is per-contract-instance |
| `persistent()` | Keyed storage; each key has its own TTL | Per-key | Rent is per-key |
| `temporary()` | Keyed storage; expires automatically | Ephemeral | No rent (expires by default) |

Every Mux contract makes a deliberate choice about which storage type to use for each piece of state. This document explains the rationale so that auditors, maintainers, and TypeScript binding authors can understand why the contracts are structured as they are.

---

## Storage Choices Encoded in Tests

The choices below are not prose-only: each is asserted by executable tests so that a
regression in storage placement, TTL extension, or authorization fails CI instead of
silently shipping. The table maps every documented choice to the test that encodes it.

| Documented choice | Encoded assertion | Test location |
|---|---|---|
| Singleton config (admin, roles, metadata) lives in **instance** storage | `env.storage().instance().has(&key)` is true and `persistent().has(&key)` is false after the write | `mux-permissions/src/test.rs`, `mux-registry/src/test.rs` |
| Per-entity data (wallet limits, delegate grants) lives in **persistent** storage | `env.storage().persistent().has(&key)` is true and `instance().has(&key)` is false | `mux-policy/src/test.rs`, `mux-delegation/src/test.rs` |
| No contract uses **temporary** storage | No `temporary()` access appears on any write path; state survives a ledger advance past the temporary window | `*/src/test.rs` (shared helper `assert_no_temporary_storage`) |
| Every write path extends TTL (T-21) | After a write, `extend_ttl` has been applied to the same storage type and key that was written | `mux-policy/src/test.rs`, `mux-delegation/src/test.rs` |
| mux-delegation keeps the **instance** alive while data is persistent | `instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO)` is invoked on every mutating entrypoint | `mux-delegation/src/test.rs` |
| mux-batcher metadata is write-once | Second `set_registry_metadata` returns `MetadataAlreadySet` and does not mutate storage | `mux-batcher/src/test.rs` |

### Authorization invariants on storage

Storage placement is only half the contract; the tests also pin down who may write:

- **Owner/admin/guardian entries are not overwritable by non-privileged callers.** A call
  from an address that is not the current owner, admin, or guardian must fail with the
  documented auth error and leave the stored value byte-for-byte unchanged.
- **Revoked delegates cannot write.** After `revoke_delegate`, a subsequent call from the
  revoked delegate fails closed and the persistent `DelegatePerms(owner, delegate)` key is
  absent (or unchanged) — no partial mutation.
- **Deny-by-default for new privileged surfaces.** Any newly added privileged storage key
  must be reachable only through an authorized entrypoint; the negative test asserts the
  unauthorized path errors before any `set`/`remove` executes.

### Idempotency and replay

Where the documented storage choice implies it, repeated writes are rejected or are a
no-op rather than silently re-mutating state:

- **Re-initialization is rejected.** A second `initialize`/`set_registry_metadata` call
  returns the documented error and does not overwrite the existing instance entry.
- **Repeated writes are idempotent.** Re-applying the same `set_daily_limit` or delegate
  grant leaves the stored record equal to the first write and does not double-count.
- **Replayed requests fail closed.** A replayed storage-mutating request (same nonce /
  correlation id) is rejected and does not mutate storage.

---

## Design Principles

1. **Singleton configuration → instance storage.** Data that is shared across all callers (admin addresses, registry metadata, role definitions) lives in instance storage. This is the cheapest option when a single key is shared by all users.

2. **Per-entity data with independent lifetimes → persistent storage.** Data that is keyed per wallet, per owner, or per delegate — and must survive independently of other entities — uses persistent storage. Each key can be TTL-extended independently.

3. **No temporary storage.** The Mux contracts do not use temporary storage. All on-chain state must be durable for audit events, rollback analysis, and off-chain indexing.

4. **TTL auto-extension on every write.** Every write path calls `extend_ttl()` to prevent silent data loss (T-21). This applies to both instance and persistent storage entries.

---

## Contract-by-Contract Breakdown

### mux-permissions — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `Admin` | instance | Singleton config; shared across all callers |
| `RoleMembers(role)` | instance | Roles are global to the contract; small bounded vecs (MAX=256) |
| `RolePermissions(role)` | instance | Global role definitions |
| `AccountRoles(account)` | instance | Global account-to-role index (MAX=32 roles/account) |
| `PendingAdmins` | instance | Singleton admin transition state |
| `AdminThreshold` | instance | Singleton config |
| `AdminApprovals(addr)` | instance | Bounded by MAX_PENDING_ADMINS=16 |
| `Metadata` | instance | Singleton registry metadata |

**Why instance-only:** The permissions registry is a singleton. All roles, members, and permissions are global to the contract. There is no per-entity data that requires independent TTL management. The collection caps (256 members/role, 32 roles/account) prevent unbounded growth.

---

### mux-account — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `Owner` | instance | Singleton per contract instance (each account IS a contract) |
| `GuardianSet` | instance | Per-account config |
| `Delegates` | instance | Per-account map (MAX=64 entries) |
| `Nonce` | instance | Per-account counter |
| `SpendLimit(asset)` | instance | Per-asset limit within the account |
| `SessionKey(owner, key)` | instance | Per-session key record |
| `SessionKeyIndex(owner)` | instance | Per-owner session key index (MAX=32) |
| `Paused` | instance | Per-account flag |
| `Executing` | instance | Per-account reentrancy guard |
| `Metadata` | instance | Per-account registry metadata |

**Why instance-only:** Each mux-account IS its own contract instance. All data within an account is inherently scoped to that single account. There is no multi-tenant data sharing, so instance storage is the natural fit. The per-account delegate cap (64) and session key cap (32) prevent storage griefing within a single account.

---

### mux-batcher — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `Executing` | instance | Reentrancy guard; self-cleaning |
| `Meta` | instance | Singleton metadata; written once via `set_registry_metadata` |

**Why instance-only:** The batcher is stateless by design — it invokes target contracts in a loop but does not store per-entity data. The only instance storage is a reentrancy flag (cleared on every call) and optional metadata set via the one-time `set_registry_metadata` call (`MetadataAlreadySet` on a second call). There is no per-entity data that would benefit from persistent storage.

---

### mux-registry — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `Admin` | instance | Singleton admin |
| `Version(name)` | instance | Version-to-address map |
| `Names` | instance | Name index (bounded vec) |
| `Metadata` | instance | Singleton registry metadata |

**Why instance-only:** The registry is a global singleton mapping contract names to addresses. All data is shared across all callers. Collection caps prevent unbounded growth.

---

### mux-policy — Instance + Persistent (hybrid)

| Data | Storage type | Rationale |
|---|---|---|
| `Admin` | instance | Singleton admin config |
| `WalletNames` | instance | Global wallet index for griefing guard (MAX=256) |
| `WalletLimit(wallet)` | **persistent** | Per-wallet daily limit record |

**Why hybrid:** This is the key architectural decision in the codebase. The admin and wallet index are singleton/global data → instance storage. But each wallet's `DailyLimit` record is independently keyed and must survive with its own TTL. Persistent storage allows:

- **Independent TTL per wallet:** Each `WalletLimit` entry can be extended independently via `persistent().extend_ttl(&key, ...)` without affecting other wallets.
- **Clean expiry semantics:** If a wallet's limit record expires, only that wallet's limit is lost — other wallets' records are unaffected.
- **Efficient reads:** `get_daily_limit(wallet)` reads a single persistent key rather than scanning a map in instance storage.

Every `set_daily_limit`, `record_spend`, and `reset_daily_counter` call extends the persistent entry TTL:

```rust
env.storage().persistent().set(&key, &record);
env.storage().persistent().extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
```

The instance TTL is also extended via `Self::extend_ttl(&env)` to keep the admin and index alive.

---

### mux-delegation — Persistent only (with instance TTL extension)

| Data | Storage type | Rationale |
|---|---|---|
| `DelegatePerms(owner, delegate)` | **persistent** | Per-delegate permission set |
| `OwnerDelegates(owner)` | **persistent** | Per-owner delegate list (MAX=128) |

**Why persistent:** Delegate permissions are per-owner-per-delegate data. Using persistent storage provides:

- **Independent TTL per grant:** Each `(owner, delegate)` pair's permission set has its own TTL. Revoking one delegate does not affect others.
- **Per-owner delegate list isolation:** Each owner's delegate list lives independently, so different owners' data expires on different schedules.
- **Clean revocation:** `revoke_delegate` removes a single persistent key without touching other data.

Note that `extend_ttl()` still extends **instance** TTL to keep the contract instance alive:

```rust
fn extend_ttl(env: &Env) {
    env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}
```

This is necessary because the contract instance itself must not expire, even though the primary data lives in persistent storage.

---

### mux-recovery — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `Owner` | instance | Per-account singleton (each recovery contract is per-account) |
| `GuardianSet` | instance | Per-account config |
| `RecoveryRequest` | instance | Per-account pending recovery |

**Why instance-only:** Similar to mux-account, each recovery contract is per-account. All data is scoped to a single account.

---

### mux-spending-policy — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `Admin` | instance | Singleton admin |
| `Policy(account, asset)`

/* … truncated 4225 chars — edit only what you need near the top … */
