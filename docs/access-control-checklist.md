# Mux Protocol — Access Control Review Checklist

**Version:** 0.2.0  
**Date:** 2026-08-26  
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

> **Immutable by design** — no `upgrade()` entry point exists or will be added.
> See [account-upgrade-migration.md](account-upgrade-migration.md) and
> [upgrade-auth-requirements.md](upgrade-auth-requirements.md).

- [ ] `initialize` — `owner.require_auth()` called before any storage write.
- [ ] `pause` — `require_owner` helper called; sets `DataKey::Paused` to `true`; emits `paused` event.
- [ ] `set_delegate` — `require_owner` helper called; verifies `owner.require_auth()`.
- [ ] `remove_delegate` — `require_owner` helper called.
- [ ] `set_spend_limit` — `require_owner` helper called.
- [ ] `debit_spend` — `current_contract_address().require_auth()` called (contract-internal only).
- [ ] `execute` — `require_owner` helper called; spend limit is checked and the
      reentrancy guard acquired before `target` is invoked, but the debit is
      only written to storage — and the guard only released — after
      `invoke_contract` returns (checks-effects-interactions; see §5.2);
      emits `executed` event on success.
- [ ] `register_session_key` / `revoke_session_key` — `require_owner` helper called.
- [ ] `execute_with_session` — `session_key.require_auth()` called, plus revocation/expiry check against the stored `SessionKeyRecord`. **Fail-closed scope enforcement (T-40):** a key registered with an empty `scopes` list is rejected with `Unauthorized` (unit test: `test_execute_with_session_rejects_empty_scopes`). Remaining limitation: `payload` is not decoded or dispatched, so a **non-empty** scope list is not matched against the payload's target method — see `docs/aa_sequence_diagram.md`.

> **Implementation note (T-40):** Empty-scopes rejection is implemented and enforced fail-closed — a session key with an empty `scopes` list is rejected with `Unauthorized` before any state mutation (T-40 partial close). The remaining tracked limitation is that a **non-empty** scope list is not yet matched against the payload's target method; per-method payload matching requires the payload decoder to be implemented first. Until then, a key with a non-empty scope list passes scope validation regardless of what method the payload targets.

- [ ] `set_metadata` — `require_owner` helper called; emits `meta_set` event.
- [ ] No public function mutates storage without an auth check.

### 1.2 `mux-batcher`

- [ ] `execute_batch` — `caller.require_auth()` called before any operations are dispatched; emits `bat_start` before execution, `executed`/`bat_ok`/`bat_abort` on completion.
- [ ] `simulate_batch` — `caller.require_auth()` called (preflight is also auth-gated); emits `sim_done` on completion.
- [ ] `submit_batch` — delegates to `execute_batch`, deriving `caller` from the invoker; same auth guarantee applies.
- [ ] `set_registry_metadata` — `require_admin` helper called before the `MetadataAlreadySet` check (fail-closed: unauthenticated callers cannot probe metadata state); returns `NotInitialized` if `initialize` was never called.
- [ ] Batch operations are dispatched under the **caller's** auth context, not the batcher contract's.
- [ ] `initialize` — `admin.require_auth()` called before storage write; optional (batching works without it).
- [ ] `upgrade` — `require_admin` helper called; `NotInitialized` (fail-closed) if `initialize` was never called; no silent skip of the auth check.

### 1.3 `mux-permissions`

- [ ] `initialize` — `admin.require_auth()` called before storage write.
- [ ] `create_role` — `require_admin` helper called.
- [ ] `grant_role` — `require_admin` helper called.
- [ ] `revoke_role` — `require_admin` helper called.
- [ ] `has_permission`, `get_roles`, `get_role_members` — read-only; no auth required (acceptable); `has_permission` emits `perm_ok` on grant only, nothing on denial.
- [ ] `set_admin_threshold` — `require_admin` helper called.
- [ ] `propose_admin` — `require_admin` helper called.
- [ ] `approve_admin` — `require_admin` helper called, plus `approver.require_auth()` for the individual approval.
- [ ] `set_metadata` — `require_admin` helper called; emits `meta_set` event.
- [ ] No role or admin-set mutation is possible without admin signature.
- [ ] No role mutation is possible without admin signature.
- [ ] `upgrade` — `require_admin` helper called; WASM upgrade is admin-gated (same helper used by role and multisig-rotation entrypoints).
- [ ] `propose_admin` / `approve_admin` — both fail-closed with no admin auth mocked at all (unit test: `test_admin_rotation_calls_require_admin_auth`); approvals below the configured threshold do not change the stored admin (unit test: `test_multisig_admin_promotion_transfers_control`).

