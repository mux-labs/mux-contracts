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

## Security

- [Threat Model](docs/threat-model.md) — assets, trust boundaries, and mitigations
- [Access Control Review Checklist](docs/access-control-checklist.md) — pre-deployment and pre-audit checklist
- [Storage Griefing Notes](docs/storage-griefing.md) — collection caps, TTL management, keeper runbook
- [External Audit Prep](docs/audit-prep.md) — scope, entry points, known limitations, auditor checklist

To report a vulnerability, open a private security advisory on GitHub.

## License

[MIT](LICENSE)
