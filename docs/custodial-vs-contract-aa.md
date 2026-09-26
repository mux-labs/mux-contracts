# Custodial vs Contract Account Abstraction

**Version:** 1.1.0  
**Date:** 2026-08-26  
**Status:** Audit Ready  
**Issue:** #686  
**Related:** [AA Backend Orchestrator](aa-backend-orchestrator.md), [Storage Griefing](storage-griefing.md)

---

This document provides a comparison of Custodial and Contract-based Account Abstraction (AA) architectures and highlights why Contract AA is the preferred approach for the Mux Protocol. It also documents the split between custodial backend operations (mux-backend) and on-chain contract AA (mux-account) to prevent double-spend attacks on spend limits.

## Custodial Account Abstraction

In Custodial AA (often implemented via MPC or hosted custodial wallets):
- **Key Management**: Keys are generated and managed off-chain by a third-party service provider or split among parties (MPC).
- **Execution**: Transactions are signed off-chain and submitted directly as standard transactions.
- **Trust Model**: Requires trust in the custodial provider or MPC network to not collude or lose key shares.
- **Flexibility**: Feature set depends largely on the provider's API. Adding on-chain logic (like native spend limits) often requires off-chain enforcement, which is less secure.

### Pros:
- Faster initial setup for users (Web2-like onboarding).
- Zero smart contract deployment cost per user.
- High compatibility with legacy dApps.

### Cons:
- Centralized point of failure or reliance on a specific vendor.
- Security policies (spend limits, recovery) are enforced off-chain, meaning they can be bypassed if the central system is compromised.

## Contract-based Account Abstraction

In Contract-based AA (like the architecture in Mux Protocol):
- **Key Management**: The user still possesses an owner key (which can be held in a simple hardware wallet or derived from Web3Auth), but the authoritative account is a smart contract.
- **Execution**: Transactions are payloads submitted to the smart contract, which verifies rules before forwarding calls.
- **Trust Model**: Trustless. Code is law. The smart contract enforcing the rules is fully auditable on-chain.
- **Flexibility**: Infinite. Complex logic like session keys, guardian-based recovery, and granular spend limits are enforced natively at the protocol level.

### Pros:
- **True Decentralization**: No vendor lock-in or central party that can freeze funds outside of contract rules.
- **Programmable Security**: On-chain enforced spend limits and guardian recovery cannot be bypassed.
- **Session Keys**: Allows granular delegation of specific actions to secondary keys (e.g., auto-paying subscriptions) without exposing the main key.

### Cons:
- Requires deploying a smart contract per user (handled efficiently by the `mux-account-factory`).
- Slight gas overhead for the contract execution and validation logic.

## Why Mux Chooses Contract AA

Mux Protocol implements **Contract-based Account Abstraction**. We believe that while custodial solutions offer a smooth Web2-like experience, they fundamentally compromise on the decentralized ethos of Web3. By using Contract AA, Mux provides the best of both worlds:
- Gasless transactions and session keys provide the UX of custodial wallets.
- Smart contracts provide the security, transparency, and self-custody guarantees of true DeFi.

---

## Mux-Backend vs On-Chain Split (#686)

**Context:** The Mux architecture includes both:
1. **mux-backend** — an off-chain orchestrator that manages "invisible wallets" (custodial session keys, relayer coordination, gasless transaction submission)
2. **mux-account** — on-chain smart contract AA that enforces spend limits, delegation rules, and recovery policies

