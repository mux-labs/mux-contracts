# mux-recovery — Recovery Trust Model

**Version:** 0.2.0
**Status:** Living document — update whenever the recovery contract changes.

---

## 1. Purpose

`mux-recovery` provides social recovery for `mux-account` owners. If an owner loses access to their private key, a quorum of pre-registered guardians can transfer ownership to a new address after a mandatory timelock delay.

---

## 2. Roles and Trust Levels

| Role | Who | Trust Level | Capabilities |
|---|---|---|---|
| **Owner** | Account holder | Highest | Cancel any pending recovery; call `set_registry`; normal account operations |
| **Guardian** | Trusted contacts set by owner at init | High | Initiate and execute recovery |
| **Stranger** | Any other address | None | No recovery operations |

> **Key invariant:** Guardians are set at `initialize` time and are immutable. The owner cannot change guardians after deployment (prevents a compromised owner from removing guardians before an attack).

---

## 3. Recovery Lifecycle

```
         Guardian calls
         initiate_recovery()
               │
               ▼
         ┌──────────┐
         │ PENDING  │ ◄─── Owner can cancel_recovery() at any time
         └──────────┘
               │
    RECOVERY_TIMELOCK ledgers elapse
    (~24 hours at 5s close time)
               │
               ▼
    Guardian calls execute_recovery()
               │
               ▼
         ┌──────────┐
         │ EXECUTED │  ownership transferred to new_owner
         └──────────┘

    OR

    Owner calls cancel_recovery()
               │
               ▼
         ┌───────────┐
         │ CANCELLED │  no ownership change
         └───────────┘
```

### State transitions

| From | Event | To | Who |
|---|---|---|---|
| *(none)* | `initiate_recovery` | `Pending` | Guardian |
| `Pending` | `execute_recovery` (after timelock) | `Executed` | Guardian |
| `Pending` | `cancel_recovery` | `Cancelled` | Owner |
| `Executed` | — | *(terminal)* | — |
| `Cancelled` | `initiate_recovery` | `Pending` | Guardian (new request) |

---

## 4. Security Properties

### 4.1 Timelock (24-hour cancellation window)

`RECOVERY_TIMELOCK = 17_280` ledgers ≈ 24 hours.

The timelock is the primary defence against a compromised guardian set. Even if all guardians collude to steal the account, the legitimate owner has 24 hours to observe the `rec_init` event on-chain and call `cancel_recovery`.

**Assumption:** The owner monitors their account (or has an automated watcher) at least once every 24 hours.

The `rec_init` event payload carries `initiated_at`, `executable_at`, and `expires_at` so that off-chain watchers can surface deadlines without a follow-up storage read.

**TypeScript binding constants** (exported from `bindings/src/types.ts`):

```ts
import { RECOVERY_TIMELOCK_LEDGERS, RECOVERY_EXPIRY_LEDGERS } from "@mux-protocol/contracts";

// Compute deadlines from the rec_init event without an RPC call:
const executableAt = initiatedAt + RECOVERY_TIMELOCK_LEDGERS; // ~24 h
const expiresAt    = initiatedAt + RECOVERY_EXPIRY_LEDGERS;   // ~7 days
```

These mirror the on-chain `RECOVERY_TIMELOCK` and `RECOVERY_EXPIRY` constants
and are **stable ABI** — changing them is a breaking change for off-chain tooling.

### 4.2 Single active request

Only one `Pending` request may exist at a time. A second `initiate_recovery` call while a request is `Pending` returns `RecoveryAlreadyPending`. This prevents guardians from flooding the contract with requests to confuse the owner.

### 4.3 Guardian-only initiation and execution

`initiate_recovery` and `execute_recovery` both verify the caller is in the guardian set via `require_auth` + membership check. A non-guardian call returns `Unauthorized`.

### 4.4 Owner-only cancellation

`cancel_recovery` requires `owner.require_auth()`. Only the current owner can cancel, preventing guardians from cancelling their own recovery attempt.

### 4.5 Audit events

