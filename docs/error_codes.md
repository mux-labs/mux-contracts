# Error Codes Reference

This document provides a comprehensive reference for all error codes used within the Mux Protocol contracts.

## Mux Account (`contracts/mux-account`)
- `AccountError::NotAuthorized` (1001) - The caller is not authorized to perform the action.
- `AccountError::InvalidSignature` (1002) - The provided signature is invalid.
- `AccountError::NonceTooLow` (1003) - The provided nonce is too low.

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
