# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Upgrade migration notes for `mux-account` in `docs/account-upgrade-migration.md` and inline module docs
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
