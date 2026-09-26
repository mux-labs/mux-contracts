# Deployer Key Security Playbook

**Status:** Required for Mux Soroban audit and mainnet readiness  
**Issue:** #682  
**Last Updated:** 2026-08-25

This playbook establishes mandatory security controls for funded deployer keys. Funded deployer keys must **never** live in git. All deployment workflows must enforce key scanning and require HSM or environment-based secret injection.

---

## Executive Summary

| Risk | Control |
|------|---------|
| Leaked secret keys in git history | Automated pre-commit and CI scanning |
| Hardcoded keys in source files | Environment variable injection only |
| Unrotated mainnet deployer keys | Mandatory rotation after each deploy cycle |
| Overprivileged deployer accounts | Dedicated accounts with minimal balance |

**Non-negotiable rules:**
1. Secret keys (`S...`) must never appear in git history — not even in private repos
2. All deployments must read secrets from environment variables or HSM
3. CI/CD must scan every commit for leaked credentials before merge
4. Mainnet deployer keys must be rotated immediately after deployment

---

## 1. Pre-Commit Key Scanning

### 1.1 Install the pre-commit hook

All developers must install the pre-commit key scanner before pushing changes:

```bash
# From repository root
bash scripts/install-git-hooks.sh

# Or manually
cp scripts/pre-commit-key-scan.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

The hook scans staged files for Stellar secret key patterns (`S[A-Z0-9]{55}`) and blocks commits containing secrets.

### 1.2 Test the hook

```bash
# Create a test file with a fake secret key
echo "DEPLOYER_PRIVATE_KEY=SBADKEYXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" > test.env

# Try to stage and commit
git add test.env
git commit -m "test secret scan"
# Expected: hook blocks the commit with an error message

# Clean up
git reset HEAD test.env
rm test.env
```

### 1.3 Bypass only when necessary

The hook can be bypassed with `--no-verify`:

```bash
# ONLY use this for legitimate keys in example files or test fixtures
git commit --no-verify -m "Add .env.example with placeholder keys"
```

**Never bypass the hook for real secret keys.** If you need to commit example keys, use clearly fake placeholders:

```
# GOOD: clearly fake
DEPLOYER_PRIVATE_KEY="SXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX..."

# BAD: looks real
DEPLOYER_PRIVATE_KEY="SBADKEYXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
```

---

## 2. CI Key Scanning

### 2.1 Automated scanning in CI

The CI workflow runs a comprehensive key scan on every PR before tests run:

```yaml
# .github/workflows/ci.yml
scan-secrets:
  name: Scan for leaked secrets
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0  # Full history for comprehensive scan
    - name: Scan git history for Stellar secret keys
      run: bash scripts/scan-git-secrets.sh
