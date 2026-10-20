# Error Codes

This document catalogues the error enums returned by the Mux Protocol
contracts. Each section is keyed `<crate>::<EnumName>` and lists every
variant with its stable numeric code.

This file is machine-checked: `tests/error_codes_coverage.rs` parses the
`pub enum *Error` declarations in every `contracts/*/src/lib.rs` and asserts
set equality against the tables below. Adding, renaming, or removing a
variant without updating this document fails the test.

| Crate | Enum | Variants |
|-------|------|----------|
| mux-account | `MuxAccountError` | 15 |
| mux-account-factory | `MuxAccountFactoryError` | 7 |
| mux-batcher | `MuxBatcherError` | 8 |
| mux-delegation | `MuxDelegationError` | 7 |
| mux-permissions | `MuxPermissionsError` | 11 |
| mux-policy | `MuxPolicyError` | 9 |
| mux-recovery | `RecoveryError` | 15 |
| mux-registry | `RegistryError` | 5 |
| mux-registry | `MuxRegistryError` | 5 |
| mux-spending-policy | `SpendingPolicyError` | 7 |
| mux-wallet-registry | `WalletRegistryError` | 5 |

## mux-account::MuxAccountError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | `initialize()` has not been called on this instance. |
| 2 | `AlreadyInitialized` | `initialize()` was called more than once. |
| 3 | `Unauthorized` | The caller lacks the owner/admin/session authorization required for the operation. |
| 4 | `DelegateNotFound` | No delegate record exists for the `(owner, delegate)` pair. |
| 5 | `DelegateExpired` | The delegate's `expires_at` timestamp is in the past. |
| 6 | `SpendLimitExceeded` | The requested spend exceeds the rolling limit for the window. |
| 7 | `InvalidAmount` | The amount is non-positive or otherwise malformed. |
| 8 | `InvalidPeriod` | The supplied period is zero or otherwise invalid. |
| 9 | `TooManyDelegates` | The owner has reached the per-owner delegate cap. |
| 10 | `ReentrancyDetected` | A reentrant call pattern was detected and rejected. |
| 11 | `ArithmeticOverflow` | A checked arithmetic operation overflowed. |
| 12 | `TooManySessionKeys` | The per-owner session-key cap has been reached. |
| 13 | `ScopeNotGranted` | The session key's `scopes` do not authorize the target contract/method. |
| 14 | `SponsorNotAuthorized` | The sponsor is not authorized to sponsor this operation. |
| 15 | `InvalidNonce` | The supplied nonce is stale, replayed, or out of order. |

## mux-account-factory::MuxAccountFactoryError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `Unauthorized` | The caller is not the factory admin. |
| 2 | `InvalidAccount` | The account address must differ from the owner address. |
| 3 | `TooManyAccounts` | The per-owner account cap has been reached. |
| 4 | `MetadataNotFound` | No metadata is stored for the requested account. |
| 5 | `MetadataTooLarge` | The supplied metadata exceeds the size limit. |
| 6 | `NotInitialized` | `upgrade()` was called before `initialize()`. |
| 7 | `AlreadyInitialized` | `initialize()` was called more than once. |

## mux-batcher::MuxBatcherError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `EmptyBatch` | The submitted batch contains no operations. |
| 2 | `BatchTooLarge` | The batch exceeds `MAX_BATCH_SIZE`. |
| 3 | `RequiredOperationFailed` | A required operation inside the batch failed. |
| 4 | `Unauthorized` | The caller is not authorized to submit this batch. |
| 5 | `ReentrancyDetected` | A reentrant call pattern was detected and rejected. |
| 6 | `MetadataAlreadySet` | Registry metadata has already been set for this batcher. |
| 7 | `NotInitialized` | `initialize()` has not been called. |
| 8 | `AlreadyInitialized` | `initialize()` was called more than once. |

## mux-delegation::MuxDelegationError

| Code | Variant | Description |
|------|---------|-------------|
| 6001 | `NotADelegate` | No grant exists for the `(owner, delegate)` pair. |
| 6002 | `TooManyPermissions` | `permissions` exceeds the per-delegate cap. |
| 6003 | `EmptyPermissions` | `permissions` is empty. |
| 6004 | `TooManyDelegates` | The owner has reached the per-owner delegate cap. |
| 6005 | `ContractIdAlreadySet` | `link_contract_id` was called after the id was already set. |
| 6006 | `NotInitialized` | `upgrade` was called before `initialize`. |
| 6007 | `AlreadyInitialized` | `initialize` was called more than once. |

