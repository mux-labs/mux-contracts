# Mux Protocol Operations Runbook Index

**Status:** Authoritative Operations Directory  
**Issue:** #851  
**Last Updated:** 2026-09-24  
**Scope:** Soroban contract operations, key lifecycle, upgrades, emergency incident response, and keeper operations  

---

## 1. Operational Overview & Guiding Principles

This directory serves as the single source of truth for **Mux Protocol** operations on Stellar and Soroban. Mux Protocol powers non-custodial smart accounts, account abstraction, multi-operation batching, and delegated execution.

All operational activities must follow these mandatory principles:

1. **Fail-Closed Security:** In the event of network ambiguity, RPC outages, dependency failures, or incomplete execution, all operations must fail closed. No money-path action may assume success without verified on-chain inclusion.
2. **Deny-by-Default Access Control:** Privileged operations require explicit, verifiable authorization (stored owner, admin, guardian quorum, or signed session key). No operation may bypass authentication.
3. **Zero Plaintext Secrets in Repository or Logs:** Secret keys (`S...`), private keys, API secrets, and sensitive tokens must **never** be committed or logged. Secrets are injected at runtime via environment variables or hardware security modules (HSM).
4. **Append-Only Operational Logs:** Critical operational events (deployer key drain/rotation, contract rollback, emergency pauses) must be recorded in designated append-only markdown logs in `ops/` and committed to git.
5. **Correlation Identifiers & Traceability:** Multi-step operational procedures must record transaction hashes, ledger sequence numbers, and run IDs for end-to-end traceability.

---

## 2. Operations Runbook Directory

### 2.1 Deployment & Release Management

| Runbook / Document | Description | Key Scripts |
|---|---|---|
| [Mainnet Deploy Checklist](../docs/MAINNET_DEPLOY_CHECKLIST.md) | Comprehensive step-by-step checklist before, during, and after mainnet deployments. | `scripts/deploy.sh` |
| [WASM Build & Usage](../docs/wasm-build-usage.md) | Compilation instructions for deterministic release WASM bytecode. | `scripts/build-wasm.sh` |
| [Release Profile Verification](../docs/release-profile-verification.md) | Validation of compiler flags (`opt-level = "z"`, `overflow-checks = true`, `panic = "abort"`). | `Cargo.toml` |
| [No Testutils in Release WASM](../docs/no-testutils-wasm.md) | Guarantees host mock code is excluded from deployable WASM. | `scripts/check-no-testutils.sh` |
| [Contract IDs Sync](../docs/contract-ids.md) & [CONTRACT_IDS.md](../CONTRACT_IDS.md) | Authoritative mapping of deployed contract IDs across testnet and mainnet. | `scripts/check-contract-ids-sync.sh` |
| [Contract Size Monitoring](../docs/storage-griefing.md) | Bytecode size budget checks against Soroban limits. | `scripts/check-contract-sizes.sh` |

---

### 2.2 Key Management & Secrets Hygiene

| Runbook / Document | Description | Validation & Logging |
|---|---|---|
| [Deployer Key Security Playbook](../docs/deployer-key-security-playbook.md) | Mandatory controls for secret key handling, pre-commit scanners, and CI safeguards. | `scripts/pre-commit-key-scan.sh`<br>`scripts/scan-git-secrets.sh` |
| [Deployer Key Requirements](../docs/deployer-key-requirements.md) | Operational guidelines for funding, minimal balance allocation, and key revocation. | `scripts/check-deploy-secret-name.sh` |
| [Funded Deployer Key Runbook](../docs/funded-deployer-key.md) | Safe procedures for provisioning and draining temporary deployer accounts. | `scripts/fund-accounts.sh` |
| [Deployer Key Rotation Log](deployer-key-rotation-log.md) | Append-only record of deployer key draining and rotation post-deployment. | `scripts/check-deployer-key-rotation-log.sh` |

---

### 2.3 Contract Upgrades, Migrations & Rollbacks

| Runbook / Document | Description | Applicability |
|---|---|---|
| [Contract Upgrade Pattern](../docs/contract-upgrade-pattern.md) | Standard procedure for migrating Soroban contract bytecode via `upgrade()`. | `mux-batcher`, `mux-permissions`, `mux-policy`, etc. |
| [Upgrade Auth Requirements](../docs/upgrade-auth-requirements.md) | Access control constraints and multi-signature gates for contract upgrades. | `scripts/check-upgrade-preflight.sh` |
| [Rollback Guide](../docs/rollback-guide.md) & [Rollback Deploy](../docs/rollback-deploy.md) | Emergency procedures for contract rollback: redeploy, bytecode reversion, and circuit breaker. | All deployed contracts |
| [Rollback Execution Log](rollback-log.md) | Append-only audit record of emergency rollback executions. | `scripts/check-rollback-log.sh` |
| [Account Immutability & Upgrade Decision](../docs/account-upgrade-migration.md) | Policy documentation confirming `mux-account` is permanently immutable. | `mux-account` |
| [Permissions Upgrade Migration](../docs/permissions-upgrade-migration.md) | Storage layout evolution and migration steps for RBAC. | `mux-permissions` |
| [Delegation Upgrade Model](../docs/delegation-upgrade.md) | Upgrading delegation state without breaking active delegate grants. | `mux-delegation` |

