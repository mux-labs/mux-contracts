# Mux Contracts

Soroban smart contracts for **Mux Protocol** — core logic for account abstraction, batching, and automation on Stellar.

## Overview
This repository contains the **core Soroban smart contracts** that power Mux. Contracts handle:
- Account abstraction logic
- Transaction batching
- Permissions and delegation
- Automated workflows for Stellar accounts

See [`docs/aa_sequence_diagram.md`](docs/aa_sequence_diagram.md) for the authoritative account-abstraction sequence diagram covering account creation, delegation, spending policy, recovery, and batcher flows.

## Contracts

| Contract | Description |
|---|---|
| [`contracts/mux-account`](contracts/mux-account/) | Account abstraction: owner, delegates, spend limits, guardian set |
| [`contracts/mux-account-factory`](contracts/mux-account-factory/) | Factory for deploying and registering account instances with metadata |
| [`contracts/mux-batcher`](contracts/mux-batcher/) | Atomic multi-operation batching with optional per-op failure handling |
| [`contracts/mux-delegation`](contracts/mux-delegation/) | Scoped delegate permission management — grant/revoke named permissions per (owner, delegate) pair |
| [`contracts/mux-permissions`](contracts/mux-permissions/) | RBAC registry — roles, permissions, grant/revoke |
| [`contracts/mux-policy`](contracts/mux-policy/) | Per-wallet daily spend-limit policy with automatic window reset |
| [`contracts/mux-recovery`](contracts/mux-recovery/) | Guardian-initiated account recovery with mandatory 24-hour timelock |
| [`contracts/mux-registry`](contracts/mux-registry/) | Contract version and metadata registry — tracks deployed crate names and versions |
| [`contracts/mux-spending-policy`](contracts/mux-spending-policy/) | Per-account/per-asset spend-limit policy and validation |
| [`contracts/mux-wallet-registry`](contracts/mux-wallet-registry/) | Named wallet address registry — register and look up wallet addresses by symbolic name |

## Registry vs Wallet-Registry Split

Mux ships **two distinct registries**. They are not interchangeable and must not be conflated — each has a different trust model, ownership, and set of invariants. See [`docs/registry-contracts-comparison.md`](docs/registry-contracts-comparison.md) for the full comparison.

| | `mux-registry` | `mux-wallet-registry` |
|---|---|---|
| **Purpose** | Contract version/metadata registry | Named wallet address registry |
| **Keyed by** | Deployed crate name | Symbolic wallet name |
| **Value** | Version + metadata | Wallet address |
| **Ownership** | Registry admin (owner) | Per-owner namespace |
| **Authz** | Owner/admin only for writes | Owner (or authorized delegate) for writes; reads are public |
| **Money path** | No — informational only | No — address lookup only; never authorizes spends |
| **Source of truth** | Contract metadata | Wallet address mapping |

### Invariants

- **`mux-registry`** is the single source of truth for *which contract version is deployed under a given crate name*. Writes are restricted to the registry owner/admin; clients cannot self-register versions. Reads are public and side-effect free.
- **`mux-wallet-registry`** maps a symbolic name to a wallet address **within an owner's namespace**. Writes require the owner (or an explicitly authorized delegate); a name cannot be silently reassigned by a non-owner. Reads are public.
- **Neither registry authorizes spends, recovery, or admin actions.** Spend authorization lives in `mux-spending-policy` / `mux-account`; recovery lives in `mux-recovery`. A registry entry is a lookup, never a capability.
- **Fail-closed:** unknown names/crates return a not-found error rather than a default address or version. Callers must treat a missing entry as a hard failure, not a fallback.

### Authz boundaries

| Surface | Owner | Delegate | Guardian | API-key / JWT |
|---|---|---|---|---|
| `mux-registry` write | ✅ | ❌ | ❌ | ❌ |
| `mux-registry` read | ✅ | ✅ | ✅ | ✅ |
| `mux-wallet-registry` write | ✅ | ✅ (if granted) | ❌ | ❌ |
| `mux-wallet-registry` read | ✅ | ✅ | ✅ | ✅ |

