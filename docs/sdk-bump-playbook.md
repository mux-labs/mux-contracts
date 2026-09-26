# Soroban SDK Bump Playbook

**Status:** Production Runbook  
**Issue:** #852  
**Last Updated:** 2026-09-24  
**Audience:** Core maintainers, contract developers, on-call operators, and Stellar Wave contributors  

---

## 1. Executive Summary & Purpose

Mux Protocol provides invisible smart wallets and account abstraction on Stellar and Soroban. The smart contracts (`contracts/mux-*`) rely on the `soroban-sdk` Rust crate and its underlying host environment.

Upgrading `soroban-sdk` is **not a routine dependency bump**. It directly touches the execution environment, gas models, serialization rules, authorization semantics, storage rent/TTL behaviors, and cryptographic bindings across all 10 on-chain contracts.

An uncoordinated or improper SDK bump risks:
1. **Broken Account Abstraction (AA) or Wallet Operations:** Altered host functions or macro expansions can break signature verification, nonce handling, or session key validation.
2. **Loss of Funds or Stuck Storage:** Changes in Soroban storage interfaces (instance vs persistent storage, TTL extensions) can lead to premature entry expiration or griefing.
3. **WASM Bloat & Size Limits:** Unintentional inclusion of host test utilities (`testutils`) or unoptimized code in release WASM exceeding Stellar's strict bytecode limits.
4. **Supply Chain Incompatibilities:** Transitive dependencies (e.g. `ed25519-dalek` 2.x vs 3.x breaking `CryptoRng` with host 21) causing build or runtime failures.

This playbook establishes the mandatory, fail-closed procedure for evaluating, implementing, validating, and deploying `soroban-sdk` upgrades across Mux Protocol.

---

## 2. Invariants & Rules

Every SDK bump must maintain the following non-negotiable invariants:

| Invariant | Description | Enforcement Mechanism |
|---|---|---|
| **INV-SDK-01: Exact Version Pinning** | `soroban-sdk` must be pinned with exact `=version` in root `Cargo.toml`. Wildcards (`*`), carets (`^`), and tildes (`~`) are prohibited. | Verified in `Cargo.toml` and CI |
| **INV-SDK-02: Zero `testutils` in Release WASM** | Release WASM artifacts must never compile with `testutils` enabled. Host mock code in production WASM inflates bytecode and creates severe security hazards. | `scripts/check-no-testutils.sh` |
| **INV-SDK-03: Strict WASM Size Budgets** | Compiled WASM contracts must remain below the network limit (max 64 KB compressed target; < 128 KB uncompressed). | `scripts/check-contract-sizes.sh` |
| **INV-SDK-04: Transitive Crypto Pinning** | Transitive cryptographic crates (such as `ed25519-dalek`) must remain pinned to host-compatible versions (e.g. 2.x for host 21). | `Cargo.lock` review and compile check |
| **INV-SDK-05: Stable Error Codes & Auth** | Contract error enums (`#[contracterror]`) and authorization gates (`require_auth`) must retain their numeric values, serialization, and fail-closed behavior. | `docs/error_codes.md` and integration tests |
| **INV-SDK-06: Bindings Synchronization** | TypeScript SDK bindings (`bindings/`) must be regenerated and validated against the new contract interfaces with zero drift. | `scripts/generate-bindings.sh` & CI `check-binding-drift` |
| **INV-SDK-07: Safe Rollback Path** | Any deployment affecting mainnet contracts must maintain a documented rollback and feature-flag kill-switch strategy before merge. | PR review & `docs/rollback-guide.md` |

---

## 3. Pre-Requisites & Version Compatibility Matrix

Before initiating an SDK bump, verify compatibility across all layers:

```
┌─────────────────────────────────────────────────────────────┐
│ Stellar Network Protocol Version (e.g. Protocol 21 / 22)    │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ Soroban Host Environment (`soroban-env-host` / RPC)         │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ Rust Toolchain (`rust-toolchain.toml`, e.g. 1.80.0 / stable)│
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ `soroban-sdk = "=X.Y.Z"` (Workspace Root `Cargo.toml`)      │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ TypeScript Bindings (`@stellar/stellar-sdk` & `@mux-protocol`)│
└─────────────────────────────────────────────────────────────┘
```

### Checklist before bumping:
- [ ] Review the official Stellar / Soroban release notes and breaking changes log for the target SDK version.
- [ ] Check target network deployment: Is the target protocol version active on Testnet? On Mainnet?
- [ ] Inspect host environment dependencies: Check whether `soroban-env-common` or `soroban-env-host` have changed guest interface requirements.
- [ ] Verify `rust-toolchain.toml` compatibility with the new SDK version.

---

## 4. Step-by-Step Upgrade Workflow

### Phase 1: Branch Creation & Manifest Update

1. Create a dedicated bump branch:
   ```bash
   git checkout -b chore/soroban-sdk-bump-v<TARGET_VERSION>
   ```

2. Update the workspace dependency in the root `Cargo.toml`:
   ```toml
   [workspace.dependencies]
   # Pin exact SDK version according to docs/sdk-bump-playbook.md
   soroban-sdk = { version = "=<TARGET_VERSION>" }
   ```

3. Update the `Cargo.lock` file specifically for the SDK:
   ```bash
   cargo update -p soroban-sdk
   ```