## mux-permissions::MuxPermissionsError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | `initialize()` has not been called. |
| 2 | `AlreadyInitialized` | `initialize()` was called more than once. |
| 3 | `Unauthorized` | The caller lacks the required admin role. |
| 4 | `RoleNotFound` | The requested role does not exist. |
| 5 | `AccountNotInRole` | The account is not a member of the role. |
| 6 | `PermissionNotFound` | The requested permission does not exist. |
| 7 | `TooManyMembers` | The role has reached the member cap. |
| 8 | `TooManyRoles` | The account has reached the role-count cap. |
| 9 | `AdminNotFound` | The requested admin does not exist. |
| 10 | `AlreadyApproved` | The pending admin change was already approved. |
| 11 | `TooManyPendingAdmins` | The pending-admin queue is full. |

## mux-policy::MuxPolicyError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | `initialize()` has not been called. |
| 2 | `AlreadyInitialized` | `initialize()` was called more than once. |
| 3 | `Unauthorized` | The caller is not the policy admin. |
| 4 | `LimitNotFound` | No daily limit is configured for the wallet. |
| 5 | `LimitExceeded` | The spend would exceed the wallet's daily limit. |
| 6 | `InvalidAmount` | The amount is non-positive or otherwise malformed. |
| 7 | `InvalidPeriod` | `day_ledgers` is zero. |
| 8 | `TooManyWallets` | The wallet-limit table has reached its cap. |
| 9 | `RegistryNotFound` | The linked registry contract is unreachable or not a valid mux-registry deployment. |

## mux-recovery::RecoveryError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | `initialize()` has not been called. |
| 2 | `AlreadyInitialized` | `initialize()` was called more than once. |
| 3 | `Unauthorized` | The caller lacks the required owner/guardian authorization. |
| 4 | `RecoveryAlreadyPending` | A recovery is already in progress. |
| 5 | `NoActiveRecovery` | No recovery request is currently active. |
| 6 | `TimelockNotExpired` | The recovery timelock has not yet elapsed. |
| 7 | `TooManyGuardians` | The guardian set has reached its cap. |
| 8 | `GuardianAlreadyExists` | The address is already a guardian. |
| 9 | `GuardianNotFound` | The address is not a guardian. |
| 10 | `MinGuardiansRequired` | Fewer than the minimum number of guardians are configured. |
| 11 | `RecoveryExpired` | The recovery request outlived its validity window. |
| 12 | `QuorumNotReached` | Guardian approvals have not reached the quorum threshold. |
| 13 | `DuplicateApproval` | The guardian already approved this recovery. |
| 14 | `InvalidQuorumThreshold` | The quorum threshold is out of range. |
| 15 | `RegistryNotFound` | The address passed to `set_registry` failed cross-contract validation. |

## mux-registry::RegistryError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | The contract has not been initialised. |
| 2 | `AlreadyInitialized` | The contract has already been initialised. |
| 3 | `Unauthorized` | The caller is not the contract admin. |
| 4 | `ContractNotFound` | No entry exists for the requested contract name. |
| 5 | `InvalidVersion` | The supplied version string is empty or invalid. |

## mux-registry::MuxRegistryError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | The contract has not been initialised. |
| 2 | `AlreadyInitialized` | The contract has already been initialised. |
| 3 | `Unauthorized` | The caller is not the contract admin. |
| 4 | `ContractNotFound` | No entry exists for the requested contract name. |
| 5 | `TooManyContracts` | The registry has reached the maximum number of registered contracts. |

## mux-spending-policy::SpendingPolicyError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | The contract has not been initialized. |
| 2 | `AlreadyInitialized` | The contract was already initialized. |
| 3 | `Unauthorized` | The caller is not authorized to perform the requested action. |
| 4 | `PolicyNotFound` | No spend policy exists for the requested account/asset pair. |
| 5 | `SpendLimitExceeded` | The requested spend exceeds the configured policy limit. |
| 6 | `InvalidInput` | The supplied input is invalid (for example a non-positive limit). |
| 7 | `InvalidPeriod` | The supplied period is invalid (zero is not allowed). |

## mux-wallet-registry::WalletRegistryError

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | `initialize` has not been called yet; the owner is unknown. |
| 2 | `AlreadyInitialized` | `initialize` was called more than once on the same instance. |
| 3 | `Unauthorized` | Reserved for future use; auth failures surface as host-level errors from `Address::require_auth`. |
| 4 | `WalletNotFound` | No wallet is registered under the requested name. |
| 5 | `TooManyWallets` | The registry has reached the maximum number of wallets. |
