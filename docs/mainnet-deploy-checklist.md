# Mainnet Deployment Checklist

Use this checklist before every mainnet deployment. Complete every item in order. Do not skip steps — each exists because a past incident or near-miss made it necessary.

> **When to use:** Run through this checklist for every first-time deployment and every upgrade that changes on-chain state or contract addresses.

---

## Pre-Deployment

### 1. Deployer Key Funded and Secured

- [ ] Deployer public key is known and recorded: `_________________________`
- [ ] Account exists on mainnet (Horizon returns HTTP 200, not 404)
- [ ] XLM balance ≥ 10 XLM (check: `stellar account show <KEY> --network mainnet`)
- [ ] Secret key is stored in a secrets manager or HSM — **not** in any file on disk
- [ ] `DEPLOYER_SECRET_KEY` is set in the deployment environment (CI secret or local env)
- [ ] Deployer key has **not** been used for any other on-chain activity since last rotation

See [docs/deployer-key.md](deployer-key.md) for full key setup instructions.

---

### 2. Contract Audits Completed

- [ ] All contracts in this deployment have a completed audit report
- [ ] Audit report is referenced in the PR / release notes: `_________________________`
- [ ] All **critical** and **high** severity findings are resolved or formally accepted with written rationale
- [ ] Medium severity findings are triaged and their status documented
- [ ] Audit firm has signed off on any fix diffs applied post-audit
- [ ] Final WASM hashes match the audited source commit:
  - `mux-account`          : `________________________________`
  - `mux-account-factory`  : `________________________________`
  - `mux-batcher`          : `________________________________`
  - `mux-permissions`      : `________________________________`

---

### 3. Dry-Run Executed Successfully on Testnet

- [ ] `scripts/deploy.sh --dry-run --network testnet` completed with exit code 0
- [ ] All `[DRY RUN]` steps logged without errors
- [ ] Full live deployment run on **testnet** completed successfully (not just dry-run)
- [ ] Deployed testnet contract IDs recorded: `_________________________`
- [ ] TypeScript bindings regenerated against testnet deployment and tests pass:
  ```bash
  bash scripts/generate-bindings.sh --network testnet
  cd bindings && npm test
  ```
- [ ] Integration smoke tests pass against testnet contracts

---

### 4. Environment Variables Verified for Mainnet

- [ ] `SOROBAN_RPC_URL` points to mainnet RPC: `https://soroban-mainnet.stellar.org` (or approved private RPC)
- [ ] `STELLAR_NETWORK` is set to `Public Global Stellar Network ; September 2015`
- [ ] `SOROBAN_NETWORK` is set to `mainnet`
- [ ] `DEPLOYER_SECRET_KEY` is the **mainnet** deployer key (not a testnet key)
- [ ] No testnet/localnet values remain in any env file being sourced
- [ ] Run: `bash scripts/deploy.sh --dry-run --network mainnet --skip-build` and confirm output shows `mainnet` in configuration banner

---

### 5. Admin/Owner Addresses Confirmed

- [ ] Initial `admin` address for each contract is confirmed with the team lead: `_________________________`
- [ ] Address is a **multisig** or **governance contract** — not a single EOA
- [ ] Address has been verified on-chain (account exists and has expected signers)
- [ ] Admin transfer plan is documented (deployer → multisig) and scheduled immediately after deploy
- [ ] Emergency pause / upgrade key holders are identified and reachable
- [ ] `config/addresses.json` is pre-populated with expected admin addresses for post-deploy verification

---

## Deployment Execution

- [ ] Announce deployment window in the team channel (minimum 24 h notice for mainnet)
- [ ] Confirm at least two engineers are online and available during the deployment window
- [ ] Run the deployment script:
  ```bash
  source .env.mainnet   # or inject secrets from CI
  bash scripts/deploy.sh --network mainnet
  ```
- [ ] Record all deployed contract IDs as they are printed to stdout
- [ ] Script exits with code 0

---

## Post-Deploy Verification

### 6. Contract IDs Recorded

- [ ] All contract IDs saved to `config/addresses.json`:
  ```json
  {
    "mainnet": {
      "muxAccount":         "<CONTRACT_ID>",
      "muxAccountFactory":  "<CONTRACT_ID>",
      "muxBatcher":         "<CONTRACT_ID>",
      "muxPermissions":     "<CONTRACT_ID>"
    }
  }
  ```
- [ ] PR opened to commit updated `config/addresses.json` and merge to `main`

### 7. On-Chain Verification

- [ ] Query each contract via Stellar CLI to confirm it responds:
  ```bash
  stellar contract invoke \
    --id <CONTRACT_ID> \
    --network mainnet \
    -- version
  ```
- [ ] Contract admin/owner matches the expected address confirmed in step 5
- [ ] TypeScript bindings regenerated against mainnet contract IDs:
  ```bash
  bash scripts/generate-bindings.sh --network mainnet --skip-build
  cd bindings && npm test
  ```

### 8. Admin Transfer

- [ ] Admin ownership transferred from deployer key to multisig/governance contract
- [ ] Deployer key balance drained or key destroyed after transfer
- [ ] Confirm deployer key can no longer call privileged functions

### 9. Monitoring and Alerts

- [ ] Monitoring for the new contract IDs is active (event indexer, Horizon stream, or off-chain monitor)
- [ ] On-call rotation is aware of the new deployment
- [ ] Rollback / upgrade plan is documented and rehearsed on testnet if this is an upgrade

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Lead Engineer | | | |
| Security Reviewer | | | |
| Protocol Lead | | | |

All three sign-offs are required before the deployment window begins.

---

## Related Documents

- [Deployer Key Setup](deployer-key.md)
- [Audit Preparation](audit-prep.md)
- [Access Control Checklist](access-control-checklist.md)
- [Architecture Overview](architecture-overview.md)
- [scripts/deploy.sh](../scripts/deploy.sh) — deployment script with `--dry-run` support
