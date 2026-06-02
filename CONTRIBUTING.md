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
- **Coverage** — Aim for >90% coverage on new code

Example test:
```rust
#[test]
fn test_session_key_valid_returns_true() {
    let (env, client, owner) = setup();
    let session_key = Address::generate(&env);
    let expires_at = env.ledger().timestamp() + 3600;
    let scopes = Vec::new(&env);

    client.register_session_key(&owner, &session_key, &expires_at, &scopes);
    assert!(client.is_session_key_valid(&owner, &session_key));
}
```

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
