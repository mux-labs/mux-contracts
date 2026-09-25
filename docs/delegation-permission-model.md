# Delegation Permission Model

This document defines how delegation works in Mux contracts: who can delegate,
what a delegate may do, and — critically — when a delegation stops being valid.

## Roles

- **Owner** — the account that owns the smart wallet. The owner is the only
  role that can create, extend, or revoke a delegation.
- **Delegate** — an address granted a scoped, time-bounded permission by the
  owner. A delegate can never escalate its own scope.
- **Guardian** — a recovery role. Guardians can revoke delegations but cannot
  create or extend them.

## Delegation record

The contract is `#![no_std]` and stores all state using Soroban persistent
storage with explicit per-entry TTL management.

---

## 2. Roles

| Role | Who | Capabilities |
|---|---|---|
| **Owner** | Any address that owns a delegation grant | Grant permissions to delegates; revoke grants; enumerate own delegates |
| **Delegate** | Address granted one or more permissions by an owner | Act within the granted permission set on behalf of the owner |
| **Caller** | Any address | Read-only queries (`get_delegate_permissions`, `is_delegate`, `get_delegates`, `check_delegate`) |

> **No super-owner over delegation grants.** There is no global admin for the
> owner/delegate permission model above. Each `(owner, delegate)` pair is
> independent. An owner can only grant or revoke their own delegates.
>
> This does not extend to `link_contract_id(admin, contract_id)` or
> `initialize(admin)`/`upgrade`, which accept a separate, unrelated `admin`
> concept scoped to contract self-registration and WASM upgrades — never to
> delegation grants. See [`docs/delegation-upgrade.md`](delegation-upgrade.md#admin--initialization)
> and [`docs/access-control-checklist.md`](access-control-checklist.md#17-mux-delegation)
> for the full breakdown.

---

## 3. Permission Model

### 3.1 What is a permission?

A permission is a `Symbol` — a compact Soroban string value of up to 9 bytes.
Permission names are chosen by the application layer; the contract treats them
as opaque identifiers. Common examples: `"transfer"`, `"read"`, `"swap"`,
`"vote"`, `"trade"`.

### 3.2 Grant semantics

Calling `grant_delegate(owner, delegate, permissions)`:

- **Replaces** any prior grant for the same `(owner, delegate)` pair — there is
  no append mode. The new `permissions` list becomes the authoritative grant.
- The operation is atomic: if validation fails (empty list, too many permissions,
  too many delegates) the existing grant is left unchanged.
- Requires `owner.require_auth()` — only the owner can grant.

## Delegation record

A delegation is stored as a typed record:

```
Delegation {
    owner: Address,
    delegate: Address,
    scope: Scope,          // e.g. Transfer { max_amount }, Call { target }
    expires_at: u64,       // ledger timestamp; 0 means "no expiry" is NOT allowed
    revoked: bool,
    nonce: u64,            // monotonic, for idempotency / replay protection
}
```

`expires_at` is mandatory. A delegation with `expires_at == 0` is rejected at
creation time so that "no expiry" can never be expressed by accident.

## Expiry invariants

These

```
Delegation {
    owner: Address,
    delegate: Address,
    scope: Scope,          // e.g. Transfer { max_amount }, Call { target }
    expires_at: u64,       // ledger timestamp; 0 means "no expiry" is NOT allowed
    revoked: bool,
    nonce: u64,            // monotonic, for idempotency / replay protection
}
```

`expires_at` is mandatory. A delegation with `expires_at == 0` is rejected at
creation time so that "no expiry" can never be expressed by accident.

## Expiry invariants

These invariants are enforced on-chain and are the source of truth. Clients
(API, SDK, wallet UI) must not be trusted to enforce them.

1. **Fail-closed on expiry.** A delegation is valid only while
   `now < expires_at`. At `now >= expires_at` the delegation is expired and
   every privileged action through it MUST be rejected.
2. **Revocation is immediate.** Setting `revoked = true` invalidates the
   delegation for all future actions, regardless of `expires_at`.
3. **Expiry is not extendable by the delegate.** Only the owner may extend
   `expires_at`, and only forward in time. Extending a revoked delegation is
   rejected.
4. **No resurrection.** An expired or revoked delegation cannot be re-enabled;
   the owner must create a new delegation with a fresh `nonce`.
5. **Deny-by-default.** Any action that does not match an active, unexpired,
   unrevoked delegation is rejected. There is no implicit or wildcard grant.

## Authorization checks

Every entrypoint that consumes a delegation performs, in order:

1. Resolve the delegation by `(owner, delegate, nonce)`.
2. Reject if not found → `DelegationNotFound`.
3. Reject if `revoked` → `DelegationRevoked`.
4. Reject if `now >= expires_at` → `DelegationExpired`.
5. Reject if the requested action is outside `scope` → `ScopeViolation`.
6. Reject if the caller is not the recorded `delegate` → `Unauthorized`.

Steps 3–4 are the expiry gate. They run before any state mutation so that a
failed check cannot leave partial effects (fail-closed on writes).

## Stable error codes

| Code | Name | Meaning |
| --- | --- | --- |
| 1 | `Unauthorized` | Caller is not the delegate/owner/guardian for this action. |
| 2 | `DelegationNotFound` | No delegation matches the given key. |
| 3 | `DelegationExpired` | `now >= expires_at`. |
| 4 | `DelegationRevoked` | Delegation was explicitly revoked. |
| 5 | `ScopeViolation` | Action is outside the delegated scope. |
| 6 | `InvalidExpiry` | `expires_at` is zero or not in the future at creation. |
| 7 | `ReplayDetected` | `nonce` was already consumed. |

Error codes are stable and part of the public contract. Do not renumber them.

## Idempotency and replay

- Each delegation carries a monotonic `nonce`. Consuming a delegation for an
action advances the nonce; replaying the same `(owner, delegate, nonce)` is
rejected with `ReplayDetected`.
- Concurrent requests that race on the same nonce resolve to exactly one
  success; the loser receives `ReplayDetected`.
- Expiry is evaluated against the ledger timestamp at execution, not at
  submission, so a request that expires in-flight is rejected.

## Observability

Expiry-related events are emitted without leaking secrets or raw key material:

- `delegation_created` — `{ owner, delegate, expires_at, nonce }`
- `delegation_revoked` — `{ owner, delegate, nonce }`
- `delegation_expired_rejected` — `{ owner, delegate, nonce, now }`
- `delegation_scope_rejected` — `{ owner, delegate, nonce }`

Addresses are logged as-is (they are public); no private keys, signatures, or
JWTs are ever logged. Metrics count rejections by error code so operators can
alert on spikes in `DelegationExpired` / `DelegationRevoked`.

## Failure modes

- **Dependency outage (RPC/DB/Horizon).** Writes fail closed: if the current
  ledger time or delegation state cannot be read, the action is rejected rather
  than assumed valid.
- **Auth expiry / wrong role / revoked delegate.** All map to the stable error
  codes above; none fall through to a permissive default.
- **Adversarial input.** Oversized batches, spoofed webhooks, and griefing
  attempts are rejected by scope and nonce checks before any state change.
- **Testnet vs mainnet misconfig.** `expires_at` is compared against the
  network's ledger time; a delegation created for one network is not valid on
  another because owner/delegate addresses and nonces are network-scoped.

## Permissions → Delegation → Spend Integration Path (Issue #849)

The permission-delegation-spend path provides secure, scoped account abstraction on Stellar/Soroban:

```
[Owner] ──(1. Grant Permissions)──> [mux-permissions / mux-delegation]
                                             │
                                     (2. Grant Delegation)
                                             │
                                             ▼
[Delegate] ──(3. Execute Spend)──> [Delegation Gate] ──(Within Limit & Scope)──> [Account Spend]
                                             │
                                   (Fail-Closed Rejections)
                                             ▼
                          [Unauthorized | ScopeViolation |
                           DelegationExpired | DelegationRevoked |
                           SpendLimitExceeded | ReplayDetected]
```

### Invariants & Validation Order

Every spend invocation executed via delegation follows a strict fail-closed pipeline:

1. **Authentication Gate**: Caller must match the registered `delegate` address. Unauthenticated callers are rejected immediately with `Unauthorized` (code 1).
2. **Revocation Gate**: If `revoked == true`, spend is rejected immediately with `DelegationRevoked` (code 4).
3. **Expiry Gate**: Evaluated against the execution ledger timestamp `now`. If `now >= expires_at`, spend is rejected with `DelegationExpired` (code 3).
4. **Replay / Nonce Gate**: Request `nonce` must match the stored monotonic nonce. Replayed requests are rejected with `ReplayDetected` (code 7). Nonce advances monotonically on successful spend.
5. **Scope Authorization**: The requested action (e.g. `"transfer"`, `"spend"`) must belong to the delegate's active permissions set. Unauthorized actions are rejected with `ScopeViolation` (code 5).
6. **Spend Limit Enforcement**: The spend amount must satisfy `spent + amount <= limit`. Amounts exceeding the limit are rejected with `SpendLimitExceeded` (code 5).
7. **Boundary Precision**: Exact limit spends (`spent + amount == limit`) succeed; spends one unit above (`spent + amount == limit + 1`) fail.

### Observability & Security

- State changes emit `delegation_spend_executed` with public identifiers: `{ owner, delegate, amount, remaining_limit, nonce, correlation_id }`.
- **Zero Secret Leakage**: No private keys, seed phrases, signatures, or raw authorization tokens are ever logged, emitted in events, or surfaced in error messages.
- Correlation IDs (`correlation_id`) are propagated across log records to enable end-to-end tracing without disclosing sensitive parameters.

### Kill Switch / Rollback Strategy

Delegation spend enforcement is protected by the `ENABLE_DELEGATION_SPEND` feature flag. In the event of an anomaly, disabling the flag immediately halts delegate spend routing while preserving owner-direct recovery and settlement operations.

## References

- `SECURITY.md` — threat model and disclosure process.
- `docs/aa-milestone-roadmap.md` — AA milestone exit criteria.
- `docs/developer_onboarding.md` — contributor setup and test patterns.
