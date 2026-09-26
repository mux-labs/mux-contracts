# Release Profile Panic Abort Verification

**Version:** 0.1.0  
**Date:** 2026-09-25  
**Status:** Verified (enforced in CI, #780)  
**Related:** [Storage Griefing Notes](storage-griefing.md), [Audit Prep](audit-prep.md)

---

## Purpose

This document verifies that the Mux Protocol workspace Cargo.toml release profile is correctly configured for Soroban WASM contract deployment, with particular focus on `panic = "abort"` and related hardening settings.

---

## Current release profile

**File:** `Cargo.toml` (workspace root)

```toml
[profile.dev]
overflow-checks = true

[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true

[profile.release-with-logs]
inherits = "release"
debug-assertions = true
```

---

## Verification checklist

| Setting | Value | Status | Rationale |
|---------|-------|--------|-----------|
| `panic = "abort"` | `"abort"` | **Verified** | No stack unwinding in WASM; smaller binary, deterministic abort on panic |
| `opt-level = "z"` | `"z"` | **Verified** | Minimises binary size — critical for WASM deployment cost on Stellar |
| `overflow-checks = true` | `true` | **Verified** | Arithmetic overflow panics in both dev and release; prevents silent wrapping |
| `debug = 0` | `0` | **Verified** | No debug info in release binary; reduces WASM size |
| `strip = "symbols"` | `"symbols"` | **Verified** | Strips all symbols; reduces WASM size further |
| `debug-assertions = false` | `false` | **Verified** | No debug assertions in release; expected for production |
| `codegen-units = 1` | `1` | **Verified** | Single codegen unit enables maximum optimisation across the crate |
| `lto = true` | `true` | **Verified** | Link-time optimization across all crates; smaller and faster output |

---

## Why `panic = "abort"` matters for Soroban

1. **No unwinding in WASM** — The `wasm32-unknown-unknown` target does not support stack unwinding. `panic = "abort"` is the only viable option and avoids linking the unwinding runtime.

2. **Binary size** — Abort-on-panic produces a smaller WASM binary because no unwind tables or cleanup code is emitted.

3. **Deterministic behaviour** — On panic, the contract aborts immediately. There is no partially-executed state to reason about; the transaction fails atomically.

4. **Security** — Unwinding could theoretically execute destructors that interact with storage in unexpected ways. Abort eliminates this surface.

---

## `overflow-checks = true` rationale

Soroban arithmetic operations on `i128` values are common in financial contracts. With `overflow-checks = true`:

- Integer overflow/underflow panics immediately (which aborts the contract call)
- No silent wrapping of balances or amounts
- This setting is enforced in **both** `dev` and `release` profiles in the Mux workspace

---

## `release-with-logs` profile

A derived profile for diagnostic builds:

```toml
[profile.release-with-logs]
inherits = "release"
debug-assertions = true
```

Use this profile when:
- Debugging production issues on testnet
- Verifying assertion-heavy code paths
- Testing edge cases that trigger `debug_assert!` macros

Build with:
```bash
cargo build --target wasm32-unknown-unknown --profile release-with-logs
```

---

## Verification commands

To independently verify these settings are active in a built WASM:

```bash
# Build release WASM
cargo build --target wasm32-unknown-unknown --release -p mux-account-factory

# Check binary size (should be small with opt-level=z + strip)
ls -lh target/wasm32-unknown-unknown/release/mux_account_factory.wasm

# Verify no debug symbols (strip = "symbols")
wasm-objdump -x target/wasm32-unknown-unknown/release/mux_account_factory.wasm | grep -c "name"

# Verify panic=abort by checking for unwind sections (should be none)
wasm-objdump -x target/wasm32-unknown-unknown/release/mux_account_factory.wasm | grep -i "unwind"
```

---

## CI integration

Release profile verification is enforced, fail-closed, by
`scripts/check-release-profile.sh` (#780). It runs in:

| Workflow | Step | When |
|----------|------|------|
| `.github/workflows/ci.yml` | `Verify release profile invariants` | Before the wasm build (also runs `scripts/test-check-release-profile.sh`) |
| `.github/workflows/ci.yml` | `Build contracts (wasm)` with `RUSTFLAGS="-D warnings"` | Release compilation must be warning-free |
| `.github/workflows/ci.yml` | `Verify release wasm artifacts` | After the build, before the size budget and upload |
| `.github/workflows/deploy.yml` | Same three gates in the `build` job | Before deployable WASMs are uploaded |

### What the check enforces

1. **Profile invariants** — every row of the [verification checklist](#verification-checklist)
   must be present in `[profile.release]` with exactly the listed value.
   Missing keys, different values, keys set twice, or a second
   `[profile.release]` table all fail.
2. **No per-crate overrides** — `[profile.release.package.*]` and
   `[profile.release.build-override]` tables are rejected, because they can
   weaken the invariants for a single contract without touching the main table.
3. **Artifact checks** (`--wasm-dir`) — every `*.wasm` in the release output
   must carry the WASM magic header and contain no DWARF `.debug_*` sections.
   An empty or missing artifact directory fails.

### Artifact publication

The `wasm-artifacts` upload in `ci.yml` runs only when every preceding gate
succeeded (`if: success()`, `if-no-files-found: error`), so a non-conforming
or unoptimised WASM never becomes a downloadable CI artifact. In `deploy.yml`
the `Upload WASMs` step already runs only after the build job's gates pass,
and the `deploy` job requires `needs.build.result == 'success'` (or an
explicit `skip_build`).

### Running locally

```bash
# Profile invariants only
bash scripts/check-release-profile.sh

# Profile + artifact checks (after `make wasm`)
make check-release-profile

# Self-tests for the checker (conforming and non-conforming fixtures)
bash scripts/test-check-release-profile.sh
```

### Changing the release profile

Any change to `[profile.release]` must update, in the same PR:

1. the [verification checklist](#verification-checklist) above,
2. the `REQUIRED` map in `scripts/check-release-profile.sh`, and
3. the `GOOD_PROFILE` fixture in `scripts/test-check-release-profile.sh`.

### Rollback

The gates only add checks; they do not change build output. If a gate
misfires, revert the commit that introduced it (or remove the step from the
workflow) — the `[profile.release]` table itself is unchanged by #780.

---

## Known Soroban-specific considerations

| Concern | How the profile addresses it |
|---------|------------------------------|
| WASM doesn't support unwinding | `panic = "abort"` eliminates unwinding requirement |
| Deployment cost scales with binary size | `opt-level = "z"` + `strip = "symbols"` + `lto = true` minimise size |
| Arithmetic bugs in financial logic | `overflow-checks = true` catches them at runtime |
| Debug info leaking into production | `debug = 0` strips all debug sections |
| Suboptimal code generation | `codegen-units = 1` + `lto = true` enable full cross-crate optimisation |

---

## Related

- [Storage Griefing Notes](storage-griefing.md) — collection caps and TTL management
- [Audit Prep](audit-prep.md) — pre-audit build verification
- [Rust Cargo Profile Reference](https://doc.rust-lang.org/cargo/reference/profiles.html)
