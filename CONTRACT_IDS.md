# Mux Contract IDs

`config/addresses.json` is the machine-readable source of truth for deployed Mux Protocol contract addresses. This document is the human-readable companion and must contain the same contract key set as `bindings/src/types.ts` and `docs/contract-ids.md`.

## Canonical contract set

| Key | Contract crate | Responsibility |
|---|---|---|
| `muxAccount` | `mux-account` | Per-user smart account and authorized execution |
| `muxAccountFactory` | `mux-account-factory` | Deterministic account deployment and owner index |
| `muxBatcher` | `mux-batcher` | Bounded atomic multi-operation dispatch |
| `muxDelegation` | `mux-delegation` | Scoped delegate grants and revocation |
| `muxPermissions` | `mux-permissions` | Role and permission policy evaluation |
| `muxPolicy` | `mux-policy` | Per-wallet daily spend policy |
| `muxRecovery` | `mux-recovery` | Guardian-driven account recovery |
| `muxRegistry` | `mux-registry` | Generic contract/version metadata registry |
| `muxSpendingPolicy` | `mux-spending-policy` | Spend-limit and allow/deny enforcement |
| `muxWalletRegistry` | `mux-wallet-registry` | Wallet discovery and account associations |

All ten keys are present for `localnet`, `testnet`, and `mainnet` in `config/addresses.json`. An empty value means that the contract has not yet been deployed on that network; it is not permission to substitute an address from another network.

## Address update procedure

After a deployment, update only the matching network and contract key. Validate the result before opening a PR:

```bash
jq empty config/addresses.json
bash scripts/check-contract-ids-sync.sh
```

A mainnet address change requires the approval marker and audit record described by the `_review` object in `config/addresses.json`. Never commit secret keys or treat an environment override as a replacement for a reviewed manifest update.

## Network and environment overrides

The supported networks are `localnet`, `testnet`, and `mainnet`. Runtime overrides follow the pattern `{NETWORK}_MUX_*_ID` and take precedence over the manifest for explicitly configured environments, for example:

```bash
SOROBAN_NETWORK=testnet
TESTNET_MUX_ACCOUNT_ID=C...
TESTNET_MUX_RECOVERY_ID=C...
TESTNET_MUX_SPENDING_POLICY_ID=C...
```

See [`.env.deploy.example`](.env.deploy.example) and [`docs/contract-ids.md`](docs/contract-ids.md) for deployment history and rollback guidance. The synchronization check treats the `MuxContractIds` interface as the canonical key set and fails closed when any network or document drifts.