Clients cannot bypass policy: privileged writes are deny-by-default and require the owner (or an explicitly granted delegate for the wallet registry). API-key/JWT callers are read-only against both registries.

## WASM Size Budget & CI Artifacts

The CI pipeline ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) builds every contract to `wasm32-unknown-unknown` and enforces a **fail-closed WASM size budget**: if any compiled contract exceeds the configured limit, the build fails and the PR cannot merge.

- **Budget:** `MAX_WASM_SIZE_BYTES` (default `262144` bytes / 256 KiB) is defined in the `wasm-size-budget` job in `.github/workflows/ci.yml`.
- **Adjusting the budget:** edit `MAX_WASM_SIZE_BYTES` in that job. Raising it is a deliberate, reviewable change — keep it as small as the largest legitimate contract allows so accidental bloat is caught early.
- **Artifacts:** the built `.wasm` files are uploaded as the `wasm-artifacts` artifact on every CI run, so contributors and reviewers can download and inspect the exact binaries that were size-checked.

To reproduce the check locally:

```bash
cargo build --target wasm32-unknown-unknown --release --workspace
find target/wasm32-unknown-unknown/release -maxdepth 1 -name '*.wasm' -exec ls -l {} \;
```

## TypeScript Bindings

Pre-built clients for every contract live in [`bindings/`](bindings/).  
Install from npm:

```bash
npm install @mux-protocol/contracts
```

To regenerate bindings from local WASM (after editing contracts):

```bash
bash scripts/generate-bindings.sh
```

The CI pipeline ([`.github/workflows/bindings.yml`](.github/workflows/bindings.yml)) regenerates, type-checks, and tests bindings on every PR and publishes to npm on tagged releases.

### Usage example

See [`examples/bindings-usage.ts`](examples/bindings-usage.ts) for a working end-to-end example showing `check_spend` and `register_wallet`.
See [`examples/wallet-registry-invoke.ts`](examples/wallet-registry-invoke.ts) for a dedicated wallet registry invoke script.
See [`examples/authorize-flow.ts`](examples/authorize-flow.ts) for the owner → scoped session key → optional relayer → revocation authorization flow. The flow requires explicit environment variables and verifies that a revoked key is rejected.

The runtime call relationships between contracts are documented in [`docs/dependency_graph.md`](docs/dependency_graph.md); the address key set is maintained in [`CONTRACT_IDS.md`](CONTRACT_IDS.md) and [`config/addresses.json`](config/addresses.json).

```ts
import {
  MuxSpendingPolicyClient,
  MuxWalletRegistryClient,
  MuxAccountFactoryClient,
} from "@mux-protocol/contracts";

// Account factory example
const factoryClient = new MuxAccountFactoryClient({ contractId, networkPassphrase, rpcUrl });
await factoryClient.deployAccount(signer, owner, accountAddress);
await factoryClient.deployAccountWithMetadata(signer, owner, accountAddress, "1.0.0", "My account", "user");
const accounts = await factoryClient.getAccounts(owner);

// Spending policy example
const spendingClient = new MuxSpendingPolicyClient({ contractId, networkPassphrase, rpcUrl });
await spendingClient.checkSpend(signer, account, asset, 500n);

// Wallet registry example
const walletClient = new MuxWalletRegistryClient({ contractId, networkPassphrase, rpcUrl });
await walletClient.registerWallet(signer, "treasury", walletAddress);
const addr = await walletClient.getWallet(signer, "treasury");
```

## Account Abstraction (`mux-account`)

`mux-account` is the core Soroban smart account. It stores one owner, a
bounded delegate map, a guardian set, per-asset spend limits, and a session key
registry. All write entrypoints extend instance-storage TTL. The full interface
is specified in [`docs/mux-account-interface.md`](docs/mux-account-interface.md).

### Authorization

Every write entrypoint requires a specific authorization level — there is no
default-allow. Clients cannot bypass policy.

