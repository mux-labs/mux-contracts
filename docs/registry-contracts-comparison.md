# Registry Contracts: mux-registry vs mux-wallet-registry

**Status:** Reference  
**Related:** [Delegation Permission Model](delegation-permission-model.md), [Error Codes](error_codes.md), [Bindings Error Mapping](bindings-error-mapping.md), [Recovery Trust Model](recovery-trust-model.md), [SECURITY](../SECURITY.md)

---

## Overview

The Mux Protocol workspace contains two "registry" contracts. They have similar
names but serve entirely different purposes. This document clarifies the
distinction to prevent confusion during integration, auditing, and deployment.

| | `mux-registry` | `mux-wallet-registry` |
|---|---|---|
| **Purpose** | Protocol component version registry | Named wallet address lookup |
| **What it stores** | Crate name → version/metadata | Symbolic name → wallet `Address` |
| **Key type** | `Symbol` (crate name) → `String` (semver) | `Symbol` (label) → `Address` |
| **Admin model** | Stored `admin` — `initialize(admin)` required | Stored `owner` — `initialize(owner)` required |
| **Auth on writes** | Admin auth (`require_admin`) | Owner auth (`require_owner`) |
| **Cap** | 128 contracts (`MAX_CONTRACTS`) | 128 wallets (`MAX_WALLETS`) |
| **Error enum** | `MuxRegistryError` | `WalletRegistryError` |
| **Error codes** | 1–5 | 1–5 |
| **Contract tag** | `mux_reg` | `mux_wreg` |
| **Metadata support** | Yes: `version`, `description`, `author`, `repository` | Yes: `label`, `description` |
| **WASM** | Compiles to `mux_registry.wasm` | Compiles to `mux_wallet_registry.wasm` |

---

## Registry split: responsibilities, ownership, and invariants

The two registries are **independent surfaces** with no cross-contract calls.
The split is deliberate: `mux-registry` is protocol infrastructure, while
`mux-wallet-registry` is an application-layer lookup. Keeping them separate
means a compromise or misconfiguration of one cannot affect the other's
invariants.

### `mux-registry` — protocol component version registry

- **Responsibility:** record which version of each Mux component is deployed,
  for tooling, indexers, and upgrade pipelines. Not part of end-user flows.
- **Ownership:** a single stored `admin`, set once at `initialize(admin)`.
  The admin is the sole write authority; reads are public.
- **Invariants:**
  - `initialize` may be called at most once (`AlreadyInitialized` otherwise).
  - Every state-mutating entrypoint requires the stored admin's auth
    (`Unauthorized` otherwise).
  - The `Names` index is capped at `MAX_CONTRACTS` (128); exceeding it fails
    with `TooManyContracts` and leaves state unchanged.
  - `check_version` is a dry-run and never mutates state.

### `mux-wallet-registry` — named wallet address lookup

- **Responsibility:** resolve a human-readable label (e.g. `"treasury"`,
  `"hot-wallet"`) to a Stellar `Address` for end-user and integrator flows.
- **Ownership:** a single stored `owner`, set once at `initialize(owner)`.
  The owner is the sole write authority; reads are public.
- **Invariants:**
  - `initialize` may be called at most once (`AlreadyInitialized` otherwise).
  - Every state-mutating entrypoint requires the stored owner's auth
    (`Unauthorized` otherwise).
  - The `WalletNames` index is capped at `MAX_WALLETS` (128); exceeding it
    fails with `TooManyWallets` and leaves state unchanged.
  - A label resolves to exactly one `Address`; re-registering a label
    overwrites the previous address atomically.

### Authz boundaries (deny-by-default)

Neither registry exposes a delegate, guardian, or API-key surface. Write
access is limited to the single stored authority, and every privileged
entrypoint is deny-by-default: an uninitialized contract rejects writes with
`NotInitialized`, and a caller that is not the stored authority is rejected
with `Unauthorized`. Clients cannot bypass policy by supplying a different
signer — the contract compares against the stored address and calls
`require_auth()` on it. Delegate/guardian/API-key roles belong to other
contracts (see [Delegation Permission Model](delegation-permission-model.md)
and [Recovery Trust Model](recovery-trust-model.md)) and must not be assumed
here.

---

## mux-registry

**Crate path:** `contracts/mux-registry`  
**Purpose:** Tracks deployed Mux Protocol component versions for tooling, indexers,
and upgrade pipelines.

### What it tracks

Each entry maps a symbolic crate name to a version string and optional full
metadata:

```
"mux-account"        →  "1.0.0"
"mux-batcher"        →  "1.2.0"  +  {description, author, repository}
```

### Who writes

Only the stored **admin** (set once via `initialize(admin)`) may register or
update entries. Reads are public.

### Entrypoints

| Entrypoint | Auth | Description |
|---|---|---|
| `initialize(admin)` | A | One-time setup |
| `register(name, version)` | A | Register or update a version string |
| `register_with_metadata(name, version, desc, author, repo)` | A | Register with full metadata |
| `check_version(name)` | P | Dry-run; no state mutation |
| `get_version(name)` | P | Read registered version |
| `get_metadata(name)` | P | Read full metadata |
| `list_contracts()` | P | List all registered names |

### Error variants (`MuxRegistryError`)

