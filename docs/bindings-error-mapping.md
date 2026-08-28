# Bindings Error Mapping

This document explains how Soroban contract error enums flow into TypeScript
union types and HTTP status codes in the `@mux-protocol/contracts` package.

## Pipeline

```
Rust #[contracterror] enum
  │
  ▼
Stellar CLI codegen (bindings/src/generated/<contract>.ts)
  │
  ▼
TS string-union type in bindings/src/types.ts
  │
  ▼
ERROR_HTTP_MAP in bindings/src/errors.ts
  │
  ▼
contractErrorToHttp() → HttpErrorResponse
```

### Step 1 — Rust Error Enum

Each contract defines a single `#[contracterror]` enum with `#[repr(u32)]`:

```rust
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxAccountError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    // ...
}
```

When a contract function returns `Err(MuxAccountError::Unauthorized)`, the
Soroban runtime encodes the error as an `ScError` with the enum discriminant
(the `u32` code).

### Step 2 — Codegen Output

Running `stellar contract bindings generate` produces a TypeScript client file
per contract in `bindings/src/generated/`. The generated types include a
**string literal union** for each error enum:

```ts
// auto-generated in bindings/src/generated/mux-account.ts
export type MuxAccountError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "DelegateNotFound"
  | ...;
```

The Stellar SDK automatically decodes on-chain `ScError` values back to these
string names when you read the error from a transaction result.

### Step 3 — Manual Union Type (`types.ts`)

For contracts whose generated types don't include a standalone error union (or
when you need a consolidated union across contracts), define it in
`bindings/src/types.ts`:

```ts
export type MuxAccountError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "DelegateNotFound"
  | "DelegateExpired"
  | "SpendLimitExceeded"
  | "InvalidAmount"
  | "InvalidPeriod"
  | "TooManyDelegates"
  | "ReentrancyDetected"
  | "ArithmeticOverflow"
  | "TooManySessionKeys"
  | "ScopeNotGranted"
  | "SponsorNotAuthorized"
  | "InvalidNonce";
```

Each variant name **must** match the Rust enum variant exactly (case-sensitive).

`types.ts` also provides optional `*ErrorMessage()` helpers that map both the
string name and the raw `u32` code to a human-readable description:

```ts
import { muxAccountErrorMessage } from "@mux-protocol/contracts";

muxAccountErrorMessage("DelegateNotFound"); // "delegate not found"
muxAccountErrorMessage(4);                  // "delegate not found"
```

### Step 4 — HTTP Status Map (`errors.ts`)

`bindings/src/errors.ts` exports `ERROR_HTTP_MAP`, a `Record<string, number>`
that maps variant names to HTTP status codes:

```ts
export const ERROR_HTTP_MAP: Record<string, number> = {
  Unauthorized: 401,
  DelegateNotFound: 404,
  InvalidAmount: 400,
  AlreadyInitialized: 409,
  NotInitialized: 500,
  // ...
};
```

The helper `contractErrorToHttp()` wraps this map:

```ts
const response = contractErrorToHttp("Unauthorized");
// { statusCode: 401, message: "Unauthorized", errorType: "Unauthorized" }
```

Unknown errors default to **500 Internal Server Error**.

## HTTP Status Code Conventions

| Status | Category | Examples |
|--------|----------|---------|
| **401** | Authentication / authorization | `Unauthorized` |
| **400** | Invalid input / constraint violation | `InvalidAmount`, `SpendLimitExceeded`, `EmptyBatch`, `BatchTooLarge` |
| **404** | Resource not found | `DelegateNotFound`, `RoleNotFound`, `ContractNotFound`, `WalletNotFound` |
| **409** | State conflict / capacity limit | `AlreadyInitialized`, `TooManyDelegates`, `ReentrancyDetected` |
| **500** | Internal / uninitialized | `NotInitialized`, `ArithmeticOverflow`, `RequiredOperationFailed` |

## Adding a New Error Variant

When you add a variant to a Rust `#[contracterror]` enum, update these files:

| File | Change |
|------|--------|
| `contracts/<crate>/src/lib.rs` | Add variant with the next `u32` code |
| `docs/error_codes.md` | Add row with variant, code, HTTP status, and description |
| `bindings/src/types.ts` | Add variant to the TS union type and update the `*ErrorMessage` maps |
| `bindings/src/errors.ts` | Add entry to `ERROR_HTTP_MAP` with the appropriate HTTP status |
| `contracts/README.md` | No change needed unless the contract summary changes |

