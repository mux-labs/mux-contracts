import type { MuxAccountFactoryError } from "./generated/mux-account-factory";
import type { MuxRegistryError } from "./generated/mux-registry";
import type { MuxWalletRegistryError } from "./generated/mux-wallet-registry";
import type {
  MuxAccountError,
  MuxBatcherError,
  MuxDelegationError,
  MuxPermissionsError,
  MuxPolicyError,
  MuxRecoveryError,
  SpendingPolicyError,
} from "./types";

export interface HttpErrorResponse {
  statusCode: number;
  message: string;
  errorType: string;
}

type ContractError =
  | MuxAccountError
  | MuxBatcherError
  | MuxDelegationError
  | MuxPermissionsError
  | MuxPolicyError
  | MuxAccountFactoryError
  | MuxRegistryError
  | MuxWalletRegistryError
  | MuxRecoveryError
  | SpendingPolicyError;

/**
 * Maps contract error variants to HTTP status codes.
 * - 401: Unauthorized (authentication/permission issues)
 * - 404: Not Found (missing resources)
 * - 400: Bad Request (invalid input, constraint violations)
 * - 409: Conflict (state conflicts)
 * - 500: Internal Server Error (initialization or unknown errors)
 *
 * MuxAccount error codes (contracts/mux-account):
 *   NotInitialized      (1)  → 500
 *   AlreadyInitialized  (2)  → 409
 *   Unauthorized        (3)  → 401
 *   DelegateNotFound    (4)  → 404
 *   DelegateExpired     (5)  → 400
 *   SpendLimitExceeded  (6)  → 400
 *   InvalidAmount       (7)  → 400
 *   InvalidPeriod       (8)  → 400
 *   TooManyDelegates    (9)  → 409
 *   ReentrancyDetected  (10) → 409
 *   ArithmeticOverflow  (11) → 500
 *   TooManySessionKeys  (12) → 409
 *   ScopeNotGranted     (13) → 403
 *   SponsorNotAuthorized (14) → 403
 *   InvalidNonce        (15) → 409
 *
 * MuxAccountFactory error codes (contracts/mux-account-factory):
 *   Unauthorized      (1) → 401  caller is not the registered owner
 *   InvalidAccount    (2) → 400  account_address must differ from owner
 *   TooManyAccounts   (3) → 409  per-owner 64-account cap reached
 *   MetadataNotFound  (4) → 404  no metadata stored for the account
 *   MetadataTooLarge  (5) → 400  metadata field exceeds size limit
 *
 * MuxBatcher error codes (contracts/mux-batcher):
 *   EmptyBatch                (1) → 400
 *   BatchTooLarge             (2) → 400
 *   RequiredOperationFailed   (3) → 500
 *   Unauthorized              (4) → 401
 *   ReentrancyDetected        (5) → 409
 *   MetadataAlreadySet        (6) → 409
 *   NotInitialized            (7) → 500
 *   AlreadyInitialized        (8) → 409
 *
 * MuxDelegation error codes (contracts/mux-delegation):
 *   NotADelegate          (6001) → 404
 *   TooManyPermissions    (6002) → 400
 *   EmptyPermissions      (6003) → 400
 *   TooManyDelegates      (6004) → 409
 *   ContractIdAlreadySet  (6005) → 409
 *   NotInitialized        (6006) → 500
 *   AlreadyInitialized    (6007) → 409
 *
 * MuxPermissions error codes (contracts/mux-permissions):
 *   NotInitialized         (1)  → 500
 *   AlreadyInitialized     (2)  → 409
 *   Unauthorized           (3)  → 401
 *   RoleNotFound           (4)  → 404
 *   AccountNotInRole       (5)  → 404
 *   PermissionNotFound     (6)  → 404
 *   TooManyMembers         (7)  → 409
 *   TooManyRoles           (8)  → 409
 *   AdminNotFound          (9)  → 404
 *   AlreadyApproved        (10) → 409
 *   TooManyPendingAdmins   (11) → 409
 *
 * MuxPolicy error codes (contracts/mux-policy):
 *   NotInitialized     (1) → 500
 *   AlreadyInitialized (2) → 409
 *   Unauthorized       (3) → 401
 *   LimitNotFound      (4) → 404
 *   LimitExceeded      (5) → 400
 *   InvalidAmount      (6) → 400
 *   InvalidPeriod      (7) → 400
 *   TooManyWallets     (8) → 409
 *
 * RecoveryError / MuxRecovery error codes (contracts/mux-recovery):
 *   NotInitialized          (1)  → 500
 *   AlreadyInitialized      (2)  → 409
 *   Unauthorized            (3)  → 401
 *   RecoveryAlreadyPending  (4)  → 409
 *   NoActiveRecovery        (5)  → 404
 *   TimelockNotExpired      (6)  → 400
 *   TooManyGuardians        (7)  → 409
 *   GuardianAlreadyExists   (8)  → 409
 *   GuardianNotFound        (9)  → 404
 *   MinGuardiansRequired    (10) → 400
 *   RecoveryExpired         (11) → 400
 *
 * MuxRegistry error codes (contracts/mux-registry):
 *   NotInitialized     (1) → 500
 *   AlreadyInitialized (2) → 409
 *   Unauthorized       (3) → 401
 *   ContractNotFound   (4) → 404
 *   TooManyContracts   (5) → 409
 *
 * SpendingPolicyError / MuxSpendingPolicy error codes (contracts/mux-spending-policy):
 *   NotInitialized     (1) → 500
 *   AlreadyInitialized (2) → 409
 *   Unauthorized       (3) → 401
 *   PolicyNotFound     (4) → 404
 *   SpendLimitExceeded (5) → 400
 *   InvalidInput       (6) → 400
 *
 * WalletRegistryError / MuxWalletRegistry error codes (contracts/mux-wallet-registry):
 *   NotInitialized     (1) → 500
 *   AlreadyInitialized (2) → 409
 *   Unauthorized       (3) → 401
 *   WalletNotFound     (4) → 404
 *   TooManyWallets     (5) → 409
 */
