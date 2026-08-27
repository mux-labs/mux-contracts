# Mux Protocol — Threat Model

**Version:** 0.2.0  
**Date:** 2026-08-26  
**Status:** Living document — update whenever contracts or trust boundaries change.

---

## 1. Scope

This document covers **all ten on-chain Soroban contracts** that make up Mux Protocol. Every crate under `contracts/` that ships WASM is in scope:

| Contract | Responsibility |
|---|---|
| `mux-account` | Account abstraction, delegate management, spend limits, session keys |
| `mux-account-factory` | Deterministic deployment and registration of account instances |
| `mux-batcher` | Atomic multi-operation batching |
| `mux-delegation` | Grant / revoke permission-scoped signing authority to delegates |
| `mux-permissions` | RBAC registry (roles, permissions, grant/revoke, multisig admin rotation) |
| `mux-policy` | Per-wallet daily spend-limit policy with auto-reset |
| `mux-recovery` | M-of-N guardian account recovery with timelock |
| `mux-registry` | Contract version and metadata registry |
| `mux-spending-policy` | Per-account/per-asset spend-limit policy and validation |
| `mux-wallet-registry` | Named wallet registry for address lookup |

The `soroban-test-helpers` crate is a test utility (`rlib` only, no WASM) and is out of scope. Off-chain components (TypeScript SDK, frontend, deployment scripts) are out of scope for on-chain threat analysis but are noted where they affect trust boundaries.

> **Coverage is enforced:** `tests/threat_model_coverage.rs` fails CI if any `contracts/mux-*` crate is not named in this document, so a new contract can never ship without a threat-model entry.

---

## 2. Assets

