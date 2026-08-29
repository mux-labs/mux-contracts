# Entrypoint Matrix

This document classifies every `#[contractimpl]` entrypoint across the Mux Protocol
contracts as **admin** (requires stored admin/owner authorization), **user** (requires
caller authorization or specific actor auth), or **public** (no authorization required,
read-only queries).

Use this matrix when binding contracts from TypeScript or auditing the attack surface:
admin entrypoints must be called by the stored admin; user entrypoints must be called
by a specific actor; public entrypoints are callable by anyone.

## Legend

| Tag | Meaning |
|-----|---------|
| **A** | Admin / owner — requires the stored admin or owner address to authorize |
| **U** | User / actor — requires a specific caller (e.g. wallet, guardian, session key) to authorize |
| **P** | Public — no authorization required; read-only query |
| **R** | Read-only — no state mutation; may still require auth for actor-scoped reads |

## mux-account

> **Immutable by design** — `mux-account` has no `upgrade()` entry point and none will be added.
> Immutability is a user trust guarantee for core account-abstraction logic.
> Migration means deploying a new instance; see
> [account-upgrade-migration.md](account-upgrade-migration.md).

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(owner, guardians)` | A | One-time setup; owner authorizes |
| `pause()` | A | Owner only; sets Paused flag; emits `paused` event |
| `unpause()` | A | Owner only |
| `is_paused()` | P | Read-only |
| `set_delegate(delegate, expires_at, can_spend)` | A | Owner only; paused check; `expires_at` is a Unix timestamp |
| `remove_delegate(delegate)` | A | Owner only; paused check |
| `set_spend_limit(asset, amount, period)` | A | Owner only; paused check |
| `debit_spend(asset, spend)` | U | Caller (contract) authorizes; paused check; reentrancy guard |
| `execute(target, function, args, asset, spend, nonce)` | A | Owner only; paused check; validates spend limit, consumes `nonce` (must equal `nonce()`, else `InvalidNonce`), then invokes `target` while the reentrancy guard is held, then persists the debit (checks-effects-interactions); emits `executed` event |
| `owner()` | P | Read-only |
| `delegates()` | P | Read-only; filters expired |
| `get_delegate(delegate)` | P | Read-only |
| `guardians()` | P | Read-only |
| `register_session_key(session_key, expires_at, scopes)` | A | Owner only; paused check; capped at `MAX_SESSION_KEYS`; emits `sk_reg` |
| `revoke_session_key(session_key)` | A | Owner only; paused check; removes the key from `SessionKeyIndex`; emits `sk_rev` |
| `is_session_key_valid(session_key)` | P | Read-only; `true` iff registered, not revoked, and not expired |
| `execute_with_session(session_key, target, function, args, nonce)` | U | Session key auth; validates registration/revocation/expiry **and enforces scopes fail-closed** — a key registered with an empty `scopes` list, or one not covering `function`, is rejected with `Unauthorized` (T-40 in [threat-model.md](threat-model.md)). Consumes `nonce`, then dispatches `function(args)` on `target` while the reentrancy guard is held; the target's return value is forwarded to the caller; emits `ses_exe` |
| `execute_with_session_sponsored(session_key, sponsor, target, function, args, nonce)` | A+U | Both `sponsor` (must be on the owner's sponsor allowlist, else `SponsorNotAuthorized`) and `session_key` authorize; otherwise identical dispatch/scope/nonce rules to `execute_with_session`; emits `ses_exe` with `sponsor: Some(sponsor)` |
| `set_metadata(meta)` | A | Owner only; emits `meta_set` event |
| `get_metadata()` | P | Read-only |

## mux-account-factory

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(admin)` | A | Optional, one-time; sets the upgrade admin only — account registration works without it |
| `upgrade(new_wasm_hash)` | A | Admin only; `NotInitialized` if `initialize` was never called |
| `deploy_account(owner, addr)` | U | Owner authorizes; enforces `MAX_ACCOUNTS_PER_OWNER = 64` cap |
| `deploy_account_with_metadata(owner, addr, ...)` | U | Owner authorizes; enforces cap and metadata string size limits |
| `simulate_deploy(owner, addr)` | P | Dry-run; no state mutation; mirrors same cap check as `deploy_account` |
| `simulate_deploy_with_metadata(owner, addr, ...)` | P | Dry-run; no state mutation; mirrors cap and metadata size checks |
| `get_accounts(owner)` | P | Read-only |
| `account_count()` | P | Read-only; global counter across all owners |
| `get_account_metadata(owner, addr)` | P | Read-only |
| `max_accounts_per_owner()` | P | Returns `MAX_ACCOUNTS_PER_OWNER` constant (64); allows clients to preflight cap checks |

## mux-batcher

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(admin)` | A | Optional, one-time; sets the upgrade admin only — batching works without it |
| `upgrade(new_wasm_hash)` | A | Admin only; `NotInitialized` if `initialize` was never called |
| `execute_batch(caller, ops)` | U | Caller authorizes; reentrancy guard; emits `bat_start` before execution, `executed`/`bat_ok`/`bat_abort` on completion |
| `submit_batch(ops)` | U | Delegates to `execute_batch` |
| `estimate_fees(op_count)` | P | Pure computation |
| `max_batch_size()` | P | Returns constant |
| `set_registry_metadata(desc, author)` | A | Admin only; one-time; returns `NotInitialized` if `initialize` was never called, `MetadataAlreadySet` on a second call |
| `get_registry_metadata()` | P | Read-only |
| `simulate_batch(caller, ops)` | U | Caller authorizes; no state mutation; emits `sim_done` on completion |

## mux-delegation

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(admin)` | A | Optional, one-time; sets the upgrade admin only — delegation grants work without it |
| `upgrade(new_wasm_hash)` | A | Admin only; `NotInitialized` if `initialize` was never called |
| `grant_delegate(owner, delegate, perms)` | U | Owner authorizes; capped at `MAX_DELEGATE_PERMS` / `MAX_DELEGATES_PER_OWNER` |
| `revoke_delegate(owner, delegate)` | U | Owner authorizes |
| `get_delegate_permissions(owner, delegate)` | P | Read-only |
| `is_delegate(owner, delegate, perm)` | P | Read-only |
| `get_delegates(owner)` | P | Read-only |
| `check_delegate(owner, delegate, perm)` | P | Read-only; `Ok(())`/`Err(NotADelegate)` variant of `is_delegate` |
| `link_contract_id(admin, contract_id)` | A | Admin authorizes; write-once; emits `dlg_link` event |
| `get_contract_id()` | P | Read-only |

