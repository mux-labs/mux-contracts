# Architectural Decision Record: Pause and Freeze Mechanisms in Mux Protocol

**Status:** Accepted & Implemented  
**Issue:** #845  
**Date:** 2026-09-24  
**Scope:** `contracts/mux-account`, account abstraction architecture, operational incident response  

---

## 1. Context & Problem Statement

Mux Protocol provides invisible smart wallets and account abstraction (AA) on Stellar and Soroban. Smart accounts hold user assets, delegate spending rights to sub-signers, authenticate temporary session keys for Web3 applications, and execute atomic batch operations.

In decentralized finance and account abstraction, emergency stop mechanisms ("pause" or "freeze" switches) present a classic architectural tension:
- **Safety / Incident Containment:** If a user suspects a session key leak, a compromised device, or an exploit in an integrated dApp, they need an immediate, fail-closed circuit breaker to halt all outbound transfers and privileged operations.
- **Decentralization / Non-Custodial Invariants:** If protocol administrators or third parties hold a global "freeze" key over all user accounts, the system ceases to be non-custodial. A single compromised protocol admin key could freeze every wallet in the ecosystem or censor individual users.

This document formally records Mux Protocol's **Pause/Freeze Architectural Decision**, its implementation invariants, authorization model, error semantics, and operational procedures.

---

## 2. The Decision: Per-Account Self-Custodial Circuit Breaker

### 2.1 Core Architectural Decision

1. **Per-Account Circuit Breaker (Implemented in `mux-account`):**
   - Each `mux-account` smart contract instance includes an autonomous, owner-controlled circuit breaker via `pause()`, `unpause()`, and `is_paused()` entrypoints.
   - The pause state is stored in the contract's instance storage under `DataKey::Paused` (`bool`).
   - Only the cryptographically authenticated account owner (`owner.require_auth()`) can toggle the pause state.

2. **Rejection of Global Protocol-Wide Freeze Backdoor:**
   - Mux Protocol **explicitly rejects** implementing a global protocol-wide admin pause or freeze backdoor across user accounts.
   - **Rationale:**
     - *Decentralization & Censorship Resistance:* Users retain true ownership of their smart accounts. No central entity can freeze user funds or selectively block transactions.
     - *Elimination of Honeypot Admin Key:* A global admin freeze key creates an existential target for attackers. If compromised, an adversary could hold the entire protocol hostage.
     - *Self-Sovereign Security:* Account isolation guarantees that an incident affecting one account cannot cascade into a protocol-wide operational shutdown.

3. **Autonomous Guardian Recovery Alternative:**
   - In the event that an account owner loses access to their private key or detects compromise before being able to pause, emergency mitigation is delegated to `mux-recovery`.
   - The account recovery path requires an M-of-N guardian quorum and a mandatory 24-hour timelock window (`RECOVERY_TIMELOCK`), allowing the legitimate owner to cancel unauthorized recovery attempts via `cancel_recovery()`.

---

## 3. Technical Design & Invariants

### 3.1 State Machine

```
              ┌───────────────────────────┐
              │                           │
              │       ACTIVE (Normal)     │
              │    DataKey::Paused = false│
              │                           │
              └─────────────┬─────────────┘
                            │
               pause()      │     unpause()
         [owner.require_auth]│  [owner.require_auth]
                            │
                            ▼
              ┌───────────────────────────┐
              │                           │
              │       PAUSED (Halted)     │
              │    DataKey::Paused = true │
              │                           │
              └───────────────────────────┘
```

### 3.2 Invariants

| ID | Invariant | Description |
|---|---|---|
| **INV-PF-01** | **Fail-Closed Execution** | When `is_paused()` is true, any entrypoint that moves funds or mutates authorization state MUST reject immediately with `MuxAccountError::Unauthorized`. |
| **INV-PF-02** | **Owner-Only Authentication** | Only `owner.require_auth()` may alter the pause flag. Delegates, session keys, relayers, and guardians CANNOT pause or unpause the account. |
| **INV-PF-03** | **Read-Only Observability** | Query methods (`is_paused()`, `owner()`, `get_spend_limit()`, `nonce()`) remain accessible when paused to ensure transparent observability. |
| **INV-PF-04** | **Reentrancy Protection** | Pause state mutations cannot be executed while an execution frame is open (`DataKey::Executing` held). |
| **INV-PF-05** | **Audit Event Emission** | Every pause transition emits a typed Soroban event: `(mux_acct, paused)` or `(mux_acct, unpaused)`. |
| **INV-PF-06** | **Idempotent Transitions** | Calling `pause()` on an already paused account, or `unpause()` on an unpaused account, safely succeeds or asserts state without corruption. |

