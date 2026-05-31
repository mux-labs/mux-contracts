# Error Codes Reference

This document provides a comprehensive reference for all error codes used within the Mux Protocol contracts.

## Mux Account (`contracts/mux-account`)
- `AccountError::NotAuthorized` (1001) - The caller is not authorized to perform the action.
- `AccountError::InvalidSignature` (1002) - The provided signature is invalid.
- `AccountError::NonceTooLow` (1003) - The provided nonce is too low.

## Mux Account Factory (`contracts/mux-account-factory`)
- `FactoryError::InitFailed` (2001) - Initialization of the account failed.
- `FactoryError::AlreadyInitialized` (2002) - The factory or account is already initialized.

## Mux Batcher (`contracts/mux-batcher`)
- `BatcherError::BatchTooLarge` (3001) - The batch contains too many transactions.
- `BatcherError::ExecutionFailed` (3002) - Execution of a batched transaction failed.

## Mux Permissions (`contracts/mux-permissions`)
- `PermissionError::RoleNotFound` (4001) - The specified role does not exist.
- `PermissionError::AccessDenied` (4002) - Access to the resource is denied.

## Mux Registry (`contracts/mux-registry`)
- `RegistryError::ContractNotFound` (5001) - The specified contract was not found in the registry.
- `RegistryError::VersionMismatch` (5002) - The contract version does not match the expected version.
