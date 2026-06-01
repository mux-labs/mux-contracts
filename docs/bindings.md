# TypeScript Bindings Generation

This document explains how to generate, use, and maintain the TypeScript bindings for Mux Protocol Soroban contracts.

---

## Overview

TypeScript bindings are typed client classes and interfaces auto-generated from compiled WASM artifacts using the [Stellar CLI](https://developers.stellar.org/docs/tools/stellar-cli). They live in [`bindings/src/generated/`](../bindings/src/generated/) and are published to npm as `@mux-protocol/contracts`.

Do **not** edit files inside `bindings/src/generated/` by hand — they will be overwritten the next time bindings are regenerated.

---

## Quick Start

### Install the published package

```bash
npm install @mux-protocol/contracts
```

### Use a typed contract client

```typescript
import { MuxAccountClient } from "@mux-protocol/contracts";
import { Networks, SorobanRpc, Keypair } from "@stellar/stellar-sdk";

const server = new SorobanRpc.Server("https://soroban-testnet.stellar.org");
const keypair = Keypair.fromSecret(process.env.MY_SECRET_KEY!);

const client = new MuxAccountClient({
  contractId: "CABC...XYZ",
  networkPassphrase: Networks.TESTNET,
  rpcUrl: "https://soroban-testnet.stellar.org",
});

const result = await client.getOwner();
console.log("Owner:", result);
```

See [`bindings/src/examples/frontend-usage.ts`](../bindings/src/examples/frontend-usage.ts) for more complete examples.

---

## Regenerating Bindings

Regenerate bindings whenever you modify contract source code.

### Option 1 — Shell script (CI and simple local use)

```bash
# From repo root
bash scripts/generate-bindings.sh --network testnet

# Skip cargo build if WASMs are already built
bash scripts/generate-bindings.sh --network testnet --skip-build

# Generate bindings for a single contract
bash scripts/generate-bindings.sh --network testnet --contract mux-account
```

### Option 2 — TypeScript script (npm workflow)

```bash
# From bindings/ directory
npm run generate:bindings

# Equivalent from repo root
npx ts-node scripts/generate-bindings.ts --network testnet

# Dry-run: print commands without executing
npx ts-node scripts/generate-bindings.ts --dry-run

# Single contract
npx ts-node scripts/generate-bindings.ts --contract mux-batcher
```

### Available flags (both scripts)

| Flag | Default | Description |
|------|---------|-------------|
| `--network <name>` | `testnet` | Target network for RPC references in generated types |
| `--skip-build` | `false` | Use pre-built WASMs from `target/`; skip `cargo build` |
| `--contract <name>` | all | Generate bindings for one contract only |
| `--dry-run` | `false` | Print commands without running stellar CLI or cargo |

---

## Project Structure

```
bindings/
├── src/
│   ├── generated/           ← Auto-generated; do NOT edit by hand
│   │   ├── mux-account/
│   │   ├── mux-account-factory/
│   │   ├── mux-batcher/
│   │   └── mux-permissions/
│   ├── addresses.ts         ← Contract addresses per network
│   ├── addresses-config.ts  ← Address configuration helpers
│   ├── errors.ts            ← Typed contract error classes
│   ├── horizon.ts           ← Horizon API helpers
│   ├── index.ts             ← Package entry point
│   ├── network.ts           ← Network configuration
│   └── types.ts             ← Shared types
├── __tests__/               ← Jest tests for bindings
├── package.json
└── tsconfig.json

scripts/
├── generate-bindings.sh     ← Bash generation script
└── generate-bindings.ts     ← TypeScript generation script (this document)
```

---

## Checking for Binding Drift

To verify that committed bindings match the current contracts (e.g. in CI or pre-commit):

```bash
cd bindings
npm run check:bindings
```

This regenerates bindings from existing WASMs and fails if the output differs from the committed state.

The CI pipeline also runs this check automatically on every PR that touches `contracts/**` or `scripts/generate-bindings.*` (see [`.github/workflows/bindings.yml`](../.github/workflows/bindings.yml)).

---

## CI Pipeline

The [Bindings CI workflow](../.github/workflows/bindings.yml) runs on every PR and push to `main`:

1. **Build contracts** — `cargo build --target wasm32-unknown-unknown --release`
2. **Generate bindings** — `bash scripts/generate-bindings.sh --skip-build`
3. **TypeScript compile check** — `tsc --noEmit`
4. **Lint** — `eslint src`
5. **Tests** — `jest`
6. **Drift check** (PRs only) — fail if committed bindings differ from freshly generated ones
7. **Publish to npm** (main branch, `chore: release` commit message only)

---

## Adding a New Contract

1. Create the contract under `contracts/<contract-name>/`.
2. Add it to the `CONTRACTS` array in both `scripts/generate-bindings.sh` and `scripts/generate-bindings.ts`.
3. Run `bash scripts/generate-bindings.sh` to generate its bindings.
4. Export the new client from `bindings/src/index.ts`.
5. Add an entry to `config/addresses.json` for each network once deployed.

---

## Related Documents

- [Architecture Overview](architecture-overview.md)
- [Deployer Key Setup](deployer-key.md)
- [Mainnet Deploy Checklist](mainnet-deploy-checklist.md)
- [`bindings/src/examples/frontend-usage.ts`](../bindings/src/examples/frontend-usage.ts)
