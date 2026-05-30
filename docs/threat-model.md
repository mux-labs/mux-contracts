# Mux Protocol — Threat Model

**Version:** 0.1.0  
**Date:** 2026-05-30  
**Status:** Living document — update whenever contracts or trust boundaries change.

---

## 1. Scope

This document covers the three on-chain Soroban contracts that make up Mux Protocol:

| Contract | Responsibility |
|---|---|
| `mux-account` | Account abstraction, delegate management, spend limits |
| `mux-batcher` | Atomic multi-operation batching |
| `mux-permissions` | RBAC registry used by other contracts |

Off-chain components (TypeScript SDK, frontend, deployment scripts) are out of scope for on-chain threat analysis but are noted where they affect trust boundaries.

---

## 2. Assets

| Asset | Description | Impact if Compromised |
|---|---|---|
| Owner private key | Controls the `mux-account` contract | Full account takeover |
| Admin keypair | Controls the `mux-permissions` registry | All role assignments can be forged |
| Delegate list | Set of authorized sub-signers | Unauthorized spending or operations |
| Spend limits | Per-asset caps on delegate spending | Financial loss |
| Guardian set | Recovery addresses for the account | Account recovery hijacked |
| Contract WASM | Deployed bytecode | Backdoor if upgrade key is compromised |
| npm package | Published TypeScript bindings | Supply chain attack on integrators |

---

## 3. Trust Boundaries

```
┌────────────────────────────────────────────┐
│  Off-chain (untrusted)                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │  User    │  │  DApp    │  │ Backend  │ │
│  │ Browser  │  │ Frontend │  │  Server  │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘ │
│       │              │              │       │
│       └──────────────┴──────────────┘       │
│                      │ XDR transaction       │
├──────────────────────┼────────────────────── ┤
│  Stellar Network      │                      │
│  ┌────────────────────▼──────────────────┐   │
│  │  Soroban VM (trusted execution)       │   │
│  │  ┌──────────┐ ┌──────────┐ ┌───────┐ │   │
│  │  │mux-acct  │ │mux-batch │ │mux-perm│ │   │
│  │  └──────────┘ └──────────┘ └───────┘ │   │
│  └───────────────────────────────────────┘   │
└────────────────────────────────────────────── ┘
```

**Key boundary:** Anything outside the Soroban VM is untrusted. Auth checks (`require_auth`) enforce that callers have signed the transaction with the expected keypair.

---

## 4. Threats and Mitigations

### 4.1 Account Takeover

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-01 | Owner key compromise | Spoofing | Medium | Critical | Guardian recovery set; hardware wallet recommendation; time-locked admin operations |
| T-02 | Delegate key compromise | Spoofing | Medium | High | Spend limits cap damage; time-bounded delegate expiry (`expiry_ledger`) |
| T-03 | Delegate expiry not enforced | Elevation of Privilege | Low | High | `expiry_ledger` checked on every invocation; stale delegates rejected |
| T-04 | Guardian collusion | Spoofing | Low | Critical | M-of-N guardian threshold (future roadmap) |

### 4.2 Unauthorized Spending

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-05 | Spend limit bypass via period reset manipulation | Elevation of Privilege | Low | High | Reset ledger set at initialization; only the contract increments it |
| T-06 | Integer overflow in spend accounting | Tampering | Low | High | Rust's checked arithmetic in release; `overflow-checks = true` in Cargo profile |
| T-07 | Re-entrancy via `debit_spend` | Elevation of Privilege | Low | Medium | Soroban does not allow re-entrant calls into the same contract instance |

### 4.3 Batch Execution Abuse

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-08 | Gas griefing via oversized batch | Denial of Service | Medium | Medium | `MAX_BATCH_SIZE = 50` hard cap enforced before execution |
| T-09 | Required-op failure ignored | Tampering | Low | High | `require_success` flag panics the transaction, rolling back all operations |
| T-10 | Cross-contract call to malicious contract | Tampering | Medium | High | Caller is authenticated; target contracts are user-supplied — document that callers must vet targets |

