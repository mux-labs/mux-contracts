# Policy Contract Semantics

This document describes the design, data model, and behavioral guarantees of the `mux-policy` contract.

## Overview

`mux-policy` enforces per-wallet daily spend limits on the Mux Protocol. It is a standalone Soroban contract that stores a `DailyLimit` record for each wallet and exposes functions to configure limits, record spends, and reset counters.

## Data Model

### `DailyLimit`

```rust
pub struct DailyLimit {
    pub limit: i128,        // Maximum amount allowed per day window
    pub spent: i128,        // Amount spent in the current window
    pub reset_ledger: u32,  // Ledger sequence at which the window expires
    pub day_ledgers: u32,   // Window length in ledgers (set at creation)
}
```

Storage key: `DataKey::WalletLimit(Address)` — persistent storage, one record per wallet.

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
- Auto-resets the counter if the day window has elapsed (persists the reset).
- Debits `amount` from the remaining allowance.
- Fails with `LimitExceeded` if `spent + amount > limit`.
- Fails with `InvalidAmount` if `amount <= 0`.
- Fails with `LimitNotFound` if no limit is configured for `wallet`.


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

## Events

All state-mutating operations emit a structured event with topics `[mux_pol, action]`:

| Action | Emitted by | Data |
|---|---|---|
| `init` | `initialize` | admin address |
| `lmt_set` | `set_daily_limit` | `(wallet, limit, day_ledgers)` |
| `spent` | `record_spend` | `(wallet, amount)` |

## Storage TTL

Instance storage TTL is extended on every write (`TTL_THRESHOLD = 17 280`, `TTL_EXTEND_TO = 518 400` ledgers ≈ 30 days). Deployers should run a keeper job to extend TTL proactively; see [storage-griefing.md](storage-griefing.md).

## Security Considerations

- Only the admin can configure limits.
- `record_spend` requires the wallet to authorize the call, preventing third parties from debiting a wallet's allowance.
- Arithmetic overflow in `spent + amount` is caught via `checked_add` and returns `LimitExceeded`.
- Persistent storage is used for `WalletLimit` records so they survive instance TTL expiry.
