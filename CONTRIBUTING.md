# Contributing to Mux Contracts

Thank you for your interest in contributing to Mux! This guide explains how to submit changes, what we expect, and how we work together.

## Code of Conduct

Be respectful and constructive. We're committed to providing a welcoming and inclusive environment.

## Getting Started

1. **Fork the repository** — Click the "Fork" button on GitHub
2. **Clone your fork** — `git clone https://github.com/your-username/mux-contracts.git`
3. **Create a branch** — `git checkout -b feature/your-feature-name`
4. **Make your changes** — See guidelines below
5. **Test** — Run `cargo test --workspace --all-features`
6. **Commit** — Follow commit message conventions
7. **Push** — `git push origin feature/your-feature-name`
8. **Open a Pull Request** — Describe your changes clearly

## Commit Message Convention

Use descriptive commit messages following this format:

```
<type>(<scope>): <short description> (#<issue>)

<optional body explaining the change in detail>
```

**Type** — Choose one:
- `feat:` — New feature or functionality
- `fix:` — Bug fix
- `docs:` — Documentation changes
- `test:` — Test additions or modifications
- `refactor:` — Code refactoring without feature changes
- `perf:` — Performance improvements
- `chore:` — Build, dependency, or tooling changes

**Scope** — One of:
- `contracts:` — Contract code changes
- `tests:` — Test-specific changes
- `docs:` — Documentation files
- `scripts:` — Build or utility scripts
- `bindings:` — TypeScript bindings

**Examples:**
```
feat(contracts): add session key validation for account abstraction (#26)
fix(tests): handle ledger timestamp overflow in session key tests (#26)
docs(docs): add account abstraction design guide (#27)
```

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
- [ ] `cargo build --target wasm32-unknown-unknown --release -p <crate>` succeeds
- [ ] Error enum follows the convention (single `#[contracterror]`, `#[repr(u32)]`, codes start at 1)
- [ ] `docs/error_codes.md` updated for new or changed error variants
- [ ] TypeScript bindings regenerated (`bash scripts/generate-bindings.sh`)
- [ ] `bindings/src/types.ts` union type and error-message map updated
- [ ] `bindings/src/errors.ts` HTTP map updated for new variants
- [ ] All collection storage has a cap (`MAX_*` constant + `TooMany*` error)
- [ ] Persistent storage entries call `extend_ttl` on write
- [ ] Unit tests cover happy path, each error variant, and edge cases
- [ ] `cargo clippy --workspace --all-features` is clean
- [ ] `cargo fmt --check` passes

## Code Style

### Rust

- **Format** — Run `cargo fmt` before committing
- **Lint** — Run `cargo clippy` and fix warnings
- **Comments** — Add doc comments (`///`) to public functions and types
- **Tests** — All new public functionality must have unit tests
- **Error Handling** — Use Result types; avoid unwrap() in library code

### Documentation

- **README** — Keep up-to-date with new features
- **Inline Comments** — Explain *why*, not *what* (code explains what)
- **Public APIs** — Document with examples in doc comments
- **Architecture** — Document design decisions in `docs/` directory

## Testing

- **Unit Tests** — Run `cargo test --lib`
- **All Tests** — Run `cargo test --workspace --all-features`
- **Integration Tests** — Require localnet setup (see README.md)
- **Coverage** — Aim for >90% coverage on new code. Generate a report with
  `make coverage` or `bash scripts/coverage.sh` (add `--html` / `--lcov` as needed).
  If `llvm-tools-preview` is not installed, the script prints a **coverage report stub**
  listing workspace crates; validate the stub with `bash scripts/test-coverage.sh`.

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
    let scopes = Vec::new(&env);

    client.register_session_key(&session_key, &expires_at, &scopes);
    let payload = Bytes::new(&env);
    let _ = client.execute_with_session(&session_key, &payload);
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