### 1.4 `mux-policy`

- [ ] `initialize` — `admin.require_auth()` called before storage write.
- [ ] `set_daily_limit` — `require_admin` helper called; only admin can configure limits.
- [ ] `record_spend` — `wallet.require_auth()` called before any storage write; third parties cannot debit a wallet's allowance.
- [ ] `reset_daily_counter` — `require_admin` helper called; only admin can perform emergency resets.
- [ ] `upgrade` — `require_admin` helper called; WASM upgrade is admin-gated.
- [ ] No policy mutation is possible without the correct authorization.

### 1.5 `mux-registry`

- [ ] `initialize` — `admin.require_auth()` called before storage write.
- [ ] `register` — `require_admin` helper called.
- [ ] `register_with_metadata` — `require_admin` helper called.
- [ ] `get_version`, `get_metadata`, `list_contracts`, `check_version` — read-only; no auth required (acceptable).
- [ ] `upgrade` — `require_admin` helper called; `NotInitialized` (fail-closed) if `initialize` was never called.
- [ ] No registry mutation is possible without admin signature.

### 1.6 `mux-recovery`

- [ ] `initialize` — `owner.require_auth()` called before storage write.
- [ ] `initiate_recovery` — `guardian.require_auth()` + `require_guardian` helper called.
- [ ] `approve_recovery` — `guardian.require_auth()` + `require_guardian` helper called; rejects duplicate approvals.
- [ ] `cancel_recovery` — `require_owner` helper called; only current owner can cancel.
- [ ] `execute_recovery` — `guardian.require_auth()` + `require_guardian` helper called.
- [ ] `approve_recovery_admin` — `require_owner` helper called **and** `co_guardian.require_auth()` + `require_guardian` called; both the owner and a registered guardian must co-sign to bypass the timelock — owner alone cannot execute the fast path.
- [ ] `add_guardian` / `remove_guardian` — `require_owner` helper called; `remove_guardian` additionally rejects removing the last guardian.
- [ ] `set_quorum_threshold` — `require_owner` helper called; threshold must be >= 1 and <= guardian count.
- [ ] `set_registry` — the caller-supplied `owner` must equal the stored owner (`Unauthorized` otherwise); `owner.require_auth()` called before storage write.
- [ ] `upgrade` — `require_owner` helper called; `NotInitialized` (fail-closed) if `initialize` was never called; should not be called while a `Pending` recovery is in flight.
- [ ] No recovery mutation is possible without guardian or owner authorization.

### 1.7 `mux-delegation`

- [ ] `grant_delegate` — `owner.require_auth()` called before any storage write.
- [ ] `revoke_delegate` — `owner.require_auth()` called before any storage write.
- [ ] `link_contract_id` — the caller-supplied `admin` parameter authorizes **itself**; this is **not** checked against any stored admin identity, so it is not a privileged gate against other callers — see `docs/delegation-upgrade.md`; emits `dlg_link` event on success.
- [ ] `initialize` — `admin.require_auth()` called before storage write; optional (delegation grants work without it) and establishes a **separate** stored admin used only by `upgrade`.
- [ ] `upgrade` — `require_admin` helper called; `NotInitialized` (fail-closed) if `initialize` was never called.
- [ ] `get_delegate_permissions`, `is_delegate`, `get_delegates`, `check_delegate`, `get_contract_id` — read-only; no auth required (acceptable).

### 1.8 `mux-spending-policy`

- [ ] `initialize` — `admin.require_auth()` called before storage write.
- [ ] `set_policy` — `require_admin` helper called **before** input validation (fail-closed: unauthenticated callers cannot probe validation state); only admin can configure limits.
- [ ] `set_policy` — rejects `period_ledgers == 0` with `InvalidPeriod` and `limit <= 0` with `InvalidInput`; no policy is stored and no `lmt_set` event is emitted on either rejection (unit tests: `test_set_policy_rejects_zero_period`, `test_set_policy_rejects_non_positive_limit`, `test_set_policy_auth_checked_before_period_validation`).
- [ ] `get_policy`, `check_spend` — read-only or validation-only; no auth required (acceptable).
- [ ] `upgrade` — `require_admin` helper called; `NotInitialized` (fail-closed) if `initialize` was never called; WASM upgrade is admin-gated.
- [ ] No policy mutation is possible without admin signature.

