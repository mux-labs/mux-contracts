# Security Policy

## Reporting a Vulnerability

Please report suspected vulnerabilities privately to the Mux Protocol security team
rather than opening a public issue. Include a description, reproduction steps, and
impact assessment. We aim to acknowledge reports within 72 hours.

Do not include secrets, private keys, JWTs, or webhook secrets in reports or logs.

## Scope

This repository (`mux-contracts`) covers the on-chain wallet registry and related
account-abstraction contracts. The server/contract remains the source of truth for
spends, recovery, and admin operations.

## Named Wallet Registry: Register / Lookup Authorization

The wallet registry exposes typed entrypoints for registering and looking up
**named wallets**. These surfaces are privileged and are **deny-by-default**.

### Invariants

1. **Source of truth.** The registry contract is authoritative for name ownership,
   delegate/guardian grants, and admin actions. Clients cannot bypass policy by
   calling entrypoints directly.
2. **Deny-by-default.** Any caller without an explicit, current grant is rejected.
   Unknown roles, expired grants, and revoked delegates fail closed.
3. **Idempotency.** Register requests carry a correlation id. Replayed or concurrent
   requests with the same correlation id resolve to the same outcome and never
   create duplicate name bindings.
4. **Fail-closed writes.** If a required dependency (RPC/DB/Horizon) is unavailable,
   write entrypoints reject rather than partially applying state.
5. **No secret leakage.** Errors, logs, and metrics never contain raw key material,
   JWTs, API keys, or webhook secrets.

### Authorization model

| Actor     | Register | Lookup | Notes                                             |
|-----------|----------|--------|---------------------------------------------------|
| Owner     | allow    | allow  | Must match the name's owner of record.            |
| Delegate  | allow    | allow  | Only while the grant is current and unrevoked.    |
| Guardian  | allow    | allow  | Recovery-scoped; cannot escalate to admin.        |
| API key   | allow    | allow  | Scoped to the registry; deny-by-default otherwise.|
| JWT       | allow    | allow  | Must be unexpired and carry the registry scope.   |
| Anonymous | deny     | deny   | No implicit access.                               |

### Stable error codes

Register/lookup entrypoints return stable, typed error codes so clients can react
without parsing free-form messages:

- `UNAUTHORIZED` — missing or invalid credential.
- `FORBIDDEN` — credential valid but lacks the required role/scope.
- `GRANT_EXPIRED` — delegate/guardian grant expired or revoked.
- `NAME_TAKEN` — name already bound to another owner.
- `CONFLICT` — replayed correlation id with a divergent payload.
- `DEPENDENCY_UNAVAILABLE` — required dependency down; write rejected (fail-closed).
- `INVALID_INPUT` — malformed or oversized request.

### Edge cases & failure modes

- **Concurrent/replayed requests:** deduplicated by correlation id; divergent payloads
  return `CONFLICT`.
- **Dependency outage:** writes fail closed with `DEPENDENCY_UNAVAILABLE`; no partial state.
- **Auth expiry / wrong role / revoked delegate:** rejected with `GRANT_EXPIRED` or `FORBIDDEN`.
- **Adversarial input:** oversized batches and griefing attempts are rejected with
  `INVALID_INPUT`; entrypoints are rate-limited.
- **Testnet vs mainnet misconfig:** mainnet-affecting changes are gated behind a
  feature flag / kill-switch and require a readiness checklist before enablement.

### Observability

Registry entrypoints emit ops-safe metrics and structured logs with correlation ids.
Logs redact keys, JWTs, and webhook secrets. Actionable errors use the stable codes
above so on-call can triage without inspecting raw payloads.

### Rollback / kill-switch

Any money-path or mainnet-affecting change to the registry lands behind a feature
flag. Rollback is documented in the corresponding PR description; disabling the flag
restores prior behavior without redeploying contracts.

## Contributor Checklist (Stellar Wave)

Before opening a PR touching the wallet registry:

- [ ] Authz enforced on every new entrypoint (deny-by-default).
- [ ] Idempotency handled via correlation id.
- [ ] Writes fail closed on dependency outage.
- [ ] Stable error codes used; no free-form auth failures.
- [ ] No secrets in code, logs, or metrics.
- [ ] Docs/runbooks updated; mainnet safety flags respected.
- [ ] Rollback/flag strategy described in the PR.

See `examples/wallet-registry-invoke.ts` for a reference invocation of the
register/lookup entrypoints.
