# Account Factory Flow

**Contract:** `mux-account-factory`  
**Source:** `contracts/mux-account-factory/src/lib.rs`  
**TypeScript client:** `bindings/src/generated/mux-account-factory.ts`  
**Event helpers:** `bindings/src/factory-events.ts`  
**Related:** [ABI Reference](abi_reference.md) · [Audit Events](audit-events.md) · [Storage Griefing](storage-griefing.md) · [Error Codes](error_codes.md)

---

## Overview

`mux-account-factory` is the on-chain registry for `mux-account` instances. It does two things:

1. **Registration** — records that a given owner has deployed a specific account contract address, and increments a global counter so the total fleet size is always auditable.
2. **Discovery** — lets any caller enumerate all accounts for a given owner and retrieve optional structured metadata (version, description, author) attached to each one.

The factory does **not** deploy `mux-account` WASM; that is done off-chain via the Stellar CLI (`stellar contract deploy`). The factory is the ledger-native index that links owners to their deployed instances.

---

## Full Lifecycle

```
┌──────────────┐     1. stellar contract deploy mux_account.wasm
│  Deployer    │ ──────────────────────────────────────────────────► WASM uploaded
│  (CLI / CI)  │     2. stellar contract deploy --wasm-hash ...   ► account_address returned
└──────┬───────┘
       │
       │ 3. deploy_account(owner, account_address)            owner.require_auth()
       │    OR deploy_account_with_metadata(owner, ...)
       ▼
┌──────────────────────────────┐
│   mux-account-factory        │  Validates:
│   (this contract)            │   • owner ≠ account_address  → InvalidAccount
│                              │   • accounts.len() < 64      → TooManyAccounts
│   Storage writes:            │   • metadata size limits     → MetadataTooLarge
│   Accounts(owner) += addr    │
│   AccountCount    += 1       │  Events emitted:
│   Metadata(o, a)  = meta     │   • deployed(owner, account_address)
│   extend_ttl()               │   • meta_set(owner, account_address, version)
└──────────────────────────────┘       (meta_set only on the _with_metadata path)
       │
       │ 4. get_accounts(owner) / get_account_metadata(owner, addr)
       │    account_count()     — any caller, no auth required
       ▼
┌──────────────┐
│  Indexer /   │  Reads the per-owner account list and optional metadata
│  TypeScript  │  for display, routing, or further on-chain calls
└──────────────┘
```

---

## Entrypoints

### `deploy_account(owner, account_address) → Result<Address>`

Register a new account for `owner`. The owner must sign the transaction.

| Parameter | Type | Constraints |
|---|---|---|
| `owner` | `Address` | Must authorize the call (`require_auth`) |
| `account_address` | `Address` | Must differ from `owner`; appended to Accounts vec |

**Returns:** `account_address` on success.

**Errors:**
- `InvalidAccount (2)` — `account_address == owner`
- `TooManyAccounts (3)` — owner's Accounts vec already has 64 entries
- Auth host error — `owner.require_auth()` failed

**Events emitted:** `deployed(owner, account_address)`

---

### `deploy_account_with_metadata(owner, account_address, version, description, author) → Result<Address>`

Register a new account and attach structured metadata. Identical to `deploy_account` plus metadata validation and a second event.

| Parameter | Type | Max length |
|---|---|---|
| `version` | `String` | 32 bytes (`MAX_VERSION_LENGTH`) |
| `description` | `String` | 256 bytes (`MAX_DESCRIPTION_LENGTH`) |
| `author` | `String` | 64 bytes (`MAX_AUTHOR_LENGTH`) |

**Additional errors:**
- `MetadataTooLarge (5)` — any metadata field exceeds its limit

**Events emitted (in order):**
1. `deployed(owner, account_address)`
2. `meta_set(owner, account_address, version)`

---

### `simulate_deploy(owner, account_address) → Result<Address>`

Preflight / dry-run of `deploy_account`. **No state is written and no events are emitted.** Returns the same result that the real deploy would, including `TooManyAccounts` when the owner is at the cap.

Use this before submitting a transaction to avoid a failed on-chain call.