| Entrypoint | Required authorization |
|---|---|
| `initialize` | supplied `owner` |
| `pause` / `unpause` | stored owner |
| `set_delegate` / `remove_delegate` | stored owner |
| `set_spend_limit` / `debit_spend` | stored owner / contract address |
| `execute` | stored owner |
| `register_session_key` / `revoke_session_key` | stored owner |
| `execute_with_session` | authorized session key |
| `execute_with_session_sponsored` | allowlisted sponsor **and** authorized session key |
| `set_sponsor` / `is_sponsor` | stored owner (write); public (read) |
| `set_metadata` | stored owner |
| Read-only entrypoints | none |

### Session keys, scopes, and nonces

Session keys enable scoped, time-bounded delegation without exposing the owner's key.

- **`register_session_key(session_key, expires_at, scopes)`** — registers a session key
  with a Unix-timestamp expiry and a set of `Scope` capabilities (method names). New keys
  are capped at `MAX_SESSION_KEYS` (32) per owner.
- **`revoke_session_key(session_key)`** — marks a key revoked; revoked keys are rejected
  even before expiry.
- **`execute_with_session(session_key, target, function, args, nonce)`** — executes a
  contract call on behalf of the account using a session key. Fail-closed scope
  enforcement: a key with an empty `scopes` list returns `Unauthorized`; a `function`
  not listed in a non-empty `scopes` set returns `ScopeNotGranted`. The `nonce` must
  match `nonce()` exactly or the call is rejected with `InvalidNonce` — a rejected call
  does not burn the nonce. The reentrancy guard is held across the invocation.
- **`execute_with_session_sponsored(session_key, sponsor, target, function, args, nonce)`** —
  the gas-abstracted variant: the relayer submits and pays fees, and both the sponsor and
  the session key must authorize. The sponsor must be on the owner-managed allowlist or the
  call is rejected with `SponsorNotAuthorized` before any session state is read. Sponsorship
  never widens a session key's scopes. See [`docs/relayer-integration.md`](docs/relayer-integration.md).

### Key invariants

- **Fail-closed scope enforcement:** empty scope list → `Unauthorized`; unlisted function
  → `ScopeNotGranted`.
- **Nonce integrity:** `nonce` must equal `nonce()` or the call is rejected with
  `InvalidNonce`; a rejected call does not increment the nonce.
- **Reentrancy guard:** the guard is held across all external invocations; callbacks into
  `execute`, `debit_spend`, or `execute_with_session` are rejected.
- **Checks-effects-interactions on spend:** the debit is persisted only after the external
  call returns successfully.
- **`expires_at` is a Unix timestamp (`u64`)**, not a ledger sequence. See
  [`docs/mux-account-interface.md`](docs/mux-account-interface.md) and
  `tests/expiry_naming.rs` for the enforced naming invariant.

### Error codes

| Code | Variant | Meaning |
|---:|---|---|
| 1 | `NotInitialized` | Required account state is absent |
| 2 | `AlreadyInitialized` | Initialization was already completed |
| 3 | `Unauthorized` | Contract state disallows the call |
| 4 | `DelegateNotFound` | Delegate is absent |
| 5 | `DelegateExpired` | Delegate is no longer active |
| 6 | `SpendLimitExceeded` | Limit is absent or would be exceeded |
| 7 | `InvalidAmount` | Amount is not positive |
| 8 | `InvalidPeriod` | Reset period is zero |
| 9 | `TooManyDelegates` | Delegate cap is reached |
| 10 | `ReentrancyDetected` | Spend accounting is already executing |
| 11 | `ArithmeticOverflow` | Spend addition overflowed |
| 12 | `TooManySessionKeys` | Session-key cap is reached |
| 13 | `ScopeNotGranted` | Invoked method is not in the session key's scopes |
| 14 | `SponsorNotAuthorized` | Relayer is not on the sponsor allowlist |
| 15 | `InvalidNonce` | Supplied nonce is not the account's current nonce |

### TypeScript example

