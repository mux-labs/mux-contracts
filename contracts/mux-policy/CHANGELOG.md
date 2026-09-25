# Changelog

All notable changes to the mux-policy contract will be documented in this file.

## [0.1.0] - 2026-06-27

### Added
- Initial release of mux-policy contract
- `initialize` — set the contract admin
- `set_daily_limit` — configure a per-wallet daily spend limit
- `get_daily_limit` — query the current daily limit and spent amount
- `record_spend` — debit against a wallet's daily limit (wallet-authenticated)
- Audit events emitted on each state-mutating operation (`init`, `lmt_set`, `spent`)
- Automatic daily counter reset based on ledger sequence
- Storage TTL extension on every write to prevent data eviction
