# Contract IDs

This document explains the structure and lifecycle of `config/addresses.json` — the canonical source for deployed Mux Protocol contract addresses.

## File location

[`config/addresses.json`](config/addresses.json)

## Structure

```json
{
  "localnet":  { "muxAccount": "", "muxBatcher": "", "muxPermissions": "" },
  "testnet":   { "muxAccount": "", "muxBatcher": "", "muxPermissions": "" },
  "mainnet":   { "muxAccount": "", "muxBatcher": "", "muxPermissions": "" }
}
```

### Contracts

| Key | Contract | Purpose |
|---|---|---|
| `muxAccount` | `contracts/mux-account` | Account abstraction: owner management, delegates, spend limits |
| `muxBatcher` | `contracts/mux-batcher` | Atomic multi-op batching with per-op failure handling |
| `muxPermissions` | `contracts/mux-permissions` | RBAC registry — roles, grant/revoke |

### Networks

| Key | Network | Notes |
|---|---|---|
| `localnet` | Local Docker node | Populated after `stellar contract deploy` against the Docker Compose node |
| `testnet` | Stellar testnet | Populated by CI or a manual testnet deploy |
| `mainnet` | Stellar mainnet | Populated after an audited mainnet release; treat as immutable once set |

## How IDs are updated

1. Build the WASM: `cargo build --target wasm32-unknown-unknown --release --workspace`
2. Deploy via `stellar contract deploy --wasm <path>.wasm --network <network>`
3. Copy the returned contract ID into the appropriate key in `config/addresses.json`
4. Commit the updated file on a release branch — IDs are intentionally tracked in VCS

## Environment variable overrides

Runtime overrides follow the pattern `{NETWORK}_MUX_*_ID` and take precedence over `addresses.json`:

```bash
SOROBAN_NETWORK=testnet
TESTNET_MUX_ACCOUNT_ID=C...
TESTNET_MUX_BATCHER_ID=C...
TESTNET_MUX_PERMISSIONS_ID=C...
```

See [`.env.deploy.example`](.env.deploy.example) for the full variable reference.

## Upgrade authority

Contract upgrades require the deployer keypair that owns the upgrade authority.  
Store the corresponding secret key in `SOROBAN_SECRET_KEY` (never commit it).  
Mainnet upgrade authority is held by the Mux Labs multisig; contact the core team before deploying to mainnet.
