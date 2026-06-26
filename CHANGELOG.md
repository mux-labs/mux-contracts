# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `mux-registry`: contract version registry with `register`, `register_with_metadata`,
  `get_version`, `get_metadata`, and `list_contracts` entry points (#306)
- `mux-registry`: `ContractMetadata` struct (version, description, author) for
  rich contract metadata storage (#306)
- `mux-registry`: `TooManyContracts` error and `MAX_CONTRACTS = 128` cap to bound
  instance storage growth and prevent storage-griefing (#306)
- `mux-wallet-registry`: wallet name → address registry with `register_wallet` and
  `get_wallet` entry points (#306)
- `mux-wallet-registry`: unit tests covering initialize, register/get, update, and
  all negative paths (unauthorized, double-init, not-found) (#308, #309)

### Changed
- `mux-registry`: TTL management (`TTL_THRESHOLD = 17_280`, `TTL_EXTEND_TO = 518_400`)
  extended on every mutating call to keep instance storage live for ~30 days (#306)

### Migration Notes
- `mux-registry` upgrade (any prior → Unreleased): storage layout is additive;
  no `migrate()` call required. Upload new WASM, call `upgrade()` as admin, then
  verify with `get_version` / `list_contracts`. See
  [docs/contract-upgrade-pattern.md](docs/contract-upgrade-pattern.md) for the
  full checklist (#307)
- `mux-wallet-registry` upgrade: no storage changes; straightforward WASM swap
  with no migration step (#307)

## [0.1.0] - 2026-06-26

### Added
- Initial workspace with `mux-account`, `mux-account-factory`, `mux-batcher`,
  `mux-delegation`, `mux-permissions`, `mux-policy`, `mux-registry`, and
  `mux-wallet-registry` contracts
- Soroban SDK v21 dependency across all workspace members
- TypeScript bindings scaffolding under `bindings/`
- Deployment scripts, network configuration, and Docker Compose localnet setup