After updating, regenerate bindings and run tests:

```bash
bash scripts/generate-bindings.sh
cd bindings && npm test
```

## Cross-Contract Error Overlap

Multiple contracts may use the same variant name (e.g. `Unauthorized` appears
in 9 of 10 contracts). The `ERROR_HTTP_MAP` is **shared** — the same variant
name always maps to the same HTTP status regardless of which contract produced
it. This is intentional: API consumers only need to handle one HTTP status per
error name.

If two contracts need different HTTP semantics for the same error name, rename
one of the variants in the Rust enum to avoid ambiguity.

## TypeScript Union Types — Canonical Reference

The following table lists every TS union type exported from `bindings/src/types.ts`
and its authoritative Rust source enum. Each variant name **must** match the
Rust variant exactly (case-sensitive). The `*ErrorMessage()` helper function
maps both the string name and the raw `u32` code to a human-readable string.

### `MuxAccountError` (15 variants)

Source: `contracts/mux-account/src/lib.rs → MuxAccountError`  
Helper: `muxAccountErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `NotInitialized` | 1 | 500 |
| `AlreadyInitialized` | 2 | 409 |
| `Unauthorized` | 3 | 401 |
| `DelegateNotFound` | 4 | 404 |
| `DelegateExpired` | 5 | 400 |
| `SpendLimitExceeded` | 6 | 400 |
| `InvalidAmount` | 7 | 400 |
| `InvalidPeriod` | 8 | 400 |
| `TooManyDelegates` | 9 | 409 |
| `ReentrancyDetected` | 10 | 409 |
| `ArithmeticOverflow` | 11 | 500 |
| `TooManySessionKeys` | 12 | 409 |
| `ScopeNotGranted` | 13 | 403 |
| `SponsorNotAuthorized` | 14 | 403 |
| `InvalidNonce` | 15 | 409 |

### `MuxAccountFactoryError` (5 variants)

Source: `contracts/mux-account-factory/src/lib.rs → MuxAccountFactoryError`  
Generated in: `bindings/src/generated/mux-account-factory.ts`  
Helper: `muxAccountFactoryErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `Unauthorized` | 1 | 401 |
| `InvalidAccount` | 2 | 400 |
| `TooManyAccounts` | 3 | 409 |
| `MetadataNotFound` | 4 | 404 |
| `MetadataTooLarge` | 5 | 400 |

> Note: The generated file at `bindings/src/generated/mux-account-factory.ts`
> currently only includes 4 variants (`MetadataTooLarge` is absent). The full
> 5-variant type is declared in `bindings/src/types.ts` and should be used in
> application code. Regenerate bindings after any contract interface change.

### `MuxBatcherError` (8 variants)

Source: `contracts/mux-batcher/src/lib.rs → MuxBatcherError`  
Helper: `muxBatcherErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `EmptyBatch` | 1 | 400 |
| `BatchTooLarge` | 2 | 400 |
| `RequiredOperationFailed` | 3 | 500 |
| `Unauthorized` | 4 | 401 |
| `ReentrancyDetected` | 5 | 409 |
| `MetadataAlreadySet` | 6 | 409 |
| `NotInitialized` | 7 | 500 |
| `AlreadyInitialized` | 8 | 409 |

### `MuxDelegationError` (7 variants)

Source: `contracts/mux-delegation/src/lib.rs → MuxDelegationError`  
Helper: `muxDelegationErrorMessage(error)`

Note: uses non-sequential codes starting at 6001 to avoid collision with other contracts.

| Variant | Code | HTTP |
|---------|------|------|
| `NotADelegate` | 6001 | 404 |
| `TooManyPermissions` | 6002 | 400 |
| `EmptyPermissions` | 6003 | 400 |
| `TooManyDelegates` | 6004 | 409 |
| `ContractIdAlreadySet` | 6005 | 409 |
| `NotInitialized` | 6006 | 500 |
| `AlreadyInitialized` | 6007 | 409 |

### `MuxPermissionsError` (11 variants)

Source: `contracts/mux-permissions/src/lib.rs → MuxPermissionsError`  
Helper: `muxPermissionsErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `NotInitialized` | 1 | 500 |
| `AlreadyInitialized` | 2 | 409 |
| `Unauthorized` | 3 | 401 |
| `RoleNotFound` | 4 | 404 |
| `AccountNotInRole` | 5 | 404 |
| `PermissionNotFound` | 6 | 404 |
| `TooManyMembers` | 7 | 409 |
| `TooManyRoles` | 8 | 409 |
| `AdminNotFound` | 9 | 404 |
| `AlreadyApproved` | 10 | 409 |
| `TooManyPendingAdmins` | 11 | 409 |

