# Contributing to mux-contracts

Thanks for contributing to Mux Protocol's Soroban contracts. This guide covers the
basics; for deeper protocol context see the canonical docs linked below.

## Canonical documentation

- [`README.md`](./README.md) — repo overview, build/test instructions, and layout.
- [`SECURITY.md`](./SECURITY.md) — vulnerability disclosure and security policy.
- [`CONTRACT_IDS.md`](./CONTRACT_IDS.md) — deployed contract IDs per network.
- [`Somzilla.md`](./Somzilla.md) — status/audit notes for the Somzilla review.
  This file is a status document only; where it disagrees with `README.md`,
  `SECURITY.md`, or `CONTRACT_IDS.md`, those canonical docs win.

## Getting started

1. Fork and clone the repository.
2. Install the Rust toolchain and `soroban-cli` per `README.md`.
3. Build and run the test suite as described in `README.md`.

## CI: wasm size budget and artifacts

The CI workflow (`.github/workflows/ci.yml`) enforces a **wasm size budget** on
every compiled contract. The build fails closed if any `*.wasm` exceeds the
configured limit, so oversized contracts cannot land unnoticed.

- The budget is defined by the `WASM_SIZE_BUDGET_BYTES` environment variable in
the workflow (default `65536` bytes / 64 KiB per contract).
- To adjust the budget, change that value in `.github/workflows/ci.yml` and
explain the rationale in your PR description.
- Built wasm artifacts are uploaded from each CI run as the `wasm-artifacts`
artifact, so contributors and reviewers can download and inspect them directly
from the workflow run page.

If a contract legitimately needs more space, raise the budget in the same PR
that grows the contract and note the reason; do not bypass the check.

## Pull requests

- Keep changes scoped to a single issue; avoid unrelated refactors.
- Include tests for new behavior and authz/idempotency negatives where relevant.
- Update docs (`README.md`, `SECURITY.md`, `CONTRACT_IDS.md`, `Somzilla.md`)
  when behavior or status changes so they stay consistent.
- Do not commit secrets, keys, JWTs, or webhook secrets.

## Reporting security issues

pace, raise the budget in the same PR
that grows the contract and note the reason; do not bypass the check.

## Pull requests

- Keep changes scoped to a single issue; avoid unrelated refactors.
- Include tests for new behavior and authz/idempotency negatives where relevant.
- Update docs (`README.md`, `SECURITY.md`, `CONTRACT_IDS.md`, `Somzilla.md`)
  when behavior or status changes so they stay consistent.
- Do not commit secrets, keys, JWTs, or webhook secrets.

## Reporting security issues

Do not open public issues for vulnerabilities. Follow the process in
[`SECURITY.md`](./SECURITY.md).

## Pull Request Process

1. **Reference an issue** — PRs should reference GitHub issues: "Closes #42"
2. **Include changelog entry** — Add your changes to the unreleased section of CHANGELOG.md following the [changelog template](.github/CHANGELOG_TEMPLATE.md)
3. **Describe the change** — Explain what changed, why, and how to test it
4. **Ensure tests pass** — Run `cargo test --workspace --all-features` locally before pushing
5. **Request review** — Assign reviewers based on the files changed

## Changelog Guidelines

Every PR must include a changelog entry. See [CHANGELOG_TEMPLATE.md](.github/CHANGELOG_TEMPLATE.md) for detailed guidelines.

**Quick reference:**
- **Added** — New features
- **Changed** — Improvements to existing functionality (backwards compatible)
- **Fixed** — Bug fixes
- **Removed** — Breaking changes (require major version bump)
- **Deprecated** — Upcoming removals
- **Security** — Security patches

Example entry:
```markdown
### Added
- `execute_with_session()` function for session-key-authenticated transactions (#23)

### Fixed
- Session key validation now correctly handles zero timestamps (#25)
```

## Contract PR Guidelines

All PRs that modify Soroban contract code under `contracts/` must satisfy the following before merge.

