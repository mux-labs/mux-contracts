# Mainnet Deploy Checklist

Step-by-step checklist for safe Mux Protocol contract deployments to Stellar Mainnet. Complete every item in order — do not skip steps.

---

## Phase 1 — Pre-Deploy

### Audit & Review

- [ ] Security audit complete and all critical/high findings resolved
- [ ] Audit report reviewed and sign-off obtained from lead engineer
- [ ] All contract changes since last audit reviewed for new risk surface
- [ ] Dependency versions pinned in `Cargo.lock` and reviewed for known CVEs (`cargo deny check`)

### Testnet Verification

- [ ] All contracts deployed and initialized on testnet
- [ ] Full integration test suite passed on testnet deployment
- [ ] All user-facing flows tested end-to-end on testnet
- [ ] Upgrade path tested on testnet (if this deploy includes an upgrade)
- [ ] Testnet contract addresses recorded in `addresses.json` or equivalent

### Deployer Key & Funding

- [ ] Dedicated deployer key generated (not shared with admin or personal keys)
- [ ] Deployer account funded with sufficient XLM (≥ 10 XLM per contract + 5 XLM buffer)
- [ ] Deployer balance verified: `stellar account balance --network mainnet`
- [ ] Deployer key stored in secrets manager (not in local files or shell history)
- [ ] `DEPLOYER_PRIVATE_KEY` and `ADMIN_ADDRESS` set in deploy environment

See [funded-deployer-key.md](funded-deployer-key.md) for key setup details.

### Multisig & Admin Configuration (if applicable)

- [ ] Admin multisig quorum verified and all signers confirmed available
- [ ] Admin key rotation completed if any signer has changed since last deploy
- [ ] Multisig hardware wallets charged and accessible

### Environment & Config

- [ ] `SOROBAN_NETWORK=mainnet` set in deploy environment
- [ ] RPC endpoint confirmed operational: `curl https://rpc-mainnet.stellar.org/health`
- [ ] `config/networks.toml` reviewed for correct mainnet RPC and passphrase
- [ ] `Cargo.lock` committed and up to date
- [ ] Deploy script version pinned — confirm `git log scripts/deploy.sh` matches expected commit

### Dry-Run Passed

- [ ] Dry-run executed successfully with production config:
  ```bash
  DEPLOYER_PRIVATE_KEY=$DEPLOYER_PRIVATE_KEY \
  ADMIN_ADDRESS=$ADMIN_ADDRESS \
  bash scripts/deploy.sh --network mainnet --dry-run
  ```
- [ ] Dry-run output reviewed — all contracts listed, no unexpected warnings

---

## Phase 2 — Deploy

### Final Checks at Deploy Time

- [ ] `SOROBAN_NETWORK` confirmed as `mainnet` (not testnet/localnet)
- [ ] No unrelated staged changes in the working tree (`git status` clean)
- [ ] Team notified that mainnet deploy is beginning (Slack/Discord/etc.)
- [ ] Rollback plan reviewed — prior WASM hashes retained and documented

### Execute Deploy

- [ ] Run deploy script:
  ```bash
  DEPLOYER_PRIVATE_KEY=S... \
  ADMIN_ADDRESS=G... \
  bash scripts/deploy.sh --network mainnet
  ```
- [ ] Script completed without errors (exit code 0)
- [ ] `deployment.env` or equivalent output reviewed — contract IDs present for all contracts

### Fee & Transaction Verification

- [ ] All upload and deploy transactions visible in Stellar Explorer
- [ ] Transaction fees within expected range (no runaway fee bumps)
- [ ] Deployer account balance post-deploy recorded

---

## Phase 3 — Post-Deploy

### Contract Address Recording

- [ ] Contract IDs copied from deploy output into `addresses.json` (or network-specific config)
- [ ] `addresses.json` committed and pushed to main
- [ ] Contract addresses shared with frontend/SDK teams

### On-Chain Verification

- [ ] Each contract ID resolvable via Stellar Explorer
- [ ] Contract WASM hash matches expected value (run `scripts/verify-wasm-hash.sh`)
- [ ] Contract `initialize()` completed for each contract (if not done by deploy script)
- [ ] Admin address set correctly: query contract admin storage and confirm `ADMIN_ADDRESS`

### Ownership & Access Control

- [ ] Admin ownership confirmed on each contract
- [ ] Deployer key permissions revoked or isolated (no admin role held by deployer key)
- [ ] Multisig quorum tested with a low-risk admin call
- [ ] Access control matrix updated if roles changed

### Monitoring & Alerts

- [ ] Contract IDs registered in monitoring/alerting system
- [ ] Event indexer updated with new contract IDs (if applicable)
- [ ] On-call runbook updated with new contract addresses
- [ ] Error/anomaly alerts confirmed active for new contracts

### Communication

- [ ] Deploy completion announced to team
- [ ] Release notes / changelog updated with new contract addresses and version
- [ ] External partners or integrators notified of new addresses (if breaking change)

---

## Rollback Procedure

If a critical issue is discovered post-deploy:

1. **Do not delete the contract** — Soroban contracts are immutable ledger entries.
2. **Pause user-facing access** — disable frontend endpoints or route to maintenance page.
3. **Assess severity** — determine if the issue requires an upgrade or is mitigatable off-chain.
4. **Upgrade path** — if the contract exposes `upgrade()`:
   ```bash
   # Re-deploy prior WASM (prior wasm hash is content-addressed, always available)
   stellar contract invoke \
     --id $CONTRACT_ID \
     --source-account $ADMIN_KEY \
     --network mainnet \
     -- upgrade \
     --new_wasm_hash $PRIOR_WASM_HASH
   ```
5. **Incident report** — document root cause, timeline, and remediation.

---

## Related

- [Funded deployer key setup](funded-deployer-key.md)
- [Deploy dry-run flag](../scripts/deploy.sh) — `--dry-run` usage
- [WASM hash verification](../scripts/verify-wasm-hash.sh) — post-deploy hash check
- [Audit prep](audit-prep.md) — pre-audit requirements
