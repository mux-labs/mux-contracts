# Mux Contracts

Soroban smart contracts for **Mux Protocol** — core logic for account abstraction, batching, and automation on Stellar.

## Overview
This repository contains the **core Soroban smart contracts** that power Mux. Contracts handle:
- Account abstraction logic
- Transaction batching
- Permissions and delegation
- Automated workflows for Stellar accounts

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
- **409 Conflict** — `AlreadyInitialized`
- **500 Internal Server Error** — Unexpected or initialization errors

## Local Soroban Development

### Using Docker Compose

[`docker-compose.yml`](docker-compose.yml) starts the official `stellar/quickstart` image in standalone
mode with **core, horizon, and RPC** all enabled.  It replaces the deprecated
`QUICKSTART_SOROBAN` environment variable with the modern `--enable` flag and
adds resource limits, a bounded named volume, and an extended health-check
`start_period` so slow hosts don't produce spurious failures.

#### Quick start

```bash
# 1. (Optional) copy and customise the env file
cp .env.localnet.example .env.localnet
# edit .env.localnet — at minimum set QUICKSTART_CPUS / QUICKSTART_MEMORY
#                      and fill in contract IDs after deploying

# 2. Start the localnet (waits until the RPC endpoint is healthy)
docker-compose --env-file .env.localnet up --wait

# 3. Verify the RPC endpoint
curl -s -X POST http://localhost:8000 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getNetwork","params":[]}' | jq .

# 4. In another terminal, run integration tests against localnet
cd bindings
SOROBAN_NETWORK=localnet npm test

# 5. Stop the localnet (ledger data is preserved in the named volume)
docker-compose down

# 6. Wipe persisted ledger data and start fresh
docker-compose down -v
```

#### What the compose file configures

| Setting | Value | Why |
|---|---|---|
| Image tag | `QUICKSTART_IMAGE_TAG` (default `soroban-latest`) | Pin to a digest for reproducible CI; use `soroban-latest` for local dev |
| Services | `--enable core,horizon,rpc` | Explicit — replaces the deprecated `QUICKSTART_SOROBAN=true` env var |
| Network mode | `--standalone` | Private chain with fast ledger closes and built-in Friendbot |
| Protocol version | `--protocol-version` (default `21`) | Matches current mainnet to catch protocol-level incompatibilities early |
| Health check | `getNetwork` JSON-RPC probe every 10 s, `start_period` 90 s | Quickstart boots core → horizon → rpc via supervisord; 90 s avoids false failures on slow hosts |
| CPU limit | `QUICKSTART_CPUS` (default `2.0`) | Prevents the container from monopolising the developer machine |
| Memory limit | `QUICKSTART_MEMORY` (default `2G`) | Soroban RPC keeps a ledger DB in memory; 2 GB is comfortable |
| Volume cap | `SOROBAN_DATA_SIZE` (default `2g`) | Bounds disk growth from a long-running local chain |
| Network | named bridge `mux-network` | Isolates the node from other Docker projects |

#### Environment configuration

See [`.env.localnet.example`](.env.localnet.example) for the full variable reference.
Key variables:

```
QUICKSTART_IMAGE_TAG   Image tag or digest (default: soroban-latest)
PROTOCOL_VERSION       Soroban protocol version to activate (default: 21)
QUICKSTART_CPUS        CPU limit for the container (default: 2.0)
QUICKSTART_MEMORY      Memory limit for the container (default: 2G)
SOROBAN_DATA_DRIVER    Volume driver: local (persist) or tmpfs (ephemeral CI)
SOROBAN_DATA_SIZE      Volume size cap (default: 2g)
LOCALNET_RPC_URL       RPC base URL consumed by tests (default: http://localhost:8000)
LOCALNET_MUX_*_ID     Contract addresses — populate after deployment
```

#### Storage and TTL notes

Soroban ledger entries expire after a TTL (the Mux contracts default to ~30 days).
On a long-running local node, extend TTLs with the Stellar CLI before they
expire:

```bash
stellar contract extend \
  --id $LOCALNET_MUX_ACCOUNT_ID \
  --ledgers-to-extend 518400 \
  --source <KEEPER_SECRET> \
  --network localnet
```

Repeat for every contract ID. Run at least once every 25 days. See
[`docs/storage-griefing.md`](docs/storage-griefing.md) for per-contract
collection caps and the full keeper runbook.

#### Local contract invocation helper

```bash
bash scripts/local-invoke.sh --contract-name mux-account --function owner --secret-key S... --arg true
```