### `no_std` Safety

Every contract crate is `#![no_std]`. Do **not** add `std` imports or any dependency
that pulls in the standard library. The WASM target (`wasm32-unknown-unknown`) does
not provide `std`.

- Use `soroban_sdk` types (`Vec`, `Map`, `String`, `BytesN`, …) instead of `alloc` /
  `std` collections where possible.
- If you genuinely need `alloc` (e.g. `Vec` in a non-Soroban context), gate it behind
  `extern crate alloc;` and ensure the crate compiles with `--target wasm32-unknown-unknown`.
- Verify with `cargo build --target wasm32-unknown-unknown --release -p <crate>` before pushing.

### Error Enums

Every contract **must** define a single `#[contracterror]` enum in its root `lib.rs`.

```rust
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MyContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    // … contract-specific variants
}
```

Rules:
- Variants are `#[repr(u32)]` with unique codes. Start at `1` and increment sequentially.
- Always include `NotInitialized` (1), `AlreadyInitialized` (2), and `Unauthorized` (3)
  where applicable — these map to standard HTTP status codes in the TypeScript bindings.
- Do **not** reuse codes across contracts; each contract owns its own code space.
- Add a brief doc comment on every variant explaining when it is returned.
- After adding or changing variants, update:
  - `docs/error_codes.md` — canonical Rust-side reference
  - `bindings/src/types.ts` — the TS union type and `*ErrorMessage` map
  - `bindings/src/errors.ts` — the `ERROR_HTTP_MAP` entry for the new variant

### Storage Bounds

All collection-backed storage (Vec, Map) **must** have an explicit cap to prevent
storage griefing. Use a `const MAX_*: u32` constant and return a dedicated error
when the cap is reached.

```rust
const MAX_WALLETS: u32 = 256;

if wallet_names.len() >= MAX_WALLETS {
    return Err(MuxPolicyError::TooManyWallets);
}
```

Document the cap value and rationale in a comment next to the constant.

### TTL Management

Persistent storage entries **must** call `extend_ttl` on every write so that active
data survives beyond the default ledger TTL. Follow the existing pattern:

```rust
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

env.storage()
    .persistent()
    .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
```

Instance storage should also be extended after any state-mutating function.

### Unit Tests

Every public function must have at least one unit test. Tests live in a `#[cfg(test)] mod tests` block at the bottom of the contract's `lib.rs`.

Minimum coverage per contract:
- Happy-path for every public entry point.
- Each error variant returned at least once.
- Boundary / edge cases (zero amounts, overflow, capacity limits).
- Event emission checks where events are emitted.

Run `cargo test --package <crate>` and `cargo clippy --package <crate>` before
pushing. The CI also runs `cargo test --workspace --all-features`.

### Checklist

Before requesting review on a contract PR:

- [ ] `#![no_std]` — no `std` imports
- [ ] `cargo build --target wasm32-unknown-unknown --release -p <crate>` succeeds (or `make wasm` for all contracts)
- [ ] Error enum follows the convention (single `#[contracterror]`, `#[repr(u32)]`, codes start at 1)
- [ ] `docs/error_codes.md` updated for new or changed error variants
- [ ] TypeScript bindings regenerated (`make bindings` or `bash scripts/generate-bindings.sh`)
- [ ] `bindings/src/types.ts` union type and error-message map updated
- [ ] `bindings/src/errors.ts` HTTP map updated for new variants
- [ ] All collection storage has a cap (`MAX_*` constant + `TooMany*` error)
- [ ] Persistent storage entries call `extend_ttl` on write
- [ ] Unit tests cover happy path, each error variant, and edge cases
- [ ] `cargo clippy --workspace --all-features` is clean (or `make lint`)
- [ ] `cargo fmt --check` passes (or `make fmt`, format via `make fmt-fix`)
- [ ] Workspace test suite passes: `cargo test --workspace --all-features` (or `make test`)

## Code Style

### Rust

