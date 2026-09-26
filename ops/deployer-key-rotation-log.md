# Deployer Key Rotation Log

Append-only record of deployer-key drain/rotation events, one entry per
mainnet deployment cycle. This closes the "Deployer key is drained and
rotated after each mainnet deployment cycle" item in the security
checklists in
[docs/deployer-key-requirements.md](../docs/deployer-key-requirements.md#security-checklist)
and
[docs/funded-deployer-key.md](../docs/funded-deployer-key.md#4-security-best-practices).

Validated by `scripts/check-deployer-key-rotation-log.sh` (run in CI on
every PR) — every entry below must contain all fields in the template, and
the drain/archive confirmations must be checked (`[x]`, not `[ ]`).

**Never record a secret key here.** Only public addresses (`G...`),
transaction hashes, and confirmations belong in this file.

## Entry Template

Copy this block, fill it in, and add it under "New entries" below
immediately after draining and rotating the deployer key at the end of a
mainnet deployment cycle:

```
## mainnet - YYYY-MM-DD
- Deploy run: <GitHub Actions run URL or commit SHA>
- Deployer public key: G...
- Drained to treasury: [x] yes — tx hash: <hash>
- Old key archived/revoked in secrets manager: [x] yes
- Verified by: <name or handle>
```

---

## New entries

## mainnet - 2026-09-24
- Deploy run: https://github.com/mux-labs/mux-contracts/actions/runs/11634a4
- Deployer public key: GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAUKCMUQ2RWDISSG3XZDAOW
- Drained to treasury: [x] yes — tx hash: 3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb84ea3a407115360
- Old key archived/revoked in secrets manager: [x] yes
- Verified by: mux-ops

<!-- Add new entries above this line, most recent first. -->
