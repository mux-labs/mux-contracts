# Deployer Key Requirements

Quick-reference for the minimum requirements needed to deploy Mux Protocol contracts on any Stellar network.

For a full operational guide (key rotation, CI/CD, HSM usage) see [funded-deployer-key.md](funded-deployer-key.md).

---

## What is a funded deployer key?

A **deployer key** is a Stellar keypair whose secret key is used by the deployment scripts to:

1. Sign `stellar contract upload` transactions (uploads the WASM bytecode on-chain).
2. Sign `stellar contract deploy` transactions (instantiates the contract from the uploaded WASM).

Both operations consume transaction fees paid in XLM from the deployer account balance.  The account must therefore be **funded** (exist on-ledger with a non-zero XLM balance) before any deployment can proceed.

The deployer key is distinct from the **admin key**, which controls post-deployment operations (upgrades, access control changes).  Separating the two limits the blast radius if either key is compromised.

---

## Minimum XLM balance

| Network  | Recommended minimum | Notes |
|----------|--------------------:|-------|
| Testnet  | 5 XLM               | Friendbot provides 10 000 XLM — more than enough |
| Mainnet  | 10 XLM per contract | Budget ~0.1–0.5 XLM per upload + deploy pair; add a 5 XLM safety buffer |
| Localnet | 100 XLM (Quickstart)| Quickstart Friendbot funds accounts automatically |

> A full mainnet deployment of all ten Mux contracts typically consumes 5–20 XLM in fees.  Start with at least 50 XLM on the deployer account to deploy the full suite with room for retries.

---

## Generate a keypair

### Stellar CLI (recommended)

```bash
# Generate and store a named keypair in the local Stellar CLI keystore
stellar keys generate deployer --network testnet

# Print the public key (G...)
stellar keys address deployer

# Print the secret key (S...) — store this securely, never commit it
stellar keys show deployer
```

### Offline (Node.js / stellar-base)

```bash
node -e "
  const { Keypair } = require('@stellar/stellar-base');
  const k = Keypair.random();
  console.log('Public :', k.publicKey());
  console.log('Secret :', k.secret());
"
```

---

## Fund the deployer account

### Testnet — Friendbot (free)

```bash
# Via Stellar CLI (simplest)
stellar keys fund deployer --network testnet

# Via curl
curl "https://friendbot.stellar.org?addr=$(stellar keys address deployer)"

# Via the repo helper script
bash scripts/fund-accounts.sh "$(stellar keys address deployer)"
```

Friendbot provides **10 000 XLM** — sufficient for hundreds of test deployments.

### Mainnet — transfer from an exchange or another account

```bash
# Send XLM to the deployer public key
stellar tx new payment \
  --source-account "$YOUR_FUNDED_ACCOUNT" \
  --destination    "$(stellar keys address deployer)" \
  --asset native \
  --amount 50 \
  --network mainnet
```

Always verify the balance before deploying:

```bash
stellar account balance \
  --account "$(stellar keys address deployer)" \
  --network testnet   # or mainnet
```

---

## Set the secret key for scripts

Deployment scripts read credentials from environment variables.  **Never hardcode secret keys in files.**

```bash
# Minimum required
export DEPLOYER_PRIVATE_KEY="SXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
export ADMIN_ADDRESS="GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"

# Network selection (default: testnet)
export SOROBAN_NETWORK="testnet"
```

For CI/CD (GitHub Actions example):

```yaml
- name: Deploy contracts
  env:
    DEPLOYER_PRIVATE_KEY: ${{ secrets.DEPLOYER_PRIVATE_KEY }}
    ADMIN_ADDRESS:        ${{ secrets.ADMIN_ADDRESS }}
  run: bash scripts/deploy.sh --network testnet
```

---

## Security checklist

- [ ] Secret key is **not** committed to version control (checked in CI by
      `scripts/scan-git-secrets.sh`, run in the `scan-secrets` job on every PR)
- [ ] `.env`, `*.secret`, `deployment.env`, and `deployer.json` are listed in
      `.gitignore` (checked in CI by `scripts/check-gitignore-secret-patterns.sh`)
- [ ] Deployer key is **separate** from personal and admin keys
- [ ] For mainnet: key is stored in a secrets manager or hardware wallet
- [ ] Deployer key is **drained and rotated** after each mainnet deployment
      cycle — record the drain/rotation in
      [`ops/deployer-key-rotation-log.md`](../ops/deployer-key-rotation-log.md)
      (checked in CI by `scripts/check-deployer-key-rotation-log.sh`)

### CI/CD key handling

`.github/workflows/deploy.yml` currently authenticates with a long-lived
`DEPLOYER_PRIVATE_KEY` GitHub Actions secret — there is no OIDC/keyless
signing path yet. Until that changes:

- [ ] The `DEPLOYER_PRIVATE_KEY` repo secret is scoped to the `mainnet` /
      `testnet` GitHub Environments (Settings → Environments), not a bare
      repository secret, so deploys require environment protection rules.
- [ ] The key is rotated (drained on-chain, replaced in the secret store)
      immediately after every mainnet deployment run — do not reuse a
      mainnet deployer key across deployment cycles.
- [ ] Anyone who can trigger `workflow_dispatch` on `deploy.yml` is treated
      as having spend authority over the funded deployer account.

---

## Related documents

- [funded-deployer-key.md](funded-deployer-key.md) — full operational guide
- [deployer-key.md](deployer-key.md) — key setup walkthrough
- [MAINNET_DEPLOY_CHECKLIST.md](MAINNET_DEPLOY_CHECKLIST.md) — pre-deploy verification steps
- [scripts/deploy.sh](../scripts/deploy.sh) — deployment script (`--dry-run` supported)
- [scripts/fund-accounts.sh](../scripts/fund-accounts.sh) — testnet funding helper
- [ops/deployer-key-rotation-log.md](../ops/deployer-key-rotation-log.md) — drain/rotation completion record