- **Format** — Run `cargo fmt` before committing (or `make fmt-fix`)
- **Lint** — Run `cargo clippy` and fix warnings (or `make lint`)
- **Comments** — Add doc comments (`///`) to public functions and types
- **Tests** — All new public functionality must have unit tests
- **Error Handling** — Use Result types; avoid unwrap() in library code

### Documentation

- **README** — Keep up-to-date with new features
- **Inline Comments** — Explain *why*, not *what* (code explains what)
- **Public APIs** — Document with examples in doc comments
- **Architecture** — Document design decisions in `docs/` directory

## Testing & Makefile Reference

The root `Makefile` provides standardized targets mirroring the CI checks:

| Target | Command | Description |
|---|---|---|
| `make all` | `fmt`, `lint`, `build`, `test` | Run all standard pre-push checks |
| `make build` | `cargo build --workspace --all-targets` | Compile all workspace targets |
| `make test` | `cargo test --workspace --all-features` | Run complete test suite with all features enabled |
| `make test-unit` | `cargo test --lib` | Run unit tests across workspace libraries |
| `make fmt` | `cargo fmt --all -- --check` | Verify code formatting |
| `make fmt-fix` | `cargo fmt --all` | Automatically format code |
| `make lint` / `make clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Run Clippy linter with strict warning denial |
| `make wasm` | `bash scripts/build-wasm.sh --release` | Build release WASM artifacts |
| `make check-sizes` | `bash scripts/check-contract-sizes.sh` | Verify contract sizes against budget |
| `make bindings` | `bash scripts/generate-bindings.sh` | Generate TypeScript contract bindings |
| `make deny` | `cargo deny check` | Supply-chain advisory and license check |
| `make coverage` | `bash scripts/coverage.sh` | Measure LLVM source coverage |
| `make test-coverage` | `bash scripts/test-coverage.sh` | Validate coverage script stub behavior |

- **Unit Tests** — Run `make test-unit` or `cargo test --lib`
- **All Tests** — Run `make test` or `cargo test --workspace --all-features`
- **Integration Tests** — Require localnet setup (see README.md)
- **Coverage** — Aim for >90% coverage on new code. Generate a report with
  `make coverage` or `bash scripts/coverage.sh` (add `--html` / `--lcov` as needed).
  If `llvm-tools-preview` is not installed, the script prints a **coverage report stub**
  listing workspace crates; validate the stub with `make test-coverage` or `bash scripts/test-coverage.sh`.

## Cargo.lock Policy

This repository **commits `Cargo.lock`** and keeps it under version control.

- **Why** — Soroban WASM builds must be reproducible for audits, CI cache keys, and
  mainnet deploy checklists (`docs/MAINNET_DEPLOY_CHECKLIST.md`). Pinning transitive
  crates via the lockfile reduces supply-chain drift between developers and CI.
- **Do** — Commit lockfile updates in the same PR that bumps dependencies in
  `Cargo.toml` / workspace members. Run `cargo update -p <crate>` (or a full
  `cargo update` when intentional) and include the resulting `Cargo.lock` diff.
- **Do not** — Add `Cargo.lock` to `.gitignore`, delete it from the tree, or regenerate
  it casually without reviewing the diff (`cargo deny check` is recommended after
  dependency changes).
- **CI** — Workflows hash `Cargo.lock` for cache keys; keep the committed file in sync
  with what CI builds.

Example test:
```rust
#[test]
fn test_execute_with_session_succeeds_for_registered_key() {
    let (env, client, owner) = setup();
    let session_key = Address::generate(&env);
    let expires_at = env.ledger().timestamp() + 3600;
    // A key must be granted at least one scope; an empty list fails closed.
    let scopes = vec![
        &env,
        Scope {
            method: symbol_short!("ping"),
        },
    ];
    let target = env.register_contract(None, ExecuteTarget);

    client.register_session_key(&session_key, &expires_at, &scopes);
    let _ = client.execute_with_session(
        &session_key,
        &target,
        &symbol_short!("ping"),
        &Vec::new(&env),
    );
}
```

`register_session_key` takes `(session_key, expires_at, scopes)` — the owner is
read from stored account state and must `require_auth()`, so it is not passed
explicitly. Validity can be checked directly via the `is_session_key_valid(session_key)`
read-only query, and is also checked internally by `execute_with_session` (see
[`docs/entrypoint-matrix.md`](docs/entrypoint-matrix.md) for the full list of
`mux-account` entrypoints).

## Security

### Reporting Vulnerabilities

**Do not open public issues for security vulnerabilities.**

Instead, open a private security advisory:
1. Go to the Security tab
2. Click "Report a vulnerability"
3. Describe the issue and provide steps to reproduce

We will investigate and provide a patch before public disclosure.

### Security Checklist

Before submitting code that touches authorization, storage, or cryptographic operations:

- [ ] Access control is enforced (use `require_auth()`)
- [ ] No integer overflows or underflows
- [ ] Storage keys cannot be manipulated by untrusted input
- [ ] Error messages don't leak sensitive information
- [ ] Timestamp dependencies are explicit and documented
- [ ] All assumptions are validated

See [Access Control Review Checklist](docs/access-control-checklist.md) for details.

## Breaking Changes

Breaking changes require:

1. **Major version bump** (e.g., 1.0.0 → 2.0.0)
2. **Clear migration guide** in CHANGELOG.md
3. **Advance notice** — Deprecate in N-1 release if possible
4. **Documentation** — Update all relevant docs

Example breaking change:
```markdown
### Removed
- **BREAKING:** `pay(asset, amount)` signature changed to `pay(asset, amount, metadata)` (#48)

  **Migration:** See [migration guide](docs/migration-v2.md)
```

## Generating TypeScript Bindings

TypeScript bindings are auto-generated from compiled contract WASMs using the Stellar CLI. Two scripts are available:

**Shell script** (CI-friendly, no Node.js required):
```bash
bash scripts/generate-bindings.sh [--network testnet] [--skip-build]
# or via npm
cd bindings && npm run generate
```

**TypeScript script** (richer flags, programmatic use):
```bash
npx ts-node scripts/generate-bindings.ts [options]
# or via npm
cd bindings && npm run generate:bindings
```

Options for the TypeScript script:

| Flag | Description | Default |
|------|-------------|---------|
| `--network <name>` | Stellar network (`testnet`\|`mainnet`\|`localnet`) | `testnet` |
| `--skip-build` | Skip `cargo build`; use pre-built WASMs | false |
| `--contract <name>` | Generate bindings for a single contract | all contracts |
| `--dry-run` | Print commands without executing | false |

Generated files are written to `bindings/src/generated/` and should not be edited by hand. Re-run either script after changing contract interfaces.

## Documentation

- **README.md** — Main entry point; keep concise and updated
- **docs/** — Detailed guides on architecture, design decisions, and features
- **Inline Comments** — Explain non-obvious logic
- **PR Descriptions** — Include examples and rationale

## Releases

Releases follow [Semantic Versioning](https://semver.org/):

- **MAJOR** — Breaking changes
- **MINOR** — Backwards-compatible new features
- **PATCH** — Backwards-compatible bug fixes

Process:
1. Update version in `Cargo.toml` and `bindings/package.json`
2. Update CHANGELOG.md (move Unreleased → version)
3. Create git tag: `git tag v1.0.0`
4. Push tag: `git push origin v1.0.0`
5. GitHub Actions publishes to npm automatically

## Questions?

- **Design question?** — Open a GitHub Discussion
- **Bug report?** — Open an issue with reproduction steps
- **Documentation confusion?** — Open an issue; we'll improve it
- **Security issue?** — See "Reporting Vulnerabilities" above

## License

By contributing, you agree that your contributions will be licensed under the MIT License (see LICENSE).

---

Thank you for contributing to Mux! 🚀