Every state mutation emits a structured event under the `mux_recv` contract tag. The topics and data payload for each entrypoint are:

| Entrypoint | Action topic | Data payload |
|---|---|---|
| `initialize` | `init` | `owner: Address` |
| `initiate_recovery` | `rec_init` | `(guardian: Address, new_owner: Address, initiated_at: u32, executable_at: u32, expires_at: u32)` |
| `approve_recovery` | `rec_appr` | `(guardian: Address, approval_count: u32)` |
| `execute_recovery` | `rec_exec` | `(guardian: Address, new_owner: Address)` |
| `approve_recovery_admin` | `rec_adm` | `new_owner: Address` |
| `cancel_recovery` | `rec_cncl` | `()` |
| `add_guardian` | `grd_add` | `guardian: Address` |
| `remove_guardian` | `grd_rm` | `guardian: Address` |
| `set_quorum_threshold` | `qrm_set` | `threshold: u32` |
| `set_registry` | `reg_link` | `registry_id: Address` |
| `recovery_request` | _(read-only)_ | Returns the full `RecoveryRequest` struct, no event emitted |

> **Note:** The `rec_init` payload carries the full timelock window as a
> five-tuple. `initiated_at` is the ledger sequence at initiation, and
> `executable_at`/`expires_at` are the same deadlines stored in the request
> struct (`initiated_at + RECOVERY_TIMELOCK` / `initiated_at + RECOVERY_EXPIRY`).
> Indexers surface deadlines directly from the event data without a
> follow-up storage read.

All events follow the two-topic convention defined in [`docs/event-topic-conventions.md`](event-topic-conventions.md):

```text
topics[0]  "mux_recv"   — contract tag (Symbol)
topics[1]  <action>     — action name (Symbol, ≤ 8 chars)
data       <payload>    — action-specific value
```

Off-chain watchers should subscribe to `rec_init` events and alert the owner immediately when a recovery is initiated.

### 4.6 Registry link

An optional registry contract address (`registry_id`) can be associated with the recovery contract after initialization via `set_registry(owner, registry_id)`.

- The field is stored under `DataKey::RegistryId` in instance storage.
- Reading `registry_id()` returns `Option<Address>` — `None` if not set.
- Setting the registry requires the current **owner's authorization**: the
  caller-supplied `owner` argument must equal the stored owner (a mismatch is
  rejected with `Unauthorized`), and `owner.require_auth()` is called before
  the storage write.
- A `reg_link` audit event is emitted each time the registry address is written, providing a full on-chain audit trail of any registry re-links.
- The stored address is informational: the contract does **not** call the registry at link time. Off-chain tooling should cross-check that the registry contract at that address contains the expected metadata for this recovery deployment.
- TypeScript binding methods: `setRegistry(sourceKeypair, owner, registryId)` and `getRegistryId(sourceKeypair)`.

### 4.7 Recovery request struct query

The `recovery_request()` entrypoint returns the full `RecoveryRequest` struct (not just the status), which includes:

| Field | Type | Description |
|---|---|---|
| `new_owner` | `Address` | The proposed new owner address |
| `initiated_at` | `u32` | Ledger sequence when recovery was initiated |
| `executable_at` | `u32` | Earliest ledger for `execute_recovery` (`initiated_at + RECOVERY_TIMELOCK`) |
| `expires_at` | `u32` | Latest ledger; auto-expires after this (`initiated_at + RECOVERY_EXPIRY`) |
| `status` | `RecoveryStatus` | Current lifecycle state |

This entrypoint is read-only (no event emitted) and is designed primarily for off-chain indexers and TypeScript bindings that need the complete struct.

- TypeScript binding method: `recoveryRequest(sourceKeypair)` returns `Promise<RecoveryRequest | null>`.
- The on-chain `RecoveryRequest` struct is serialised via `#[contracttype]` and directly deserialisable in the TypeScript client.

### 4.8 Guardian N-of-M threshold (quorum)