```

The scan checks:
- All tracked files in the current commit
- The last 100 commits in the branch history
- Common secret patterns: `S[A-Z0-9]{55}`, `SECRET_KEY=`, `PRIVATE_KEY=`

### 2.2 Scan results

| Exit code | Meaning | Action |
|-----------|---------|--------|
| 0 | No secrets found | ✓ Proceed |
| 1 | Secrets detected | ✗ Block merge; remediate immediately |
| 2 | Scan tool error | ✗ Fix scan script before merging |

If secrets are detected, the workflow fails and prints the offending commit SHA and file path. **Do not merge until remediation is complete.**

---

## 3. Secret Remediation Procedure

### 3.1 If a key is committed but not pushed

```bash
# Remove the key from the last commit (amend)
git reset HEAD~1
# Remove the file or redact the key
$EDITOR path/to/file
# Re-commit without the secret
git add path/to/file
git commit -m "Deploy config (secret removed)"
```

### 3.2 If a key is committed and pushed to a feature branch

```bash
# Rewrite the branch history to remove the secret
git rebase -i HEAD~5  # adjust the number to cover the commits with the secret
# In the interactive rebase, mark the offending commit with 'edit'
# When rebase pauses, remove the secret from the file
$EDITOR path/to/file
git add path/to/file
git commit --amend --no-edit
git rebase --continue
# Force-push the rewritten branch
git push --force-with-lease
```

### 3.3 If a key is merged to main

**This is a security incident.** Follow the incident response procedure:

1. **Rotate the key immediately** — the old key is considered compromised
2. **Rewrite main history** (requires org admin approval):
   ```bash
   git filter-repo --invert-paths --path path/to/leaked-file
   git push --force origin main
   ```
3. **Notify the security team** — document the incident in the security log
4. **Audit on-chain activity** — check if the compromised key signed any unexpected transactions

---

## 4. Environment Variable Injection

### 4.1 Allowed secret sources

| Environment | Allowed sources |
|-------------|----------------|
| Local dev (testnet) | Shell environment variables, `.env` files (gitignored) |
| CI/CD (testnet) | GitHub Actions secrets, environment variables |
| Mainnet | HSM, AWS Secrets Manager, GCP Secret Manager, Vault |

**Never allowed:**
- Hardcoded strings in `.sh`, `.ts`, `.rs`, or any source file
- Committed `.env` files (only `.env.*.example` is allowed)
- Plaintext secrets in CI logs

### 4.2 Local deployment (testnet)

```bash
# Copy the example env file
cp .env.deploy.example .env.deploy

# Edit with your test credentials
$EDITOR .env.deploy

# Load and deploy
source .env.deploy
bash scripts/deploy.sh --network testnet --dry-run  # validate first
bash scripts/deploy.sh --network testnet
```

Confirm `.env.deploy` is in `.gitignore`:

```bash
git check-ignore .env.deploy
# Expected: .env.deploy (file is ignored)
```

### 4.3 CI/CD deployment (testnet)

Add secrets to **GitHub → Settings → Secrets and variables → Actions**:

- `DEPLOYER_PRIVATE_KEY` — Stellar secret key (`S...`)
- `ADMIN_ADDRESS` — Stellar public key (`G...`)

Reference secrets in the workflow, never print them:

```yaml
- name: Deploy to testnet
  env:
    DEPLOYER_PRIVATE_KEY: ${{ secrets.DEPLOYER_PRIVATE_KEY }}
    ADMIN_ADDRESS: ${{ secrets.ADMIN_ADDRESS }}
  run: |
    set +x  # Disable command echoing
    bash scripts/deploy.sh --network testnet
```

**Never:**
```yaml
# BAD: prints the secret to CI logs
- run: echo "Key is $DEPLOYER_PRIVATE_KEY"

# BAD: debug mode reveals secrets
- run: set -x; bash scripts/deploy.sh
```

### 4.4 Mainnet deployment (HSM or secrets manager)

For mainnet, keys must be stored in a hardware security module (HSM) or enterprise secrets manager:

| Option | Use case | Setup guide |
|--------|----------|-------------|
| **AWS Secrets Manager** | AWS-hosted deployments | [AWS Secrets Manager setup](#51-aws-secrets-manager) |
| **GCP Secret Manager** | GCP-hosted deployments | [GCP Secret Manager setup](#52-gcp-secret-manager) |
| **Ledger hardware wallet** | Manual mainnet deploys | [Ledger integration](#53-ledger-hardware-wallet) |
| **HashiCorp Vault** | Multi-cloud or on-prem | [Vault setup](#54-hashicorp-vault) |

The deployment script must fetch the secret at runtime and never persist it to disk:

```bash
# AWS Secrets Manager example
export DEPLOYER_PRIVATE_KEY=$(aws secretsmanager get-secret-value \
  --secret-id mux/mainnet/deployer-key \
  --query SecretString --output text | jq -r .private_key)

# Deploy immediately; secret lives only in memory
bash scripts/deploy.sh --network mainnet