---

### `simulate_deploy_with_metadata(owner, account_address, version, description, author) → Result<Address>`

Preflight / dry-run of `deploy_account_with_metadata`. Enforces the same cap and all three metadata size limits through the same validator used by the state-changing deploy path. No state written, no events.

---

### `get_accounts(owner) → Vec<Address>`

Return all account addresses registered for `owner`. Returns an empty vec for owners with no accounts — never errors. No auth required.

---

### `get_account_metadata(owner, account_address) → Result<AccountMetadata>`

Return stored metadata for a specific account. Errors with `MetadataNotFound (4)` if no metadata was stored (i.e., the account was registered via `deploy_account` rather than `deploy_account_with_metadata`).

---

### `account_count() → u64`

Return the global total of registered accounts across all owners. No auth required. Does not extend TTL.

---

### `max_accounts_per_owner() → u32`

Return the compile-time constant `MAX_ACCOUNTS_PER_OWNER` (currently `64`). Query this before deploying to know the cap without hardcoding it in client code.

---

## Preflight Pattern

Always simulate before submitting to avoid burning fees on a predictably-failing transaction:

```ts
import {
  MuxAccountFactoryClient,
  muxAccountFactoryErrorMessage,
} from "@mux-protocol/contracts";

async function safeDeployAccount(
  client: MuxAccountFactoryClient,
  signer: Keypair,
  owner: Address,
  accountAddress: Address,
): Promise<Address | null> {
  // 1. Check the cap before doing anything.
  const cap = await client.maxAccountsPerOwner(signer);
  const existing = await client.getAccounts(signer, owner);
  if (existing.length >= cap) {
    console.error(`Owner has reached the ${cap}-account cap.`);
    return null;
  }

  // 2. Dry-run to catch any other validation errors.
  try {
    await client.simulateDeploy(signer, owner, accountAddress);
  } catch (err) {
    console.error("Simulation failed:", err);
    return null;
  }

  // 3. Submit.
  return client.deployAccount(signer, owner, accountAddress);
}
```

---

## Storage Design

All factory state lives in **instance storage** — the cheapest billing unit on Soroban, shared across all callers of this contract instance.

| Key | Type | Description | Cap |
|---|---|---|---|
| `Accounts(owner: Address)` | `Vec<Address>` | Per-owner list of registered account addresses | 64 per owner |
| `AccountCount` | `u64` | Global counter across all owners | unbounded (integer) |
| `Metadata(owner, account_address)` | `AccountMetadata` | Optional metadata per registered account | — |

### Why instance storage?

The factory is a **singleton utility contract**. All data is shared across callers; there are no per-entity records with independent TTL requirements. Instance storage is the right choice because:

- All reads/writes share a single rent unit.
- The 64-account-per-owner cap keeps each `Accounts` vec small (≤ 64 × 32 bytes = ~2 KB worst case).
- Metadata strings are individually bounded (32 + 256 + 64 bytes max per account).

See [docs/storage-choices.md](storage-choices.md) for the full rationale.

### TTL management

Every write path calls:

```rust
env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
```

| Constant | Ledgers | Duration |
|---|---|---|
| `TTL_THRESHOLD` | 17,280 | ~1 day — extends when remaining TTL drops below this |
| `TTL_EXTEND_TO` | 518,400 | ~30 days — extended to this value |

Read-only calls (`get_accounts`, `account_count`, `get_account_metadata`, `simulate_deploy*`, `max_accounts_per_owner`) **do not** extend TTL. If the factory is idle for more than 30 days, a keeper must extend TTL externally:

```bash
stellar contract extend \
  --id "$FACTORY_CONTRACT_ID" \
  --ledgers-to-extend 518400 \
  --source "$KEEPER_SECRET" \
  --network mainnet
```

