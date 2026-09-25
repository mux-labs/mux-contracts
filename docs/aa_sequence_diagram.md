# AA Sequence Diagram (Authoritative)

This document is the authoritative sequence diagram for Mux Protocol Account
Abstraction (AA) on Stellar/Soroban. It covers the AA/wallet/payment critical
path and is the reference for the invariants enforced by the contracts under
`contracts/` and the TypeScript bindings under `bindings/`.

Related references:

- [README.md](../README.md) — project overview and contributor entry points
- [SECURITY.md](../SECURITY.md) — security policy, threat model, and reporting
- [docs/aa-milestone-roadmap.md](aa-milestone-roadmap.md) — AA invariants and exit criteria
- [docs/threat-model.md](threat-model.md) — trust boundaries and mitigations
- [docs/rollback-guide.md](rollback-guide.md) — rollback invariants and kill-switch strategy

## Actors

- **Owner** — root authority for a smart account; can add/revoke delegates and guardians.
- **Delegate** — scoped operator authorized by the owner for specific actions.
- **Guardian** — recovery authority; can initiate/approve recovery, cannot spend.
- **Client** — wallet/SDK caller presenting an API-key/JWT and signed intent.
- **Factory** — account creation entrypoint (deterministic address derivation).
- **Account** — per-user smart account contract enforcing policy.
- **Policy** — spending policy module (limits, allowlists, velocity).
- **Batcher** — multi-call executor for atomic AA operations.
- **Recovery** — recovery module coordinating guardian approvals.
- **RPC/Horizon** — external dependencies for submission and state reads.

## Critical Path Overview

```
Client -> Factory        : create_account(owner, salt, init_policy)
Factory -> Account       : deploy + initialize(owner, policy, guardians)
Client -> Account        : execute(intent, authz_proof, correlation_id)
Account -> Policy        : check_spend(intent, context)
Account -> Batcher       : batch(calls[])  (optional, atomic)
Client -> Recovery       : initiate_recovery(guardian_set, correlation_id)
Recovery -> Account      : rotate_owner(new_owner)  (after threshold)
Account -> RPC/Horizon   : submit / read state (fail-closed on writes)
```

## 1. Account Creation (Factory)

```
Client                Factory                 Account
  |                      |                       |
  |-- create_account --->|                       |
  |   (owner, salt,      |                       |
  |    init_policy,      |                       |
  |    correlation_id)   |                       |
  |                      |-- derive_address ---->|
  |                      |-- deploy + init ----->|
  |                      |                       |-- set owner
  |                      |                       |-- set policy
  |                      |                       |-- set guardians
  |<-- account_address --|<-- ok / error_code ---|
```

Invariants:

- Address derivation is deterministic from `(owner, salt)`; replaying the same
  `(owner, salt)` returns the existing account and MUST NOT redeploy.
- `owner` is the only authority set at initialization; delegates and guardians
  are empty until explicitly added by the owner.
- Creation is idempotent keyed by `correlation_id`; duplicate submissions return
  the original result.

## 2. Delegation & Permissions

```
Owner                 Account                Delegate
  |                      |                       |
  |-- add_delegate ----->|                       |
  |   (delegate, scope,  |                       |
  |    expiry)           |                       |
  |                      |-- store scope ------->|
  |<-- ok / error_code --|                       |
  |                      |                       |
  |-- revoke_delegate -->|                       |
  |<-- ok / error_code --|                       |
```

Invariants:

- Only the owner may add or revoke delegates; delegate self-escalation is denied.
- Every delegate action is checked against `scope` and `expiry`; expired or
  revoked delegates fail closed with a stable error code.
- Authorization is deny-by-default: unknown roles and missing proofs are rejected.

## 3. Spending Policy (Payment Path)

```
Client                Account                Policy
  |                      |                       |
  |-- execute ---------->|                       |
  |   (intent, proof,    |-- check_spend ------->|
  |    correlation_id)   |   (amount, dest,      |
  |                      |    context)           |
  |                      |<-- allow / deny ------|
  |                      |                       |
  |                      |-- apply state ------->|
  |<-- ok / error_code --|                       |
```

Invariants:

- The contract is the source of truth for spends; clients cannot bypass policy.
- Policy checks are evaluated before any state mutation; a deny leaves state
  unchanged (fail-closed).
- Writes fail closed on RPC/Horizon outage; reads may degrade but never authorize
  a spend.
- `correlation_id` is recorded for idempotency; replayed intents are rejected or
  return the original result without double-spend.

## 4. Recovery

```
Guardian A            Recovery               Account
  |                      |                       |
  |-- initiate --------->|                       |
  |   (guardian_set,     |                       |
  |    correlation_id)   |                       |
  |                      |-- collect approvals ->|
  |                      |   (threshold)         |
  |                      |-- rotate_owner ------>|
  |<-- ok / error_code --|<-- ok / error_code ---|
```

Invariants:

- Recovery requires a guardian threshold; a single guardian cannot rotate the
  owner.
- Guardians can rotate ownership but cannot spend; recovery never moves funds.
- Recovery is idempotent per `correlation_id`; concurrent initiations converge
  on a single rotation.
- Revoked guardians are rejected; recovery fails closed if the guardian set is
  stale or the threshold is unmet.

## 5. Batcher

```
Client                Batcher                Account
  |                      |                       |
  |-- batch ------------>|                       |
  |   (calls[],          |-- execute each ------>|
  |    correlation_id)   |   (atomic)            |
  |                      |<-- ok / error_code ---|
  |<-- ok / error_code --|                       |
```

Invariants:

- Batches are atomic: if any call fails, the whole batch reverts.
- Oversized batches are rejected before execution (griefing protection).
- Each call is authorized individually; a batch cannot escalate privileges.
- Batches are idempotent per `correlation_id`.

## Cross-Cutting Invariants

- **Authz:** owner/delegate/guardian/API-key/JWT enforced server-side; clients
  cannot bypass policy. Deny-by-default for all privileged surfaces.
- **Idempotency:** every external entrypoint accepts a `correlation_id`; replayed
  or concurrent requests converge on a single effect.
- **Fail-closed:** dependency outages (RPC/DB/Horizon) block writes; no partial
  money-path state is committed.
- **Source of truth:** the contract remains authoritative for spends, recovery,
  and admin actions.
- **Observability:** stable error codes and correlation ids on every path; logs
  redact keys, JWTs, and webhook secrets.
- **Mainnet safety:** money-path or mainnet-affecting changes land behind a
  feature flag/kill-switch with a documented rollback (see
  [docs/rollback-guide.md](rollback-guide.md)).

## Stable Error Codes

| Code | Meaning |
|------|---------|
| `AA_UNAUTHORIZED` | Missing or invalid authz proof / role |
| `AA_DELEGATE_EXPIRED` | Delegate scope expired or revoked |
| `AA_POLICY_DENIED` | Spending policy rejected the intent |
| `AA_REPLAY` | Duplicate `correlation_id` |
| `AA_DEPENDENCY_DOWN` | RPC/DB/Horizon unavailable (fail-closed) |
| `AA_RECOVERY_THRESHOLD` | Guardian threshold not met |
| `AA_BATCH_TOO_LARGE` | Oversized batch rejected |

## Test Coverage

Automated coverage for these invariants lives alongside the contracts and
bindings. Unit tests cover authz negatives and idempotency; integration/e2e
cover the critical path using the existing suite patterns. See
[CONTRIBUTING.md](../CONTRIBUTING.md) for how to run the suites.
