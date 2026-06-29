# Error Codes Reference

This document provides a comprehensive reference for all error codes used within the Mux Protocol contracts.

## Mux Account (`contracts/mux-account`)

Errors are defined in `MuxAccountError` (`contracts/mux-account/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized; call `initialize` first |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not the owner or contract is paused |
| `DelegateNotFound` | 4 | 404 | Delegate does not exist in the delegate map |
| `DelegateExpired` | 5 | 410 | Delegate has expired (current ledger ≥ `expiry_ledger`) |
| `SpendLimitExceeded` | 6 | 429 | Spend would exceed the configured per-asset limit |
| `InvalidAmount` | 7 | 400 | Spend limit amount is zero or negative |
| `InvalidPeriod` | 8 | 400 | Spend limit period is zero |
| `TooManyDelegates` | 9 | 409 | Delegate map has reached `MAX_DELEGATES` (64) |
| `ReentrancyDetected` | 10 | 409 | Reentrant `debit_spend` call detected |
| `ArithmeticOverflow` | 11 | 500 | Arithmetic overflow in spend tracking |

## Mux Account Factory (`contracts/mux-account-factory`)
- `MuxAccountFactoryError::Unauthorized` (1) → HTTP 401 - The caller is not the registered owner; `require_auth()` failed.
- `MuxAccountFactoryError::InvalidAccount` (2) → HTTP 400 - `account_address` must differ from `owner`.
- `MuxAccountFactoryError::TooManyAccounts` (3) → HTTP 409 - Owner has reached the 64-account-per-owner cap (storage-griefing guard).

## Mux Batcher (`contracts/mux-batcher`)
- `BatcherError::BatchTooLarge` (3001) - The batch contains too many transactions.
- `BatcherError::ExecutionFailed` (3002) - Execution of a batched transaction failed.

## Mux Permissions (`contracts/mux-permissions`)
- `PermissionError::RoleNotFound` (4001) - The specified role does not exist.
- `PermissionError::AccessDenied` (4002) - Access to the resource is denied.

## Mux Registry (`contracts/mux-registry`)
- `RegistryError::ContractNotFound` (5001) - The specified contract was not found in the registry.
- `RegistryError::VersionMismatch` (5002) - The contract version does not match the expected version.

## Mux Wallet Registry (`contracts/mux-wallet-registry`)
- `WalletRegistryError::NotInitialized` (1) — The registry has not been initialized.
- `WalletRegistryError::AlreadyInitialized` (2) — The registry has already been initialized.
- `WalletRegistryError::Unauthorized` (3) — The caller is not the registry owner.
- `WalletRegistryError::WalletNotFound` (4) — No wallet is registered under the given name.

### HTTP mapping
| Error variant         | HTTP status |
|-----------------------|-------------|
| `NotInitialized`      | 500         |
| `AlreadyInitialized`  | 409         |
| `Unauthorized`        | 401         |
| `WalletNotFound`      | 404         |
