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

The contract's job is to decide **who may submit on the account's behalf**. That
is the sponsor allowlist.

## Allowlist

| Entrypoint | Auth | Effect |
|---|---|---|
| `set_sponsor(sponsor, allowed)` | owner | Adds (`true`) or removes (`false`) a relayer; emits `spn_set` |
| `is_sponsor(sponsor)` | none | Read-only membership check |

The allowlist is fail-closed: `execute_with_session_sponsored` rejects any
relayer that is not currently allowlisted with `SponsorNotAuthorized`, even when
the session-key signature is valid. Removal takes effect on the next call — there
is no grace window.

## Sponsored execution

```
execute_with_session_sponsored(session_key, sponsor, target, function, args, nonce)
```

Both parties authorize:

- `sponsor.require_auth()` proves the relayer submitted this exact call, so an
  allowlisted relayer's identity cannot be spoofed by a third party.
- `session_key.require_auth()` proves the account granted the capability.

The sponsor is checked **before** the session key is loaded, so an unknown
relayer learns nothing about the account's session-key state.

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

## Relayer checklist

1. Owner allowlists the relayer with `set_sponsor(relayer, true)`.
2. Relayer builds the transaction with its own account as source.
3. Relayer adds the `execute_with_session_sponsored` invocation.
4. Relayer reads `nonce()` and builds the call with that exact value; a relayer
   with several queued calls must submit them in nonce order.
5. Relayer simulates, then collects the session key's signature on the assembled
   transaction and adds its own.
6. Relayer submits and pays the fee.
7. Relayer indexes `ses_exe` events for billing; each carries the session key,
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