4. Verify that transitive crypto dependencies remain on supported versions:
   ```bash
   cargo tree -p ed25519-dalek
   # Ensure no conflicting dalek 3.x is pulled that breaks CryptoRng on env-host 21
   ```

---

### Phase 2: Compilation & Codebase Adaptation

1. Run workspace syntax and type checks:
   ```bash
   cargo check --workspace --all-targets
   ```

2. Audit API diffs across all contract crates:
   - **Attribute macros:** Check for changes in `#[contract]`, `#[contractimpl]`, `#[contracttype]`, `#[contracterror]`.
   - **Storage APIs:** Verify methods on `env.storage().instance()`, `env.storage().persistent()`, `env.storage().temporary()`, and `extend_ttl()`.
   - **Authentication:** Verify `Address::require_auth()` and `Address::require_auth_for_args()`.
   - **Crypto / Hashing:** Check `env.crypto().sha256()`, `ed25519_verify()`, or secp256r1 APIs.
   - **Events & Logging:** Verify `env.events().publish()` topic format conventions match `docs/event-topic-conventions.md`.

3. Fix any deprecations or compile errors, ensuring all error codes and invariants remain fail-closed.

---

### Phase 3: WASM Release Build & Budget Verification

1. Compile all contracts in release mode to WASM:
   ```bash
   bash scripts/build-wasm.sh
   ```

2. Verify that `testutils` feature is absent from the release WASM:
   ```bash
   bash scripts/check-no-testutils.sh
   ```

3. Verify contract size budgets:
   ```bash
   bash scripts/check-contract-sizes.sh
   ```
   *If a contract exceeds the target budget, review optimization flags in `Cargo.toml` (`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`).*

4. Compute SHA-256 hashes for all compiled WASMs:
   ```bash
   bash scripts/compute-wasm-hashes.sh
   ```

---

### Phase 4: Bindings Regeneration & Validation

1. Regenerate TypeScript bindings:
   ```bash
   bash scripts/generate-bindings.sh
   ```

2. Check that generated bindings do not have unintended drift or breaking type changes:
   ```bash
   git diff bindings/
   ```

3. Build and test bindings:
   ```bash
   cd bindings
   npm run build
   npm test
   cd ..
   ```

---

### Phase 5: Automated Testing & Verification Suite

Execute the full testing matrix locally:

1. **Unit & Contract Suite:**
   ```bash
   cargo test --workspace --all-features
   ```

2. **Preflight & Doc Coverage Guards:**
   ```bash
   bash scripts/check-architecture-docs.sh
   bash scripts/check-security-policy.sh
   bash scripts/check-threat-model-coverage.sh
   bash scripts/check-upgrade-preflight.sh
   ```

3. **Coverage Check:**
   ```bash
   bash scripts/coverage.sh --stub
   ```

---

### Phase 6: Localnet / Testnet Deployment Validation

1. Start local Soroban environment:
   ```bash
   docker-compose up -d soroban-preview
   ```

2. Run local invoke smoke tests:
   ```bash
   bash scripts/local-invoke-smoke.sh
   ```

3. Deploy to Testnet using a funded testnet deployer key:
   ```bash
   bash scripts/deploy-testnet.sh
   ```

4. Verify interaction on Testnet:
   - Account initialization (`mux-account`)
   - Spend limit check (`mux-spending-policy`)
   - Daily limit check (`mux-policy`)
   - Batch execution (`mux-batcher`)
   - Recovery request creation (`mux-recovery`)

---

## 5. Rollback & Feature Flag Strategy

If an unexpected behavior, consensus mismatch, or RPC failure is detected post-upgrade:

1. **Pre-Deployment / Testnet:**
   - Immediately revert the bump PR:
     ```bash
     git revert <MERGE_COMMIT_SHA> -m 1
     ```
   - Re-pin previous `soroban-sdk = "=X.Y.Z"` in `Cargo.toml`.
   - Re-run `scripts/build-wasm.sh` and `scripts/generate-bindings.sh`.

2. **Post-Mainnet Deployment:**
   - Follow the procedures in [docs/rollback-guide.md](rollback-guide.md) and [docs/rollback-deploy.md](rollback-deploy.md).
   - If a specific contract was upgraded via WASM hash migration, invoke `upgrade()` with the previously verified WASM bytecode hash.
   - If state inconsistency is identified, use owner-authorized circuit breakers (`pause()`) on `mux-account` to prevent fund debits while diagnosing.
   - Record the rollback in [ops/rollback-log.md](../ops/rollback-log.md).

---

## 6. PR & Review Sign-Off Checklist

Any PR bumping `soroban-sdk` must include this completed checklist in the description:

- [ ] Target `soroban-sdk` version: `X.Y.Z`
- [ ] Network protocol version verified (Stellar Protocol N)
- [ ] `Cargo.toml` workspace dependency uses exact `=X.Y.Z` pin
- [ ] `Cargo.lock` updated cleanly with no conflicting transitive crypto dependencies
- [ ] All 10 contracts compile without warnings or errors
- [ ] `scripts/check-no-testutils.sh` passes (no mock host in WASM)
- [ ] `scripts/check-contract-sizes.sh` passes (under size budgets)
- [ ] TypeScript bindings regenerated via `scripts/generate-bindings.sh`
- [ ] Full automated test suite passes (`cargo test --workspace`)
- [ ] Localnet / Testnet smoke deployment successful
- [ ] Rollback strategy verified and recorded in PR description