```ts
import { MuxAccountClient } from "@mux-protocol/contracts";

const accountClient = new MuxAccountClient({ contractId, networkPassphrase, rpcUrl });

// Register a session key scoped to a single method
await accountClient.registerSessionKey(owner, sessionPublicKey, expiresAt, ["transfer"]);

// Execute via session key (nonce is consumed on success only)
const currentNonce = await accountClient.nonce(owner);
await accountClient.executeWithSession(sessionKeypair, sessionPublicKey, target, "transfer", args, currentNonce);

// Revoke a session key immediately
await accountClient.revokeSessionKey(owner, sessionPublicKey);
```

See [`examples/authorize-flow.ts`](examples/authorize-flow.ts) for the full
owner → scoped session key → optional relayer → revocation flow, and
[`docs/mux-account-interface.md`](docs/mux-account-interface.md) for the
complete entrypoint reference.

## Tech Stack
- Soroban smart contracts (Rust)
- Stellar Soroban SDK v21
- TypeScript SDK bindings (`@stellar/stellar-sdk`)
- Docker & Docker Compose for local Soroban development
- GitHub Actions CI

## Getting Started

```bash
git clone https://github.com/mux-labs/mux-contracts.git
cd mux-contracts

# Build all contracts
cargo build --target wasm32-unknown-unknown --release --workspace

# Run unit tests
cargo test --workspace --all-features

# Generate TypeScript bindings
bash scripts/generate-bindings.sh

# Build TypeScript package
cd bindings && npm ci && npm run build
```

## Deploying Contracts

Copy the deployment environment template and fill in your values before running any deploy script:

```bash
cp .env.deploy.example .env.deploy
# edit .env.deploy with your network, keypair, and RPC endpoint
source .env.deploy && bash scripts/generate-bindings.sh
```

See [`.env.deploy.example`](.env.deploy.example) for the full list of required and optional variables.

## Integration Tests

Integration tests connect to a live Soroban RPC endpoint (localnet, testnet, or mainnet) and verify contract deployment.

**Run integration tests:**

```bash
cd bindings

# Against localnet (requires docker-compose to be running)
SOROBAN_NETWORK=localnet npm test

# Against testnet
SOROBAN_NETWORK=testnet npm test

# Tests gracefully skip if the network is unavailable
npm test
```

**Configuration:**

Network endpoints are configured in `bindings/src/network.ts` via environment variables:
- `SOROBAN_NETWORK` - Which network to use (default: `localnet`)
- `LOCALNET_RPC_URL` - RPC endpoint for localnet (default: `http://localhost:8000`)
- `LOCALNET_NETWORK_PASSPHRASE` - Network ID for localnet
- `LOCALNET_MUX_*_ID` - Contract addresses on localnet

**Setting up localnet locally:**

See [docker-compose.yml](docker-compose.yml) for spinning up a local Stellar/Soroban node.

## Contract Address Configuration

Contract addresses are managed per network via `config/addresses.json` and environment variables.

**Configuration structure:**

```json
{
  "localnet": {
    "muxAccount": "CADDRESS...",
    "muxBatcher": "CADDRESS...",
    "muxPermissions": "CADDRESS..."
  },
  "testnet": { ... },
  "mainnet": { ... }
}
```

**Using contract addresses in your application:**

```typescript
import { getNetworkConfig } from "@mux-protocol/contracts";

// Get active network from SOROBAN_NETWORK env var (default: localnet)
const config = getNetworkConfig();
console.log(config.contracts.muxAccount);  // Contract address
console.log(config.rpcUrl);                // RPC endpoint
```

**Environment variable overrides:**

Override addresses per network using environment variables:

```bash
SOROBAN_NETWORK=testnet
TESTNET_MUX_ACCOUNT_ID=CADDRESS...
TESTNET_MUX_BATCHER_ID=CADDRESS...
TESTNET_MUX_PERMISSIONS_ID=CADDRESS...
```

The pattern is `{NETWORK}_MUX_*_ID`. Environment variables take precedence over `config/addresses.json`.

**Validating addresses at startup:**

```typescript
import { getValidatedAddresses, DEFAULT_ADDRESSES } from "@mux-protocol/contracts";

// Fails fast if any required addresses are missing for the active network
const addresses = getValidatedAddresses("testnet", DEFAULT_ADDRESSES);
```

