# Architecture Overview — Current Crate Set

This document is the authoritative architecture overview for the Mux Protocol
contracts. It describes the current crate set, each crate's responsibilities and
entrypoints, and how the crates relate to one another. It also states the
invariants that all crates must uphold.

> Scope: this overview covers the Soroban contracts under `contracts/`. It does
> not describe off-chain services beyond their trust relationship to the
> contracts.

## Invariants

These invariants hold across the entire crate set. Any change that violates one
of them is a security regression.

1. **Source of truth.** The contract is the source of truth for spends,
   recovery, and admin actions. Off-chain services (RPC, DB, Horizon, indexers)
   are caches/views and MUST NOT be trusted to authorize or finalize a money
   path.
2. **Deny-by-default.** Every privileged surface denies by default. A caller
   must present an explicit, verifiable authorization (owner, delegate,
   guardian, API-key, or JWT) for the specific action; absence of a matching
   grant is a denial, not an implicit allow.
3. **Fail-closed on writes.** When a dependency (RPC, DB, Horizon) is
   unavailable or returns an ambiguous result, write paths fail closed. Reads
   may degrade; writes must not proceed on unverified state.
4. **Idempotency.** Money-path and admin entrypoints are idempotent under
   concurrent or replayed requests, keyed by a caller-supplied correlation id.
   A replayed request with the same correlation id MUST NOT double-apply.
5. **No secret leakage.** Logs, errors, and metrics never contain raw key
   material, JWTs, API keys, or webhook secrets.

## Crate Set

### `mux-account`

- **Responsibility:** The per-user smart account. Holds account state, executes
  authorized operations, and is the ultimate authority for spends and recovery
  on behalf of the owner.
- **Entrypoints:** account initialization, `execute` (authorized operations),
  spend authorization checks, recovery entrypoints.
- **Relationships:** created by `mux-account-factory`; consults
  `mux-permissions` and `mux-spending-policy` before executing; delegates
  recovery to `mux-recovery`; registered in `mux-registry`.

### `mux-account-factory`

- **Responsibility:** Deterministic, authorized creation of `mux-account`
  instances. Owns the account-creation policy and the mapping from owner to
  account address.
- **Entrypoints:** `create_account`, account-address derivation/lookup.
- **Relationships:** produces `mux-account` instances; records them in
  `mux-registry`; may consult `mux-permissions` for creation authorization.

### `mux-registry`

- **Responsibility:** Canonical registry mapping owners/identifiers to their
  account and wallet addresses. Read-mostly; writes are authorized and
  idempotent.
- **Entrypoints:** register, lookup, and update of account/wallet mappings.
- **Relationships:** written by `mux-account-factory` and `mux-wallet-registry`;
  read by all crates that need to resolve an account or wallet.

### `mux-wallet-registry`

- **Responsibility:** Registry of wallet-level metadata and wallet-to-account
  associations, including delegate and guardian bindings.
- **Entrypoints:** wallet registration, delegate/guardian binding updates,
  wallet lookups.
- **Relationships:** feeds `mux-delegation` and `mux-permissions`; updates
  `mux-registry`.

### `mux-delegation`

- **Responsibility:** Delegate authorization. Grants, revokes, and evaluates
  delegate rights scoped to an account or wallet.
- **Entrypoints:** grant delegate, revoke delegate, evaluate delegate
  authorization.
- **Relationships:** consulted by `mux-account` and `mux-permissions`; bindings
  sourced from `mux-wallet-registry`. Revoked delegates MUST be denied
  immediately.

### `mux-permissions`

- **Responsibility:** Central authorization policy. Evaluates owner, delegate,
  guardian, API-key, and JWT grants against a requested action and returns an
  allow/deny decision with a stable error code.
- **Entrypoints:** permission evaluation, grant/revoke of policy entries.
- **Relationships:** consulted by `mux-account`, `mux-account-factory`, and
  `mux-spending-policy`; delegate state sourced from `mux-delegation`.

### `mux-spending-policy`

- **Responsibility:** Spend limits and policy enforcement (per-transaction,
  per-window, allow/deny lists). Enforces policy before any spend is executed.
- **Entrypoints:** policy configuration, spend evaluation.
- **Relationships:** consulted by `mux-account` before executing a spend;
  authorization decisions sourced from `mux-permissions`.

### `mux-recovery`

- **Responsibility:** Account recovery. Guardian-driven and owner-driven
  recovery flows, with fail-closed semantics and replay protection.