See [docs/storage-griefing.md](storage-griefing.md#deployment-runbook--ttl-keeper) for the full keeper runbook.

---

## Storage Griefing Mitigations

The factory enforces three independent bounds to prevent malicious owners from bloating instance storage:

| Attack surface | Mitigation | Constant | Error |
|---|---|---|---|
| Owner fills Accounts vec indefinitely | Cap at 64 accounts per owner | `MAX_ACCOUNTS_PER_OWNER = 64` | `TooManyAccounts` |
| Owner uploads arbitrarily long `version` string | Length check before write | `MAX_VERSION_LENGTH = 32` | `MetadataTooLarge` |
| Owner uploads arbitrarily long `description` | Length check before write | `MAX_DESCRIPTION_LENGTH = 256` | `MetadataTooLarge` |
| Owner uploads arbitrarily long `author` | Length check before write | `MAX_AUTHOR_LENGTH = 64` | `MetadataTooLarge` |

All four checks are enforced on **both** the real deploy paths and the simulate paths so that dry-run and live behavior are identical.

The cap is **per-owner**, not global. One owner reaching their cap does not affect any other owner.

---

## Events

Contract tag: `mux_fac` (topics[0])

| Action | topics[1] | Data | Trigger |
|---|---|---|---|
| `deployed` | `"deployed"` | `(owner: Address, account_address: Address)` | Every successful `deploy_account` or `deploy_account_with_metadata` |
| `meta_set` | `"meta_set"` | `(owner: Address, account_address: Address, version: String)` | Every successful `deploy_account_with_metadata` only |

Within a single `deploy_account_with_metadata` call, `deployed` is always emitted first, `meta_set` second.

**Zero-event paths** — the following never emit events:
`get_accounts`, `account_count`, `get_account_metadata`, `simulate_deploy`, `simulate_deploy_with_metadata`, `max_accounts_per_owner`, and all error/auth-failure paths.

### Filtering events from TypeScript

```ts
import {
  FACTORY_CONTRACT_TAG,
  FACTORY_EVENT_TOPICS,
  parseFactoryEvent,
  type FactoryEvent,
} from "@mux-protocol/contracts";

const rawEvents = await server.getEvents({
  startLedger,
  filters: [{
    type: "contract",
    contractIds: [FACTORY_CONTRACT_ID],
    topics: [[FACTORY_CONTRACT_TAG]],
  }],
});

const events: FactoryEvent[] = rawEvents.records
  .map(parseFactoryEvent)
  .filter((e): e is FactoryEvent => e !== null);

const deploys = events.filter(e => e.action === "deployed");
const metaUpdates = events.filter(e => e.action === "meta_set");
```

---

## Error Reference

| Variant | Code | HTTP | When |
|---|---|---|---|
| `Unauthorized` | 1 | 401 | `owner.require_auth()` failed |
| `InvalidAccount` | 2 | 400 | `account_address == owner` |
| `TooManyAccounts` | 3 | 409 | Owner's Accounts vec is at 64 entries |
| `MetadataNotFound` | 4 | 404 | `get_account_metadata` called for an account with no stored metadata |
| `MetadataTooLarge` | 5 | 400 | Any metadata field exceeds its size limit |

```ts
import { contractErrorToHttp, muxAccountFactoryErrorMessage } from "@mux-protocol/contracts";

try {
  await client.deployAccount(signer, owner, accountAddress);
} catch (err) {
  const errorType = String(err);
  const { statusCode } = contractErrorToHttp(errorType);
  const message = muxAccountFactoryErrorMessage(errorType as any);
  console.error(`[${statusCode}] ${message}`);
  // e.g. "[409] owner has reached the 64-account cap"
}
```

---

## TypeScript Usage

### Installation

```bash
npm install @mux-protocol/contracts
```

### Minimal deploy

```ts
import { Address, Keypair, Networks } from "@stellar/stellar-sdk";
import { MuxAccountFactoryClient } from "@mux-protocol/contracts";

const client = new MuxAccountFactoryClient({
  contractId: "C...",
  networkPassphrase: Networks.TESTNET,
  rpcUrl: "https://soroban-testnet.stellar.org",
});

const signer = Keypair.fromSecret(process.env.SECRET_KEY!);
const owner = Address.fromString(signer.publicKey());
const accountAddress = Address.fromString("C..."); // deployed mux-account address

const registered = await client.deployAccount(signer, owner, accountAddress);
console.log("Registered:", registered.toString());
```

### Deploy with metadata

```ts
const registered = await client.deployAccountWithMetadata(
  signer,
  owner,
  accountAddress,
  "1.0.0",          // version   (≤32 chars)
  "My smart wallet", // description (≤256 chars)
  "acme-corp",       // author    (≤64 chars)
);
```

### Preflight before deploy

```ts
// Query the cap constant from the contract (no hardcoding needed).
const cap = await client.maxAccountsPerOwner(signer); // 64

// Check how many the owner has already.
const current = await client.getAccounts(signer, owner);
if (current.length >= cap) {
  throw new Error(`Account cap reached (${cap})`);
}

// Dry-run to validate inputs without spending fees.
const simulated = await client.simulateDeploy(signer, owner, accountAddress);
// simulated === accountAddress — the address that would be registered.

// Now submit.
await client.deployAccount(signer, owner, accountAddress);
```

### Read registered accounts

```ts
const accounts = await client.getAccounts(signer, owner);
for (const addr of accounts) {
  console.log(addr.toString());
  try {
    const meta = await client.getAccountMetadata(signer, owner, addr);
    console.log(" ", meta.version, "-", meta.description);
  } catch {
    console.log("  (no metadata)");
  }
}

const total = await client.accountCount(signer);
console.log(`Total registered globally: ${total}`);
```

---

## Multi-owner Isolation

Each owner's account list is keyed independently — `DataKey::Accounts(owner)`. Filling one owner's 64-account quota has no effect on any other owner's ability to deploy.

The global `AccountCount` counter increments for every successful deploy regardless of owner, so fleet-wide accounting is always accurate.

---

## Duplicate Registration and Idempotency

The factory handles duplicate registrations idempotently: calling `deploy_account` or `deploy_account_with_metadata` with an `account_address` that is already registered for that `owner` succeeds without inserting duplicate entries into `get_accounts` or incrementing the global `account_count()`.

Metadata attached to an already-registered account can be updated idempotently via `deploy_account_with_metadata`. Furthermore, re-registering an existing account succeeds even if the owner's account list is currently at the 64-account cap (`MAX_ACCOUNTS_PER_OWNER`).

---

## No-Op Paths

| Entrypoint | Writes state? | Emits events? | Extends TTL? |
|---|---|---|---|
| `deploy_account` | ✅ yes | ✅ `deployed` | ✅ yes |
| `deploy_account_with_metadata` | ✅ yes | ✅ `deployed` + `meta_set` | ✅ yes |
| `simulate_deploy` | ❌ no | ❌ no | ❌ no |
| `simulate_deploy_with_metadata` | ❌ no | ❌ no | ❌ no |
| `get_accounts` | ❌ no | ❌ no | ❌ no |
| `account_count` | ❌ no | ❌ no | ❌ no |
| `get_account_metadata` | ❌ no | ❌ no | ❌ no |
| `max_accounts_per_owner` | ❌ no | ❌ no | ❌ no |

---

## Integration with mux-account

The factory is intentionally decoupled from `mux-account` at the contract level — it stores an address but does not call into the account contract. This means:

- The factory can register any address (the contract does not verify that `account_address` is a deployed `mux-account`). Callers are responsible for passing a correctly deployed account address.
- A `mux-account` instance can be deployed and initialized independently of the factory, then registered afterwards.
- Deregistration is not supported — once an address is in the Accounts vec, it stays. If an account is upgraded or migrated, register the new address as an additional entry.

---

## See Also

- [ABI Reference — mux-account-factory](abi_reference.md#mux-account-factory)
- [Audit Events — mux-account-factory](audit-events.md#mux-account-factory-events)
- [Error Codes](error_codes.md#mux-account-factory-contractsmux-account-factory)
- [Storage Griefing](storage-griefing.md)
- [Entrypoint Matrix](entrypoint-matrix.md#mux-account-factory)
- [Access Control Checklist §6a](access-control-checklist.md#6a-storage-griefing-caps)
- [examples/account-factory-usage.ts](../examples/account-factory-usage.ts)