### 1.9 `mux-wallet-registry`

- [ ] `initialize` — `owner.require_auth()` called before storage write.
- [ ] `register_wallet` — `require_owner` helper called; only owner can register wallets.
- [ ] `register_wallet_with_metadata` — `require_owner` helper called; only owner can register wallets.
- [ ] `get_wallet`, `get_metadata`, `list_wallets` — read-only; no auth required (acceptable).
- [ ] `upgrade` — `require_owner` helper called; `NotInitialized` (fail-closed) if `initialize` was never called; WASM upgrade is owner-gated.
- [ ] No wallet registry mutation is possible without owner signature.

### 1.10 `mux-account-factory`

- [ ] `initialize` — `admin.require_auth()` called before storage write; optional (account registration works without it) and establishes a stored admin used only by `upgrade`.
- [ ] `deploy_account` — `owner.require_auth()` called per-call; no stored admin required.
- [ ] `deploy_account_with_metadata` — `owner.require_auth()` called per-call.
- [ ] `simulate_deploy` / `simulate_deploy_with_metadata` — read-only dry-runs; no auth required (acceptable).
- [ ] `get_accounts`, `account_count`, `get_account_metadata`, `max_accounts_per_owner` — read-only; no auth required (acceptable).
- [ ] `upgrade` — stored `DataKey::Admin` read and `admin.require_auth()` called; `NotInitialized` (fail-closed) if `initialize` was never called.
- [ ] No upgrade path is possible without the explicitly initialized admin signature.

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
- [ ] Delegate `expires_at` timestamp is enforced at call time, not just at creation time.
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

### 5.1 Reentrancy Guard (#690)

The `execute_batch` function implements a reentrancy guard using `DataKey::Executing`:

**Guard lifecycle:**
1. **Set** immediately after size validation passes, before any operations execute
2. **Checked** at guard-set time — returns `ReentrancyDetected` if already set
3. **Cleared** on **all exit paths**, including:
   - Success path: after batch loop completes normally
   - Abort path: before returning `RequiredOperationFailed` when a required operation fails
   - **Not** set/cleared on early returns (`EmptyBatch`, `BatchTooLarge`) — guard never set on those paths

**Critical properties:**
- [x] Guard is set after `ops.is_empty()` and `ops.len() > MAX_BATCH_SIZE` checks, so those early returns never touch the guard
- [x] Guard is explicitly removed before returning `Err(RequiredOperationFailed)` on the abort path
- [x] Guard is removed after successful batch completion before returning `Ok(BatchResult)`
- [x] Guard prevents recursive `execute_batch` calls from within a batched operation
- [x] Subsequent calls in the same session work because guard is always cleared (verified by unit test: `test_reentrancy_guard_clears_after_success`)
- [x] Guard is cleared even when a required operation fails (verified by unit test: `test_reentrancy_guard_clears_after_required_op_fails`)
- [x] Pre-seeded `Executing=true` is detected and rejected (verified by unit test: `test_reentrancy_detected_when_executing_flag_already_set`)

**Soroban rollback semantics:**
- **Contract-level error** (`return Err(...)`) does NOT auto-rollback instance storage
- **Host-level trap** (`panic!`) auto-rolls back all storage writes for the invocation
- `mux-batcher` uses contract-level errors so callers can inspect error codes, therefore the guard must be manually cleared before each `return Err(...)`

**Test coverage:**
- [x] `test_reentrancy_guard_clears_after_success` — two sequential batches succeed
- [x] `test_reentrancy_guard_clears_after_required_op_fails` — abort path clears guard; second batch succeeds
- [x] `test_reentrancy_detected_when_executing_flag_already_set` — pre-seeded flag returns `ReentrancyDetected`

