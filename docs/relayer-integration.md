# Relayer Integration

This document describes how a relayer sponsors gas for `mux-account` session
calls, and what the contract does and does not delegate to it.

Related: [account-abstraction.md](account-abstraction.md),
[aa_sequence_diagram.md](aa_sequence_diagram.md),
[entrypoint-matrix.md](entrypoint-matrix.md).

## What sponsorship means here

There is no `Paymaster` or `EntryPoint` contract in this codebase. Gas
abstraction on Soroban is simpler: the network fee is paid by the **transaction
source account**, which does not have to be the account whose authority is being
exercised. A relayer therefore sponsors a call by submitting the transaction
from its own account while the session key supplies the authorization.

The contract's job is to decide **who may submit on the account's behalf** and
**how much sponsorship each relayer is permitted to consume**. Those are the
sponsor allowlist and the sponsorship limits.

## Allowlist

| Entrypoint | Auth | Effect |
|---|---|---|
| `set_sponsor(sponsor, allowed)` | owner | Adds (`true`) or removes (`false`) a relayer; emits `spn_set` |
| `is_sponsor(sponsor)` | none | Read-only membership check |

The allowlist is fail-closed: `execute_with_session_sponsored` rejects any
relayer that is not currently allowlisted with `SponsorNotAuthorized`, even when
the session-key signature is valid. Removal takes effect on the next call — there
is no grace window.

## Sponsorship limits

Allowlisting a relayer grants it the *ability* to sponsor; it does not grant an
unbounded budget. The owner sets explicit caps so a compromised or buggy relayer
cannot drain the account's sponsored-call budget or grief the account with an
unbounded stream of submissions.

| Entrypoint | Auth | Effect |
|---|---|---|
| `set_sponsor_limit(sponsor, max_calls, max_fee)` | owner | Sets the per-relayer cap; emits `spn_lim` |
| `sponsor_limit(sponsor)` | none | Read-only `(max_calls, max_fee)` for a relayer |
| `sponsor_usage(sponsor)` | none | Read-only `(calls_used, fee_used)` for the current window |
| `reset_sponsor_usage(sponsor)` | owner | Clears usage counters; emits `spn_rst` |

Limits are enforced **before** the session key is loaded, alongside the allowlist
check, so an over-limit relayer learns nothing about the account's session-key
state. The contract is the source of truth: a relayer cannot raise its own cap,
reset its own usage, or bypass the check by submitting from a different source
account (the sponsor argument is bound by `sponsor.require_auth()`).

### Semantics

- `max_calls` is the maximum number of sponsored executions a relayer may submit
  per window. `0` means **deny** — a relayer with a zero cap is treated as
  unauthorized even if it is on the allowlist.
- `max_fee` is the maximum cumulative Soroban resource fee (in stroops) the
  relayer may consume per window. `0` means **deny**.
- A relayer with no limit set is **deny-by-default**: it is rejected with
  `SponsorLimitNotSet` until the owner sets a cap. Allowlisting alone is not
  sufficient to sponsor.
- Usage is incremented atomically with the sponsored execution. If the call
  would exceed either cap it is rejected with `SponsorLimitExceeded` and no
  counters are mutated (fail-closed, no partial accounting).
- The window is the account's current `sponsor_window()`; the owner advances it
  with `reset_sponsor_usage`, which is the only way counters decrease. This keeps
  the money path deterministic and replay-safe.

### Stable error codes

| Error | Meaning |
|---|---|
| `SponsorNotAuthorized` | Relayer is not on the allowlist |
| `SponsorLimitNotSet` | Relayer is allowlisted but has no cap (deny-by-default) |
| `SponsorLimitExceeded` | Call would exceed `max_calls` or `max_fee` |
| `InvalidSponsorLimit` | `max_calls`/`max_fee` rejected (e.g. negative or malformed) |

## Sponsored execution

```
execute_with_session_sponsored(session_key, sponsor, target, function, args, nonce)
```

Both parties authorize:

- `sponsor.require_auth()` proves the relayer submitted this exact call, so an
  allowlisted relayer's identity cannot be spoofed by a third party.
- `session_key.require_auth()` proves the account granted the capability.

The sponsor is checked **before** the session key is loaded, so an unknown
relayer learns nothing about the account's session-key state. The allowlist and
sponsorship-limit checks run together, before any session-key state is read.

Sponsorship changes who pays, never what is permitted. After the sponsor check,
the sponsored path runs the identical validation as the unsponsored path:
registration, revocation, expiry, non-empty scopes, per-method scope matching,
and the account nonce. A relayer cannot invoke a method the session key was not
scoped for, and cannot replay a session authorization it has already submitted —
`nonce` must equal the account's current `nonce()` or the call is rejected with
`InvalidNonce`.

## Fee accounting

The relayer pays the Soroban resource fee out of its own XLM balance. The
account contract records the sponsor in the `ses_exe` audit event
(`sponsor: Some(relayer)`; `None` for a direct call), which is what off-chain
billing should reconcile against. The contract performs no on-chain fee
refund or accounting — reimbursement between the account owner and the relayer
is an off-chain arrangement and belongs in `mux-backend`.

The on-chain `sponsor_usage` counters are the authoritative cap enforcement; the
`ses_exe` event stream is the authoritative audit trail. Off-chain billing must
reconcile against both: a relayer that hits `SponsorLimitExceeded` should be
visible as a rejected submission in the relayer's own logs, not as a `ses_exe`
event.

## Relayer checklist

1. Owner allowlists the relayer with `set_sponsor(relayer, true)`.
2. Owner sets a cap with `set_sponsor_limit(relayer, max_calls, max_fee)`.
   Without this step the relayer is denied by default.
3. Relayer builds the transaction with its own account as source.
4. Relayer adds the `execute_with_session_sponsored` invocation.
5. Relayer reads `nonce()` and builds the call with that exact value; a relayer
   with several queued calls must submit them in nonce order.
6. Relayer simulates, then collects the session key's signature on the assembled
   transaction and adds its own.
7. Relayer submits and pays the fee.
8. Relayer indexes `ses_exe` events for billing; each carries the session key,
   target, function, and sponsor.

A runnable version of this flow is in
[`examples/session-key-usage.ts`](../examples/session-key-usage.ts).

## Operational notes

- Keep the allowlist small. Each entry is one instance-storage key, and instance
  storage is shared with delegates and session keys.
- Rotate a relayer by allowlisting the new address before removing the old one;
  in-flight transactions signed against the old address will fail closed.
- Pausing the account (`pause()`) blocks sponsored execution along with every
  other non-admin entrypoint.
- When a relayer is suspected compromised, remove it from the allowlist first
  (`set_sponsor(relayer, false)`), then reset its usage. Removal is immediate and
  fail-closed; resetting usage alone does not revoke the ability to sponsor.
- Limits are per-account. A relayer serving many accounts has an independent cap
  on each; there is no global relayer budget in the contract.
- Testnet and mainnet deployments have independent instance storage. A cap set on
  testnet does not carry to mainnet — re-apply limits as part of the mainnet
  readiness checklist before enabling a relayer.
