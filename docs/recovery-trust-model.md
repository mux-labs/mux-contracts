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

---

## 5. Threat Scenarios

| Threat | Mitigation |
|---|---|
| Attacker compromises one guardian | Single guardian can initiate but owner has 24 h to cancel |
| All guardians collude | Owner has 24 h cancellation window; monitor `rec_init` events |
| Owner loses key, no guardians | Recovery impossible — owner must set guardians at init |
| Attacker spams recovery requests | Only one Pending request allowed; each requires guardian auth |
| Replay of old recovery request | Each request stores `initiated_at`; executed/cancelled requests cannot be re-executed |
| Owner tries to remove guardians | Guardian set is immutable after `initialize` |
| Compromised owner bypasses 24 h timelock via `approve_recovery_admin` | `approve_recovery_admin` now requires **both** owner auth and a registered guardian co-sign; a compromised owner key alone cannot execute the fast path |
| Malicious registry link | `set_registry` requires owner auth; `reg_link` event provides on-chain audit trail |

---

## 6. Limitations and Future Work

- **M-of-N quorum implemented.** `execute_recovery` now requires `approvals.len() >= quorum_threshold`. Guardians call `approve_recovery(guardian)` to add their approval after `initiate_recovery` records the first. The threshold is set at `initialize` time and adjustable by the owner via `set_quorum_threshold`.
- **Immutable guardian set after initialization.** Guardians cannot be rotated without redeploying the contract. A guardian rotation mechanism with its own timelock is planned.
- **No guardian liveness check.** If all guardians lose their keys, recovery is impossible.
- **No on-chain registry validation.** The `registry_id` field stores an address but does not call the registry at initialization time. Off-chain tooling must verify the link is correct and that the registry entry matches the deployed contract.

---

## 7. Guardian Liveness and Last-Resort Recovery

### 7.1 The Liveness Problem

`mux-recovery` has no on-chain fallback path when the entire guardian set loses access. If every
registered guardian key is permanently lost or destroyed, recovery becomes **permanently
impossible** for that account. There is no administrative override, no time-expired bypass, and no
protocol-level last resort built into the current contract.

This is a deliberate trade-off: adding an admin-controlled escape hatch would introduce a
centralised trust vector that contradicts the social-recovery security model. However, the
consequence must be clearly understood by operators and account holders before deploying.

### 7.2 Failure Scenarios

| Scenario | Effect | Can it be recovered? |
|---|---|---|
| One guardian loses their key | Remaining guardians can still initiate/execute | Yes — if ≥ 1 guardian remains |
| All guardians lose their keys simultaneously | Recovery is permanently blocked | No — on-chain state is unrecoverable |
| Guardian device is compromised (key not lost) | Attacker can initiate recovery; owner has 24-h window to cancel | Yes — owner must act within `RECOVERY_TIMELOCK` |
| Owner loses key AND all guardians lose keys | Account is permanently locked | No |

### 7.3 Operational Mitigations

The following practices dramatically reduce the probability of total guardian liveness failure:

**1. Use geographically distributed guardian keys**

Store each guardian key in a different physical location and under the control of a different
person. A fire, flood, or hardware failure at a single location cannot wipe out the entire guardian
set.

**2. Maintain at least 3 guardians**

The contract enforces a minimum of 1 guardian (`MinGuardiansRequired`), but 1 is insufficient for
liveness — any single key loss destroys the recovery path. The recommended minimum is **3
guardians**: this tolerates one key loss while keeping a 2-guardian majority.

> **Rule of thumb:** For a maximum-security account, use 5 guardians and require a policy that any
> 3 can execute recovery (on-chain quorum threshold is planned for a future version).

**3. Periodic guardian key attestation**

Run a regular off-chain attestation process (e.g. quarterly) where each guardian signs a
challenge to prove their key is still accessible. Automate alerts when a guardian fails to attest.
This catches key loss before it becomes unrecoverable.

```bash
# Example: guardian signs a ledger-specific challenge
stellar transaction sign --network mainnet --source <GUARDIAN_SECRET> <CHALLENGE_TX>
```

**4. Store guardian seeds in hardware security modules (HSMs) or hardware wallets**

Software-only key storage is susceptible to device failure and data corruption. Use hardware
wallets (Ledger, Trezor) or cloud HSMs (AWS CloudHSM, Azure Key Vault) for at least two of the
three guardians.

**5. Maintain encrypted cold-storage backups**

For each guardian key, maintain at least one encrypted backup in a geographically separate
location (e.g. a safety deposit box). Use a strong encryption standard (AES-256) and store the
decryption key separately from the backup.

**6. Document and test the recovery runbook**

Write a formal recovery runbook that every guardian can follow without external guidance. Test the
runbook at least once per year by initiating a recovery on a test account, running through the
full timelock, and then cancelling it. Confirm:

- Each guardian can locate their key and use it.
- The off-chain monitoring system correctly detects `rec_init` events.
- The owner can call `cancel_recovery` before `executable_at`.

### 7.4 Monitoring for `rec_init` Events

Off-chain watchers **must** subscribe to `rec_init` events on-chain. Each guardian liveness
incident (key rotation, suspected compromise, failed attestation) should trigger an immediate
review:

```
Event: mux_recv / rec_init
Data:  (guardian, new_owner, initiated_at, executable_at, expires_at)
```

Set up alerts for:
- Any `rec_init` event not initiated by the account holder.
- Any `rec_init` event where `new_owner` does not match the expected address.
- Any guardian failing the periodic attestation.

### 7.5 What Happens When All Guardians Are Lost

There is **no on-chain recovery path** when every guardian key is permanently inaccessible. The
contract will return:

- `NotInitialized` or `Unauthorized` from `initiate_recovery` (no valid guardian can sign).
- The account is effectively frozen: the owner can still operate it using their own key, but
  recovery is permanently disabled.

**The only mitigations are preventative** (see §8.3). Once all guardian keys are lost, the
account is unrecoverable at the protocol level. The owner must accept this risk or redeploy a new
account with a fresh guardian set.

### 7.6 Future Protocol Improvements

The following features are planned for future contract versions to improve liveness guarantees:

| Feature | Description | Priority |
|---|---|---|
| **M-of-N guardian quorum** | Require M signatures from N guardians to execute recovery, tolerating up to N-M key losses | High |
| **Guardian key rotation** | Allow the owner to rotate individual guardian keys with a timelock, without redeploying | High |
| **Social recovery with time-lock escalation** | If recovery is not initiated within a configured inactivity window, escalate to a social fallback (e.g. a protocol multisig) | Medium |
| **Guardian liveness oracle** | On-chain heartbeat mechanism where guardians periodically attest liveness; missed attestations generate alerts | Low |

Until M-of-N quorum is implemented, operators should compensate operationally by following the
mitigations in §7.3.

---

## 8. Related Documents

- [`docs/threat-model.md`](threat-model.md) — overall Mux Protocol threat model
- [`docs/audit-events.md`](audit-events.md) — full event schema reference
- [`contracts/mux-recovery/src/lib.rs`](../contracts/mux-recovery/src/lib.rs) — contract source
- `#403` — Recovery registry link implementation; tracks the `set_registry()` entrypoint and `registry_id` storage
- `#616` — On-chain registry validation for `set_registry`; closes the gap documented in §4.6
- `#618` — Guardian liveness documentation; closes the gap documented in §8
