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
| [`contracts/mux-batcher`](contracts/mux-batcher/) | Atomic multi-operation batching with optional per-op failure handling |
| [`contracts/mux-permissions`](contracts/mux-permissions/) | RBAC registry — roles, permissions, grant/revoke |

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

Run a complete local Stellar/Soroban node for offline development and testing:

```bash
# Start the localnet
docker-compose up --wait

# Verify the node is ready
curl -X POST http://localhost:8000 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getNetwork","params":[]}'

# In another terminal, run tests against localnet
cd bindings
SOROBAN_NETWORK=localnet npm test

# Stop the localnet
docker-compose down

# Remove persisted data and start fresh
docker-compose down -v
```

**Environment Configuration:**

Copy `.env.localnet.example` to `.env.localnet` to customize:
```bash
cp .env.localnet.example .env.localnet
# Edit .env.localnet and set contract addresses after deployment
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

- [Account Abstraction Design](docs/account-abstraction.md) — Goals, architecture, session key design, and transaction flows
- [Threat Model](docs/threat-model.md) — assets, trust boundaries, and mitigations
- [Access Control Review Checklist](docs/access-control-checklist.md) — pre-deployment and pre-audit checklist
- [Contributing Guide](CONTRIBUTING.md) — Commit message format, changelog template, testing requirements

## Security

To report a vulnerability, open a private security advisory on GitHub.

## License

[MIT](LICENSE)
