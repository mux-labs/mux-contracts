# Mux Protocol — Storage Griefing Notes

**Version:** 0.1.0  
**Date:** 2026-05-30  
**Related:** [Threat Model §4.5](threat-model.md#45-storage-griefing)

---

## What is storage griefing?

On Soroban, every contract pays **rent** for the ledger entries it occupies.  All three Mux contracts use **instance storage**, which is billed as a single rent unit shared across all callers.  Two distinct attack surfaces exist:

1. **Unbounded collection growth** — a privileged caller (owner, admin) floods a map or vec, inflating the rent cost for every other user of the contract.
2. **TTL expiry** — if no one extends the instance storage TTL, the entry expires and all contract state is silently lost.

---

## Mitigations in code

### Collection caps

| Contract | Collection | Key | Cap constant | Error on overflow |
|---|---|---|---|---|
| `mux-account` | `Delegates` map | `DataKey::Delegates` | `MAX_DELEGATES = 64` | `TooManyDelegates` |
| `mux-permissions` | `RoleMembers` vec | `DataKey::RoleMembers(role)` | `MAX_ROLE_MEMBERS = 256` | `TooManyMembers` |
| `mux-permissions` | `AccountRoles` vec | `DataKey::AccountRoles(account)` | `MAX_ROLES_PER_ACCOUNT = 32` | `TooManyRoles` |
| `mux-registry` | `Names` vec | `DataKey::Names` | `MAX_CONTRACTS = 128` | `TooManyContracts` |

Caps are enforced on **new insertions only**; updates to existing entries are always allowed.

### TTL auto-extension

Every write operation in all three contracts calls:

```rust
env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
```

| Constant | Value | Approximate duration |
|---|---|---|
| `TTL_THRESHOLD` | 17,280 ledgers | ~1 day (5 s/ledger) |
| `TTL_EXTEND_TO` | 518,400 ledgers | ~30 days |

This means the TTL is bumped to 30 days whenever the remaining TTL drops below 1 day **and** a write occurs.  Contracts that are not written to for more than 30 days will expire unless a keeper extends the TTL externally.

---

## Deployment runbook — TTL keeper

> **Required before mainnet deployment.**

A keeper job must periodically call `extend_ttl` on each contract's instance storage to prevent expiry during idle periods.

Recommended approach using the Stellar CLI:

```bash
stellar contract extend \
  --id <CONTRACT_ID> \
  --ledgers-to-extend 518400 \
  --source <KEEPER_KEYPAIR> \
  --network mainnet
```

Run this job at least once every **25 days** to stay ahead of the 30-day TTL window.

---

## Storage sizing reference

| Collection | Entry size (approx.) | Cap | Max storage |
|---|---|---|---|
| `Delegates` map | ~72 bytes | 64 | ~4.6 KB |
| `RoleMembers` vec | ~32 bytes | 256 | ~8 KB |
| `AccountRoles` vec | ~8 bytes | 32 | ~256 bytes |
| `Names` vec (`mux-registry`) | ~16 bytes | 128 | ~2 KB |
| `SpendLimit` per asset | ~80 bytes | owner-controlled | unbounded (owner only) |

`SpendLimit` keys are written only by the contract owner and are not publicly writable, so no cap is enforced.  Owners should avoid registering an excessive number of distinct assets.

---

## Threat cross-reference

| Threat ID | Description | Mitigation |
|---|---|---|
| T-17 | Owner floods delegate map | `MAX_DELEGATES = 64` in `set_delegate` |
| T-18 | Admin floods role member list | `MAX_ROLE_MEMBERS = 256` in `grant_role` |
| T-19 | Admin assigns excessive roles to one account | `MAX_ROLES_PER_ACCOUNT = 32` in `grant_role` |
| T-20 | Spend limits accumulate unbounded per-asset keys | No public write path; owner-only |
| T-21 | Instance storage TTL expiry causes silent data loss | `extend_ttl` on every write + keeper job |
| T-22 | Admin floods registry Names vec via `register` or `register_with_metadata` | `MAX_CONTRACTS = 128` enforced in both functions |
