# Mux Protocol — External Audit Preparation

**Version:** 0.1.0  
**Date:** 2026-05-30  
**Status:** Living document — update before each audit engagement.

---

## 1. Audit Scope

### In scope

| Contract | Source file | Description |
|---|---|---|
| `mux-account` | `contracts/mux-account/src/lib.rs` | Account abstraction: owner, delegates, spend limits, guardian set |
| `mux-batcher` | `contracts/mux-batcher/src/lib.rs` | Atomic multi-operation batching |
| `mux-permissions` | `contracts/mux-permissions/src/lib.rs` | RBAC registry: roles, permissions, grant/revoke |

### Out of scope

- TypeScript bindings (`bindings/`) — generated code, no on-chain logic
- Deployment scripts (`scripts/`) — operational tooling
- GitHub Actions workflows (`.github/workflows/`) — CI/CD infrastructure
- Stellar network consensus and validator behaviour
- Frontend / DApp key management

---

## 2. Repository Snapshot

Before handing off to auditors, produce a pinned snapshot:

```bash
# Tag the commit under review
git tag audit/v0.1.0
git push origin audit/v0.1.0

# Record the exact commit SHA in this document
# Commit: <fill in>
```

Build reproducible WASM artifacts and publish their SHA-256 hashes in the release notes:

```bash
cargo build --target wasm32-unknown-unknown --release --workspace

sha256sum target/wasm32-unknown-unknown/release/mux_account.wasm
sha256sum target/wasm32-unknown-unknown/release/mux_batcher.wasm
sha256sum target/wasm32-unknown-unknown/release/mux_permissions.wasm
```

Auditors should reproduce these hashes from source to confirm the WASM matches the reviewed code.

---

## 3. Build & Test Instructions

```bash
# Prerequisites: Rust stable toolchain, wasm32-unknown-unknown target
rustup target add wasm32-unknown-unknown

# Run all unit tests
cargo test --workspace --all-features

# Lint (must pass with zero warnings)
cargo clippy --workspace --all-features -- -D warnings
cargo fmt --all -- --check

# Build release WASMs
cargo build --target wasm32-unknown-unknown --release --workspace
```

CI enforces all of the above on every PR via `.github/workflows/ci.yml`.

---

## 4. Architecture Summary

All three contracts are independent Soroban contracts deployed on Stellar.  They share no storage and communicate only through explicit cross-contract calls initiated by callers.

```
Off-chain caller (DApp / backend)
        │
        │  XDR transaction (signed)
        ▼
┌───────────────────────────────────────────┐
│  Soroban VM                               │
│                                           │
│  mux-account          mux-permissions     │
│  ┌──────────────┐     ┌───────────────┐   │
│  │ owner        │     │ admin         │   │
│  │ delegates    │     │ roles         │   │
│  │ spend limits │     │ permissions   │   │
│  │ guardians    │     └───────────────┘   │
│  └──────────────┘                         │
│                                           │
│  mux-batcher                              │
│  ┌──────────────────────────────────┐     │
│  │ execute_batch(caller, ops[])     │     │
│  │  └─ try_invoke_contract(target)  │     │
│  └──────────────────────────────────┘     │
└───────────────────────────────────────────┘
```

### Storage model

All contracts use **instance storage** exclusively.  Instance storage is billed as a single rent unit and shared across all callers of a given contract instance.  See [storage-griefing.md](storage-griefing.md) for cap details and TTL management.

### Auth model

Every state-mutating entry point calls `require_auth()` on the expected signer before touching storage.  Read-only entry points (`owner`, `delegates`, `guardians`, `has_permission`, `get_roles`, `get_role_members`) require no auth.

---

## 5. Entry Points

### mux-account