Recovery requires an **M-of-N guardian quorum**: `M` distinct guardians must
approve a pending request before it can be executed, out of the `N` guardians
registered at `initialize` time. This bounds the blast radius of a single
compromised guardian while keeping recovery possible if some guardians are
unreachable.

**Definitions**

| Symbol | Meaning |
|---|---|
| `N` | Number of guardians registered at `initialize` (immutable) |
| `M` | Quorum threshold — distinct approvals required to execute |
| `approval_count` | Number of distinct guardians that have called `approve_recovery` for the current pending request |

**Invariants (fail-closed)**

1. `1 ≤ M ≤ N`. A threshold of `0` or `M > N` is rejected at configuration time.
2. `N ≥ 1`. Initializing with an empty guardian set is rejected.
3. Guardian addresses are **unique**; a duplicate in the init set is rejected.
4. Approvals are counted **per distinct guardian**. A guardian calling
   `approve_recovery` twice does not increment `approval_count` (idempotent).
5. `execute_recovery` requires `approval_count ≥ M`; otherwise it fails closed
   with `QuorumNotMet` and no ownership change occurs.
6. The threshold is **owner-controlled**: only the current owner may call
   `set_quorum_threshold`, and the new value must satisfy invariant (1).
   Guardians cannot raise or lower the threshold.

**Configuration entrypoints**

| Entrypoint | Auth | Behavior |
|---|---|---|
| `initialize(owner, guardians, threshold)` | `owner.require_auth()` | Validates invariants (1)–(3); stores `N`, `M`, and the guardian set |
| `set_quorum_threshold(owner, threshold)` | `owner.require_auth()` | Validates invariant (1) against the stored `N`; emits `qrm_set` |
| `approve_recovery(guardian)` | `guardian.require_auth()` + membership | Records a distinct approval; emits `rec_appr` with the running `approval_count` |
| `execute_recovery(guardian)` | `guardian.require_auth()` + membership | Requires `approval_count ≥ M`; emits `rec_exec` |

**Stable error codes**

| Error | Raised when |
|---|---|
| `InvalidThreshold` | `M == 0` or `M > N` (init or `set_quorum_threshold`) |
| `EmptyGuardianSet` | `initialize` called with `N == 0` |
| `DuplicateGuardian` | `initialize` guardian set contains a repeated address |
| `QuorumNotMet` | `execute_recovery` called with `approval_count < M` |
| `Unauthorized` | Caller is not the owner (threshold changes) or not a guardian (approve/execute) |
| `RecoveryAlreadyPending` | `initiate_recovery` while a request is `Pending` |

**Fail-closed behavior**

- Invalid threshold configuration is rejected **before** any storage write; the
  contract never persists an out-of-range `M`.
- If `execute_recovery` is called below quorum, the request stays `Pending` and
  the owner's 24 h cancellation window is unaffected.
- Auth failures (expired/absent signature, wrong role, revoked guardian) return
  `Unauthorized` and perform no state mutation.

**Test coverage**

Unit and integration tests in `contracts/mux-recovery/src/test.rs` assert the
invariants above, including auth negatives:

- `init` rejects `M == 0`, `M > N`, `N == 0`, and duplicate guardians.
- `set_quorum_threshold` rejects non-owner callers and out-of-range values.
- `approve_recovery` is idempotent per guardian (double-approve does not raise
  `approval_count`).
- `execute_recovery` fails with `QuorumNotMet` below `M` and succeeds at exactly
  `M` distinct approvals.
- Non-guardian `approve_recovery`/`execute_recovery` returns `Unauthorized`.

---

## 5. Threat Scenarios

| Threat | Mitigation |
|---|---|
| Attacker compromises one guardian | Single guardian can initiate but owner has 24 h to cancel; execution still requires `M` distinct approvals |
| All guardians collude | Owner has 24 h cancellation window; monitor `rec_init` events |
| Attacker compromises fewer than `M` guardians | Quorum not met — `execute_recovery` fails closed with `QuorumNotMet` |
| Owner tries to weaken quorum | `set_quorum_threshold` is owner-only and bounded by `1 ≤ M ≤ N` |