### 4.4 Permission Registry

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-11 | Admin key compromise | Elevation of Privilege | Low | Critical | Admin key should be a multisig account; rotate post-deployment |
| T-12 | Role granted to wrong address | Tampering | Medium | High | Admin-only `grant_role`; all operations emit events (Soroban events) |
| T-13 | Stale role membership | Information Disclosure | Low | Low | `get_role_members` always returns current state from storage |

### 4.5 Storage Griefing

All three contracts use **instance storage**, which is shared across all callers and billed as a single rent unit. Unbounded growth in any collection raises rent costs for every user of the contract and can eventually make the contract economically unviable.

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-17 | Owner floods delegate map to bloat instance storage | Denial of Service | Low | Medium | `MAX_DELEGATES = 64` hard cap in `set_delegate`; new entries beyond cap return `TooManyDelegates` |
| T-18 | Admin floods a role's member list | Denial of Service | Low | Medium | `MAX_ROLE_MEMBERS = 256` cap in `grant_role`; returns `TooManyMembers` |
| T-19 | Admin assigns excessive roles to one account | Denial of Service | Low | Low | `MAX_ROLES_PER_ACCOUNT = 32` cap in `grant_role`; returns `TooManyRoles` |
| T-20 | Spend limits accumulate unbounded per-asset keys | Denial of Service | Low | Low | Each asset key is a separate instance entry; owner controls which assets are registered; no public write path |
| T-21 | Instance storage TTL expiry causes silent data loss | Denial of Service | Medium | High | Callers must extend TTL via `env.storage().instance().extend_ttl()`; document minimum TTL extension in deployment runbook |

**Storage sizing reference (approximate):**

| Collection | Entry size | Cap | Max storage |
|---|---|---|---|
| `Delegates` map | ~72 bytes/entry | 64 | ~4.6 KB |
| `RoleMembers` vec | ~32 bytes/entry | 256 | ~8 KB |
| `AccountRoles` vec | ~8 bytes/entry | 32 | ~256 bytes |

> See [docs/storage-griefing.md](storage-griefing.md) for full mitigation details, TTL constants, and the deployment keeper runbook.

### 4.6 Supply Chain

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-14 | Malicious npm package publish | Tampering | Low | High | npm provenance attestation in CI; scoped package name `@mux-protocol/contracts` |
| T-15 | WASM tampering before deployment | Tampering | Low | Critical | SHA-256 of compiled WASM published in release notes; reproduce from source |
| T-16 | Dependency confusion attack | Tampering | Low | High | Scoped npm package; Cargo.lock pinned; Dependabot alerts enabled |

---

## 5. Security Controls

| Control | Where Applied |
|---|---|
| `require_auth()` on all write operations | All three contracts |
| `overflow-checks = true` in release profile | Cargo.toml |
| `MAX_BATCH_SIZE` cap | `mux-batcher` |
| Delegate `expiry_ledger` | `mux-account` |
| Spend limit period reset via ledger sequence | `mux-account` |
| npm provenance (`--provenance`) | CI publish job |
| Drift check: committed bindings vs generated | CI `check-binding-drift` job |
| RBAC admin-only mutation | `mux-permissions` |

---

## 6. Out-of-Scope / Residual Risks

- **Stellar network-level attacks** (consensus failures, validator collusion) — outside contract scope.
- **RPC node trust** — users should use multiple RPC endpoints or run their own node.
- **Frontend key management** — private keys in browser localStorage are a known risk; hardware wallets are recommended.
- **Upgrade authority** — if a contract admin can upgrade the WASM, a compromised admin key is catastrophic. Consider time-lock or DAO governance before mainnet.

---

## 7. Revision History

| Date | Version | Change |
|---|---|---|
| 2026-05-30 | 0.1.0 | Initial threat model |
| 2026-05-30 | 0.1.1 | Storage griefing: added T-21 TTL expiry threat; added `extend_ttl` mitigation in all contracts; added `docs/storage-griefing.md` |
| 2026-05-30 | 0.1.2 | Added `docs/audit-prep.md` — scope, entry points, known limitations, auditor checklist |
