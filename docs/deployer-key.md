# Deployer Key Setup

This document covers everything needed to set up and manage the deployer key used for Mux Protocol contract deployments.

---

## Overview

A **deployer key** is a Stellar account whose secret key is used by the deployment scripts to sign and submit contract upload and deploy transactions. The account must be funded on the target network before any deployment can proceed.

---

## 1. Key Generation

### Option A — Stellar CLI (recommended)

```bash
# Generate a new key pair and print the public key and secret key
stellar keys generate deployer --network testnet

# Print the public key
stellar keys address deployer

# Print the secret key (store this securely — never commit it)
stellar keys show deployer
```

### Option B — From a hardware wallet or KMS

For mainnet deployments, generate and store the key in a hardware wallet (Ledger) or cloud KMS (AWS KMS, GCP Cloud HSM). Contact the Mux security team for KMS integration guidance.

---

## 2. Funding Requirements

The deployer account must hold enough XLM to cover transaction fees and minimum balance reserves for each contract deployment.

| Network | Minimum XLM balance | Per-contract upload fee (est.) | Per-contract deploy fee (est.) |
|---------|--------------------|---------------------------------|---------------------------------|
| Testnet | 5 XLM | ~0.01 XLM | ~0.01 XLM |
| Mainnet | 10 XLM | ~0.01–0.05 XLM | ~0.01–0.05 XLM |
| Localnet | 100 XLM (Friendbot) | negligible | negligible |

> **Note:** Fees vary with network congestion and WASM size. Budget generously for mainnet — a full deployment of all four contracts plus initialization transactions typically requires 5–20 XLM.

### Fund on testnet (Friendbot)

```bash
# Using the fund-accounts.sh helper
bash scripts/fund-accounts.sh <YOUR_DEPLOYER_PUBLIC_KEY>

# Or directly via Friendbot
curl "https://friendbot.stellar.org?addr=<YOUR_DEPLOYER_PUBLIC_KEY>"
```

### Fund on mainnet

Transfer XLM from an exchange or existing Stellar account:

```bash
# Verify the deployer account exists and is funded
stellar account show <YOUR_DEPLOYER_PUBLIC_KEY> --network mainnet
```

---

## 3. Environment Variable Configuration

All deployment scripts read the deployer key from environment variables. **Never hardcode secret keys in source files.**

### Required

```bash
# The secret key of the funded deployer account (starts with 'S')
export DEPLOYER_SECRET_KEY="SABC...XYZ"
```

### Optional overrides

```bash
# Override the default RPC endpoint
export SOROBAN_RPC_URL="https://soroban-testnet.stellar.org"

# Override the network passphrase
export STELLAR_NETWORK="Test SDF Network ; September 2015"

# Select the target network (testnet | mainnet | localnet)
export SOROBAN_NETWORK="testnet"
```

### Recommended: use a `.env` file (never committed)

```bash
# Copy the example env file
cp .env.localnet.example .env.testnet

# Edit and fill in your values
$EDITOR .env.testnet

# Load before running scripts
source .env.testnet
bash scripts/deploy.sh --network testnet
```

The `.gitignore` already excludes `.env.*` files (except `.env.*.example`).

---

## 4. Verifying Key Balance Before Deployment

Always confirm the deployer account is funded before deploying, especially on mainnet.

```bash
# Using Stellar CLI
stellar account show <DEPLOYER_PUBLIC_KEY> --network testnet

# Using Horizon REST API (testnet)
curl "https://horizon-testnet.stellar.org/accounts/<DEPLOYER_PUBLIC_KEY>" \
  | python3 -m json.tool | grep '"balance"' | head -5

# Using Horizon REST API (mainnet)
curl "https://horizon.stellar.org/accounts/<DEPLOYER_PUBLIC_KEY>" \
  | python3 -m json.tool | grep '"balance"' | head -5
```

Expected output includes a `native` (XLM) balance entry:

```json
{
  "balance": "25.0000000",
  "asset_type": "native"
}
```

If the account does not exist yet (HTTP 404 from Horizon), it needs to be created and funded.

---

## 5. Security Best Practices

### Key storage

- **Never commit** secret keys to version control — even in private repositories.
- Use **environment variables** or a secrets manager (Vault, AWS Secrets Manager, GCP Secret Manager) in CI/CD.
- For mainnet, prefer a **hardware security module (HSM)** or air-gapped key ceremony.
- Rotate deployer keys after each major deployment cycle.

### Least privilege

- The deployer key should have **only the XLM balance required for the deployment** — no more.
- Do not reuse the deployer key as an application hot wallet or operational account.
- For factory deployments, the deployer key is the initial `admin` — transfer admin rights to a multisig or governance contract immediately after deployment.

### CI/CD secrets

```yaml
# .github/workflows — reference the secret; never print it
- name: Deploy contracts
  env:
    DEPLOYER_SECRET_KEY: ${{ secrets.DEPLOYER_SECRET_KEY }}
  run: bash scripts/deploy.sh --network testnet
```

Add `DEPLOYER_SECRET_KEY` to **GitHub → Settings → Secrets and variables → Actions** before running the workflow.

### Post-deployment

- Revoke or drain the deployer key immediately after a mainnet deployment.
- Record the deployed contract IDs in `config/addresses.json` and open a PR for audit trail.

---

## Related Documents

- [Mainnet Deploy Checklist](mainnet-deploy-checklist.md)
- [Architecture Overview](architecture-overview.md)
- [scripts/deploy.sh](../scripts/deploy.sh) — deployment script with `--dry-run` support
- [scripts/fund-accounts.sh](../scripts/fund-accounts.sh) — testnet account funding helper
