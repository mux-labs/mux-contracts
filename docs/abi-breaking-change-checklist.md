# ABI Breaking-Change Checklist

This checklist is mandatory for any change to a Soroban contract ABI, storage schema, error discriminant, event topic, authorization rule, or observable behavior. It complements [`BREAKING_CHANGES.md`](BREAKING_CHANGES.md) and is intended to make review and release decisions reproducible.

## Classification

Before implementation, classify the change as **non-breaking**, **deprecating**, or **breaking**. Treat uncertainty as breaking. A change to a numeric error discriminant is breaking even when the generated TypeScript union appears unchanged. A new required argument, changed authorization requirement, renamed storage key, or changed event payload is breaking.

The PR description must contain `BREAKING CHANGE:` and identify the affected contract, entrypoint, event, error, storage key, or behavior. It must state the current and proposed ABI and the first release in which the new behavior is available.

## Compatibility and migration

Prefer additive compatibility: introduce a new entrypoint, preserve the old entrypoint during the deprecation window, and document a concrete before/after migration. A breaking release requires a major version bump for both the Cargo workspace and `@mux-protocol/contracts`. Generated bindings, numeric error maps, `docs/abi_reference.md`, `docs/error_codes.md`, and examples must be regenerated or reviewed together.

Every migration must include an owner, affected consumers, a minimum 30-day deprecation period unless a documented security exception applies, and a removal target. A migration is not complete until the relevant contract tests and binding tests prove both the old compatibility path and the new path, where compatibility is promised.

## Security gates

Production or mainnet-affecting behavior must remain **fail closed**. Do not bypass authorization, signature validation, replay protection, network checks, or input bounds to preserve compatibility. Any feature flag must default to the safe behavior, be explicitly scoped to localnet/testnet when appropriate, and include a kill switch and operator-visible audit log. Never put secrets, private keys, JWTs, or raw authorization material in changelogs, logs, fixtures, or PR descriptions.

Set `BREAKING_CHANGE_ACK=1` in the release or deployment environment only after the checklist has been reviewed. CI and release tooling should refuse to proceed when the acknowledgement is absent for a breaking release.

## Rollback

Document whether rollback means reverting the release, disabling a feature flag, or deploying a compatible contract version. Verify that storage remains readable by the rollback version and that no irreversible migration has been applied. Record the exact WASM hash, network, contract IDs, and operator responsible for the rollback. Never perform an irreversible mainnet migration as part of an unreviewed CI job.

## Review checklist

- [ ] `BREAKING CHANGE:` appears in the PR description or commit body.
- [ ] `CHANGELOG.md` contains an Unreleased breaking-change entry and migration link.
- [ ] The Cargo workspace and bindings package use the intended semver version.
- [ ] Generated bindings and hand-maintained error maps were reviewed together.
- [ ] ABI, storage, error, event, and authorization compatibility tests pass.
- [ ] `BREAKING_CHANGE_ACK=1` is required for release/deploy execution.
- [ ] The safe default is fail closed and no secrets are exposed.
- [ ] Rollback, feature flag, and storage compatibility are documented.

Run `bash scripts/check-breaking-change.sh` from the repository root before requesting review.
