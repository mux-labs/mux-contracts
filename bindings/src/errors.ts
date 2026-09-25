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
 * Stable, typed error codes for cross-network invoke guards.
 *
 * These are emitted by the bindings layer (not the contracts) when an
 * invocation is blocked because the target network does not match the
 * resolved/configured network. Callers can branch on these codes instead
 * of matching ad-hoc strings.
 *
 * Cross-network guard codes (bindings/src/network.ts):
 *   CrossNetworkInvokeBlocked  → 403  target network does not match resolved network
 *   NetworkConfigMissing       → 403  no network configured (deny-by-default)
 *   NetworkConfigInvalid       → 400  malformed/unknown network configuration
 */
export const CROSS_NETWORK_ERROR_CODES = {
  CrossNetworkInvokeBlocked: "CrossNetworkInvokeBlocked",
  NetworkConfigMissing: "NetworkConfigMissing",
  NetworkConfigInvalid: "NetworkConfigInvalid",
} as const;

export type CrossNetworkErrorCode =
  (typeof CROSS_NETWORK_ERROR_CODES)[keyof typeof CROSS_NETWORK_ERROR_CODES];

/**
 * Stable, typed error codes for on-chain batching DoS caps.
 *
 * These are emitted by the bindings layer (not the contracts) when a batch
 * is rejected before submission because it would violate the on-chain
 * batching caps enforced by the mux-batcher contract. Fail-closed: an
 * oversized or malformed batch is blocked client-side rather than being
 * forwarded to the contract.
 *
 * Batching cap codes (bindings/src/batcher.ts):
 *   BatchTooLarge          → 400  batch length exceeds the on-chain max batch size
 *   BatchAggregateTooLarge → 400  aggregate operation count exceeds the on-chain cap
 *   BatchEmpty             → 400  batch contains no operations
 *   BatchCapConfigMissing  → 403  no batching cap configured (deny-by-default)
 *   BatchCapConfigInvalid  → 400  malformed/unknown batching cap configuration
 */
export const BATCHING_CAP_ERROR_CODES = {
  BatchTooLarge: "BatchTooLarge",
  BatchAggregateTooLarge: "BatchAggregateTooLarge",
  BatchEmpty: "BatchEmpty",
  BatchCapConfigMissing: "BatchCapConfigMissing",
  BatchCapConfigInvalid: "BatchCapConfigInvalid",
} as const;

export type BatchingCapErrorCode =
  (typeof BATCHING_CAP_ERROR_CODES)[keyof typeof BATCHING_CAP_ERROR_CODES];

/**
 * Typed error thrown when a batch violates the on-chain batching caps.
 *
 * Fail-closed by default: a missing/invalid cap configuration blocks the
 * batch rather than falling through to an unbounded submission.
 */
export class BatchingCapError extends Error {
  readonly code: BatchingCapErrorCode;
  readonly statusCode: number;
  readonly batchSize?: number;
  readonly maxBatchSize?: number;
  readonly aggregateOps?: number;
  readonly maxAggregateOps?: number;
  readonly correlationId?: string;

  constructor(
    code: BatchingCapErrorCode,
    message: string,
    options: {
      batchSize?: number;
      maxBatchSize?: number;
      aggregateOps?: number;
      maxAggregateOps?: number;
      correlationId?: string;
    } = {},
  ) {
    super(message);
    this.name = "BatchingCapError";
    this.code = code;
    this.statusCode = ERROR_HTTP_MAP[code] ?? 400;
    this.batchSize = options.batchSize;
    this.maxBatchSize = options.maxBatchSize;
    this.aggregateOps = options.aggregateOps;
    this.maxAggregateOps = options.maxAggregateOps;
    this.correlationId = options.correlationId;
  }
}

/**
 * Typed error thrown when a cross-network invocation is blocked.
 *
 * Fail-closed by default: unknown/missing network configuration blocks the
 * invocation rather than falling through to a default network.
 */
export class CrossNetworkInvokeError extends Error {
  readonly code: CrossNetworkErrorCode;
  readonly statusCode: number;
  readonly targetNetwork?: string;
  readonly resolvedNetwork?: string;
  readonly correlationId?: string;

  constructor(
    code: CrossNetworkErrorCode,
    message: string,
    options: {
      targetNetwork?: string;
      resolvedNetwork?: string;
      correlationId?: string;
    } = {},
  ) {
    super(message);
    this.name = "CrossNetworkInvokeError";
    this.code = code;
    this.statusCode = ERROR_HTTP_MAP[code] ?? 403;
    this.targetNetwork = options.targetNetwork;
    this.resolvedNetwork = options.resolvedNetwork;
    this.correlationId = options.correlationId;
  }
}

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
 *
 * Relayer fee sponsorship limits (contracts/mux-account, relayer fee path):
 *   RelayerNotAuthorized      → 403  relayer is not an authorized sponsor
 *   RelayerSponsorshipLimit   → 400  per-relayer sponsorship cap exceeded
 *   AccountSponsorshipLimit   → 400  per-account sponsorship cap exceeded
 *   SponsorshipWindowExceeded → 400  sponsorship window/period cap exceeded
 *   SponsorshipDisabled       → 403  sponsorship kill-switch engaged
 *   InvalidSponsorshipConfig  → 400  malformed sponsorship limit config
 *   DuplicateSponsorship      → 409  replayed/idempotent sponsorship request
 *
 * Batching DoS caps (bindings/src/batcher.ts):
 *   BatchTooLarge          → 400  batch length exceeds the on-chain max batch size
 *   BatchAggregateTooLarge → 400  aggregate operation count exceeds the on-chain cap
 *   BatchEmpty             → 400  batch contains no operations
 *   BatchCapConfigMissing  → 403  no batching cap configured (deny-by-default)
 *   BatchCapConfigInvalid  → 400  malformed/unknown batching cap configuration
 *
 * Cross-network invoke guard (bindings/src/network.ts):
 *   CrossNetworkInvokeBlocked → 403  target network does not match resolved network
 *   NetworkConfigMissing      → 403  no network configured (deny-by-default)
 *   NetworkConfigInvalid      → 400  malformed/unknown network configuration
 */
export const ERROR_HTTP_MAP: Record<string, number> = {
  // Authentication/Authorization errors → 401
  Unauthorized: 401,

  // Not Found errors → 404
  NotADelegate: 404,           // MuxDelegationError (6001): no grant for (owner, delegate)
  DelegateNotFound: 404,       // MuxAccountError (4): no delegate registered for owner

  // Batching DoS cap errors → 400 (deny-by-default config → 403)
  BatchTooLarge: 400,
  BatchAggregateTooLarge: 400,
  BatchEmpty: 400,
  BatchCapConfigMissing: 403,
  BatchCapConfigInvalid: 400,
};