| Asset | Description | Impact if Compromised |
|---|---|---|
| Owner private key | Controls the `mux-account` contract | Full account takeover |
| Admin keypair | Controls `mux-permissions`, `mux-registry`, `mux-policy`, `mux-spending-policy` | Role assignments, registry entries, and policy limits can be forged |
| Delegate list | Set of authorized sub-signers (`mux-account`, `mux-delegation`) | Unauthorized spending or operations |
| Session keys | Short-lived delegated signing keys with per-method scopes | Unauthorized actions within the granted window |
| Spend limits | Per-asset caps on delegate spending (`mux-account`, `mux-policy`, `mux-spending-policy`) | Financial loss |
| Guardian set | Recovery addresses for the account (`mux-account`, `mux-recovery`) | Account recovery hijacked |
| Recovery request | Pending M-of-N recovery with timelock window | Account takeover via forged quorum |
| Account registry | Factory's per-owner account list + metadata | Account discovery poisoning |
| Wallet registry | Named → address mapping | Name squatting / address hijack |
| Contract registry | Version/metadata records | Supply-chain confusion for integrators |
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
│  │  ┌──────────┐ ┌──────────┐ ┌───────┐ │   │
│  │  │mux-fac   │ │mux-dlg   │ │mux-recv│ │   │
│  │  └──────────┘ └──────────┘ └───────┘ │   │
│  │  ┌──────────┐ ┌──────────┐ ┌───────┐ │   │
│  │  │mux-reg   │ │mux-pol   │ │mux-spnd│ │   │
│  │  └──────────┘ └──────────┘ └───────┘ │   │
│  │  ┌──────────┐                        │   │
│  │  │mux-wreg  │                        │   │
│  │  └──────────┘                        │   │
│  └───────────────────────────────────────┘   │
└────────────────────────────────────────────── ┘
```

**Key boundary:** Anything outside the Soroban VM is untrusted. Auth checks (`require_auth`) enforce that callers have signed the transaction with the expected keypair. Each contract stores its own admin/owner and enforces it independently; no contract reads or writes another contract's storage directly (all cross-contract effects go through `invoke_contract`).

---

## 4. Threats and Mitigations

### 4.1 Account Takeover (`mux-account`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-01 | Owner key compromise | Spoofing | Medium | Critical | Guardian recovery set; hardware wallet recommendation; time-locked admin operations |
| T-02 | Delegate key compromise | Spoofing | Medium | High | Spend limits cap damage; time-bounded delegate expiry (`expires_at`) |
| T-03 | Delegate expiry not enforced | Elevation of Privilege | Low | High | `expires_at` checked against ledger time on every invocation; stale delegates rejected |
| T-04 | Guardian collusion | Spoofing | Low | Critical | M-of-N guardian quorum enforced on-chain — `execute_recovery` requires `approvals.len() >= quorum_threshold`; threshold is set at initialization and adjustable by owner via `set_quorum_threshold` |

### 4.2 Unauthorized Spending (`mux-account`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-05 | Spend limit bypass via period reset manipulation | Elevation of Privilege | Low | High | Reset ledger set at initialization; only the contract increments it |
| T-06 | Integer overflow in spend accounting | Tampering | Low | High | `checked_add` in `debit_spend` returns `ArithmeticOverflow` error on overflow; `saturating_add` for ledger sequence arithmetic; `overflow-checks = true` in both dev and release Cargo profiles |
| T-07 | Re-entrancy via `debit_spend` or `execute_batch` | Elevation of Privilege | Low | Medium | Defense-in-depth storage lock (`DataKey::Executing`) set on entry and cleared on success; Soroban VM also prevents recursive same-contract calls at the host level |
| T-40 | Session key with empty scopes silently accepted | Elevation of Privilege | Low | Medium | **Fail-closed scope enforcement:** `execute_with_session` rejects any session key whose `scopes` list is empty (`Unauthorized`), and rejects an invoked method that is not named in a non-empty `scopes` list (`ScopeNotGranted`). A key granted zero capabilities must not be able to execute anything, and a granted capability list is not a blanket permit. Covered by unit tests `test_execute_with_session_rejects_empty_scopes` and `test_execute_with_session_rejects_method_outside_scopes`. |

### 4.3 Batch Execution Abuse (`mux-batcher`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-08 | Gas griefing via oversized batch | Denial of Service | Medium | Medium | `MAX_BATCH_SIZE = 50` hard cap enforced before execution |
| T-09 | Required-op failure ignored | Tampering | Low | High | `require_success` flag panics the transaction, rolling back all operations |
| T-10 | Cross-contract call to malicious contract | Tampering | Medium | High | Caller is authenticated; target contracts are user-supplied — document that callers must vet targets |

### 4.4 Permission Registry (`mux-permissions`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-11 | Admin key compromise | Elevation of Privilege | Low | Critical | Admin key should be a multisig account; rotate post-deployment; `set_admin_threshold` / `propose_admin` / `approve_admin` enable on-chain multisig rotation |
| T-12 | Role granted to wrong address | Tampering | Medium | High | Admin-only `grant_role`; all operations emit events (Soroban events) |
| T-13 | Stale role membership | Information Disclosure | Low | Low | `get_role_members` always returns current state from storage |

### 4.5 Storage Griefing

All contracts use **instance storage** (and, for `mux-policy` / `mux-delegation`, per-entry persistent storage), which is shared across all callers and billed as rent units. Unbounded growth in any collection raises rent costs for every user of the contract and can eventually make the contract economically unviable.

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-17 | Owner floods delegate map to bloat instance storage | Denial of Service | Low | Medium | `MAX_DELEGATES = 64` hard cap in `set_delegate`; new entries beyond cap return `TooManyDelegates` |
| T-18 | Admin floods a role's member list | Denial of Service | Low | Medium | `MAX_ROLE_MEMBERS = 256` cap in `grant_role`; returns `TooManyMembers` |
| T-19 | Admin assigns excessive roles to one account | Denial of Service | Low | Low | `MAX_ROLES_PER_ACCOUNT = 32` cap in `grant_role`; returns `TooManyRoles` |
| T-20 | Spend limits accumulate unbounded per-asset keys | Denial of Service | Low | Low | Each asset key is a separate instance entry; owner controls which assets are registered; no public write path |
| T-21 | Instance storage TTL expiry causes silent data loss | Denial of Service | Medium | High | Callers must extend TTL via `env.storage().instance().extend_ttl()`; document minimum TTL extension in deployment runbook |
| T-22 | Owner floods wallet registry with distinct names | Denial of Service | Low | Medium | `MAX_WALLETS = 128` in `register_wallet*`; returns `TooManyWallets` |
| T-23 | Owner floods session key index for an account | Denial of Service | Low | Low | `MAX_SESSION_KEYS = 32` in `require_session_key_cap`; returns `TooManySessionKeys` |
| T-45 | Owner floods factory per-owner account list | Denial of Service | Low | Medium | `MAX_ACCOUNTS_PER_OWNER = 64` in `deploy_account*`; returns `TooManyAccounts` |
| T-46 | Admin floods delegation maps | Denial of Service | Low | Medium | `MAX_DELEGATES_PER_OWNER = 128` and `MAX_DELEGATE_PERMS = 64` in `grant_delegate` |
| T-47 | Admin floods policy wallet list | Denial of Service | Low | Medium | `MAX_WALLETS = 256` in `set_daily_limit`; returns `TooManyWallets` |
| T-48 | Admin floods registry contract list | Denial of Service | Low | Medium | `MAX_CONTRACTS = 128` in `register*`; returns `TooManyContracts` |
| T-49 | Guardian set bloat | Denial of Service | Low | Low | `MAX_GUARDIANS = 16` in `add_guardian`; returns `TooManyGuardians` |

**Storage sizing reference (approximate):**

| Collection | Entry size | Cap | Max storage |
|---|---|---|---|
| `Delegates` map (`mux-account`) | ~72 bytes/entry | 64 | ~4.6 KB |
| `SessionKeyIndex` vec (`mux-account`) | ~32 bytes/entry | 32 | ~1 KB |
| `Accounts` vec (`mux-account-factory`, per owner) | ~32 bytes/entry | 64 | ~2 KB |
| `RoleMembers` vec (`mux-permissions`) | ~32 bytes/entry | 256 | ~8 KB |
| `AccountRoles` vec (`mux-permissions`) | ~8 bytes/entry | 32 | ~256 bytes |
| `OwnerDelegates` vec (`mux-delegation`, per owner) | ~32 bytes/entry | 128 | ~4 KB |
| `DelegatePerms` vec (`mux-delegation`, per pair) | ~8 bytes/entry | 64 | ~0.5 KB |
| `Names` vec (`mux-registry`) | ~16 bytes/entry | 128 | ~2 KB |
| `Names` vec (`mux-wallet-registry`) | ~16 bytes/entry | 128 | ~2 KB |
| `WalletNames` vec (`mux-policy`) | ~42–50 bytes/entry | 256 | ~12 KB |
| `SpendLimit` per asset (`mux-account`, `mux-spending-policy`) | ~80 bytes/entry | owner/admin-controlled | bounded by privileged writers only |
| `Guardians` vec (`mux-recovery`) | ~32 bytes/entry | 16 | ~0.5 KB |

> See [docs/storage-griefing.md](storage-griefing.md) for full mitigation details, TTL constants, and the deployment keeper runbook.

### 4.6 Supply Chain

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-14 | Malicious npm package publish | Tampering | Low | High | npm provenance attestation in CI; scoped package name `@mux-protocol/contracts` |
| T-15 | WASM tampering before deployment | Tampering | Low | Critical | SHA-256 of compiled WASM published in release notes; reproduce from source |
| T-16 | Dependency confusion attack | Tampering | Low | High | Scoped npm package; Cargo.lock pinned; Dependabot alerts enabled |

### 4.7 Account Factory (`mux-account-factory`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-24 | Unauthorized account registration | Spoofing | Low | Medium | `owner.require_auth()` called per deploy; `simulate_deploy*` dry-runs are read-only |
| T-25 | Factory `upgrade()` hijack | Elevation of Privilege | Low | Critical | `upgrade()` requires the stored `DataKey::Admin` auth; `NotInitialized` (fail-closed) if `initialize` was never called |

### 4.8 Delegation (`mux-delegation`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-26 | Delegate permission escalation | Elevation of Privilege | Low | High | `grant_delegate` requires `owner.require_auth()`; grants are replace-only (no append), so a delegate cannot self-grant additional permissions |
| T-27 | Stale / unrevoked grants | Tampering | Low | Medium | `revoke_delegate` removes the full permission set; grants are permission-scoped `Symbol`s vetted by the application layer |
| T-28 | `link_contract_id` identity spoofing | Spoofing | Low | Medium | Caller-supplied `admin` must authorize itself (`admin.require_auth()`), and the link is **write-once** (`ContractIdAlreadySet`); documented as self-gated, not a stored-admin gate — see [delegation-upgrade.md](delegation-upgrade.md) |
| T-29 | `mux-delegation` upgrade hijack | Elevation of Privilege | Low | Critical | `upgrade()` requires stored `DataKey::Admin` auth; `NotInitialized` (fail-closed) if `initialize` was never called |

### 4.9 Daily Spend Policy (`mux-policy`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-30 | Admin limit manipulation | Elevation of Privilege | Low | High | `set_daily_limit` and `reset_daily_counter` are admin-only (`require_admin`) |
| T-31 | Cross-wallet debit | Spoofing | Low | High | `record_spend` requires `wallet.require_auth()` — a third party cannot debit another wallet's allowance |
| T-32 | Registry validation bypass | Tampering | Low | Medium | When `registry_id` is set, `record_spend` calls the registry and fails closed (`RegistryNotFound`) if it is unreachable or errors |

### 4.10 Recovery (`mux-recovery`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-33 | Quorum bypass | Elevation of Privilege | Low | Critical | `execute_recovery` requires `approvals.len() >= quorum_threshold`; `DuplicateApproval` rejects double-votes; threshold validated at init (`1 <= t <= guardians.len()`) |
| T-34 | Timelock bypass | Tampering | Low | Critical | `execute_recovery` checks `executable_at` (`initiated_at + RECOVERY_TIMELOCK`); owner can `cancel_recovery()` during the window |
| T-35 | Admin/owner+guardian bypass | Elevation of Privilege | Low | Critical | `approve_recovery_admin` requires **both** `owner.require_auth()` and a registered co-guardian (`co_guardian.require_auth()` + membership check) |
| T-36 | Expired recovery request | Denial of Service | Low | Medium | `RECOVERY_EXPIRY` (120,960 ledgers ≈ 7d) bounds the window; stale `Pending` requests are overwritten by the next `initiate_recovery` |

### 4.11 Registry (`mux-registry`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-37 | Registry poisoning | Tampering | Low | High | `register` / `register_with_metadata` are admin-only; `MAX_CONTRACTS = 128` cap; all entries emit `reg` / `regmeta` events |

### 4.12 Spending Policy (`mux-spending-policy`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-38 | Policy manipulation | Elevation of Privilege | Low | High | `set_policy` is admin-only (`require_admin`); `check_spend` is read-only (no auth, no storage write on the check itself) |
| T-39 | Window reset manipulation | Elevation of Privilege | Low | Low | `reset_ledger` is advanced only by the contract from the current ledger sequence |

### 4.13 Wallet Registry (`mux-wallet-registry`)

| # | Threat | STRIDE | Likelihood | Impact | Mitigation |
|---|--------|--------|------------|--------|------------|
| T-41 | Name squatting / wallet hijack | Spoofing | Low | Medium | `register_wallet*` requires `owner.require_auth()`; `MAX_WALLETS = 128` cap; existing names are overwritten only by the owner |

---

## 5. Security Controls

| Control | Where Applied |
|---|---|
| `require_auth()` on all write operations | All ten contracts |
| `overflow-checks = true` in dev and release profiles | Cargo.toml |
| `checked_add` for spend accumulation | `mux-account::debit_spend` |
| `saturating_add` for ledger sequence arithmetic | `mux-account::set_spend_limit`, `debit_spend`; `mux-recovery` timelock/expiry |
| `DataKey::Executing` reentrancy guard | `mux-account::debit_spend`, `mux-account::execute`, `mux-batcher::execute_batch` |
| `MAX_BATCH_SIZE` cap | `mux-batcher` |
| Delegate `expires_at` timestamp | `mux-account` |
| Spend limit period reset via ledger sequence | `mux-account`, `mux-policy`, `mux-spending-policy` |
| **Fail-closed session-scope enforcement** | `mux-account::execute_with_session` (T-40) |
| M-of-N guardian quorum + timelock | `mux-recovery` |
| Admin-only registry / policy / role writes | `mux-permissions`, `mux-registry`, `mux-policy`, `mux-spending-policy`, `mux-wallet-registry`, `mux-account-factory` (upgrade), `mux-delegation` (upgrade) |
| Wallet-only spend recording | `mux-policy::record_spend` |
| Storage caps (`MAX_*`) with dedicated error variants | All collection-backed contracts |
| npm provenance (`--provenance`) | CI publish job |
| Drift check: committed bindings vs generated | CI `check-binding-drift` job |
| RBAC admin-only mutation | `mux-permissions` |

---

## 6. Out-of-Scope / Residual Risks

- **Stellar network-level attacks** (consensus failures, validator collusion) — outside contract scope.
- **RPC node trust** — users should use multiple RPC endpoints or run their own node.
- **Frontend key management** — private keys in browser localStorage are a known risk; hardware wallets are recommended.
- **Upgrade authority** — `mux-account` is immutable by design (no `upgrade()` will be added; see [account-upgrade-migration.md](account-upgrade-migration.md)); the other contracts gate `upgrade()` behind a stored admin/owner, but a compromised admin key is still catastrophic. Consider time-lock or DAO governance for admin keys on mainnet.
- **Session scopes match methods, not targets** — `execute_with_session` dispatches to the caller-supplied `target` after matching `function` against the session key's `scopes`. A key scoped to `pay` may therefore call `pay` on any contract address the caller supplies; target-scoped sessions remain future work (see [aa_sequence_diagram.md](aa_sequence_diagram.md)).
- **Session execution has no spend accounting** — per-asset spend limits are enforced on the owner-authorized `execute` path only. A target invoked through a session key must call back into `debit_spend`, which the held reentrancy guard rejects for the duration of the call.
- **Sponsor allowlist is owner-managed** — an owner that allowlists a malicious relayer gains no protection from the contract beyond the session key's own scopes; the relayer still cannot exceed them (see [relayer-integration.md](relayer-integration.md)).
- **`mux-delegation::link_contract_id`** is self-gated (the caller-supplied `admin` authorizes itself) rather than checked against the stored upgrade admin — a design tradeoff documented in [delegation-upgrade.md](delegation-upgrade.md).
- **`mux-policy` persistent-storage TTL** — `WalletLimit` records (and `mux-delegation`'s `DelegatePerms`/`OwnerDelegates`) live in persistent storage with independent TTLs; they are extended on write but still need keeper attention (see [access-control-checklist.md](access-control-checklist.md) §6a).
- **Caller responsibility for batch targets** — `mux-batcher` executes user-supplied target contracts; callers must vet targets (T-10).

---

## 7. Revision History

| Date | Version | Change |
|---|---|---|
| 2026-05-30 | 0.1.0 | Initial threat model |
| 2026-05-30 | 0.1.1 | Storage griefing: added T-21 TTL expiry threat; added `extend_ttl` mitigation in all contracts; added `docs/storage-griefing.md` |
| 2026-05-30 | 0.1.2 | Added `docs/audit-prep.md` — scope, entry points, known limitations, auditor checklist |
| 2026-08-26 | 0.2.0 | **Expanded to all ten production contracts** — previously covered only `mux-account`, `mux-batcher`, `mux-permissions`. Added §4.7 (factory), §4.8 (delegation), §4.9 (policy), §4.10 (recovery), §4.11 (registry), §4.12 (spending policy), §4.13 (wallet registry); added storage-griefing rows T-45…T-49; added T-40 fail-closed session-scope enforcement (`execute_with_session` rejects empty-scope keys) and its unit test; added threat-model coverage guard (`tests/threat_model_coverage.rs`) |