| Code | Variant | HTTP |
|------|---------|------|
| 1 | `NotInitialized` | 500 |
| 2 | `AlreadyInitialized` | 409 |
| 3 | `Unauthorized` | 401 |
| 4 | `ContractNotFound` | 404 |
| 5 | `TooManyContracts` | 409 |

---

## mux-wallet-registry

**Crate path:** `contracts/mux-wallet-registry`  
**Purpose:** Provides a human-readable name → `Address` lookup for wallet addresses.
Useful for applications that need to reference wallets by label (e.g. "treasury",
"hot-wallet") rather than raw Stellar addresses.

### What it tracks

Each entry maps a symbolic label to a Stellar `Address`:

```
"treasury"    →  G... (Stellar address)
"hot-wallet"  →  G...
```

Optional `WalletMetadata` (label + description) can be stored alongside the address.

### Who writes

Only the stored **owner** (set once via `initialize(owner)`) may register or
update entries. Reads are public.

### Entrypoints

| Entrypoint | Auth | Description |
|---|---|---|
| `initialize(owner)` | U | One-time setup; owner authorizes |
| `register_wallet(name, wallet)` | U | Register or overwrite a wallet address |
| `register_wallet_with_metadata(name, wallet, label, desc)` | U | Register with label and description |
| `get_wallet(name)` | P | Read wallet address by name |
| `get_metadata(name)` | P | Read metadata for a named wallet |
| `list_wallets()` | P | List all registered wallet names |

### Error variants (`WalletRegistryError`)

| Code | Variant | HTTP |
|------|---------|------|
| 1 | `NotInitialized` | 500 |
| 2 | `AlreadyInitialized` | 409 |
| 3 | `Unauthorized` | 401 |
| 4 | `WalletNotFound` | 404 |
| 5 | `TooManyWallets` | 409 |

---

## Key differences

### Purpose

`mux-registry` is an **infrastructure component** — it is written to by the
protocol deployer/operator and read by tooling that needs to know which version
of each contract is deployed. It is not typically called by end-user flows.

`mux-wallet-registry` is an **application-layer component** — it is written to
by a wallet owner and read by any caller that needs to resolve a named wallet
address. It is intended for end-user and integrator flows.

### Error code overlap

Both contracts use codes 1–5, but the variant names differ:

| Code | `MuxRegistryError` | `WalletRegistryError` |
|------|---------------------|----------------------|
| 1 | `NotInitialized` | `NotInitialized` |
| 2 | `AlreadyInitialized` | `AlreadyInitialized` |
| 3 | `Unauthorized` | `Unauthorized` |
| 4 | `ContractNotFound` | `WalletNotFound` |
| 5 | `TooManyContracts` | `TooManyWallets` |

The overlapping names (`NotInitialized`, `AlreadyInitialized`, `Unauthorized`)
map to the same HTTP status codes in `ERROR_HTTP_MAP`, which is intentional —
see [bindings-error-mapping.md](bindings-error-mapping.md#cross-contract-error-overlap).
The diverging names (`ContractNotFound` vs `WalletNotFound`, `TooManyContracts`
vs `TooManyWallets`) must each appear in `ERROR_HTTP_MAP` separately.

### Auth model terminology

`mux-registry` uses the term **admin** for its write authority (consistent with
other admin-gated contracts like `mux-policy` and `mux-batcher`).

`mux-wallet-registry` uses the term **owner** for its write authority
(consistent with `mux-account` and `mux-recovery`).

Despite different terminology, both patterns follow the same Soroban auth model:
the address is stored at `initialize` time and `require_auth()` is called on
every state-mutating entrypoint.

### Storage layout

Both contracts store a `Names` / `WalletNames` `Vec<Symbol>` in instance
storage as an index for enumeration. Both cap the index at 128 entries to bound
instance storage growth.

`mux-registry` additionally stores per-entry metadata in instance storage
(via `DataKey::Metadata(name)`). `mux-wallet-registry` stores per-entry
metadata in instance storage (via `DataKey::Metadata(name)`) and the wallet
address itself in `DataKey::Wallet(name)`.

---

## When to deploy both

These contracts are independent and serve different audiences:

- Deploy **one `mux-registry` instance** per protocol environment
  (localnet / testnet / mainnet) to track deployed contract versions.
- Deploy **one or more `mux-wallet-registry` instances** per application or
  namespace that needs symbolic wallet address lookup.

There is no cross-contract dependency between them — neither calls the other.
Because the split is enforced at the contract boundary, a failure or
misconfiguration of one registry cannot alter the other's state or authz.

---

## TypeScript bindings

Both contracts have generated TypeScript clients:

```ts
import { MuxRegistryClient }       from "@mux-protocol/contracts";
import { MuxWalletRegistryClient } from "@mux-protocol/contracts";
```

Error types are defined separately:

```ts
import type { MuxRegistryError }       from "@mux-protocol/contracts";
import type { MuxWalletRegistryError } from "@mux-protocol/contracts";
```

Helper functions for error messages are available in `bindings/src/types.ts`:

```ts
import { muxRegistryErrorMessage } from "@mux-protocol/contracts";
// muxRegistryErrorMessage("ContractNotFound") → "no contract registered under the given name"
```

See [bindings-error-mapping.md](bindings-error-mapping.md) for the full
error mapping pipeline.
