# Policy Contract Semantics

This document describes the design, data model, and behavioral guarantees of the `mux-policy` contract.

## Overview

`mux-policy` enforces per-wallet daily spend limits on the Mux Protocol. It is a standalone Soroban contract that stores a `DailyLimit` record for each wallet and exposes functions to configure limits, record spends, and reset counters.

## Data Model

### `DailyLimit`

```rust
pub struct DailyLimit {
    pub limit: i128,                  // Maximum amount allowed per day window
    pub spent: i128,                  // Amount spent in the current window
    pub reset_ledger: u32,            // Ledger sequence at which the window expires
    pub day_ledgers: u32,             // Window length in ledgers (set at creation)
    pub registry_id: Option<Address>, // Optional registry contract for cross-contract validation
}
```

Storage key: `DataKey::WalletLimit(Address)` — persistent storage, one record per wallet.

The `registry_id` field links this policy record to an external registry contract. When set,
`record_spend` performs a live cross-contract call to the registry at that address to confirm it is
accessible before recording the spend. If the registry contract is unreachable or returns an error,
`record_spend` returns `RegistryNotFound` (fail-closed). When `registry_id` is `None`, no registry
call is made and the spend proceeds normally.

## Day Window

A "day" is measured in ledgers, not wall-clock time. At 5-second ledger close, one day ≈ 17 280 ledgers. The `day_ledgers` value is fixed when the limit is created and does not change unless the admin calls `set_daily_limit` again.

The window expires when `env.ledger().sequence() >= reset_ledger`. At that point:

- `spent` is reset to `0`
- `reset_ledger` is advanced by `day_ledgers` from the current ledger sequence

## Functions

### `initialize(admin)`

- One-time setup. Stores the admin address.
- Fails with `AlreadyInitialized` if called more than once.

### `set_daily_limit(wallet, limit, day_ledgers)` — admin only

- Creates or replaces the `DailyLimit` record for `wallet`.
- Resets `spent` to `0` and sets `reset_ledger = current_ledger + day_ledgers`.
- Fails with `InvalidAmount` if `limit <= 0`.
- Fails with `InvalidPeriod` if `day_ledgers == 0`.

### `get_daily_limit(wallet)`

- Returns the stored `DailyLimit`.
- If the window has elapsed, returns the record with `spent = 0` (view-only; the reset is **not** persisted).
- Fails with `LimitNotFound` if no limit is configured for `wallet`.

### `record_spend(wallet, amount)` — wallet-authorized

- Requires `wallet.require_auth()`.
- If the wallet's limit record contains a `registry_id`, performs a cross-contract call to the
  registry contract to confirm it is live. Returns `RegistryNotFound` (fail-closed) if the registry
  call fails. When `registry_id` is `None`, the validation step is skipped.
- Auto-resets the counter if the day window has elapsed (persists the reset).
- Debits `amount` from the remaining allowance.
- Fails with `LimitExceeded` if `spent + amount > limit`.
- Fails with `InvalidAmount` if `amount <= 0`.
- Fails with `LimitNotFound` if no limit is configured for `wallet`.

### `reset_daily_counter(wallet)` — admin only

- Requires admin authorization via `require_admin()`.
- Immediately clears `spent` to `0` and starts a fresh window from the current ledger
  (`reset_ledger = current_ledger + day_ledgers`).
- Intended for emergency corrections (e.g. a buggy integration double-counted a spend)
  and for post-upgrade counter resets when the window boundary changes.
- Fails with `LimitNotFound` if no limit has been configured for `wallet`.
- Does **not** modify the `limit` or `day_ledgers` fields.

### `upgrade(new_wasm_hash)` — admin only

- Replaces the contract WASM in-place. Admin only.
- Storage layout must remain compatible across versions; see
  [contract-upgrade-pattern.md](contract-upgrade-pattern.md).


## Reset Semantics

There are two reset paths:

| Path | Trigger | Who | Persisted |
|---|---|---|---|
| Auto-reset | `record_spend` called after window elapsed | Wallet (on next spend) | Yes |
| View reset | `get_daily_limit` called after window elapsed | Anyone | No |

The auto-reset advances `reset_ledger` by exactly `day_ledgers` from the current ledger sequence, starting a fresh window.

## Error Codes