| Function | Auth required | Mutates storage | Notes |
|---|---|---|---|
| `initialize(owner, guardians)` | `owner` | Yes | One-time; `AlreadyInitialized` on re-call |
| `set_delegate(delegate, expiry_ledger, can_spend)` | owner | Yes | Capped at `MAX_DELEGATES = 64` |
| `remove_delegate(delegate)` | owner | Yes | `DelegateNotFound` if absent |
| `set_spend_limit(asset, amount, period_ledgers)` | owner | Yes | `amount > 0`, `period_ledgers > 0` |
| `debit_spend(asset, spend)` | contract itself | Yes | Period auto-resets via ledger sequence |
| `owner()` | none | No | |
| `delegates()` | none | No | |
| `guardians()` | none | No | |

### mux-batcher

| Function | Auth required | Mutates storage | Notes |
|---|---|---|---|
| `execute_batch(caller, ops)` | `caller` | No (TTL only) | `1 ≤ ops.len() ≤ 50`; `require_success` aborts on failure |
| `simulate_batch(caller, ops)` | `caller` | No | Conservative preflight; does not invoke targets |

### mux-permissions

| Function | Auth required | Mutates storage | Notes |
|---|---|---|---|
| `initialize(admin)` | `admin` | Yes | One-time |
| `create_role(role, permissions)` | admin | Yes | Idempotent on re-create (overwrites) |
| `grant_role(account, role)` | admin | Yes | Capped: 256 members/role, 32 roles/account |
| `revoke_role(account, role)` | admin | Yes | `AccountNotInRole` if absent |
| `has_permission(account, permission)` | none | No | |
| `get_roles(account)` | none | No | |
| `get_role_members(role)` | none | No | `RoleNotFound` if role unknown |

---

## 6. Known Limitations and Residual Risks

These items are known before the audit and are documented here so auditors can focus effort appropriately.

| # | Item | Status |
|---|---|---|
| L-01 | `debit_spend` is auth-gated to `current_contract_address()` — it can only be called by the contract itself, not by delegates directly. The mechanism for a delegate to trigger a spend debit is not yet implemented. | Acknowledged; out of scope for v0.1 |
| L-02 | Guardian recovery (`guardians` field) is stored but no recovery flow is implemented. The guardian set is a placeholder for a future M-of-N recovery mechanism. | Acknowledged; out of scope for v0.1 |
| L-03 | `simulate_batch` returns a conservative estimate (all ops succeed) without actually invoking targets. It does not detect auth failures or contract errors in advance. | By design; documented in code |
| L-04 | `mux-batcher` does not validate that target contracts are non-malicious. Callers are responsible for vetting targets. | By design; documented in threat model T-10 |
| L-05 | No upgrade mechanism exists. Contracts are immutable once deployed. A compromised or buggy contract requires redeployment and migration. | Acknowledged; upgrade governance is future roadmap |
| L-06 | `SpendLimit` keys are unbounded in count (one per asset). Only the owner can write them, so the attack surface is limited to a self-griefing owner. | Acknowledged; see threat model T-20 |

---

## 7. Security Controls Summary

| Control | Contract(s) | Where to verify |
|---|---|---|
| `require_auth()` on all writes | All | Every mutating function; see §5 |
| `AlreadyInitialized` guard | `mux-account`, `mux-permissions` | `initialize` functions |
| `overflow-checks = true` in release | All | `Cargo.toml` `[profile.release]` |
| `panic = "abort"` in release | All | `Cargo.toml` `[profile.release]` |
| Delegate expiry (`expiry_ledger`) | `mux-account` | `set_delegate` / `DelegateInfo` struct |
| Spend limit period reset via ledger sequence | `mux-account` | `debit_spend` |
| `MAX_BATCH_SIZE = 50` | `mux-batcher` | `execute_batch` |
| `MAX_DELEGATES = 64` | `mux-account` | `set_delegate` |
| `MAX_ROLE_MEMBERS = 256` | `mux-permissions` | `grant_role` |
| `MAX_ROLES_PER_ACCOUNT = 32` | `mux-permissions` | `grant_role` |
| Instance TTL extension on every write | All | `extend_ttl` helper / inline call |
| Soroban events on every state change | All | `emit` helper; see [audit-events.md](audit-events.md) |
| npm provenance attestation | Bindings CI | `.github/workflows/bindings.yml` publish job |
| Binding drift check on PRs | Bindings CI | `check-binding-drift` job |