**Documentation:**
- [x] Rollback semantics documented in `contracts/mux-batcher/src/lib.rs` (lines 131-155)
- [x] Guard lifecycle documented in `execute_batch` function doc comment
- [x] This checklist section (#690)

### 5.2 Reentrancy Guard — `mux-account` `execute()`

`execute()` invokes an arbitrary `target` contract on the owner's behalf while
accounting for an asset spend. The invoked target can call back into the
account contract during that invocation, so the reentrancy guard
(`DataKey::Executing`) must cover the invocation itself, not just the spend
bookkeeping around it.

**Guard lifecycle (checks-effects-interactions):**
1. **Check** — spend is validated against the configured `SpendLimit` (read
   only; no storage write) before the guard is even considered.
2. **Set** — the guard is acquired immediately after the check passes, before
   `target` is invoked.
3. **Interaction** — `target` is invoked (`env.invoke_contract`) while the
   guard is held.
4. **Effect** — the debit is written to `SpendLimit` storage only after the
   invocation returns.
5. **Cleared** — on every exit path: after a successful invocation, and on
   the `SpendLimitExceeded` / `InvalidAmount` / `ArithmeticOverflow`
   rejection paths (a contract-level `Err` return does not auto-rollback
   storage on Soroban, so a guard left set on a rejection would permanently
   lock the account out of `execute()` and `debit_spend()`).

**Previously**: the debit was written and the guard cleared *before*
`target` was invoked, so the guard covered the bookkeeping but not the actual
cross-contract call — a callback from `target` into `execute()` or
`debit_spend()` during the invocation was not caught. Ordering now follows
invoke-then-debit: the interaction happens first, the effect (and guard
release) after.

**Test coverage** (`contracts/mux-account/src/lib.rs`):
- [x] `test_execute_holds_reentrancy_guard_across_invocation` — a target that
      calls back into `debit_spend` mid-`execute()` is rejected with
      `ReentrancyDetected`; the outer spend is recorded exactly once.
- [x] `test_execute_spend_limit_rejection_does_not_lock_out_future_calls` —
      after a `SpendLimitExceeded` rejection, a subsequent within-limit
      `execute()` call still succeeds (guard was released).
- [x] `test_debit_spend_rejection_does_not_lock_out_future_calls` — same
      guarantee for the `debit_spend` entrypoint.

---

## 6. Storage Isolation

- [x] Each contract uses its own `DataKey` enum with no overlapping key names across contracts (verified: `enum DataKey` defined independently in all 10 `contracts/*/src/lib.rs`; Soroban's per-contract-instance storage means cross-contract name collisions are not reachable regardless).
- [x] All storage reads use `ok_or(SomeError::NotInitialized)` — no silent `unwrap` that could panic post-deployment (verified via `rg '\.unwrap\(\)' contracts/*/src/lib.rs` restricted to non-`#[cfg(test)]` code; the one gap found — `mux-spending-policy::check_spend` reading `DataKey::SpendLimit` after a `.has()` check — is fixed to `.ok_or(SpendingPolicyError::PolicyNotFound)?`).
- [x] Persistent storage keys are namespaced by type (e.g., `SpendLimit(Address)` vs `Delegates`) (verified: tuple-variant keys such as `mux-account::DataKey::SessionKey(Address, Address)`, `mux-spending-policy::DataKey::SpendLimit(Address, Address)`, `mux-delegation::DataKey::DelegatePerms(Address, Address)`).
- [x] No contract reads or writes to another contract's storage directly (architectural: the Soroban host does not expose an API for a contract to address another contract's storage; all cross-contract effects go through `invoke_contract`).

---

## 6a. Storage Griefing Caps

See [docs/storage-griefing.md](storage-griefing.md) for full details.

- [x] `mux-account`: `set_delegate` enforces `MAX_DELEGATES = 64`; new entries beyond cap return `TooManyDelegates` (unit test: `test_delegate_cap_enforced`).
- [x] `mux-account`: updating an existing delegate at cap succeeds (unit test: `test_delegate_cap_allows_update`).
- [x] `mux-account`: expired delegates are reclaimed using ledger timestamps before enforcing the cap (unit test: `test_delegate_cap_reclaims_expired_entries`).
- [x] `mux-account-factory`: `deploy_account` and `deploy_account_with_metadata` enforce `MAX_ACCOUNTS_PER_OWNER = 64` per owner; new deploys beyond cap return `TooManyAccounts` (unit tests: `test_accounts_cap_enforced`, `test_deploy_account_with_metadata_enforces_cap`).
- [x] `mux-account-factory`: `simulate_deploy` and `simulate_deploy_with_metadata` enforce the same 64-account cap without writing state (unit tests: `test_simulate_deploy_enforces_cap`, `test_simulate_deploy_with_metadata_enforces_cap`).
- [x] `mux-account-factory`: per-owner cap is independent — one owner filling their cap does not block other owners (unit test: `test_cap_is_per_owner_not_global`).
- [x] `mux-account-factory`: metadata string sizes are bounded (`MAX_VERSION_LENGTH = 32`, `MAX_DESCRIPTION_LENGTH = 256`, `MAX_AUTHOR_LENGTH = 64`); oversized strings return `MetadataTooLarge` (unit tests: `test_metadata_version_too_long`, `test_metadata_description_too_long`, `test_metadata_author_too_long`).
- [x] `mux-account-factory`: `max_accounts_per_owner()` public entrypoint returns 64; TypeScript clients must query this before deploy to avoid `TooManyAccounts` (unit test: `test_max_accounts_per_owner_returns_64`).
- [x] `mux-permissions`: `grant_role` enforces `MAX_ROLE_MEMBERS = 256` per role; returns `TooManyMembers` (unit test: `test_role_member_cap_enforced`).
- [x] `mux-permissions`: `grant_role` enforces `MAX_ROLES_PER_ACCOUNT = 32` per account; returns `TooManyRoles` (unit test: `test_roles_per_account_cap_enforced`).
- [x] All three contracts (`mux-account`, `mux-account-factory`, `mux-permissions`) call `env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO)` on every write (T-21 mitigation) (verified: `bash scripts/test-ttl-keeper.sh` Tests 1–3 pass for all ten contract crates, confirming the constants, the per-write `extend_ttl` calls, and unit test coverage for instance-storage TTL extension).
- [x] TTL constants: `TTL_THRESHOLD = 17_280` (~1 day), `TTL_EXTEND_TO = 518_400` (~30 days) (verified identical across all ten contract crates by `scripts/test-ttl-keeper.sh` Test 1).
- [x] Deployment runbook includes a keeper job that extends TTL at least every 25 days (see [docs/storage-griefing.md](storage-griefing.md#deployment-runbook--ttl-keeper)) (verified: `scripts/test-ttl-keeper.sh` Test 5 checks the runbook section and CLI example exist).
- [ ] `mux-policy` and `mux-delegation` extend the TTL of the **persistent** storage entries they write (`WalletLimit` records in `mux-policy`; `DelegatePerms` records in `mux-delegation`), not just instance storage — **Fail**. Neither contract calls `.persistent().extend_ttl(...)` on the keys it writes with `.persistent().set(...)` (`contracts/mux-policy/src/lib.rs:197,254,279`; `contracts/mux-delegation/src/lib.rs:200`), so those entries can be archived/evicted independently of the contract instance's own TTL. `scripts/test-ttl-keeper.sh` Test 4 already catches this (currently failing) but is not yet wired into CI (`.github/workflows/ci.yml`) or `Makefile`, so the failure goes unnoticed. Remediation: add `.persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO)` calls alongside each `.persistent().set(...)` in both contracts, then add a CI step running `scripts/test-ttl-keeper.sh`. Tracked for a follow-up PR — out of scope for the doc/test changes in this one.

---

## 7. Error Handling

- [ ] All error types are `#[contracttype]` decorated enums with explicit `#[repr(u32)]` discriminants.
- [ ] No error arm uses discriminant 0 (reserved for success in some SDKs).
- [ ] Errors are propagated via `Result<_, Error>` — no `panic!` except in `require_success` abort path.
- [ ] Error codes are stable across contract versions (no re-numbering without a major version bump).

---

## 7a. Panic-Free Error Paths

- [ ] **No bare `.unwrap()` on storage reads.** Every `env.storage().*().get(...)` uses `.ok_or(Error::Variant)` or `.unwrap_or(default)`. Bare `.unwrap()` on a missing key would panic post-deployment.
- [ ] **No bare `.expect(...)` on fallible operations.** Replace with `.ok_or(Error)` or pattern matching.
- [ ] **Checked arithmetic on all user-controlled values.** `spent + amount` uses `checked_add().ok_or(Error)?`. Subtraction uses `checked_sub()` or `saturating_sub()`. The workspace `Cargo.toml` sets `overflow-checks = true` but contracts should not rely on this as a substitute for explicit checks.
- [ ] **No `panic!`, `unreachable!`, or `unimplemented!` in production paths.** These macros are acceptable in `#[cfg(test)]` code only.
- [ ] **`Vec::get(idx)` is bounds-checked.** Soroban SDK `Vec::get` panics on out-of-bounds access; always verify `idx < vec.len()` first or use `.try_get()`.
- [ ] **`require_auth()` failures propagate as host errors**, not contract panics. This is safe because the SDK handles auth failures internally.
- [ ] **No implicit integer truncation.** `u32` / `i128` conversions use `.try_into()` or explicit casts with overflow guards.
- [ ] **All error paths are tested.** Every `Err(...)` variant returned by a public function has at least one `try_*` test that asserts the error variant.

### Quick audit commands

```bash
# Find bare .unwrap() in contract source (exclude tests)
rg '\.unwrap\(\)' contracts/*/src/lib.rs | grep -v '#\[cfg(test)\]' | grep -v '// '

# Find panic!/unreachable!/unimplemented! in non-test code
rg 'panic!|unreachable!|unimplemented!' contracts/*/src/lib.rs | grep -v '#\[cfg(test)\]'
```

---

## 8. Unit Test Coverage

- [ ] `mux-account`: `initialize`, double-initialize, delegate CRUD, spend limit enforcement, invalid amount/period, `execute()` reentrancy guard held across invocation (§5.2), guard released on rejection paths, `executed`/`meta_set` event emission.
- [ ] `mux-batcher`: empty batch, oversized batch, `initialize`/double-initialize/`upgrade` before `initialize`, `upgrade` auth rejection, `bat_start`/`bat_abort`/`sim_done` event emission.
- [ ] `mux-delegation`: grant/revoke CRUD, `initialize`/double-initialize/`upgrade` before `initialize`, `upgrade` auth rejection, `dlg_link` event emission.
- [ ] `mux-permissions`: initialize, double-initialize, role create/grant/revoke, permission check, nonexistent role grant, admin-threshold promotion (below-threshold no-op, at-threshold transfer), admin-rotation auth rejection, `upgrade` before `initialize`, `upgrade` auth rejection, `perm_ok` event emission on grant only.
- [ ] `mux-registry`: initialize, double-initialize, register/register-with-metadata, `upgrade` before `initialize`, `upgrade` auth rejection.
- [ ] `mux-spending-policy`: initialize, double-initialize, set-policy (including `limit <= 0` → `InvalidInput`, `period_ledgers == 0` → `InvalidPeriod`, and auth-before-validation ordering), check-spend, `upgrade` before `initialize`, `upgrade` auth rejection.
- [ ] `mux-wallet-registry`: initialize, double-initialize, register-wallet, register-wallet-with-metadata, `upgrade` before `initialize`, `upgrade` auth rejection.
- [ ] `mux-recovery`: initialize, double-initialize, initiate/cancel/execute recovery, `upgrade` before `initialize`, `upgrade` auth rejection.
- [ ] `mux-account-factory`: deploy-account, deploy-with-metadata, cap enforcement, `initialize`/double-initialize/`upgrade` before `initialize`, `upgrade` auth rejection.
- [ ] All `require_owner` / `require_admin` paths have a negative test (unauthorized caller).
- [ ] All `AlreadyInitialized` paths have a test.
- [ ] CI runs `cargo test --workspace --all-features` on every PR (see `.github/workflows/ci.yml`).

---

## 9. CI / CD Verification

- [ ] `cargo clippy --workspace --all-features --all-targets -- -D warnings` passes with no warnings.
- [ ] `cargo fmt --check` passes.
- [ ] Bindings drift check (`check-binding-drift` job) passes on PRs.
- [ ] Release builds use `[profile.release]` with `overflow-checks = true` and `panic = "abort"`.
- [ ] WASM artifacts are uploaded and SHA-256 is published as the `wasm-hashes` artifact  
      (CI job `rust` step **Compute WASM hashes** / **Upload WASM hashes artifact** — #664).
- [ ] `cargo deny check` passes — supply-chain license and advisory policy in `deny.toml`  
      (CI job `deny` — #661; also `make deny` locally).
- [ ] No `testutils` string in release WASMs — CI job `rust` step **check-no-testutils** and  
      standalone job `check-no-testutils` both pass (#663; also `make check-no-testutils`).
- [ ] Coverage job (`coverage`) runs `cargo-llvm-cov` and emits `coverage/lcov.info` (#662).
- [ ] All four new CI jobs (`deny`, `check-no-testutils`, `verify-wasm-hash`, `coverage`) are  
      green before a mainnet deploy is approved.
- [ ] `scripts/check-gitignore-secret-patterns.sh` passes — `.env`, `*.secret`,  
      `deployment.env`, and `deployer.json` remain git-ignored (see  
      [deployer-key-requirements.md](deployer-key-requirements.md#security-checklist)).
- [ ] `scripts/check-deployer-key-rotation-log.sh` passes — any entry in  
      [`ops/deployer-key-rotation-log.md`](../ops/deployer-key-rotation-log.md) is  
      complete and its drain/archive confirmations are checked.
- [ ] `scripts/check-rollback-log.sh` passes — any entry in  
      [`ops/rollback-log.md`](../ops/rollback-log.md) is complete and both  
      checklist confirmations from [rollback-deploy.md](rollback-deploy.md) are checked.

---

## 10. Deployment Checklist

- [ ] Admin / owner keypairs generated on HSM or hardware wallet — not software-only.
- [ ] Admin keypair for `mux-permissions` is a Stellar multisig account with threshold ≥ 2.
- [ ] Initial guardian set contains ≥ 3 geographically distributed addresses.
- [ ] Contract IDs recorded in `bindings/src/network.ts` for the correct network.
- [ ] `stellar contract invoke` smoke-test run against testnet deployment before mainnet.
- [ ] Upgrade authority (if any) is a timelocked multisig — documented and reviewed.
- [ ] No `#[cfg(test)]` code or `testutils` feature enabled in the release WASM (run `make check-no-testutils` / see [no-testutils-wasm.md](no-testutils-wasm.md)).

---

## 11. Authorization Flow Examples

### Owner → Delegate → Spend (mux-account)

```
1. Owner calls initialize(owner, guardians)
   └─ owner.require_auth() ✓
   └─ Storage: Owner, Delegates={}, GuardianSet, Nonce=0

2. Owner calls set_delegate(delegate, expiry, can_spend=true)
   └─ require_owner() → owner.require_auth() ✓
   └─ Storage: Delegates[delegate] = DelegateInfo{expires_at, can_spend}

3. Delegate calls debit_spend(asset, amount)
   └─ current_contract_address().require_auth() (contract-internal only)
   └─ Checks: not paused, not re-entered, limit not exceeded
   └─ Storage: SpendLimit(asset).spent += amount
```

### Policy Record Spend (mux-policy)

```
1. Admin calls set_daily_limit(wallet, limit, day_ledgers)
   └─ require_admin() → admin.require_auth() ✓
   └─ Storage: WalletLimit(wallet) = DailyLimit{limit, spent=0, ...}

2. Wallet calls record_spend(wallet, amount)
   └─ wallet.require_auth() ✓  ← only the wallet itself can debit
   └─ Checks: limit exists, amount > 0, spent + amount <= limit
   └─ Storage: WalletLimit(wallet).spent += amount
   └─ Third-party call fails: wallet A cannot record_spend for wallet B
```

### Registry Registration (mux-registry)

```
1. Admin calls register(name, version)
   └─ require_admin() → admin.require_auth() ✓
   └─ Checks: Names.len < 128 (TooManyContracts if exceeded)
   └─ Storage: Names.push(name), Version(name) = version

2. Anyone calls get_version(name) — read-only, no auth needed
```

### Recovery Timelock (mux-recovery)

```
1. Guardian calls initiate_recovery(guardian, new_owner)
   └─ guardian.require_auth() ✓ + require_guardian() ✓
   └─ Storage: Recovery = RecoveryRequest{Pending, executable_at}

2. Owner calls cancel_recovery()  [within timelock window]
   └─ require_owner() → owner.require_auth() ✓
   └─ Storage: Recovery.status = Cancelled

3. Guardian calls execute_recovery(guardian)  [after timelock]
   └─ guardian.require_auth() ✓ + require_guardian() ✓
   └─ Checks: status == Pending, current_ledger >= executable_at
   └─ Storage: Owner = new_owner
```

---

## 12. Sign-off

| Reviewer | Role | Date | Result |
|---|---|---|---|
| | Contract author | | |
| | Security reviewer | | |
| | Protocol lead | | |

**All items must be marked Pass or N/A, and the table above signed, before deploying to mainnet.**
