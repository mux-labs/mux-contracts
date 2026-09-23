# Mux Protocol — Instance vs Persistent Storage Choices

**Version:** 1.0.0  
**Date:** 2026-08-25  
**Status:** Complete (Audit Ready)  
**Issue:** #684  
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
| `Policy(account, asset)` | instance | Per-account/asset spending policy |

**Why instance-only:** Spending policies are per-account/asset but the total number is bounded by the owner (who sets policies). All data is written and read by the same admin, so instance storage is sufficient.

---

### mux-account-factory — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `OwnerAccounts(owner)` | instance | Per-owner account list |
| `AccountCount` | instance | Global counter |
| `Metadata` | instance | Singleton metadata |

**Why instance-only:** The factory tracks which accounts each owner has created. The account list per owner is bounded, and the factory is a utility contract with limited state.

---

### mux-wallet-registry — Instance only

| Data | Storage type | Rationale |
|---|---|---|
| `Owner` | instance | Singleton owner |
| `Wallet(name)` | instance | Per-wallet entry |
| `Names` | instance | Name index (bounded vec) |

**Why instance-only:** The wallet registry is a simple name-to-wallet mapping. All data is global and bounded by `MAX_WALLETS`.

---

## Summary Table

| Contract | instance() | persistent() | temporary() | Hybrid? |
|---|---|---|---|---|
| mux-permissions | All data | — | — | No |
| mux-account | All data | — | — | No |
| mux-batcher | All data | — | — | No |
| mux-registry | All data | — | — | No |
| mux-policy | Admin, WalletNames | WalletLimit(wallet) | — | **Yes** |
| mux-delegation | — | DelegatePerms, OwnerDelegates | — | **Persistent-primary** |
| mux-recovery | All data | — | — | No |
| mux-spending-policy | All data | — | — | No |
| mux-account-factory | All data | — | — | No |
| mux-wallet-registry | All data | — | — | No |

---

## Implications for TypeScript Bindings

When binding these contracts from TypeScript:

1. **Instance storage reads** (`get_admin()`, `owner()`, etc.) are cheap — they read from a single ledger entry shared across all callers.

2. **Persistent storage reads** (`get_daily_limit(wallet)`, `get_delegate_permissions(owner, delegate)`) read from a per-key ledger entry. The key is derived from the contract address + storage key.

3. **TTL management is transparent to callers.** The contracts auto-extend TTL on every write. Callers do not need to manage TTLs for normal operations. However, off-chain indexers should be aware that persistent entries may expire if not written to for 30+ days.

4. **Storage costs are shared for instance storage.** All callers of a mux-account share the same rent for instance storage. Persistent storage rent is per-key and independent.

---

## Implications for Auditors

1. **Instance storage state is visible to all callers.** Any caller can read any instance storage key. This is by design for transparency but means sensitive data should not be stored on-chain.

2. **Persistent storage provides better isolation.** Per-wallet and per-delegate data in persistent storage has independent lifetimes, making expiry and revocation cleaner.

3. **The hybrid pattern in mux-policy is the most complex.** Pay special attention to the interaction between instance storage (admin, wallet index) and persistent storage (per-wallet limits). Both TTLs must be maintained.

4. **mux-delegation's persistent storage is unusual.** Most contracts use instance storage. The delegation contract's choice of persistent storage is deliberate and should be verified against the TTL extension logic.

5. **TTL extension is testable.** Run `bash scripts/test-ttl-keeper.sh` to verify that all contracts correctly implement TTL extension for both instance and persistent storage. This addresses audit checklist section 6.

---

## Related Testing and Verification

- **TTL keeper test suite:** `bash scripts/test-ttl-keeper.sh`
  - Validates TTL constants across all contracts
  - Confirms extend_ttl() is called on write paths
  - Verifies unit test coverage for TTL behavior
  - Checks persistent storage TTL handling

- **Storage capacity tests:** See [storage-griefing.md](storage-griefing.md) for the complete list of collection cap unit tests

- **Keeper deployment runbook:** See [storage-griefing.md#deployment-runbook](storage-griefing.md#deployment-runbook--ttl-keeper) for the production keeper script requirements
