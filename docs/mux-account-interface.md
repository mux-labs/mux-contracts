# Mux Account Public Interface

`mux-account` is a Soroban smart account that stores one owner, a bounded
delegate map, guardians, and per-asset spend limits. All write entrypoints
extend instance-storage TTL.

## Authorization

| Entrypoint | Required authorization |
|---|---|
| `initialize` | supplied `owner` |
| `unpause` | stored owner |
| `set_delegate` | stored owner |
| `remove_delegate` | stored owner |
| `set_spend_limit` | stored owner |
| `debit_spend` | current contract address |
| `set_metadata` | stored owner |
| `register_session_key` | stored owner |
| `revoke_session_key` | stored owner |
| `execute_with_session` | authorized session key (`session_key.require_auth()`) |
| `execute_with_session_sponsored` | allowlisted sponsor **and** authorized session key |
| `set_sponsor` | stored owner |
| Read-only entrypoints | none |

Owner-only calls fail with a host authorization error when the signature is
missing. Contract validation failures use `MuxAccountError`.

## Entrypoints

### Initialization and status

- `initialize(owner, guardians) -> Result<(), MuxAccountError>` initializes
  the instance once.
- `owner() -> Result<Address, MuxAccountError>` returns the stored owner.
- `guardians() -> Result<Vec<Address>, MuxAccountError>` returns guardians.
- `nonce() -> Result<u64, MuxAccountError>` returns the account's transaction
  counter — the exact value the next execution call must supply.
- `is_paused() -> bool` returns the pause flag.
- `unpause() -> Result<(), MuxAccountError>` clears the pause flag.

### Delegates

- `set_delegate(delegate, expires_at, can_spend) -> Result<(), MuxAccountError>`
  inserts or updates a delegate. New entries are capped at 64. `expires_at` is
  a Unix timestamp (`env.ledger().timestamp()`), not a ledger sequence.
- `remove_delegate(delegate) -> Result<(), MuxAccountError>` removes an entry.
- `delegates() -> Result<Map<Address, DelegateInfo>, MuxAccountError>` returns
  only delegates whose `expires_at` timestamp is still in the future.
- `get_delegate(delegate) -> Result<DelegateInfo, MuxAccountError>` returns one
  active delegate, or `DelegateNotFound` / `DelegateExpired`.

`DelegateInfo` contains `address`, `expires_at` (Unix timestamp, `u64`), and
`can_spend`.

### Spend limits

- `set_spend_limit(asset, amount, period_ledgers) -> Result<(), MuxAccountError>`
  sets a positive allowance and reset period for an asset.
- `debit_spend(asset, spend) -> Result<(), MuxAccountError>` atomically rolls
  the period forward when needed and increments `spent`. Missing or exceeded
  limits return `SpendLimitExceeded`.

`SpendLimit` contains `asset`, `amount`, `period_ledgers`, `spent`, and
`reset_ledger`.

### Sessions and metadata

- `register_session_key(session_key, expires_at, scopes) -> Result<(), MuxAccountError>`
  registers or replaces a session key with a Unix-timestamp expiry and a set
  of `Scope` capabilities. New keys are capped at `MAX_SESSION_KEYS` (32) per
  owner.
- `revoke_session_key(session_key) -> Result<(), MuxAccountError>` marks a
  registered session key as revoked.
- `execute_with_session(session_key, target, function, args, nonce) -> Result<Val, MuxAccountError>`
  validates that `session_key` is authorized, registered, non-revoked, and
  non-expired, then invokes `function` on `target` and forwards its return
  value. **Fail-closed scope enforcement:** a key registered with an empty
  `scopes` list is rejected with `Unauthorized` (T-40), and a `function` that is
  not named in a non-empty `scopes` list is rejected with `ScopeNotGranted`.
  `nonce` must equal `nonce()` or the call is rejected with `InvalidNonce`; it
  is consumed only after every other check passes, so a rejected call does not
  burn a nonce. The reentrancy guard is held across the invocation, so a
  callback into `execute`, `debit_spend`, or this entrypoint is rejected.
  Emits `ses_exe`.
- `execute_with_session_sponsored(session_key, sponsor, target, function, args, nonce) -> Result<Val, MuxAccountError>`
  is the gas-abstracted variant: the relayer submits and pays, and both the
  sponsor and the session key must authorize. The sponsor must be on the
  owner-managed allowlist or the call is rejected with `SponsorNotAuthorized`
  before any session state is read. Sponsorship never widens a session key's
  scopes. See [relayer-integration.md](relayer-integration.md).
- `set_sponsor(sponsor, allowed) -> Result<(), MuxAccountError>` adds or removes
  a relayer from the sponsorship allowlist. Owner only; emits `spn_set`.
- `is_sponsor(sponsor) -> bool` returns allowlist membership.
- `set_metadata(meta) -> Result<(), MuxAccountError>` stores owner-controlled
  `RegistryMeta`.
- `get_metadata() -> Option<RegistryMeta>` returns metadata when present.

## Errors

| Code | Variant | Meaning |
|---:|---|---|
| 1 | `NotInitialized` | Required account state is absent |
| 2 | `AlreadyInitialized` | Initialization was already completed |
| 3 | `Unauthorized` | Contract state disallows the call |
| 4 | `DelegateNotFound` | Delegate is absent |
| 5 | `DelegateExpired` | Delegate is no longer active |
| 6 | `SpendLimitExceeded` | Limit is absent or would be exceeded |
| 7 | `InvalidAmount` | Amount is not positive |
| 8 | `InvalidPeriod` | Reset period is zero |
| 9 | `TooManyDelegates` | Delegate cap is reached |
| 10 | `ReentrancyDetected` | Spend accounting is already executing |
| 11 | `ArithmeticOverflow` | Spend addition overflowed |
| 12 | `TooManySessionKeys` | Session-key cap is reached |
| 13 | `ScopeNotGranted` | Invoked method is not in the session key's scopes |
| 14 | `SponsorNotAuthorized` | Relayer is not on the sponsor allowlist |
| 15 | `InvalidNonce` | Supplied nonce is not the account's current nonce |

## Events

Events use topics `(mux_acct, action)`. The actions are `init`, `unpaused`,
`dlg_set`, `dlg_rm`, `lmt_set`, `debited`, `ses_exe`, `spn_set`, and
`meta_set`.
See [audit events](audit-events.md) for payload shapes.

## Binding notes

The Rust signatures above define the generated client ABI. After changing a
public type or entrypoint, run `bash scripts/generate-bindings.sh` and update
downstream TypeScript calls in the same release.