---

### 2.4 Emergency Response, Circuit Breakers & Incident Handling

| Runbook / Document | Description | Primary References |
|---|---|---|
| [Threat Model & Trust Boundaries](../docs/threat-model.md) | Living threat model covering all 10 contracts, STRIDE matrix, and mitigations. | `tests/threat_model_coverage.rs` |
| [Pause / Freeze Architecture Decision](../docs/pause-freeze-decision.md) | Architectural Decision Record on per-account emergency pause and rejection of global admin backdoors. | `docs/pause-freeze-decision.md` |
| [Account Recovery Model](../docs/recovery-trust-model.md) | M-of-N guardian recovery procedures, timelock windows, and recovery cancellation. | `contracts/mux-recovery` |
| [Access Control Review Checklist](../docs/access-control-checklist.md) | Pre-audit and pre-deployment access control verification matrix. | `docs/access-control-checklist.md` |
| [Audit Preparation & Scope](../docs/audit-prep.md) | Entry point inventory and security review checklist for external auditors. | `docs/audit-prep.md` |
| [Audit Event Conventions](../docs/audit-events.md) | Canonical Soroban event topics and monitoring hooks for security operations. | `docs/event-topic-conventions.md` |
| [Security Policy & SLA](../SECURITY.md) | Vulnerability disclosure policy, private reporting channels, and response timeline SLA. | `scripts/check-security-policy.sh` |
| [Security Acknowledgments](../docs/security-acknowledgments.md) | Hall of fame and researcher recognition criteria under RFC 9116. | `.well-known/security.txt` |

---

### 2.5 Storage Maintenance, Rent & Keepers

| Runbook / Document | Description | Monitoring & Execution |
|---|---|---|
| [Storage Choices & Rationale](../docs/storage-choices.md) | Instance vs persistent storage architectural decisions. | Storage layout review |
| [Storage Griefing & TTL Keepers](../docs/storage-griefing.md) | Collection sizing caps, rent calculation, and automated TTL keeper scheduling. | `scripts/test-ttl-keeper.sh` |

---

### 2.6 Developer Operations & Infrastructure

| Runbook / Document | Description | Automation |
|---|---|---|
| [Soroban SDK Bump Playbook](../docs/sdk-bump-playbook.md) | Comprehensive playbook for updating `soroban-sdk` dependencies safely. | `Cargo.toml` |
| [TypeScript Bindings Generation](../docs/bindings-error-mapping.md) | Generating, testing, and packaging client bindings from compiled WASM. | `scripts/generate-bindings.sh` |
| [NPM Publishing Playbook](../docs/npm-publish.md) | Publishing `@mux-protocol/contracts` with npm provenance attestation. | `.github/workflows/bindings.yml` |
| [AA Backend Orchestrator Integration](../docs/aa-backend-orchestrator.md) | Relayer integration, gas sponsorship, and session key dispatch architecture. | `docs/relayer-integration.md` |
| [Developer Onboarding Guide](../docs/developer_onboarding.md) | Local development environment setup, docker compose preview, and testing. | `docker-compose.yml` |

---

## 3. Incident Severity & Escalation Matrix

When an operational incident occurs, on-call operators must follow this escalation path:

| Severity | Operational Trigger | Response SLA | Initial Action |
|---|---|---|---|
| **SEV-1 (Critical)** | Active exploit, funds at risk, or contract takeover. | Immediate (< 1 hour) | Trigger account owner circuit breaker (`pause()`), notify multisig signers, initiate incident channel. |
| **SEV-2 (High)** | Stuck user funds, keeper TTL expiry risk, or RPC outage affecting writes. | < 4 hours | Engage infrastructure providers, execute keeper TTL extension, or switch RPC fallbacks. |
| **SEV-3 (Medium)** | Non-critical bug, degraded relayer performance, or rate-limiting. | < 24 hours | Re-route relayer traffic, adjust batching thresholds, triage issue. |
| **SEV-4 (Low)** | Minor documentation discrepancy, non-impacting telemetry drift. | Normal sprint | File issue and schedule fix in regular release cycle. |

---

## 4. Operational Sign-Off Protocol

Every operational procedure executed in production requires:
1. Two operator reviews (peer check on commands and network arguments).
2. Verification on localnet or testnet before running against mainnet.
3. Verification of transaction status via Horizon / Soroban RPC after broadcast.
4. Immediate logging of the action in the appropriate log file in `ops/` with the transaction hash and operator identifier.
