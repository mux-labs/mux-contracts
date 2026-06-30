# Mux Protocol — Audit Log Events

**Version:** 0.1.0  
**Status:** Living document — update whenever a new event is added or an existing one changes.

---

## Overview

Every state-mutating operation in Mux contracts emits a Soroban event via `env.events().publish(topics, data)`.  
Events are indexed on-chain and can be streamed from any Soroban RPC node using the `getEvents` method.

### Topic structure

All events use a two-element topic vector:

```
topics[0]  contract_tag  Symbol  e.g. "mux_acct", "mux_perm", "mux_bat"
topics[1]  action        Symbol  e.g. "init", "dlg_set", "role_grt"
```

The `data` field carries action-specific payload encoded as a Soroban `Val`.

---

## mux-account events

Contract tag: `mux_acct`

| Action | Trigger | Data payload |
|---|---|---|
| `init` | `initialize` succeeds | `owner: Address` |
| `dlg_set` | `set_delegate` succeeds | `(delegate: Address, expiry_ledger: u32, can_spend: bool)` |
| `dlg_rm` | `remove_delegate` succeeds | `delegate: Address` |
| `lmt_set` | `set_spend_limit` succeeds | `(asset: Address, amount: i128, period_ledgers: u32)` |
| `debited` | `debit_spend` succeeds | `(asset: Address, spend: i128)` |

---

## mux-account-factory events

Contract tag: `mux_fac`

| Action | Trigger | Data payload |
|---|---|---|
| `deployed` | `deploy_account` or `deploy_account_with_metadata` succeeds | `(owner: Address, account_address: Address)` |
| `meta_set` | `deploy_account_with_metadata` succeeds | `(owner: Address, account_address: Address, version: String)` |

> `get_accounts`, `account_count`, and `get_account_metadata` are read-only and emit no events.

---

## mux-permissions events

Contract tag: `mux_perm`

| Action | Trigger | Data payload |
|---|---|---|
| `init` | `initialize` succeeds | `admin: Address` |
| `role_crt` | `create_role` succeeds | `role: Symbol` |
| `role_grt` | `grant_role` succeeds | `(account: Address, role: Symbol)` |
| `role_rev` | `revoke_role` succeeds | `(account: Address, role: Symbol)` |
| `adm_thr` | `set_admin_threshold` succeeds | `threshold: u32` |
| `adm_prp` | `propose_admin` adds a new candidate | `new_admin: Address` |
| `adm_apr` | `approve_admin` records an approval (threshold not yet reached) | `(approver: Address, new_admin: Address)` |
| `adm_prm` | `approve_admin` promotes a candidate (threshold reached) | `new_admin: Address` |

---

## mux-delegation events

Contract tag: `mux_dlg`

| Action | Trigger | Data payload |
|---|---|---|
| `dlg_grant` | `grant_delegate` succeeds | `(owner: Address, delegate: Address)` |
| `dlg_rev` | `revoke_delegate` succeeds | `(owner: Address, delegate: Address)` |

---

## mux-batcher events

Contract tag: `mux_bat`

| Action | Trigger | Data payload |
|---|---|---|
| `executed` | `execute_batch` completes (success or partial failure) | `(caller: Address, success_count: u32, failure_count: u32)` |
| `bat_ok` | `execute_batch` completes with zero failures | `(caller: Address, success_count: u32)` |

> `simulate_batch` does not emit events — it is a read-only preflight and writes no state.

---

## mux-wallet-registry events

Contract tag: `mux_wreg`

| Action | Trigger | Data payload |
|---|---|---|
| `init` | `initialize` succeeds | `owner: Address` |
| `wlt_reg` | `register_wallet` succeeds (new entry or overwrite) | `(name: Symbol, wallet: Address)` |

> `get_wallet` is read-only and emits no events.

---

## Querying events

Use the Soroban RPC `getEvents` endpoint, filtering by `contractId` and topic:

```ts
const events = await server.getEvents({
  startLedger: fromLedger,
  filters: [{
    type: "contract",
    contractIds: [CONTRACT_ID],
    topics: [["mux_acct"], ["dlg_set"]],  // [topics[0] filter, topics[1] filter]
  }],
});
```

---

## Security notes

- Events are **append-only** and cannot be modified or deleted after emission.
- Failed operations (those returning an error) do **not** emit events — only successful state changes are logged.
- The `debited` event records the spend amount but not the cumulative total; reconstruct running totals by summing `debited` events between `lmt_set` resets.