Supported options:
- `--network <network>` — `localnet|testnet|mainnet` (default: `localnet`)
- `--contract-id <id>` or `--contract-name <name>` — contract to call
- `--function <name>` — contract function to invoke
- `--secret-key <secret>` — signer secret key for the transaction
- `--arg <value>` — argument values; repeatable
- `--simulate-only` — simulate without submitting

**Post-deploy smoke checks** (simulate-only reads across core contracts):

```bash
bash scripts/local-invoke-smoke.sh --secret-key S... --network localnet
# List planned checks without RPC:
bash scripts/local-invoke-smoke.sh --dry-run
```

If dependencies are not installed, run:

```bash
cd bindings && npm ci
```

#### Deploying contracts to localnet

After the localnet is healthy, build and deploy all contracts:

```bash
# Build WASM artifacts
cargo build --target wasm32-unknown-unknown --release --workspace

# Deploy each contract (requires `stellar` CLI installed)
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/mux_account.wasm \
  --source <DEPLOYER_SECRET> \
  --network localnet

# Save the returned contract ID to .env.localnet:
# LOCALNET_MUX_ACCOUNT_ID=C...
# Repeat for: mux-batcher, mux-permissions, mux-spending-policy,
#             mux-registry, mux-wallet-registry, mux-delegation,
#             mux-recovery, mux-policy, mux-account-factory
```

See the [Deploying Contracts](#deploying-contracts) section above for the full deploy workflow.

## Documentation

- [Contract IDs](CONTRACT_IDS.md) — Per-network program addresses, update process, and upgrade authority
- [Contract Upgrade Pattern](docs/contract-upgrade-pattern.md) — Soroban WASM upgrade flow, admin authorization, storage compatibility, rollback, and integrator checklist

## Documentation (Extended)

- [Contract Upgrade Pattern](docs/contract-upgrade-pattern.md) — canonical upgrade procedure for all Mux Soroban contracts

- [Architecture Overview](docs/architecture-overview.md) — High-level diagram and system components
- [Delegation Permission Model](docs/delegation-permission-model.md) — Permission grant/revoke semantics, storage bounds, error codes, and TypeScript binding notes
- [Policy Semantics](docs/policy-semantics.md) — Per-wallet daily spend limit design, reset logic, and error codes
- [Account Abstraction Design](docs/account-abstraction.md) — Goals, architecture, session key design, and transaction flows
- [Backend Orchestrator Integration](docs/aa-backend-orchestrator.md) — Scope and architecture for relayer integration
- [Threat Model](docs/threat-model.md) — assets, trust boundaries, and mitigations
- [Pause & Freeze Architecture Decision](docs/pause-freeze-decision.md) — Architectural decision record on self-custodial account circuit breaker vs global freeze
- [Operations Runbook Index](ops/README.md) — Comprehensive operations directory, deployment checklists, and operational logs
- [Soroban SDK Bump Playbook](docs/sdk-bump-playbook.md) — Invariants, step-by-step workflow, and verification for upgrading `soroban-sdk`
- [Access Control Review Checklist](docs/access-control-checklist.md) — pre-deployment and pre-audit checklist
- [Storage Griefing Notes](docs/storage-griefing.md) — collection caps, TTL management, keeper runbook
- [External Audit Prep](docs/audit-prep.md) — scope, entry points, known limitations, auditor checklist
- [Error Codes Reference](docs/error_codes.md) — all contract error codes and HTTP mappings
- [Bindings Error Mapping](docs/bindings-error-mapping.md) — how Rust error enums flow to TS unions and HTTP statuses
- [Registry Contracts Comparison](docs/registry-contracts-comparison.md) — mux-registry vs mux-wallet-registry: purpose, error codes, auth models, and storage layouts

## Security

See [SECURITY.md](SECURITY.md) for the vulnerability disclosure policy and
safe-harbor guidelines.

To report a vulnerability, open a private security advisory on GitHub or email
**security@mux-protocol.xyz**.

## Testing & coverage

```bash
cargo test --workspace --all-features
make coverage                          # LLVM coverage, or stub if tools missing
bash scripts/coverage.sh --stub        # print coverage report stub only
bash scripts/test-coverage.sh          # validate stub lists all mux-* crates
```

`Cargo.lock` is committed — see [CONTRIBUTING.md](CONTRIBUTING.md#cargolock-policy).

## Contributing

- [CONTRIBUTING.md](CONTRIBUTING.md) — commit conventions, PR process, contract PR guidelines
- [Breaking Change Policy](docs/BREAKING_CHANGES.md) — guidelines for backward compatibility, deprecation periods, and versioning

## License

[MIT](LICENSE)