export const ERROR_HTTP_MAP: Record<string, number> = {
  // Authentication/Authorization errors → 401
  Unauthorized: 401,

  // Not Found errors → 404
  NotADelegate: 404,           // MuxDelegationError (6001): no grant for (owner, delegate)
  DelegateNotFound: 404,       // MuxAccountError (4)
  RoleNotFound: 404,           // MuxPermissionsError (4)
  AccountNotInRole: 404,       // MuxPermissionsError (5)
  PermissionNotFound: 404,     // MuxPermissionsError (6)
  AdminNotFound: 404,          // MuxPermissionsError (9)
  ContractNotFound: 404,       // MuxRegistryError (4)
  WalletNotFound: 404,         // WalletRegistryError (4)
  MetadataNotFound: 404,       // MuxAccountFactoryError (4)
  LimitNotFound: 404,          // MuxPolicyError (4)
  PolicyNotFound: 404,         // SpendingPolicyError (4) — alias used in some bindings
  NoActiveRecovery: 404,       // RecoveryError (5): no recovery request found
  GuardianNotFound: 404,       // RecoveryError (9): address is not a registered guardian

  // Validation/Constraint errors → 400
  InvalidAmount: 400,          // MuxAccountError (7), MuxPolicyError (6)
  InvalidPeriod: 400,          // MuxAccountError (8), MuxPolicyError (7)
  InvalidInput: 400,           // SpendingPolicyError (6)
  InvalidAccount: 400,         // MuxAccountFactoryError (2)
  MetadataTooLarge: 400,       // MuxAccountFactoryError (5)
  SpendLimitExceeded: 400,     // MuxAccountError (6), SpendingPolicyError (5)
  LimitExceeded: 400,          // MuxPolicyError (5)
  DelegateExpired: 400,        // MuxAccountError (5)
  EmptyBatch: 400,             // MuxBatcherError (1)
  BatchTooLarge: 400,          // MuxBatcherError (2)
  TooManyPermissions: 400,     // MuxDelegationError (6002)
  EmptyPermissions: 400,       // MuxDelegationError (6003)
  TimelockNotExpired: 400,     // RecoveryError (6): timelock has not yet elapsed
  MinGuardiansRequired: 400,   // RecoveryError (10): cannot remove last guardian
  RecoveryExpired: 400,        // RecoveryError (11): execution window elapsed

  // State conflict → 409
  AlreadyInitialized: 409,     // all contracts (code 2)
  AlreadyApproved: 409,        // MuxPermissionsError (10)
  MetadataAlreadySet: 409,     // MuxBatcherError (6)
  ContractIdAlreadySet: 409,   // MuxDelegationError (6005)
  RecoveryAlreadyPending: 409, // RecoveryError (4)

  // Security guard violations → 409
  ReentrancyDetected: 409,     // MuxAccountError (10), MuxBatcherError (5)

  // Capacity limits → 409
  TooManyDelegates: 409,       // MuxAccountError (9), MuxDelegationError (6004)
  TooManyAccounts: 409,        // MuxAccountFactoryError (3)
  TooManyContracts: 409,       // MuxRegistryError (5)
  TooManyMembers: 409,         // MuxPermissionsError (7)
  TooManyRoles: 409,           // MuxPermissionsError (8)
  TooManyPendingAdmins: 409,   // MuxPermissionsError (11)
  TooManyWallets: 409,         // WalletRegistryError (5), MuxPolicyError (8)
  TooManySessionKeys: 409,     // MuxAccountError (12)
  ScopeNotGranted: 403,        // MuxAccountError (13): method outside the session key's scopes
  SponsorNotAuthorized: 403,   // MuxAccountError (14): relayer not on the sponsor allowlist
  InvalidNonce: 409,           // MuxAccountError (15): stale or future account nonce
  TooManyGuardians: 409,       // RecoveryError (7): guardian cap (16) reached
  GuardianAlreadyExists: 409,  // RecoveryError (8): address already a registered guardian

  // Internal/Uninitialized → 500
  NotInitialized: 500,         // all contracts (code 1 or 6006 for delegation)
  RequiredOperationFailed: 500, // MuxBatcherError (3)
  ArithmeticOverflow: 500,     // MuxAccountError (11)
};

/**
 * Converts a contract error to an HTTP error response.
 * Unknown errors default to 500 Internal Server Error.
 */
export function contractErrorToHttp(
  error: ContractError | string,
): HttpErrorResponse {
  const errorType = String(error);
  const statusCode = ERROR_HTTP_MAP[errorType] || 500;

  return {
    statusCode,
    message: errorType,
    errorType,
  };
}

/**
 * Checks if an error from a contract call should be treated as an HTTP error.
 * Can be used in middleware/error handlers.
 */
export function isContractError(error: unknown): error is string {
  if (typeof error !== "string") {
    return false;
  }
  return error in ERROR_HTTP_MAP || true;
}
