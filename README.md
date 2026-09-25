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

This exposes:
- Soroban RPC on `http://localhost:8000`
- Horizon on `http://localhost:8001`

Stop the stack with `docker compose down`.

## Security

See [`SECURITY.md`](SECURITY.md) for the threat model, secret-handling rules, and the registry authz boundaries summarized above. The registry split (version/metadata vs named wallet addresses) is documented in [`docs/registry-contracts-comparison.md`](docs/registry-contracts-comparison.md); keep both in sync when either registry changes.

## License

Apache-2.0
