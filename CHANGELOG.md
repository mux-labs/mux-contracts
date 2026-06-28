# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `WalletMetadata` struct (`label`, `description`) for `mux-wallet-registry` contract (#318)
- `register_wallet_with_metadata()` and `get_metadata()` contract functions in `mux-wallet-registry` (#318)
- `registerWalletWithMetadata()` and `getMetadata()` methods on `MuxWalletRegistryClient` TS binding (#318)
- `WalletMetadata` and `MuxWalletRegistryError` TypeScript types exported from the binding (#318, #319)
- `WalletNotFound` mapped to HTTP 404 in `ERROR_HTTP_MAP`; `MuxWalletRegistryError` added to the `ContractError` union (#319)
- Wallet registry error codes documented in `docs/error_codes.md` (#319)
- Integration test stub for `mux-wallet-registry` in `bindings/__tests__/wallet-registry.test.ts` (#320)
- All five `MuxBatcherError` variants (`EmptyBatch`, `BatchTooLarge`, `RequiredOperationFailed`, `Unauthorized`, `ReentrancyDetected`) documented with numeric codes and HTTP mappings in `docs/error_codes.md` (#244)
- Integration test stubs for batcher error cases (`BatchTooLarge`, `RequiredOperationFailed`, `Unauthorized`) added to `bindings/__tests__/batch-integration.test.ts` (#245)