## Error Handling

Contract errors are mapped to HTTP status codes for API/gateway implementations.

**Using error mapping in your API:**

```typescript
import {
  contractErrorToHttp,
  ERROR_HTTP_MAP,
  type HttpErrorResponse,
} from "@mux-protocol/contracts";

// Convert a contract error to HTTP response
const httpError: HttpErrorResponse = contractErrorToHttp("Unauthorized");
// { statusCode: 401, message: "Unauthorized", errorType: "Unauthorized" }

// Use in Express middleware example:
async function handleContractCall(req, res) {
  try {
    const result = await muxAccount.transfer(/*...*/);
    res.json(result);
  } catch (error) {
    const httpError = contractErrorToHttp(String(error));
    res.status(httpError.statusCode).json({
      error: httpError.errorType,
      message: httpError.message,
    });
  }
}
```

**Status code mappings:**

- **401 Unauthorized** — `Unauthorized`, `Expired`
- **404 Not Found** — `*NotFound`, `*NotInRole`, `*NotInitialized` (when expected to exist)
- **400 Bad Request** — Invalid input, validation failures, constraint violations
- **409 Conflict** — `AlreadyInitial

## Local Soroban Development

### Using Docker Compose

[`docker-compose.yml`](docker-compose.yml) starts the official `stellar/quickstart` image with Soroban RPC and Horizon for local development.

```bash
docker compose up -d
```

Supported options:
- `--network <network>` — `localnet|testnet|mainnet` (default: `localnet`)
- `--contract-id <id>` or `--contract-name <name>` — contract to call
- `--function <name>` — contract function to invoke
- `--secret-key <secret>` — signer secret key for the transaction
- `--arg <value>` — argument values; repeatable
- `--simulate-only` — simulate without submitting

If dependencies are not installed, run:

```bash
cd bindings && npm ci
```

**Deploying Contracts to Localnet:**

After starting the localnet, build and deploy contracts:
```bash
# Build contracts
cargo build --target wasm32-unknown-unknown --release --workspace

# Use Stellar CLI to deploy (requires `stellar` CLI installed)
stellar contract deploy --wasm target/wasm32-unknown-unknown/release/mux_account.wasm
# ... repeat for other contracts and save the contract IDs to .env.localnet
```

## Documentation

- [Contract IDs](CONTRACT_IDS.md) — Per-network program addresses, update process, and upgrade authority

## Documentation (Extended)

- [Architecture Overview](docs/architecture-overview.md) — High-level diagram and system components
- [Policy Semantics](docs/policy-semantics.md) — Per-wallet daily spend limit design, reset logic, and error codes
- [Account Abstraction Design](docs/account-abstraction.md) — Goals, architecture, session key design, and transaction flows
- [Backend Orchestrator Integration](docs/aa-backend-orchestrator.md) — Scope and architecture for relayer integration
- [Threat Model](docs/threat-model.md) — assets, trust boundaries, and mitigations
- [Access Control Review Checklist](docs/access-control-checklist.md) — pre-deployment and pre-audit checklist
- [Storage Griefing Notes](docs/storage-griefing.md) — collection caps, TTL management, keeper runbook
- [External Audit Prep](docs/audit-prep.md) — scope, entry points, known limitations, auditor checklist
- [Rollback Deploy Notes](docs/rollback-deploy.md) — Rollback strategies and operational procedures
- [Security Policy](SECURITY.md) — Overall security guidelines, rollback security, and incident response

This exposes:
- Soroban RPC on `http://localhost:8000`
- Horizon on `http://localhost:8001`

Stop the stack with `docker compose down`.

## Security

See [`SECURITY.md`](SECURITY.md) for the threat model, secret-handling rules, and the registry authz boundaries summarized above. The registry split (version/metadata vs named wallet addresses) is documented in [`docs/registry-contracts-comparison.md`](docs/registry-contracts-comparison.md); keep both in sync when either registry changes.

## License

Apache-2.0
