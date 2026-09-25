# AA Milestone Roadmap — Exit Criteria

This document is the authoritative exit-criteria reference for the Mux Protocol
account-abstraction (AA) roadmap on Stellar/Soroban. It defines the invariants,
typed API expectations, authorization model, failure modes, and observability
requirements that must hold before an AA milestone is considered complete.

It is referenced by [SECURITY.md](../SECURITY.md) and by the AA roadmap issue
(#774). Any change to AA behavior must keep this document and the implementation
in agreement; contradictory copy elsewhere must be removed.

## Phase 2: Transaction Execution & Relay (Complete)
- [x] Implement `execute_with_session()` transaction logic
- [x] Add relayer sponsorship logic and gas abstraction
- [x] Build basic frontend integration examples for session keys
- [x] Publish documentation on integrating with the relayer network

`execute_with_session(session_key, target, function, args)` now dispatches to
`target` under the account's authorization while the reentrancy guard is held,
and matches `function` against the session key's granted `scopes` fail-closed.
`execute_with_session_sponsored` adds the relayer path, gated by the
owner-managed allowlist (`set_sponsor` / `is_sponsor`). See
[relayer-integration.md](relayer-integration.md) and
[`examples/session-key-usage.ts`](../examples/session-key-usage.ts).

## Scope

Mux provides invisible wallets and account abstraction on Stellar/Soroban. The
AA roadmap covers:

- Smart-account creation and recovery
- Delegated signing (session keys / delegates)
- Guardian-assisted recovery
- Sponsored (fee-bump) transaction submission
- Admin and policy surfaces that gate the above

## Invariants

These invariants are non-negotiable. A milestone does not exit until every
invariant is enforced in code and covered by automated tests.

1. **Server/contract is the source of truth.** Spends, recovery, and admin
   actions are authorized and recorded by the contract (and the server for
   off-chain coordination). Clients are never trusted to assert authority.
2. **Deny-by-default for new privileged surfaces.** Any new entrypoint that can
   move funds, change ownership, or alter policy must be explicitly authorized;
   absence of an explicit allow is a denial.
3. **Fail-closed on writes.** If a dependency (RPC, DB, Horizon) is unavailable
   or returns an ambiguous result, write paths must fail closed rather than
   proceed optimistically.
4. **Idempotency.** Concurrent or replayed requests must not double-spend or
   double-apply state transitions. Each mutating request carries a stable
   idempotency key (or nonce) that is checked and recorded.
5. **No secrets in repo or logs.** Keys, JWTs, webhook secrets, and raw key
   material are never logged or committed; they are redacted at the boundary.
6. **Mainnet safety.** Any money-path or mainnet-affecting change lands behind a
   feature flag or kill-switch with a documented rollback.

## Typed API / Entrypoint Expectations

- Entrypoints are typed with explicit request/response shapes; no untyped
  `any`/`bytes` blobs on privileged paths.
- Every entrypoint returns a **stable error code** (enum) rather than free-form
  strings, so clients and tests can branch deterministically.
- Mutating entrypoints accept a **correlation id** (and idempotency key where
  applicable) that is echoed in responses and logs for tracing.
- Error codes are additive: existing codes are never repurposed, only new ones
  added, to preserve client compatibility.

### Error code categories

| Category | Meaning |
|----------|---------|
| `Unauthorized` | Caller lacks the required role/authority |
| `ExpiredAuth` | Auth token/session/delegate grant has expired |
| `RevokedDelegate` | Delegate grant was revoked before use |
| `Replayed` | Idempotency key/nonce already consumed |
| `DependencyUnavailable` | RPC/DB/Horizon outage; write failed closed |
| `PolicyDenied` | Request violates configured policy |
| `InvalidInput` | Malformed or oversized input rejected |

## Authorization Model

Authority is layered; a caller must satisfy every applicable layer.

- **Owner** — full control of the smart account, including recovery and admin.
- **Delegate** — scoped, time-bounded signing authority granted by the owner.
  Delegates cannot escalate their own scope or grant further delegates.
- **Guardian** — participates in recovery; cannot unilaterally move funds.
- **API-key / JWT** — authenticates off-chain server entrypoints; scoped to a
  tenant/account and never a substitute for on-chain owner authority.

Rules:

- Clients cannot bypass policy: the contract/server re-checks authority on every
  privileged call regardless of what the client claims.
- **Revoked delegates** are rejected (`RevokedDelegate`) even if their grant has
  not yet expired; revocation is checked before scope.
- **Expired auth** (JWT/session/delegate grant) is rejected (`ExpiredAuth`);
  expiry is evaluated server-side against a trusted clock.
- Wrong role is rejected (`Unauthorized`); roles are not inferred from context.
- API keys and JWTs are redacted in logs and never returned in responses.

## Edge Cases & Failure Modes

- **Concurrent/replayed requests** — idempotency key/nonce enforced; replays
  return `Replayed` without re-applying state.
- **Dependency outage** — RPC/DB/Horizon failures on write paths fail closed
  (`DependencyUnavailable`); reads may degrade but never authorize a write.
- **Auth expiry / wrong role / revoked delegate** — see authorization model.
- **Adversarial input** — oversized batches, griefing payloads, and spoofed
  webhooks are rejected (`InvalidInput` / `PolicyDenied`); webhook signatures are
  verified before processing.
- **Testnet vs mainnet misconfig** — network passphrase and contract addresses
  are validated at startup; a mismatch aborts rather than proceeding.

## Observability

- Actionable, stable error codes on every failure path (no opaque failures).
- Metrics on money and realtime paths: authorization denials by reason, replay
  rejections, dependency failures, and recovery attempts.
- Logs include correlation ids but never secrets or raw key material.

## Integration: Factory → Account → Policy Path

The end-to-end AA lifecycle connects contract deployment, account initialization, and policy enforcement across the workspace:

1. **Factory (`mux-account-factory`)**: Deploys a new `mux-account` contract instance with predictable salt and registers the deployed account address under the owner in registry metadata.
2. **Account (`mux-account`)**: Initialized with owner authorization; configures delegates, session keys, and spending limits; dispatches authorized operations fail-closed.
3. **Policy (`mux-policy` / `mux-spending-policy`)**: Enforces rate limits, daily spend caps, and authorization windows on delegated spends, reverting unauthorized or out-of-policy transactions before execution.

### Invariants & In-Flight Flow
- **Owner-gated deployment**: Factory deployment verifies caller authorization and guarantees distinct account addresses per owner.
- **Fail-closed dispatch**: The smart account evaluates policy compliance prior to executing batched or session-directed invocations.
- **Atomic state rollback**: Cross-contract invocations across factory, account, and policy abort atomically on error.

## Exit Criteria Checklist

- [x] Behavior matches this document and cited references.
- [x] Factory → Account → Policy integration path documented and enforced.
- [x] Authz, idempotency, and fail-closed behavior covered by automated tests
      (unit for invariants and auth negatives; integration/e2e on the critical
      path using existing suite patterns).
- [x] Docs/runbooks updated; contradictory copy removed.
- [x] Mainnet safety flags respected; risky changes behind a flag/kill-switch.
- [x] Observability: actionable errors and metrics on money/realtime paths.
- [x] Rollback/flag strategy documented in the PR description.

## Out of Scope

Unrelated refactors and irreversible mainnet operations without a readiness
checklist.

## References

- [SECURITY.md](../SECURITY.md)
- [docs/threat-model.md](threat-model.md)
- [docs/access-control-checklist.md](access-control-checklist.md)
- [docs/audit-prep.md](audit-prep.md)