# Unset after deployment
unset DEPLOYER_PRIVATE_KEY
```

---

## 5. HSM and Secrets Manager Setup

### 5.1 AWS Secrets Manager

```bash
# Store the secret
aws secretsmanager create-secret \
  --name mux/mainnet/deployer-key \
  --secret-string "{\"private_key\":\"SXXX...XXX\",\"public_key\":\"GXXX...XXX\"}"

# Retrieve and use in deployment
DEPLOYER_PRIVATE_KEY=$(aws secretsmanager get-secret-value \
  --secret-id mux/mainnet/deployer-key \
  --query SecretString --output text | jq -r .private_key)

bash scripts/deploy.sh --network mainnet
```

**IAM policy for CI/CD:**

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["secretsmanager:GetSecretValue"],
    "Resource": "arn:aws:secretsmanager:us-east-1:123456789012:secret:mux/mainnet/deployer-key-*"
  }]
}
```

### 5.2 GCP Secret Manager

```bash
# Store the secret
echo -n "SXXX...XXX" | gcloud secrets create mux-mainnet-deployer-key --data-file=-

# Retrieve and use
DEPLOYER_PRIVATE_KEY=$(gcloud secrets versions access latest \
  --secret=mux-mainnet-deployer-key)

bash scripts/deploy.sh --network mainnet
```

**IAM binding for CI/CD:**

```bash
gcloud secrets add-iam-policy-binding mux-mainnet-deployer-key \
  --member="serviceAccount:ci-cd@project.iam.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"
```

### 5.3 Ledger Hardware Wallet

For manual mainnet deployments, use a Ledger Nano S/X:

```bash
# The Stellar CLI supports Ledger signing
stellar keys add deployer-ledger --ledger

# Deploy with the Ledger-backed key
bash scripts/deploy.sh --network mainnet --signer deployer-ledger
```

**Limitations:**
- Requires physical device connected during deployment
- Transaction signing requires manual confirmation on the device
- Not suitable for automated CI/CD workflows

### 5.4 HashiCorp Vault

```bash
# Store the secret in Vault KV v2
vault kv put secret/mux/mainnet/deployer \
  private_key="SXXX...XXX" \
  public_key="GXXX...XXX"

# Retrieve and use
DEPLOYER_PRIVATE_KEY=$(vault kv get -field=private_key secret/mux/mainnet/deployer)

bash scripts/deploy.sh --network mainnet
```

**Vault policy for CI/CD:**

```hcl
path "secret/data/mux/mainnet/deployer" {
  capabilities = ["read"]
}
```

---

## 6. Key Rotation Policy

### 6.1 Testnet rotation schedule

- **Recommended:** Rotate every 90 days or after a key compromise
- **Minimum:** Rotate annually

### 6.2 Mainnet rotation schedule

**Mandatory:** Rotate immediately after every mainnet deployment cycle.

A "deployment cycle" is a single coordinated deploy of one or more contracts to mainnet. Once the deployment is complete:

1. **Drain the deployer account** — transfer remaining XLM to treasury
2. **Generate a new deployer keypair** — store in secrets manager
3. **Revoke CI/CD access to the old secret** — delete from GitHub secrets or secrets manager
4. **Archive the old keypair** — log the public key and deployment timestamp in the security audit log

### 6.3 Rotation procedure

```bash
# 1. Drain the old deployer account
stellar tx new payment \
  --source-account "$OLD_DEPLOYER_SECRET" \
  --destination "$TREASURY_ADDRESS" \
  --asset native \
  --amount 49.9999 \
  --network mainnet

# 2. Generate a new deployer key
stellar keys generate deployer-v2 --network mainnet

# 3. Update the secrets manager
aws secretsmanager update-secret \
  --secret-id mux/mainnet/deployer-key \
  --secret-string "{\"private_key\":\"$(stellar keys show deployer-v2)\",\"public_key\":\"$(stellar keys address deployer-v2)\"}"

# 4. Archive the old key metadata (public key only)
echo "$(date -Iseconds) OLD_PUBLIC_KEY=$(stellar keys address deployer)" >> docs/key-rotation-log.txt
```