---

## 8. Test Coverage

Run the full suite:

```bash
cargo test --workspace --all-features
```

| Contract | Test | What it covers |
|---|---|---|
| `mux-account` | `test_initialize` | Happy-path init, owner readable |
| | `test_double_initialize_fails` | `AlreadyInitialized` guard |
| | `test_set_and_remove_delegate` | Delegate CRUD |
| | `test_spend_limit_enforcement` | Limit respected; over-limit rejected |
| | `test_spend_limit_invalid_amount` | `amount = 0` rejected |
| | `test_delegate_cap_enforced` | 65th new delegate rejected |
| | `test_delegate_cap_allows_update` | Update at cap succeeds |
| | `test_initialize_emits_event` | `init` event emitted |
| | `test_set_delegate_emits_event` | `dlg_set` event emitted |
| | `test_remove_delegate_emits_event` | `dlg_rm` event emitted |
| | `test_spend_limit_emits_events` | `lmt_set` + `debited` events |
| | `test_ttl_extended_on_write` | TTL extension does not panic |
| `mux-batcher` | `test_empty_batch_rejected` | `EmptyBatch` error |
| | `test_batch_too_large_rejected` | `BatchTooLarge` error (51 ops) |
| | `test_execute_batch_emits_event` | `executed` event emitted |
| | `test_ttl_extended_on_execute_batch` | TTL extension does not panic |
| `mux-permissions` | `test_initialize` | Happy-path init |
| | `test_double_initialize_fails` | `AlreadyInitialized` guard |
| | `test_create_and_grant_role` | Role creation + grant + permission check |
| | `test_revoke_role_removes_permission` | Revoke clears permission |
| | `test_grant_nonexistent_role_fails` | `RoleNotFound` error |
| | `test_role_member_cap_enforced` | 257th member rejected |
| | `test_roles_per_account_cap_enforced` | 33rd role rejected |
| | `test_initialize_emits_event` | `init` event emitted |
| | `test_role_lifecycle_emits_events` | `role_crt` + `role_grt` + `role_rev` events |
| | `test_ttl_extended_on_write` | TTL extension does not panic |

### Coverage gaps (known before audit)

- No negative auth tests (unauthorized caller attempting a write). All current tests use `mock_all_auths()`.
- No test for `debit_spend` period rollover across ledger sequences.
- No test for `simulate_batch` with an oversized batch.
- No integration test exercising `mux-batcher` calling into `mux-account` or `mux-permissions`.

---

## 9. Reference Documents

| Document | Purpose |
|---|---|
| [threat-model.md](threat-model.md) | STRIDE threat analysis, assets, trust boundaries |
| [access-control-checklist.md](access-control-checklist.md) | Pre-deployment and pre-audit checklist |
| [storage-griefing.md](storage-griefing.md) | Storage cap rationale, TTL constants, keeper runbook |
| [audit-events.md](audit-events.md) | On-chain event schema for all contracts |

---

## 10. Auditor Checklist

Items the audit team should verify independently:

- [ ] All `require_auth()` calls are on the correct signer (not a weaker or wrong address).
- [ ] `debit_spend` cannot be called by an external account — only by the contract itself.
- [ ] Spend limit period reset cannot be manipulated by a caller to reset early.
- [ ] `execute_batch` with `require_success = true` rolls back the entire transaction on failure.
- [ ] Storage caps (`MAX_DELEGATES`, `MAX_ROLE_MEMBERS`, `MAX_ROLES_PER_ACCOUNT`) are enforced before insertion, not after.
- [ ] No integer overflow path exists in spend accounting (`spent + spend`).
- [ ] `AlreadyInitialized` guard prevents state overwrite on both contracts.
- [ ] Instance TTL is extended on every write; no write path skips `extend_ttl`.
- [ ] No contract reads or writes another contract's storage directly.
- [ ] Release WASM does not include `testutils` or `#[cfg(test)]` code (verify with `wasm-objdump`).
- [ ] Error discriminants start at 1; no variant uses 0.
- [ ] Known limitations in §6 are acceptable for the current deployment scope.
