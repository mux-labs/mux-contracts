# Mux Protocol — Access Control Review Checklist

**Version:** 0.1.0  
**Date:** 2026-05-30  
**Purpose:** Use this checklist before every contract release, audit engagement, or major feature PR to verify that access control is correctly enforced across all Mux Protocol contracts.

---

## How to Use

Work through each section.  Mark every item **Pass**, **Fail**, or **N/A** with a brief note.  All items must be **Pass** or **N/A** before a contract deployment is approved.

```
Legend:
  [x] Pass
  [ ] Fail — add remediation note
  [-] N/A  — explain why
```

---

## 1. Authentication (`require_auth`)

### 1.1 `mux-account`

- [ ] `initialize` — `owner.require_auth()` called before any storage write.
- [ ] `set_delegate` — `require_owner` helper called; verifies `owner.require_auth()`.
- [ ] `remove_delegate` — `require_owner` helper called.
- [ ] `set_spend_limit` — `require_owner` helper called.
- [ ] `debit_spend` — `current_contract_address().require_auth()` called (contract-internal only).
- [ ] No public function mutates storage without an auth check.

### 1.2 `mux-batcher`

- [ ] `execute_batch` — `caller.require_auth()` called before any operations are dispatched.
- [ ] `simulate_batch` — `caller.require_auth()` called (preflight is also auth-gated).
- [ ] Batch operations are dispatched under the **caller's** auth context, not the batcher contract's.

### 1.3 `mux-permissions`

- [ ] `initialize` — `admin.require_auth()` called before storage write.
- [ ] `create_role` — `require_admin` helper called.
- [ ] `grant_role` — `require_admin` helper called.
- [ ] `revoke_role` — `require_admin` helper called.
- [ ] `has_permission`, `get_roles`, `get_role_members` — read-only; no auth required (acceptable).
- [ ] No role mutation is possible without admin signature.

---

## 2. Initialization Guards

- [ ] `mux-account`: Second call to `initialize` returns `AlreadyInitialized` error; verified by unit test `test_double_initialize_fails`.
- [ ] `mux-permissions`: Second call to `initialize` returns `AlreadyInitialized` error; verified by unit test `test_double_initialize_fails`.
- [ ] No contract function silently overwrites initialized state on re-call.
- [ ] All contracts check `env.storage().instance().has(&DataKey::Owner/Admin)` before setting it.

---

## 3. Role and Delegate Validation

- [ ] `grant_role` rejects unknown role names (`RoleNotFound` error).
- [ ] `revoke_role` rejects accounts not in the role (`AccountNotInRole` error).
- [ ] `set_delegate` stores a well-typed `DelegateInfo` struct; no raw address coercion.
- [ ] `remove_delegate` returns `DelegateNotFound` rather than silently succeeding.
- [ ] Delegate `expiry_ledger` is enforced at call time, not just at creation time.
- [ ] `can_spend` flag is correctly propagated to spend-limit checks.

---

## 4. Spend Limit Controls

- [ ] Spend limit amount must be > 0; `InvalidAmount` returned otherwise (unit test: `test_spend_limit_invalid_amount`).
- [ ] Period ledgers must be > 0; `InvalidPeriod` returned otherwise.
- [ ] `debit_spend` rolls over the period counter using `env.ledger().sequence()` — no off-chain clock dependency.
- [ ] Accumulated `spent` is reset to 0 at period boundary, not merely decremented.
- [ ] `spent + spend > amount` check uses Rust checked arithmetic (overflow-checks = true in profile).
- [ ] Spend limit is per-asset; different assets cannot cross-cover each other.

---

## 5. Batch Execution Safety

- [ ] Empty batch (`ops.is_empty()`) returns `EmptyBatch`; transaction reverts.
- [ ] Batch size > `MAX_BATCH_SIZE` (50) returns `BatchTooLarge`; transaction reverts.
- [ ] `require_success = true` operations abort the entire batch on failure (not just skip).
- [ ] `require_success = false` operations record failure count without aborting.
- [ ] Cross-contract invocations inside the batch cannot re-enter `mux-batcher` itself.
- [ ] The caller of `execute_batch` is documented to be responsible for vetting target contracts.