### `MuxPolicyError` (8 variants)

Source: `contracts/mux-policy/src/lib.rs → MuxPolicyError`  
Helper: `muxPolicyErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `NotInitialized` | 1 | 500 |
| `AlreadyInitialized` | 2 | 409 |
| `Unauthorized` | 3 | 401 |
| `LimitNotFound` | 4 | 404 |
| `LimitExceeded` | 5 | 400 |
| `InvalidAmount` | 6 | 400 |
| `InvalidPeriod` | 7 | 400 |
| `TooManyWallets` | 8 | 409 |

### `MuxRecoveryError` (11 variants)

Source: `contracts/mux-recovery/src/lib.rs → RecoveryError`  
Helper: `muxRecoveryErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `NotInitialized` | 1 | 500 |
| `AlreadyInitialized` | 2 | 409 |
| `Unauthorized` | 3 | 401 |
| `RecoveryAlreadyPending` | 4 | 409 |
| `NoActiveRecovery` | 5 | 404 |
| `TimelockNotExpired` | 6 | 400 |
| `TooManyGuardians` | 7 | 409 |
| `GuardianAlreadyExists` | 8 | 409 |
| `GuardianNotFound` | 9 | 404 |
| `MinGuardiansRequired` | 10 | 400 |
| `RecoveryExpired` | 11 | 400 |

### `MuxRegistryError` (5 variants)

Source: `contracts/mux-registry/src/lib.rs → MuxRegistryError`  
Generated in: `bindings/src/generated/mux-registry.ts`  
Helper: `muxRegistryErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `NotInitialized` | 1 | 500 |
| `AlreadyInitialized` | 2 | 409 |
| `Unauthorized` | 3 | 401 |
| `ContractNotFound` | 4 | 404 |
| `TooManyContracts` | 5 | 409 |

### `SpendingPolicyError` (6 variants)

Source: `contracts/mux-spending-policy/src/lib.rs → SpendingPolicyError`  
Helper: `spendingPolicyErrorMessage(error)`

| Variant | Code | HTTP |
|---------|------|------|
| `NotInitialized` | 1 | 500 |
| `AlreadyInitialized` | 2 | 409 |
| `Unauthorized` | 3 | 401 |
| `PolicyNotFound` | 4 | 404 |
| `SpendLimitExceeded` | 5 | 400 |
| `InvalidInput` | 6 | 400 |

### `MuxWalletRegistryError` (5 variants)

Source: `contracts/mux-wallet-registry/src/lib.rs → WalletRegistryError`  
Generated in: `bindings/src/generated/mux-wallet-registry.ts`

| Variant | Code | HTTP |
|---------|------|------|
| `NotInitialized` | 1 | 500 |
| `AlreadyInitialized` | 2 | 409 |
| `Unauthorized` | 3 | 401 |
| `WalletNotFound` | 4 | 404 |
| `TooManyWallets` | 5 | 409 |

---

> **Keeping this table in sync:** The automated guard test
> `bindings/__tests__/error-coverage.test.ts` will fail if any variant listed
> above is missing from `ERROR_HTTP_MAP`, providing a CI-enforced contract
> between this doc and the runtime mapping.

## Cross-Contract Error Overlap

Multiple contracts may use the same variant name (e.g. `Unauthorized` appears
in 9 of 10 contracts). The `ERROR_HTTP_MAP` is **shared** — the same variant
name always maps to the same HTTP status regardless of which contract produced
it. This is intentional: API consumers only need to handle one HTTP status per
error name.

If two contracts need different HTTP semantics for the same error name, rename
one of the variants in the Rust enum to avoid ambiguity.

## Example: End-to-End Flow

```
1. Contract returns Err(MuxAccountError::Unauthorized)   [Rust, u32 code 3]
2. Soroban runtime encodes as ScError(3)                  [on-chain]
3. Stellar SDK decodes to string "Unauthorized"           [TypeScript]
4. contractErrorToHttp("Unauthorized")                    [bindings]
   → { statusCode: 401, message: "Unauthorized", errorType: "Unauthorized" }
5. API returns HTTP 401 to the client
```
