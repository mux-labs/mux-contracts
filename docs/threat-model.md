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
| T-06 | Integer overflow in spend accounting | Tampering | Low | High | `checked_add` in `debit_spend` returns `ArithmeticOverflow` error on overflow; `saturating_add` for ledger sequence arithmetic; `overflow-checks = true` in both dev and release Cargo profiles |
| T-07 | Re-entrancy via `debit_spend` or `execute_batch` | Elevation of Privilege | Low | Medium | Defense-in-depth storage lock (`DataKey::Executing`) set on entry and cleared on success; Soroban VM also prevents recursive same-contract calls at the host level |

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

### 4.5 Supply Chain

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
| `overflow-checks = true` in dev and release profiles | Cargo.toml |
| `checked_add` for spend accumulation | `mux-account::debit_spend` |
| `saturating_add` for ledger sequence arithmetic | `mux-account::set_spend_limit`, `debit_spend` |
| `DataKey::Executing` reentrancy guard | `mux-account::debit_spend`, `mux-batcher::execute_batch` |
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
| 2026-05-30 | 0.2.0 | Added reentrancy guard (T-07 updated); added checked/saturating arithmetic (T-06 updated); overflow-checks enabled for dev profile |
