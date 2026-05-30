# Changelog Template

This template follows the [Keep a Changelog](https://keepachangelog.com/) format.

Use this as a reference when updating CHANGELOG.md with your changes.

## Format Overview

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- New features or functionality

### Changed
- Changes to existing functionality

### Deprecated
- Features that will be removed in a future version

### Removed
- Removed features or functionality (breaking changes)

### Fixed
- Bug fixes

### Security
- Security patches and vulnerability fixes

## [1.0.0] - YYYY-MM-DD

### Added
- Feature 1: Description
- Feature 2: Description

### Changed
- Behavior change 1: Description

### Fixed
- Bug fix 1: Description

### Security
- Security patch 1: Description

...older releases...
```

## Section Guidelines

### Added
Use this section for **new features** or **new functionality** added to the project.

Examples:
- New contract function
- New validation rule
- New error type
- New storage structure

**When to use:** PRs implementing new features or adding capabilities

### Changed
Use this section for **changes to existing functionality** that are **backwards compatible**.

Examples:
- Modified function behavior (same signature, improved logic)
- Updated error messages
- Performance improvements
- Refactored internal logic

**When to use:** PRs that improve or modify existing features without breaking the API

### Deprecated
Use this section to **announce upcoming removals**.

Examples:
- "Function `old_pay()` is deprecated in favor of `new_pay()`"
- "SpendLimit with period_ledgers=0 is deprecated; use explicit values"

**When to use:** PRs that keep backwards compatibility but want to warn users

### Removed
Use this section for **breaking changes** — removal of features or backwards-incompatible modifications.

Examples:
- Removed function entirely
- Changed function signature (new parameters, removed parameters)
- Changed error codes
- Changed ABI or contract interface

**When to use:** Major version bumps (breaking changes require manual migration)

### Fixed
Use this section for **bug fixes** that correct unintended behavior.

Examples:
- Fixed integer overflow in spend limit calculation
- Fixed session key expiration check
- Fixed authorization bypass vulnerability

**When to use:** PRs that fix bugs without adding new features

### Security
Use this section for **security patches** and **vulnerability fixes**.

Examples:
- Fixed XSS vulnerability in validation
- Patched access control bypass
- Fixed signature verification issue

**When to use:** PRs addressing security issues; always include CVE reference if applicable

## Updating CHANGELOG.md

### When Releasing a Version

1. Review the **Unreleased** section
2. Remove the **Unreleased** header, replace with version number and date:
   ```
   ## [1.0.0] - 2026-05-30
   ```
3. Add a new **Unreleased** section at the top (with no subsections yet)
4. Update the git tag and version in all Cargo.toml files to match

### During Development

Add entries to the **Unreleased** section as you implement features or fixes:

1. In your PR, add a line to the appropriate subsection of **Unreleased**
2. Use clear, user-facing language describing *what changed*, not *how it was implemented*
3. Include the PR number or issue number for reference, e.g., `(#42)`

Example PR addition:
```markdown
### Added
- `register_session_key()` function for account abstraction (#26)
- SessionKey storage structures and validation (#26)

### Fixed
- Session key expiration validation now correctly checks ledger timestamp (#26)
```

## PR Description Requirements

Every PR should include a changelog entry in its description. This helps maintainers:

- Quickly understand what section the change belongs in
- Ensure consistency across releases
- Generate changelogs with minimal manual work

Template for PR descriptions:
```
## Changelog Entry

**Type:** Added / Changed / Fixed / Removed / Deprecated / Security

**Description:** Clear description of the change as a user would experience it
```

## Breaking Changes Policy

Breaking changes require:

1. **Major version bump** — Increment MAJOR in semantic versioning (e.g., 1.0.0 → 2.0.0)
2. **Migration guide** — Document in CHANGELOG.md how users should update their code
3. **Deprecation period** — If possible, deprecate in N-1 version before removal
4. **Clear communication** — Announce in release notes and documentation

Example breaking change entry:
```markdown
### Removed
- **BREAKING:** Removed `pay(asset, amount)` function; use `pay(asset, amount, metadata)` instead (#45)

  **Migration:** Update all calls from `pay(asset, amount)` to include metadata parameter:
  ```rust
  // Old
  contract.pay(&asset, &amount);

  // New
  contract.pay(&asset, &amount, &Metadata { ... });
  ```
```

## Validation

Before merging, ensure:

- [ ] PR title references issue number or clearly describes the change
- [ ] Changelog entry added to appropriate section
- [ ] Entry uses clear, user-facing language
- [ ] If breaking change: major version will be bumped
- [ ] If security issue: CVE reference included (if applicable)

## Questions?

Refer to [keepachangelog.com](https://keepachangelog.com/) for more examples and best practices.