---

## 6. Storage Isolation

- [ ] Each contract uses its own `DataKey` enum with no overlapping key names across contracts.
- [ ] All storage reads use `ok_or(SomeError::NotInitialized)` — no silent `unwrap` that could panic post-deployment.
- [ ] Persistent storage keys are namespaced by type (e.g., `SpendLimit(Address)` vs `Delegates`).
- [ ] No contract reads or writes to another contract's storage directly.

---

## 6a. Storage Griefing Caps

See [docs/storage-griefing.md](storage-griefing.md) for full details.

- [ ] `mux-account`: `set_delegate` enforces `MAX_DELEGATES = 64`; new entries beyond cap return `TooManyDelegates` (unit test: `test_delegate_cap_enforced`).
- [ ] `mux-account`: updating an existing delegate at cap succeeds (unit test: `test_delegate_cap_allows_update`).
- [ ] `mux-permissions`: `grant_role` enforces `MAX_ROLE_MEMBERS = 256` per role; returns `TooManyMembers` (unit test: `test_role_member_cap_enforced`).
- [ ] `mux-permissions`: `grant_role` enforces `MAX_ROLES_PER_ACCOUNT = 32` per account; returns `TooManyRoles` (unit test: `test_roles_per_account_cap_enforced`).
- [ ] All three contracts call `env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO)` on every write (T-21 mitigation).
- [ ] TTL constants: `TTL_THRESHOLD = 17_280` (~1 day), `TTL_EXTEND_TO = 518_400` (~30 days).
- [ ] Deployment runbook includes a keeper job that extends TTL at least every 25 days (see [docs/storage-griefing.md](storage-griefing.md#deployment-runbook--ttl-keeper)).

---

## 7. Error Handling

- [ ] All error types are `#[contracttype]` decorated enums with explicit `#[repr(u32)]` discriminants.
- [ ] No error arm uses discriminant 0 (reserved for success in some SDKs).
- [ ] Errors are propagated via `Result<_, Error>` — no `panic!` except in `require_success` abort path.
- [ ] Error codes are stable across contract versions (no re-numbering without a major version bump).

---

## 8. Unit Test Coverage

- [ ] `mux-account`: `initialize`, double-initialize, delegate CRUD, spend limit enforcement, invalid amount/period.
- [ ] `mux-batcher`: empty batch, oversized batch.
- [ ] `mux-permissions`: initialize, double-initialize, role create/grant/revoke, permission check, nonexistent role grant.
- [ ] All `require_owner` / `require_admin` paths have a negative test (unauthorized caller).
- [ ] All `AlreadyInitialized` paths have a test.
- [ ] CI runs `cargo test --workspace --all-features` on every PR (see `.github/workflows/ci.yml`).

---

## 9. CI / CD Verification

- [ ] `cargo clippy --workspace --all-features -- -D warnings` passes with no warnings.
- [ ] `cargo fmt --check` passes.
- [ ] Bindings drift check (`check-binding-drift` job) passes on PRs.
- [ ] Release builds use `[profile.release]` with `overflow-checks = true` and `panic = "abort"`.
- [ ] WASM artifacts are uploaded and SHA-256 is published in the release notes.

---

## 10. Deployment Checklist

- [ ] Admin / owner keypairs generated on HSM or hardware wallet — not software-only.
- [ ] Admin keypair for `mux-permissions` is a Stellar multisig account with threshold ≥ 2.
- [ ] Initial guardian set contains ≥ 3 geographically distributed addresses.
- [ ] Contract IDs recorded in `bindings/src/network.ts` for the correct network.
- [ ] `stellar contract invoke` smoke-test run against testnet deployment before mainnet.
- [ ] Upgrade authority (if any) is a timelocked multisig — documented and reviewed.
- [ ] No `#[cfg(test)]` code or `testutils` feature enabled in the release WASM (verify with `wasm-objdump`).

---

## 11. Sign-off

| Reviewer | Role | Date | Result |
|---|---|---|---|
| | Contract author | | |
| | Security reviewer | | |
| | Protocol lead | | |

**All items must be marked Pass or N/A, and the table above signed, before deploying to mainnet.**
