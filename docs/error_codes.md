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
| `DelegateExpired` | 5 | 400 | Delegate has expired (ledger timestamp >= `expires_at`) |
| `SpendLimitExceeded` | 6 | 400 | Spend would exceed the configured per-asset limit |
| `InvalidAmount` | 7 | 400 | Spend limit amount is zero or negative |
| `InvalidPeriod` | 8 | 400 | Spend limit period is zero |
| `TooManyDelegates` | 9 | 409 | Delegate map has reached `MAX_DELEGATES` (64) |
| `ReentrancyDetected` | 10 | 409 | Reentrant `debit_spend` call detected |
| `ArithmeticOverflow` | 11 | 500 | Arithmetic overflow in spend tracking |
| `TooManySessionKeys` | 12 | 409 | Session key map has reached capacity |
| `ScopeNotGranted` | 13 | 403 | Invoked method is not in the session key's `scopes` list |
| `SponsorNotAuthorized` | 14 | 403 | Relayer is not on the account's sponsor allowlist |

## Mux Account Factory (`contracts/mux-account-factory`)

Errors are defined in `MuxAccountFactoryError` (`contracts/mux-account-factory/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `Unauthorized` | 1 | 401 | Caller is not the registered owner |
| `InvalidAccount` | 2 | 400 | `account_address` must differ from `owner` |
| `TooManyAccounts` | 3 | 409 | Owner has reached the 64-account cap |
| `MetadataNotFound` | 4 | 404 | No metadata stored for the account |
| `MetadataTooLarge` | 5 | 400 | A metadata field (`version`, `description`, or `author`) exceeds its size limit |
| `MetadataTooLarge` | 5 | 400 | Metadata exceeds size limits |

## Mux Batcher (`contracts/mux-batcher`)

Errors are defined in `MuxBatcherError` (`contracts/mux-batcher/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `EmptyBatch` | 1 | 400 | The batch contains no operations |
| `BatchTooLarge` | 2 | 400 | The batch exceeds the 50-operation cap |
| `RequiredOperationFailed` | 3 | 500 | A required operation failed; the batch was aborted |
| `Unauthorized` | 4 | 401 | `require_auth()` failed for the caller |
| `ReentrancyDetected` | 5 | 409 | A reentrant call into the batcher was detected |
| `MetadataAlreadySet` | 6 | 409 | Metadata has already been set for this batch |
| `NotInitialized` | 7 | 500 | `upgrade` called before `initialize`; no admin to authorise it |
| `AlreadyInitialized` | 8 | 409 | `initialize` called more than once |

## Mux Delegation (`contracts/mux-delegation`)

Errors are defined in `MuxDelegationError` (`contracts/mux-delegation/src/lib.rs`).

Note: This contract uses a non-sequential numbering scheme starting at 6001.

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotADelegate` | 6001 | 404 | The caller is not a delegate for the delegator |
| `TooManyPermissions` | 6002 | 400 | Permission list exceeds the allowed maximum |
| `EmptyPermissions` | 6003 | 400 | An empty permission list was provided |
| `TooManyDelegates` | 6004 | 409 | Delegate list has reached capacity |
| `ContractIdAlreadySet` | 6005 | 409 | `link_contract_id` called more than once |
| `NotInitialized` | 6006 | 500 | `upgrade` called before `initialize`; no admin to authorise it |
| `AlreadyInitialized` | 6007 | 409 | `initialize` called more than once |

## Mux Permissions (`contracts/mux-permissions`)

Errors are defined in `MuxPermissionsError` (`contracts/mux-permissions/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not an authorized admin |
| `RoleNotFound` | 4 | 404 | The specified role does not exist |
| `AccountNotInRole` | 5 | 404 | Account is not a member of the role |
| `PermissionNotFound` | 6 | 404 | The specified permission does not exist |
| `TooManyMembers` | 7 | 409 | Role has too many members |
| `TooManyRoles` | 8 | 409 | Account holds too many roles |
| `AdminNotFound` | 9 | 404 | Pending admin not found |
| `AlreadyApproved` | 10 | 409 | Approver has already approved this candidate |
| `TooManyPendingAdmins` | 11 | 409 | Too many pending admin approvals |

## Mux Policy (`contracts/mux-policy`)

Errors are defined in `MuxPolicyError` (`contracts/mux-policy/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not the admin |
| `LimitNotFound` | 4 | 404 | No daily limit configured for the wallet |
| `LimitExceeded` | 5 | 400 | Spend would exceed the daily limit |
| `InvalidAmount` | 6 | 400 | Amount is zero or negative |
| `InvalidPeriod` | 7 | 400 | `day_ledgers` is zero |
| `TooManyWallets` | 8 | 409 | Wallet cap (256) reached |

## Mux Recovery (`contracts/mux-recovery`)

Errors are defined in `RecoveryError` (`contracts/mux-recovery/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not an authorized admin |
| `RecoveryAlreadyPending` | 4 | 409 | A recovery request is already pending for this account |
| `NoActiveRecovery` | 5 | 404 | No recovery request found with the given ID |
| `TimelockNotExpired` | 6 | 400 | Recovery timelock has not yet elapsed |
| `TooManyGuardians` | 7 | 409 | Guardian cap (16) reached |
| `GuardianAlreadyExists` | 8 | 409 | Address is already a registered guardian |
| `GuardianNotFound` | 9 | 404 | Address is not a registered guardian |
| `MinGuardiansRequired` | 10 | 400 | Cannot remove the last remaining guardian |
| `RecoveryExpired` | 11 | 400 | Recovery execution window has elapsed |

## Mux Registry (`contracts/mux-registry`)

Errors are defined in `MuxRegistryError` (`contracts/mux-registry/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not the admin |
| `ContractNotFound` | 4 | 404 | No contract registered under the given name |
| `TooManyContracts` | 5 | 409 | Registry cap (128) reached |

## Mux Spending Policy (`contracts/mux-spending-policy`)

Errors are defined in `SpendingPolicyError` (`contracts/mux-spending-policy/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | Contract not yet initialized |
| `AlreadyInitialized` | 2 | 409 | `initialize` called more than once |
| `Unauthorized` | 3 | 401 | Caller is not the admin |
| `PolicyNotFound` | 4 | 404 | No spend policy for the account/asset pair |
| `SpendLimitExceeded` | 5 | 400 | Requested spend exceeds the configured limit |
| `InvalidInput` | 6 | 400 | Limit is not positive or spend amount is negative |
| `InvalidPeriod` | 7 | 400 | `period_ledgers` is zero |

## Mux Wallet Registry (`contracts/mux-wallet-registry`)

Errors are defined in `WalletRegistryError` (`contracts/mux-wallet-registry/src/lib.rs`).

| Variant | Code | HTTP | Description |
|---|---|---|---|
| `NotInitialized` | 1 | 500 | The registry has not been initialized |
| `AlreadyInitialized` | 2 | 409 | The registry has already been initialized |
| `Unauthorized` | 3 | 401 | The caller is not the registry owner |
| `WalletNotFound` | 4 | 404 | No wallet is registered under the given name |
| `TooManyWallets` | 5 | 409 | Wallet registry capacity reached |

## HTTP Status Code Conventions

| Status | Category | Typical Error Names |
|--------|----------|-------------------|
| **401** | Authentication / authorization | `Unauthorized` |
| **400** | Invalid input / constraint violation | `InvalidAmount`, `InvalidPeriod`, `SpendLimitExceeded`, `EmptyBatch`, `BatchTooLarge`, `DelegateExpired`, `InvalidInput` |
| **404** | Resource not found | `*NotFound`, `*NotInRole`, `NotADelegate` |
| **409** | State conflict / capacity limit | `AlreadyInitialized`, `TooMany*`, `ReentrancyDetected`, `RecoveryAlreadyPending`, `MetadataAlreadySet` |
| **500** | Internal / uninitialized | `NotInitialized`, `ArithmeticOverflow`, `RequiredOperationFailed` |