- **Entrypoints:** initiate recovery, approve recovery (guardian), finalize
  recovery, cancel recovery.
- **Relationships:** operates on `mux-account`; guardian bindings sourced from
  `mux-wallet-registry`; authorization via `mux-permissions`.

### `mux-batcher`

- **Responsibility:** Batched execution of multiple authorized operations in a
  single transaction, with bounded batch size and per-item authorization.
- **Entrypoints:** submit batch, execute batch.
- **Relationships:** dispatches to `mux-account`; each item is independently
  authorized via `mux-permissions` and policy-checked via
  `mux-spending-policy`.

## Inter-Crate Relationships

```
mux-account-factory ──creates──▶ mux-account
        │                            │
        │                            ├─▶ mux-permissions ──▶ mux-delegation
        │                            ├─▶ mux-spending-policy
        │                            └─▶ mux-recovery
        ▼
mux-registry ◀──updates── mux-wallet-registry
        ▲                            │
        └────────reads───────────────┘

mux-batcher ──dispatches──▶ mux-account
```

- `mux-account` is the execution hub: it consults `mux-permissions` and
  `mux-spending-policy` before any spend, and `mux-recovery` for recovery.
- `mux-permissions` is the single authorization decision point; it sources
  delegate state from `mux-delegation`.
- `mux-registry` and `mux-wallet-registry` are the resolution layer; all crates
  resolve accounts/wallets through them rather than trusting off-chain state.
- `mux-batcher` is a thin dispatcher: it adds no authority of its own and
  re-checks authorization and policy per item.

## Authorization Model

Every privileged entrypoint is authorized against one of the following roles.
Clients cannot bypass policy: the contract re-evaluates authorization on-chain
for each action and does not accept a client-asserted role.

| Role | Description |
|------|-------------|
| **Owner** | The account owner; full authority over their account. |
| **Delegate** | Scoped authority granted via `mux-delegation`; revocable. |
| **Guardian** | Recovery authority bound via `mux-wallet-registry`. |
| **API-key** | Service-level credential for authorized off-chain callers. |
| **JWT** | Short-lived bearer credential for authorized off-chain callers. |

Rules:

- Deny-by-default: no matching grant means denial.
- Revoked delegates and expired JWTs/API-keys are denied immediately.
- Wrong-role requests are denied with a stable error code (see Observability).
- Authorization is evaluated on-chain; off-chain claims are never trusted.

## Idempotency and Replay Handling

- Money-path and admin entrypoints accept a caller-supplied correlation id.
- A request with a previously seen correlation id is a no-op (or returns the
  prior result) and MUST NOT double-apply.
- Concurrent requests with the same correlation id are serialized by the
  contract; only one takes effect.
- Replay of a finalized request is rejected with a stable error code.

## Observability

- **Stable error codes.** Every failure returns a stable, documented error code
  (e.g. unauthorized, wrong role, revoked delegate, expired credential,
  replayed request, dependency unavailable). Codes are part of the public
  contract and must not change without a versioned migration.
- **Correlation ids.** Money-path and admin entrypoints emit the caller's
  correlation id in events so off-chain systems can trace a request end to end.
- **Metrics.** Money and realtime paths emit metrics (success/failure counts,
  latency, denial reasons) suitable for alerting.
- **Redaction.** Logs, events, and metrics never contain raw key material,
  JWTs, API keys, or webhook secrets. Sensitive values are redacted or hashed.

## Failure Modes

- **Dependency outage (RPC/DB/Horizon):** write paths fail closed; reads may
  degrade. No write proceeds on unverified state.
- **Auth expiry / wrong role / revoked delegate:** denied with a stable error
  code; no partial application.
- **Adversarial input (oversized batch, griefing, spoofed webhooks):** batches
  are bounded; spoofed webhooks are rejected by signature/authorization checks;
  griefing is rate-limited and authorized per entrypoint.
- **Testnet vs mainnet misconfig:** network passphrase and address config are
  validated; mainnet-affecting changes are gated behind a feature flag or
  kill-switch.

## Related Documents

- [SECURITY.md](../SECURITY.md) — security policy, scope, and reporting.
- [docs/threat-model.md](threat-model.md) — trust boundaries and mitigations.
- [docs/access-control-checklist.md](access-control-checklist.md) — access
  control review checklist.
- [docs/rollback-guide.md](rollback-guide.md) — rollback invariants and flags.
- [docs/aa-milestone-roadmap.md](aa-milestone-roadmap.md) — AA exit criteria.