**Critical requirement:** Invisible wallets managed by mux-backend and on-chain mux-account spend limits **must not double-spend**. If both the backend and the contract enforce separate spend limits independently, a user could potentially exhaust their limit twice (once via the backend's invisible wallet path, once via direct on-chain execution).

### Architecture Invariants

| Component | Spend Limit Enforcement | Authority | Double-Spend Risk |
|-----------|------------------------|-----------|-------------------|
| **mux-backend (invisible wallets)** | **Off-chain tracking only** — does NOT enforce limits, only records for UI/analytics | Backend relayer holds custody of session keys | ✓ Safe — no enforcement, just telemetry |
| **mux-account (on-chain AA)** | **On-chain enforcement via `debit_spend`** — canonical source of truth | User's owner key + session keys (on-chain verified) | ✓ Safe — single authoritative ledger |
| **mux-policy (optional)** | **On-chain enforcement via `record_spend`** — independent per-wallet daily limits | Wallet self-reports spending | ✓ Safe — wallet.require_auth() prevents impersonation |

### Spend Limit Flows

#### Flow 1: Direct On-Chain Execution (No Backend)

```
User → mux-account.execute_with_session()
       ├─ session_key.require_auth() ✓
       ├─ Check session key scope & expiry
       └─ Call target contract
          └─ mux-account.debit_spend(asset, amount)  ← on-chain limit enforced here
             └─ SpendLimit(asset).spent += amount
```

**Enforcement point:** `mux-account.debit_spend()` is the **only** function that mutates `SpendLimit` storage. It requires `current_contract_address().require_auth()`, so only calls originating from within the same `mux-account` contract can debit the limit.

#### Flow 2: Backend-Orchestrated Execution (Invisible Wallet)

```
User → mux-backend API (/v1/transactions/intent)
       ├─ Backend validates user signature off-chain
       ├─ Backend estimates gas & batches via mux-batcher
       └─ Backend submits: mux-batcher.execute_batch(caller=backend_relayer, ops=[...])
          └─ mux-batcher dispatches to mux-account.execute_with_session()
             └─ mux-account.debit_spend(asset, amount)  ← same on-chain limit enforced
                └─ SpendLimit(asset).spent += amount
```

**Key insight:** Even when the backend submits transactions on behalf of the user, the **on-chain spend limit in mux-account is still enforced**. The backend does not bypass `debit_spend`; it merely relays the transaction.

#### Flow 3: Policy-Based Daily Limits (Optional)

```
Wallet → mux-policy.record_spend(wallet, amount)
         ├─ wallet.require_auth() ✓  ← only wallet itself can report spending
         ├─ Check WalletLimit(wallet).spent + amount <= limit
         └─ WalletLimit(wallet).spent += amount
```

**Independence:** `mux-policy` is a separate contract with its own per-wallet daily limits. It does **not** share storage with `mux-account.SpendLimit`. A wallet can have:
- A per-asset spend limit in `mux-account` (enforced on every `debit_spend`)
- A global daily limit in `mux-policy` (enforced on every `record_spend`)

These are **additive constraints**, not duplicative. A transaction must satisfy **both** if both are configured.

### Double-Spend Prevention Rules

1. **Backend invisible wallets MUST NOT enforce limits independently.**  
   The backend may track spending for analytics and UI purposes, but it must not reject a transaction based on its own off-chain limit counter. The on-chain `debit_spend` call is the canonical authority.

2. **All spending paths MUST flow through mux-account.debit_spend().**  
   Whether the user signs directly or the backend relays on their behalf, the final execution must invoke `debit_spend` to mutate on-chain limits.

3. **Backend relayer address is NOT exempt from limits.**  
   The fact that the backend's relayer key signs the Soroban transaction (to cover gas) does not grant it the ability to bypass spend limits. The `session_key.require_auth()` check inside `execute_with_session` still applies, and `debit_spend` still enforces limits.

4. **mux-policy daily limits are orthogonal to mux-account spend limits.**  
   If a wallet is configured with both, the wallet must satisfy **both** constraints. They are not alternatives; they are cumulative. The backend orchestrator must respect both when estimating whether a transaction will succeed.

### Backend API Requirements

To prevent double-spend attacks, the mux-backend API must:

- **Pre-flight check:** Before accepting a user intent, query the on-chain `mux-account` to fetch the current `SpendLimit(asset).spent` and `amount` values. Reject the intent if `spent + new_amount > amount`.
- **Batch atomicity:** When batching multiple operations via `mux-batcher`, ensure that the cumulative spending across all operations in the batch does not exceed remaining limits. The backend should simulate the batch to predict failure before submission.
- **Nonce synchronization:** Maintain a mapping of `account_id → latest_nonce` to prevent replay attacks. The backend must not submit a transaction with a stale nonce that could cause the on-chain execution to fail.
- **Error propagation:** If a transaction fails on-chain due to `ExceedsLimit`, the backend must surface this error to the client with the exact on-chain error code (not a generic "transaction failed" message).

### Audit Checklist

- [x] `mux-account.debit_spend()` is the only function that mutates `SpendLimit` storage (verified by code inspection).
- [x] `debit_spend()` requires `current_contract_address().require_auth()` — no external caller can directly debit (verified by unit test: `test_debit_spend_requires_contract_auth`).
- [x] `execute_with_session()` always calls `debit_spend()` for spending operations (verified by code inspection and integration tests).
- [x] Backend orchestrator API documentation specifies pre-flight limit checks (see [aa-backend-orchestrator.md](aa-backend-orchestrator.md)).
- [x] `mux-policy.record_spend()` requires `wallet.require_auth()` — third parties cannot debit a wallet's allowance (verified by unit test: `test_record_spend_requires_wallet_auth`).
- [x] Double-spend prevention rules documented in this section (#686).

### Test Coverage

| Test | Contract | What it verifies |
|------|----------|------------------|
| `test_debit_spend_enforces_limit` | `mux-account` | Spending beyond limit returns `ExceedsLimit` |
| `test_debit_spend_requires_contract_auth` | `mux-account` | External calls to `debit_spend` fail auth check |
| `test_execute_with_session_debits_spend` | `mux-account` | Session key execution increments spent counter |
| `test_record_spend_requires_wallet_auth` | `mux-policy` | Only wallet itself can call `record_spend` |
| `test_policy_limit_independent_of_account_limit` | Integration | Both limits enforced independently; satisfying one does not bypass the other |

---

## Deployment and Operational Considerations

### Invisible Wallet Key Management

If mux-backend manages invisible wallets (custodial session keys for gasless transactions):
- Keys must be stored in an HSM or secure enclave, not in plaintext.
- Each invisible wallet must map 1:1 to a user's on-chain `mux-account` address.
- The backend must never re-use an invisible wallet across multiple users.

### Monitoring and Alerts

Deploy monitoring for:
- **Limit exhaustion rate:** Alert when a significant fraction of users hit their spend limits within a short time window (could indicate a UI bug or coordinated attack).
- **Nonce gaps:** Alert when submitted transactions fail due to nonce mismatches (could indicate a backend state desync).
- **Backend vs on-chain drift:** Periodically reconcile the backend's off-chain spend tracking with on-chain `SpendLimit` storage. Large discrepancies indicate either a backend bug or an on-chain limit bypass attempt.

### Upgradeability

If `mux-account` or `mux-policy` contracts are upgradeable:
- The upgrade path must preserve `SpendLimit` and `WalletLimit` storage layouts.
- Any change to the spend limit enforcement logic must be audited to ensure no double-spend vector is introduced.
- See [mux-account-upgrade.md](mux-account-upgrade.md) and [mux-policy-upgrade.md](mux-policy-upgrade.md) for storage-compatibility rules.

---

## Summary

| Feature | Custodial AA | Contract AA (Mux) | Mux-Backend Split |
|---------|-------------|-------------------|-------------------|
| **Key custody** | Provider holds keys | User holds owner key | Backend holds relayer key only |
| **Spend limits** | Off-chain enforcement | On-chain enforcement | Backend respects on-chain limits |
| **Recovery** | Provider-controlled | Guardian-based (on-chain) | N/A |
| **Trust model** | Trust provider | Trustless (code) | Backend cannot bypass limits |
| **Double-spend risk** | Medium (provider can bypass) | Low (on-chain enforcement) | **None** (backend queries on-chain) |

Mux Protocol's architecture ensures that custodial-like UX (gasless transactions, relayer execution) does not compromise the security guarantees of on-chain contract AA. Spend limits are **always** enforced on-chain, regardless of whether the user signs directly or the backend relays on their behalf.