---

## 4. Entrypoint Matrix & Enforced Paths

When `DataKey::Paused` is `true`, all mutating entrypoints in `mux-account` are blocked:

| Entrypoint | Auth Required | Effect When Paused | Error Code |
|---|---|---|---|
| `pause()` | Owner | Idempotently sets `Paused = true` | Success |
| `unpause()` | Owner | Clears `Paused = false` | Success |
| `is_paused()` | None (Public) | Returns `true` | N/A |
| `execute(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |
| `execute_with_session(...)` | Session Key | **BLOCKED** | `Unauthorized (3)` |
| `execute_with_session_sponsored(...)`| Sponsor + Session Key | **BLOCKED** | `Unauthorized (3)` |
| `debit_spend(...)` | Self / Contract | **BLOCKED** | `Unauthorized (3)` |
| `set_delegate(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |
| `remove_delegate(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |
| `set_spend_limit(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |
| `register_session_key(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |
| `revoke_session_key(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |
| `set_sponsor(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |
| `set_metadata(...)` | Owner | **BLOCKED** | `Unauthorized (3)` |

---

## 5. Threat Modeling & Failure Modes

### 5.1 STRIDE Threat Analysis

| Threat ID | Threat Description | Category | Mitigation |
|---|---|---|---|
| **T-PF-01** | Attacker invokes `pause()` to DoS account | Denial of Service | `pause()` requires cryptographic `owner.require_auth()`. Attackers without owner key cannot invoke it. |
| **T-PF-02** | Stale session key executes after owner discovers leak | Elevation of Privilege | Owner calls `pause()`, which immediately blocks all `execute_with_session` calls regardless of session expiration. |
| **T-PF-03** | Relayer bypasses pause via sponsored call | Tampering | `execute_with_session_sponsored` checks `is_paused()` before checking relayer or session signatures. |
| **T-PF-04** | Protocol admin key leak compromises user accounts | Elevation of Privilege | There is NO protocol admin key in `mux-account`. The contract is immutable and self-custodial. |
| **T-PF-05** | Horizon / RPC outage during pause attempt | Denial of Service | Fail-closed off-chain clients refuse to process payments if RPC node is unresponsive. |

---

## 6. Operational Runbook

### 6.1 Emergency Account Pause (User / Operator)

To freeze an account immediately using the Stellar CLI:

```bash
# 1. Invoke pause with owner secret
stellar contract invoke \
  --id <MUX_ACCOUNT_CONTRACT_ID> \
  --source <OWNER_SECRET_KEY> \
  --network <mainnet|testnet> \
  -- pause

# 2. Verify account is paused
stellar contract invoke \
  --id <MUX_ACCOUNT_CONTRACT_ID> \
  --source <ANY_KEY> \
  --network <mainnet|testnet> \
  -- is_paused
# Expected output: true
```

### 6.2 Post-Incident Unpause & Remediation

Once the cause of the incident has been resolved (e.g. revoked leaked session keys or rotated delegates):

```bash
# 1. Revoke any compromised session keys or delegates while still paused if needed
# 2. Invoke unpause
stellar contract invoke \
  --id <MUX_ACCOUNT_CONTRACT_ID> \
  --source <OWNER_SECRET_KEY> \
  --network <mainnet|testnet> \
  -- unpause

# 3. Verify normal operation resumed
stellar contract invoke \
  --id <MUX_ACCOUNT_CONTRACT_ID> \
  --source <ANY_KEY> \
  --network <mainnet|testnet> \
  -- is_paused
# Expected output: false
```
