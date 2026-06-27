# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `mux-account-factory`: `MuxAccountFactory` contract that deploys and indexes
  `MuxAccount` instances per owner, with a 64-account-per-owner cap to prevent
  storage griefing (#229, #230, #231, #232).
- `mux-account-factory`: HTTP error-code mapping for all three factory error
  variants — `Unauthorized` → 401, `InvalidAccount` → 400,
  `TooManyAccounts` → 409 — wired into `ERROR_HTTP_MAP` in
  `bindings/src/errors.ts` (#229).
- `bindings/__tests__/factory-integration.test.ts`: graceful-skip integration
  test stubs for `deploy_account`, `get_accounts`, and `account_count`; tests
  skip automatically when the network or contract ID is unavailable (#230).
- `docs/error_codes.md`: corrected `mux-account-factory` error reference to
  match the live `MuxAccountFactoryError` enum (replaces stale
  `FactoryError::InitFailed` / `AlreadyInitialized` entries) (#229).
- `docs/contract-upgrade-pattern.md`: factory-specific upgrade migration note
  covering the `Accounts` vec and `AccountCount` storage keys, backward-
  compatibility rules, and rollback steps (#232).
