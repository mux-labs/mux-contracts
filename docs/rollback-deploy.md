# Rollback Deploy Notes

Soroban smart contracts are **immutable once deployed** — their bytecode cannot be changed or removed. "Rolling back" a Soroban deployment is therefore an operational procedure, not a single command: it means redirecting traffic to a previously deployed contract version and decommissioning the failed deployment.

This document describes the rollback strategies available, how to execute them, and what to record afterwards.

---

## Rollback Strategies

### Strategy 1 — Re-point via address config (preferred)

If the upgrade did not change the contract's stored state and the previous contract ID is still valid on-chain:

1. Identify the previous contract ID from `config/addresses.json` git history:
   ```bash
   git log --oneline -- config/addresses.json
   git show <PREV_COMMIT>:config/addresses.json | python3 -m json.tool | grep -A4 '"mainnet"'
   ```

2. Update `config/addresses.json` to point back to the previous contract ID.

3. Regenerate TypeScript bindings against the old contract ID:
   ```bash
   bash scripts/generate-bindings.sh --network mainnet --skip-build
   cd bindings && npm run build
   ```

4. Open a PR, get review, merge, and publish a new bindings patch release.

5. Notify dependent services to pick up the new bindings version.

**When to use:** The new contract has a bug but no state has been written to it by real users. Both contracts remain live on-chain; you are only changing which one clients reference.

---

### Strategy 2 — Deploy the previous WASM version

If the previous contract ID is no longer usable (e.g. a factory pattern deployed a per-user instance):

1. Check out the previous release tag:
   ```bash
   git checkout v<PREVIOUS_VERSION>
   ```

2. Build the old WASM:
   ```bash
   cargo build --target wasm32-unknown-unknown --release --workspace
   ```

3. Run a dry-run to verify the plan:
   ```bash
   DEPLOYER_SECRET_KEY=S... bash scripts/deploy.sh \
     --network mainnet \
     --dry-run \
     --contract <CONTRACT_NAME>
   ```

4. Deploy after confirming dry-run output:
   ```bash
   DEPLOYER_SECRET_KEY=S... bash scripts/deploy.sh \
     --network mainnet \
     --contract <CONTRACT_NAME>
   ```

5. Record the new contract ID in `config/addresses.json`, open a PR, merge, publish.

**When to use:** A fresh deployment of the known-good version is the cleanest path and the broken contract has not yet been used by real users.

---

### Strategy 3 — Admin pause / disable (state-preserving)

If users have already transacted with the new contract and their state must not be lost:

1. Call the admin `pause` or `set_active(false)` function if the contract implements it:
   ```bash
   stellar contract invoke \
     --id <BROKEN_CONTRACT_ID> \
     --network mainnet \
     --source <ADMIN_SECRET_KEY> \
     -- set_active --active false
   ```

2. Deploy the fixed version and migrate any required state via admin migration functions.

3. Re-enable the new contract after verifying the migration.

**When to use:** Users have already interacted with the broken contract and their on-chain state must be preserved. Requires the contract to implement an admin pause mechanism.

> **Note:** If the contract does not implement a pause mechanism, this strategy is unavailable. This is a strong reason to implement admin pause in every contract that handles user funds.

---

## Pre-Rollback Checklist

Before executing any rollback:

- [ ] Identify the exact failure: bug report, transaction hash, error code
- [ ] Determine which contract(s) are affected
- [ ] Confirm no user funds are at immediate risk (if yes, escalate immediately)
- [ ] Check whether the broken contract has been used by real users on-chain
- [ ] Identify the last known-good contract ID from `config/addresses.json` history
- [ ] Confirm the previous WASM build artifact is recoverable (git tag or CI artifact)
- [ ] Alert the on-call team and post in the incident channel

---

## Post-Rollback Steps

After the rollback is live:

- [ ] Update `config/addresses.json` with the restored contract IDs and open a PR
- [ ] Publish a new bindings patch release pointing to the restored contracts
- [ ] Write a brief incident report: what failed, why, how it was detected, how it was fixed
- [ ] Open a follow-up issue for the root cause fix with a regression test
- [ ] Add the broken contract ID to `docs/BREAKING_CHANGES.md` if it was ever publicly reachable

---

## Preventing the Need for Rollback

The best rollback is one you never need:

- Run `bash scripts/deploy.sh --dry-run` before every live deployment
- Complete the [Mainnet Deploy Checklist](mainnet-deploy-checklist.md) without exception
- Deploy to testnet and run integration smoke tests before touching mainnet
- Ensure every contract that handles user funds implements an admin pause function
- Keep `config/addresses.json` up to date so rollback targets are always traceable

---

## Related Documents

- [Mainnet Deploy Checklist](mainnet-deploy-checklist.md)
- [Deployer Key Setup](deployer-key.md)
- [BREAKING_CHANGES](BREAKING_CHANGES.md)
- [scripts/deploy.sh](../scripts/deploy.sh) — deployment script with `--dry-run`
- [.github/workflows/deploy.yml](../.github/workflows/deploy.yml) — GitHub Actions deploy workflow