## mux-permissions

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(admin)` | A | One-time setup |
| `upgrade(new_wasm_hash)` | A | Admin only; `NotInitialized` if `initialize` was never called |
| `create_role(role, perms)` | A | Admin only |
| `grant_role(account, role)` | A | Admin only |
| `revoke_role(account, role)` | A | Admin only |
| `has_permission(account, perm)` | P | Read-only; emits `perm_ok` on grant only, nothing on denial |
| `get_roles(account)` | P | Read-only |
| `get_role_members(role)` | P | Read-only |
| `set_admin_threshold(threshold)` | A | Admin only |
| `propose_admin(new_admin)` | A | Admin only |
| `approve_admin(approver, new_admin)` | A | Admin + approver auth |
| `get_pending_admins()` | P | Read-only |
| `set_metadata(meta)` | A | Admin only |
| `get_metadata()` | P | Read-only |

## mux-policy

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(admin)` | A | One-time setup |
| `upgrade(new_wasm_hash)` | A | Admin only |
| `set_daily_limit(wallet, limit, day_ledgers, registry_id)` | A | Admin only |
| `get_daily_limit(wallet)` | P | Read-only; auto-resets counter |
| `record_spend(wallet, amount)` | U | Wallet authorizes |
| `reset_daily_counter(wallet)` | A | Admin only |

## mux-recovery

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(owner, guardians, quorum_threshold)` | U | Owner authorizes; `quorum_threshold` must be >= 1 and <= guardians.len() |
| `upgrade(new_wasm_hash)` | A | Owner only; `NotInitialized` if `initialize` was never called; should not be called while a `Pending` recovery is in flight |
| `initiate_recovery(guardian, new_owner)` | U | Guardian authorizes; rejects if a non-expired recovery is already pending; records guardian as first approval toward quorum |
| `approve_recovery(guardian)` | U | Guardian authorizes; adds approval to the pending request; rejects duplicates |
| `cancel_recovery()` | U | Owner authorizes |
| `execute_recovery(guardian)` | U | Guardian authorizes; timelock, expiry, and quorum checks (approvals >= threshold) |
| `approve_recovery_admin()` | A | Owner + guardian dual auth; executes pending recovery immediately, bypassing timelock; both owner and co_guardian must authorize |
| `add_guardian(guardian)` | U | Owner authorizes; capped at `MAX_GUARDIANS` |
| `remove_guardian(guardian)` | U | Owner authorizes; rejects if it would leave zero guardians |
| `set_quorum_threshold(threshold)` | U | Owner authorizes; threshold must be >= 1 and <= guardian count; emits `qrm_set` |
| `owner()` | P | Read-only |
| `guardians()` | P | Read-only |
| `recovery_status()` | P | Read-only |
| `recovery_request()` | P | Read-only; full request record (includes `approvals` Vec) or `None` |
| `quorum_threshold()` | P | Read-only; returns the current M-of-N threshold |
| `set_registry(owner, registry_id)` | U | Owner authorizes; the passed `owner` must equal the stored owner |
| `registry_id()` | P | Read-only |

## mux-registry

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(admin)` | A | One-time setup |
| `upgrade(new_wasm_hash)` | A | Admin only; `NotInitialized` if `initialize` was never called |
| `register(name, version)` | A | Admin only |
| `register_with_metadata(name, version, desc, author, repo)` | A | Admin only |
| `check_version(name)` | P | Dry-run; no state mutation |
| `get_version(name)` | P | Read-only |
| `get_metadata(name)` | P | Read-only |
| `list_contracts()` | P | Read-only |

## mux-spending-policy

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(admin)` | A | One-time setup |
| `upgrade(new_wasm_hash)` | A | Admin only; `NotInitialized` if `initialize` was never called |
| `set_policy(account, asset, limit, period_ledgers)` | A | Admin only; resets `spent` to 0; `InvalidInput` if `limit <= 0`, `InvalidPeriod` if `period_ledgers == 0` (auth checked first — fail-closed) |
| `get_policy(account, asset)` | P | Read-only |
| `check_spend(account, asset, amount)` | P | Read-only; no state mutation |

## mux-wallet-registry

| Entrypoint | Auth | Notes |
|---|---|---|
| `initialize(owner)` | U | Owner authorizes |
| `upgrade(new_wasm_hash)` | A | Owner only; `NotInitialized` if `initialize` was never called |
| `register_wallet(name, wallet)` | U | Owner authorizes |
| `register_wallet_with_metadata(name, wallet, label, desc)` | U | Owner authorizes; capped at `MAX_WALLETS` |
| `get_wallet(name)` | P | Read-only |
| `get_metadata(name)` | P | Read-only |
| `list_wallets()` | P | Read-only |
