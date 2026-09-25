# Contract IDs

This file is the human-readable companion to [`config/addresses.json`](../config/addresses.json), which is the machine-readable source of truth for all deployed Mux Protocol contract addresses.

## Purpose

Tracking contract IDs is critical for:

- Directing client applications to the correct on-chain contract
- Verifying that a deployment landed at the expected address
- Providing a stable rollback target if a new deployment has to be reverted
- Auditing the history of deployments across networks

Every deploy that produces a new contract ID **must** update both this file and `config/addresses.json`.

The canonical manifest keys are `muxAccount`, `muxAccountFactory`, `muxBatcher`,
`muxDelegation`, `muxPermissions`, `muxPolicy`, `muxRecovery`, `muxRegistry`,
`muxSpendingPolicy`, and `muxWalletRegistry`.

---

## Contract ID Table

| Network | Contract Name | Contract ID | Deployed At (UTC) | WASM Hash |
|---|---|---|---|---|
| localnet | mux-account | | | |
| localnet | mux-account-factory | | | |
| localnet | mux-batcher | | | |
| localnet | mux-delegation | | | |
| localnet | mux-permissions | | | |
| localnet | mux-policy | | | |
| localnet | mux-recovery | | | |
| localnet | mux-registry | | | |
| localnet | mux-spending-policy | | | |
| localnet | mux-wallet-registry | | | |
| testnet | mux-account | | | |
| testnet | mux-batcher | | | |
| testnet | mux-delegation | | | |
| testnet | mux-permissions | | | |
| testnet | mux-wallet-registry | | | |
| testnet | mux-account-factory | | | |
| testnet | mux-registry | | | |
| testnet | mux-policy | | | |
| testnet | mux-recovery | | | |
| testnet | mux-spending-policy | | | |
| mainnet | mux-account | | | |
| mainnet | mux-batcher | | | |
| mainnet | mux-delegation | | | |
| mainnet | mux-permissions | | | |
| mainnet | mux-wallet-registry | | | |
| mainnet | mux-account-factory | | | |
| mainnet | mux-registry | | | |
| mainnet | mux-policy | | | |
| mainnet | mux-recovery | | | |
| mainnet | mux-spending-policy | | | |

The table intentionally includes all ten contract crates deployed by
`scripts/deploy.sh`, including `mux-recovery` and `mux-spending-policy`.

Fill in the table after each deployment. The `Contract ID` column holds the `C...` address returned by `stellar contract deploy`. The `WASM Hash` column holds the SHA-256 of the `.wasm` binary uploaded to the network.

---

## How to Update After a Deploy

1. Run the deploy script and capture the output:
   ```bash
   bash scripts/deploy-testnet.sh --network testnet 2>&1 | tee deploy.log
   ```

2. Extract the contract IDs from the log:
   ```bash
   grep "Deployed" deploy.log
   ```

3. Update `config/addresses.json`:
   ```json
   {
     "testnet": {
       "muxAccount":         "<new-contract-id>",
       "muxBatcher":         "<new-contract-id>",
       "muxDelegation":      "<new-contract-id>",
       "muxPermissions":     "<new-contract-id>",
       "muxWalletRegistry":  "<new-contract-id>",
       "muxAccountFactory":  "<new-contract-id>",
       "muxRegistry":        "<new-contract-id>",
       "muxPolicy":          "<new-contract-id>",
       "muxRecovery":        "<new-contract-id>",
       "muxSpendingPolicy":  "<new-contract-id>"
     }
   }
   ```

4. Add a row to the table above with the contract ID, timestamp, and WASM hash.

5. Open a PR targeting `main`. IDs are intentionally tracked in version control so
   the full deployment history is preserved.

---

## Format Reference

```
Network       — testnet | mainnet | localnet
Contract Name — matches the directory name under contracts/
Contract ID   — 56-character Stellar contract address starting with C
Deployed At   — ISO-8601 UTC timestamp, e.g. 2025-09-14T10:32:00Z
WASM Hash     — SHA-256 hex digest of the uploaded .wasm file
```

To compute the WASM hash locally:

```bash
sha256sum target/wasm32-unknown-unknown/release/mux_account.wasm
```

---

## Environment Variable Overrides

Runtime overrides follow the pattern `{NETWORK}_MUX_*_ID` and take precedence over `config/addresses.json`:

```bash
SOROBAN_NETWORK=testnet
TESTNET_MUX_ACCOUNT_ID=C...
TESTNET_MUX_BATCHER_ID=C...
TESTNET_MUX_DELEGATION_ID=C...
TESTNET_MUX_PERMISSIONS_ID=C...
TESTNET_MUX_WALLET_REGISTRY_ID=C...
TESTNET_MUX_ACCOUNT_FACTORY_ID=C...
TESTNET_MUX_REGISTRY_ID=C...
TESTNET_MUX_POLICY_ID=C...
TESTNET_MUX_RECOVERY_ID=C...
TESTNET_MUX_SPENDING_POLICY_ID=C...
```

See [`.env.deploy.example`](../.env.deploy.example) for the full variable reference.

## How Drift Is Caught

`scripts/check-contract-ids-sync.sh` (run in CI) treats the `MuxContractIds`
interface in [`../bindings/src/types.ts`](../bindings/src/types.ts) as the
canonical key set and fails if this file, `../CONTRACT_IDS.md`, or
`../config/addresses.json` stop mentioning any key in that set.

---

## Related Documents

- [`../config/addresses.json`](../config/addresses.json) — machine-readable source of truth
- [`../CONTRACT_IDS.md`](../CONTRACT_IDS.md) — top-level overview of the addresses file structure
- [`mainnet-deploy-checklist.md`](mainnet-deploy-checklist.md) — pre-deploy checklist
- [`rollback-guide.md`](rollback-guide.md) — recovering from a bad deploy
- [`../scripts/deploy-testnet.sh`](../scripts/deploy-testnet.sh) — testnet deploy script
