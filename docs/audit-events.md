# Audit Events

This document defines the canonical audit-event semantics for Mux contracts and
bindings. Audit events are the source of truth for off-chain indexers, ops
dashboards, and incident response. Any privileged or money-path action MUST emit
an audit event with a stable `type`, a `correlationId`, and a redacted payload.

## Envelope

Every audit event shares the same envelope:

| Field           | Type     | Required | Notes                                                        |
| --------------- | -------- | -------- | ------------------------------------------------------------ |
| `type`          | `string` | yes      | Stable, namespaced event type (see below).                   |
| `correlationId` | `string` | yes      | Caller-supplied id; echoed back for tracing. Never a secret. |
| `actor`         | `string` | yes      | Owner / guardian / delegate address that authorized the call.|
| `subject`       | `string` | yes      | Address or resource the event applies to.                    |
| `timestamp`     | `number` | yes      | Unix seconds, contract clock.                                |
| `data`          | `object` | no       | Event-specific, redacted payload.                            |

### Redaction rules

- Never emit raw key material, JWTs, webhook secrets, or API keys.
- Addresses are emitted in full; secrets are never emitted at all.
- `correlationId` MUST be opaque and MUST NOT encode credentials.

## Recovery events

Recovery flows (cancel / advance) emit the following event types. These match
the typed entrypoints in `bindings/src/generated/mux-delegation.ts`.

### `recovery.cancel`

Emitted when an in-flight recovery is cancelled by an authorized actor.

| Field           | Value                                                        |
| --------------- | ------------------------------------------------------------ |
| `type`          | `recovery.cancel`                                            |
| `correlationId` | Echoed from the request.                                     |
| `actor`         | Owner or guardian that authorized the cancel.                |
| `subject`       | Account whose recovery is being cancelled.                   |
| `data.reason`   | Optional, bounded string (<= 256 bytes).                     |

Semantics:

- Idempotent: cancelling an already-cancelled or non-existent recovery is a
  no-op that still emits `recovery.cancel` with `data.replayed = true`.
- Fail-closed: if the recovery store / RPC is unavailable, the write is
  rejected with `RECOVERY_UNAVAILABLE` and no event is emitted.
- Authz: only the owner or an active guardian may cancel. A revoked delegate
  MUST be rejected with `RECOVERY_UNAUTHORIZED`.

### `recovery.advance`

Emitted when a recovery is advanced to its next stage.

| Field           | Value                                                        |
| --------------- | ------------------------------------------------------------ |
| `type`          | `recovery.advance`                                           |
| `correlationId` | Echoed from the request.                                     |
| `actor`         | Owner, guardian, or delegate that authorized the advance.    |
| `subject`       | Account whose recovery is being advanced.                    |
| `data.stage`    | New stage identifier (stable enum).                          |

Semantics:

- Idempotent: replaying an advance for the current stage is a no-op that emits
  `recovery.advance` with `data.replayed = true`.
- Fail-closed: dependency outage rejects the write with
  `RECOVERY_UNAVAILABLE`; no partial state is persisted.
- Authz: deny-by-default. Only owner, active guardian, or a non-revoked
  delegate with the `recovery:advance` scope may advance.

## Stable error codes

| Code                     | Meaning                                              |
| ------------------------ | ---------------------------------------------------- |
| `RECOVERY_UNAUTHORIZED`  | Actor lacks the required role/scope.                 |
| `RECOVERY_UNAVAILABLE`   | Dependency outage; write rejected (fail-closed).     |
| `RECOVERY_REPLAYED`      | Request replayed; treated as idempotent no-op.       |
| `RECOVERY_INVALID_STAGE` | Requested stage transition is not allowed.           |

## Correlation ids

Every recovery entrypoint accepts a `correlationId`. It is echoed in the
emitted audit event and in error responses so ops can trace a request across
contract, indexer, and dashboard. Correlation ids are opaque and MUST NOT
contain secrets.

## Observability

- Metrics: `recovery_cancel_total`, `recovery_advance_total`,
  `recovery_rejected_total{code}`.
- Logs: structured, redacted; never log raw key material or secrets.
- Alerts: page on sustained `RECOVERY_UNAVAILABLE` or authz-rejection spikes.

## Kill switch

Recovery cancel/advance are gated behind the `recovery_ops` feature flag.
When disabled, entrypoints reject with `RECOVERY_UNAVAILABLE` and emit no
state-changing event. Rollback: disable the flag; no on-chain migration needed.