| Code | Value | Meaning |
|---|---|---|
| `NotInitialized` | 1 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | `initialize` called more than once |
| `Unauthorized` | 3 | Caller is not the admin |
| `LimitNotFound` | 4 | No limit configured for the wallet |
| `LimitExceeded` | 5 | Spend would exceed the daily limit |
| `InvalidAmount` | 6 | `limit` or `amount` is ≤ 0 |
| `InvalidPeriod` | 7 | `day_ledgers` is 0 |
| `TooManyWallets` | 8 | Wallet cap (`MAX_WALLETS = 256`) reached; no new wallet limits can be added |
| `RegistryNotFound` | 9 | The registry contract linked via `registry_id` is unreachable or invalid; `record_spend` is fail-closed |

## Events

All state-mutating operations emit a structured event with topics `[mux_pol, action]`:

| Action | Emitted by | Data |
|---|---|---|
| `init` | `initialize` | `admin: Address` |
| `lmt_set` | `set_daily_limit` | `(wallet: Address, limit: i128, day_ledgers: u32)` |
| `spent` | `record_spend` | `(wallet: Address, amount: i128)` |
| `ctr_rst` | `reset_daily_counter` | `wallet: Address` |

> `get_daily_limit` and `upgrade` do not emit events. Failed calls (returning an error) never
> emit events — only the success path publishes.

## Storage TTL

Instance storage TTL is extended on every write (`TTL_THRESHOLD = 17 280`, `TTL_EXTEND_TO = 518 400` ledgers ≈ 30 days). Deployers should run a keeper job to extend TTL proactively; see [storage-griefing.md](storage-griefing.md).

`WalletLimit` records use **persistent** storage keyed by `DataKey::WalletLimit(Address)`. Persistent entry TTL is also extended on every write to the record.

## Storage Griefing Bounds

The contract enforces a hard cap of **256 wallets** (`MAX_WALLETS = 256`) to prevent the admin from
inflating storage unboundedly. The `WalletNames` vec in instance storage tracks registered wallet
addresses and is checked before any new `WalletLimit` entry is created.

| Collection | Key | Cap | Error on overflow |
|---|---|---|---|
| `WalletNames` vec | `DataKey::WalletNames` (instance) | 256 | `TooManyWallets` |
| `WalletLimit` per wallet | `DataKey::WalletLimit(Address)` (persistent) | bounded by `WalletNames` cap | — |

Updating an existing wallet's limit (re-calling `set_daily_limit` for a wallet already in
`WalletNames`) never increments the count — the deduplication check runs before the push.
See [storage-griefing.md](storage-griefing.md) for the full keeper runbook.

## Authorization Requirements

Every state-mutating function in `mux-policy` is gated by a specific authorization check:

| Function | Auth check | Who can call |
|---|---|---|
| `initialize` | `admin.require_auth()` | Deployer (once) |
| `set_daily_limit` | `require_admin()` → `admin.require_auth()` | Admin only |
| `record_spend` | `wallet.require_auth()` | The wallet itself only |
| `reset_daily_counter` | `require_admin()` → `admin.require_auth()` | Admin only |
| `upgrade` | `require_admin()` → `admin.require_auth()` | Admin only |

Key invariants:
- **Wallet-only spend recording:** `record_spend` requires the wallet address to authorize the call. A third party (e.g. another wallet, a relayer, or the admin) cannot debit a wallet's allowance. This prevents unauthorized spending even if the admin is compromised.
- **Admin-only limit configuration:** Only the admin set at initialization can create or modify daily limits. The wallet cannot self-escalate its own limit.
- **No cross-wallet debit:** Wallet A's `record_spend` call cannot affect wallet B's `DailyLimit` record. Each wallet's limit is stored under `DataKey::WalletLimit(Address)` — a unique key per wallet.
- **Read-only queries are unauthenticated:** `get_daily_limit` requires no authorization; anyone can query a wallet's current limit and spent amount.

## Security Considerations

- Only the admin can configure limits.
- `record_spend` requires the wallet to authorize the call, preventing third parties from debiting a wallet's allowance.
- Arithmetic overflow in `spent + amount` is caught via `checked_add` and returns `LimitExceeded`.
- Persistent storage is used for `WalletLimit` records so they survive instance TTL expiry.
