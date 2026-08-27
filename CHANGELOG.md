# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed
- `mux-spending-policy::set_policy` accepted `period_ledgers == 0`, which would create a policy with a non-advancing period window and made the `InvalidPeriod` error variant unreachable; it now rejects zero periods with `InvalidPeriod` after the admin auth gate (fail-closed: auth before validation), matching `mux-account::set_spend_limit` and `mux-policy::set_daily_limit`
- `docs/threat-model.md` only covered `mux-account`, `mux-batcher`, and `mux-permissions`; expanded to all 10 production contracts with per-contract threats, controls, trust boundaries, and residual risks
- `mux-account` `execute_with_session` stored session-key `scopes` but never enforced them; empty-scope session keys were accepted fail-open and are now rejected with `EmptyScopes` (fail-closed)
- Negative auth-rejection tests in `mux-policy`, `mux-registry`, and `mux-wallet-registry` used `mock_all_auths()` as a restorable guard, but in soroban-sdk 21 it is a permanent switch — the tests could never reject; rewrote them to seed state via `env.as_contract` so `require_auth` actually rejects
- `mux-recovery` integration test `admin_approve` and unit test `test_initialize_emits_init_event` were not updated for the quorum-threshold `initialize` signature
- `mux-recovery` unit tests still called `initialize` with the pre-quorum 2-arg signature; fixed to pass `quorum_threshold`
- `CONTRIBUTING.md` example called a nonexistent `is_session_key_valid` entrypoint; replaced with a real `register_session_key` + `execute_with_session` example (#700)
- `.github/workflows/deploy.yml` set the `DEPLOYER_SECRET_KEY` env var but `scripts/deploy.sh` reads `DEPLOYER_PRIVATE_KEY`, so the deploy workflow always ran with an unset key; renamed for consistency across the workflow and all deployer-key docs (#702)
- `docs/architecture-overview.md` was missing `mux-policy` and `mux-spending-policy` from the contract list and diagram; added both and marked `Somzilla.md` as a non-canonical scratch note (#701)

### Added
- `mux-account` `execute_with_session` now executes: it takes `(session_key, target, function, args)`, matches `function` against the session key's granted `scopes` fail-closed (`ScopeNotGranted`), dispatches to `target` while the reentrancy guard is held, and forwards the return value — closing the AA Phase 2 milestone (#583)
- `mux-account` relayer sponsorship — `set_sponsor` / `is_sponsor` owner-managed allowlist and `execute_with_session_sponsored`, which requires both sponsor and session-key authorization and rejects un-allowlisted relayers with `SponsorNotAuthorized` (#583)
- `docs/relayer-integration.md` and `examples/session-key-usage.ts` — relayer network documentation and a frontend session-key integration example (#583)
- `tests/threat_model_coverage.rs` — regression test verifying every production contract crate (all 10 WASM-shipping `contracts/mux-*` crates) is covered in `docs/threat-model.md`, wired into CI via `scripts/check-threat-model-coverage.sh`
- `scripts/check-doc-examples.sh` — CI guard verifying `CONTRIBUTING.md` example code only calls entrypoints that exist (#700)
- `scripts/check-architecture-docs.sh` — CI guard verifying every contract crate is listed in `docs/architecture-overview.md` (#701)
- `scripts/check-deploy-secret-name.sh` — CI guard verifying the deploy workflow's secret env var name matches what `scripts/deploy.sh` reads (#702)
- `scripts/check-changelog-release-artifacts.sh` — CI guard verifying every tagged release lists WASM hashes and the bindings package version (#699)
- `### Release Artifacts` requirement added to `.github/CHANGELOG_TEMPLATE.md`: tagged releases must list contract WASM SHA-256 hashes and the bindings package version together (#699)
- CI/CD key-handling guidance (GitHub Environments scoping, mandatory post-deploy rotation) added to `docs/deployer-key-requirements.md` (#702)
- Contract PR guidelines section in `CONTRIBUTING.md` covering `no_std` safety, error enum conventions, storage bounds, TTL management, unit tests, and a pre-review checklist (#489, #490, #503, #504)
- `SECURITY.md` with vulnerability disclosure policy, scope, safe-harbor guidelines, and response timeline (#490)
- Complete contract index in `contracts/README.md` covering all 10 contract crates (#503)
- `docs/bindings-error-mapping.md` documenting how Rust error enums map to TS unions and HTTP status codes (#504)
- All 10 contract error enums documented in `docs/error_codes.md` (previously only 5 were listed; added mux-delegation, mux-policy, mux-recovery, mux-spending-policy, mux-wallet-registry) (#504)

### Changed
- Upgraded migration notes for `mux-account` in `docs/account-upgrade-migration.md` and inline module docs
- `RegistryMeta` struct (`name`, `version`, `description`) and `DataKey::Metadata` storage key for `mux-account`
- `set_metadata()` and `get_metadata()` contract functions on `mux-account` (owner-only write, public read)
- Negative-path unit tests for `mux-account-factory`: exact error assertions for `InvalidAccount` and `TooManyAccounts`, `MetadataNotFound` after deploy without metadata, wrong-owner metadata lookup, and unauthorized deploy without auth
- `WalletMetadata` struct (`label`, `description`) for `mux-wallet-registry` contract (#318)
- `register_wallet_with_metadata()` and `get_metadata()` contract functions in `mux-wallet-registry` (#318)
- `registerWalletWithMetadata()` and `getMetadata()` methods on `MuxWalletRegistryClient` TS binding (#318)
- `WalletMetadata` and `MuxWalletRegistryError` TypeScript types exported from the binding (#318, #319)
- `WalletNotFound` mapped to HTTP 404 in `ERROR_HTTP_MAP`; `MuxWalletRegistryError` added to the `ContractError` union (#319)
- Wallet registry error codes documented in `docs/error_codes.md` (#319)
- Integration test stub for `mux-wallet-registry` in `bindings/__tests__/wallet-registry.test.ts` (#320)
- All five `MuxBatcherError` variants (`EmptyBatch`, `BatchTooLarge`, `RequiredOperationFailed`, `Unauthorized`, `ReentrancyDetected`) documented with numeric codes and HTTP mappings in `docs/error_codes.md` (#244)
- Integration test stubs for batcher error cases (`BatchTooLarge`, `RequiredOperationFailed`, `Unauthorized`) added to `bindings/__tests__/batch-integration.test.ts` (#245)

### Changed
- `mux-account-factory` deploy / simulate paths share `load_accounts_under_cap` so the per-owner Accounts vec stays bounded at 64
- Documented factory Accounts cap in `docs/storage-griefing.md` and `docs/abi_reference.md`