---

## 7. Scanning Script Reference

The repository includes automated scanning scripts:

### 7.1 `scripts/scan-git-secrets.sh`

Scans git history for Stellar secret keys and common secret patterns.

**Usage:**
```bash
# Scan the current commit
bash scripts/scan-git-secrets.sh

# Scan the last 100 commits
bash scripts/scan-git-secrets.sh --depth 100

# Scan all history
bash scripts/scan-git-secrets.sh --depth 0
```

**Exit codes:**
- `0` — No secrets found
- `1` — Secrets detected (prints offending commits)
- `2` — Usage error

### 7.2 `scripts/pre-commit-key-scan.sh`

Pre-commit hook that scans staged files for secrets.

**Patterns detected:**
- Stellar secret keys: `S[A-Z0-9]{55}`
- Environment variable assignments: `SECRET_KEY=`, `PRIVATE_KEY=`, `DEPLOYER_PRIVATE_KEY=`
- AWS access keys: `AKIA[A-Z0-9]{16}`

**Installation:**
```bash
bash scripts/install-git-hooks.sh
```

**Testing:**
```bash
# The hook blocks commits with secrets
echo "SECRET=SBADKEY..." > test.txt
git add test.txt
git commit -m "test"
# Expected: Error: Potential secret key detected
```

---

## 8. Security Audit Checklist

Use this checklist for pre-audit and pre-mainnet verification:

### Pre-commit controls
- [ ] Pre-commit hook is installed and tested (`scripts/install-git-hooks.sh`)
- [ ] Hook detects and blocks commits with Stellar secret keys
- [ ] Hook detects and blocks commits with AWS/GCP credentials

### CI controls
- [ ] CI workflow includes `scan-secrets` job before all other jobs
- [ ] `scan-secrets` job scans full git history (depth: 100+ commits)
- [ ] CI workflow blocks merge if secrets are detected
- [ ] CI workflow never prints `DEPLOYER_PRIVATE_KEY` or `ADMIN_ADDRESS` values

### Environment variable injection
- [ ] All deployment scripts read secrets from environment variables only
- [ ] No secrets are hardcoded in `.sh`, `.ts`, `.rs`, or `.yml` files
- [ ] `.env` and `*.secret` files are listed in `.gitignore`
- [ ] `.gitignore` rules are confirmed with `git check-ignore .env`

### Mainnet-specific controls
- [ ] Mainnet deployer key is stored in HSM or enterprise secrets manager
- [ ] Mainnet deployer key is never stored in GitHub Actions secrets as plaintext
- [ ] Mainnet deployer key is rotated immediately after each deployment cycle
- [ ] Old mainnet deployer accounts are drained to treasury after rotation

### Incident response
- [ ] Key rotation procedure is documented and tested
- [ ] Security team contact is documented in `SECURITY.md`
- [ ] Secret leak remediation procedure is documented (this playbook, section 3)

---

## 9. Related Documents

- [funded-deployer-key.md](funded-deployer-key.md) — Operational guide for deployer key setup and funding
- [deployer-key.md](deployer-key.md) — Basic deployer key setup walkthrough
- [deployer-key-requirements.md](deployer-key-requirements.md) — Quick-reference minimum requirements
- [MAINNET_DEPLOY_CHECKLIST.md](MAINNET_DEPLOY_CHECKLIST.md) — Pre-deploy verification steps
- [SECURITY.md](../SECURITY.md) — Vulnerability disclosure policy

---

## 10. Support

For security concerns or questions about this playbook:
- **Security incidents:** security@mux-protocol.xyz
- **General questions:** Open a GitHub discussion or contact the DevOps team
- **Key rotation assistance:** Tag `@mux-labs/security` in a private issue
