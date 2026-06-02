# Funded Deployer Key

This document covers how to generate, fund, and securely configure a deployer key for Mux Protocol Soroban contract deployments.

## Overview

Every contract deployment and upgrade requires a funded Stellar account to sign and pay transaction fees. This account is called the **deployer key**. It is separate from the contract admin key (which controls post-deploy operations).

| Key | Purpose | Required balance |
|-----|---------|-----------------|
| Deployer key | Signs `contract upload` and `contract deploy` transactions | ≥ 10 XLM per deploy |
| Admin key | Signs `initialize`, `upgrade`, and admin-gated calls | Small reserve only |

---

## 1. Generate a Deployer Keypair

Use the Stellar CLI to generate a new keypair:

```bash
stellar keys generate deployer --network testnet
stellar keys address deployer      # prints the public key (G...)
stellar keys show deployer         # prints the secret key (S...) — keep this safe
```

Or generate offline and store securely:

```bash
# Offline generation using stellar-base (Node.js)
node -e "const { Keypair } = require('@stellar/stellar-base'); const k = Keypair.random(); console.log('Public:', k.publicKey(), '\nSecret:', k.secret());"
```

---

## 2. Funding Requirements by Network

### Testnet

Fund for free with Friendbot:

```bash
# Via Stellar CLI
stellar keys fund deployer --network testnet

# Via curl
curl "https://friendbot.stellar.org?addr=$(stellar keys address deployer)"

# Via the repo funding script
bash scripts/fund-accounts.sh $(stellar keys address deployer)
```

Friendbot provides **10,000 XLM** — sufficient for hundreds of test deployments.

### Mainnet

Fund from an exchange or another funded account. Minimum recommended balance:

| Operation | Cost (approximate) |
|-----------|-------------------|
| Account activation (minimum reserve) | 1 XLM |
| Per `contract upload` | 0.1–0.5 XLM (fee + ledger rent) |
| Per `contract deploy` | 0.1–0.5 XLM |
| Safety buffer | 5 XLM |
| **Recommended starting balance** | **≥ 10 XLM** per contract |

Transfer XLM to the deployer public key from your exchange withdrawal or another Stellar account:

```bash
stellar tx new payment \
  --source-account $YOUR_FUNDED_ACCOUNT \
  --destination $(stellar keys address deployer) \
  --asset native \
  --amount 50 \
  --network mainnet
```

### Verify Balance Before Deploying

Always confirm the deployer account is funded before running a deployment:

```bash
# Via Stellar CLI
stellar account balance --account $(stellar keys address deployer) --network testnet

# Via Horizon API
curl "https://horizon-testnet.stellar.org/accounts/$(stellar keys address deployer)" \
  | jq '.balances[] | select(.asset_type == "native") | .balance'
```

The script will fail at the `stellar contract upload` step if the account has insufficient balance. Use `--dry-run` first to validate config without spending fees (see [deploy dry-run flag](../scripts/deploy.sh)).

---

## 3. Environment Variable Configuration

The deploy script reads the deployer key from environment variables — never hardcode secrets in files.

| Variable | Description | Required |
|----------|-------------|---------|
| `DEPLOYER_PRIVATE_KEY` | Stellar secret key (`S...`) for the deployer account | Yes (real deploy) |
| `ADMIN_ADDRESS` | Stellar public key (`G...`) that becomes contract admin | Yes (real deploy) |
| `SOROBAN_NETWORK` | Network alias: `testnet`, `mainnet`, `localnet` | No (default: `testnet`) |
| `RPC_URL` | Override Soroban RPC endpoint | No |

Set variables in your shell session:

```bash
export DEPLOYER_PRIVATE_KEY="SXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
export ADMIN_ADDRESS="GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
export SOROBAN_NETWORK="testnet"
```

Or pass inline for a single command:

```bash
DEPLOYER_PRIVATE_KEY=S... ADMIN_ADDRESS=G... bash scripts/deploy.sh --network testnet
```

---

## 4. Security Best Practices

### Never commit secrets

Add the following to `.gitignore` and confirm they are not tracked:

```
.env
.env.*
*.secret
deployment.env
deployer.json
```

Verify no secrets are staged:

```bash
git diff --cached | grep -iE "(secret|private_key|S[A-Z0-9]{55})"
```

### Use a secrets manager in CI

For GitHub Actions, store the key as a repository secret and reference it in the workflow:

```yaml
env:
  DEPLOYER_PRIVATE_KEY: ${{ secrets.DEPLOYER_PRIVATE_KEY }}
  ADMIN_ADDRESS: ${{ secrets.ADMIN_ADDRESS }}
```

Never print secrets in CI logs (`set +x` before exporting, avoid `echo $DEPLOYER_PRIVATE_KEY`).

### Use a dedicated deployer account

Do not reuse your personal or admin key as the deployer. A dedicated key:

- Limits blast radius if the key is compromised
- Makes audit logs easier to read (deployer actions are clearly distinct)
- Can be rotated without affecting contract admin permissions

### Rotate after mainnet deployment

Once contracts are deployed, rotate the deployer key:

1. Generate a new keypair (`stellar keys generate deployer-v2`).
2. Drain remaining XLM back to your treasury account.
3. Archive the old secret in your secrets manager or revoke CI access.

---

## 5. Related Resources

- [Deploy dry-run flag](../scripts/deploy.sh) — validate deployment config without spending fees
- [Mainnet deploy checklist](MAINNET_DEPLOY_CHECKLIST.md) — pre-deploy verification steps
- [Stellar account documentation](https://developers.stellar.org/docs/learn/fundamentals/stellar-data-structures/accounts)
- [Stellar CLI reference](https://developers.stellar.org/docs/tools/stellar-cli)
