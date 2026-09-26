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
  // ── Shared / multi-contract names ───────────────────────────────────────────
  NotInitialized: 500,        // contract not yet initialized
  AlreadyInitialized: 409,    // initialize called more than once
  Unauthorized: 401,          // caller is not authorized

  // ── MuxAccount ──────────────────────────────────────────────────────────────
  DelegateNotFound: 404,      // (4)  no delegate registered for owner
  DelegateExpired: 400,       // (5)  delegate timestamp has elapsed
  SpendLimitExceeded: 400,    // (6)  spend would exceed configured per-asset limit
  InvalidAmount: 400,         // (7)  amount is zero or negative
  InvalidPeriod: 400,         // (8)  period is zero
  TooManyDelegates: 409,      // (9)  delegate map at MAX_DELEGATES (64)
  ReentrancyDetected: 409,    // (10) reentrant call detected
  ArithmeticOverflow: 500,    // (11) arithmetic overflow in spend tracking
  TooManySessionKeys: 409,    // (12) session key map at capacity
  ScopeNotGranted: 403,       // (13) method not in session key scopes list
  SponsorNotAuthorized: 403,  // (14) relayer not on sponsor allowlist
  InvalidNonce: 409,          // (15) nonce mismatch

  // ── MuxAccountFactory ───────────────────────────────────────────────────────
  InvalidAccount: 400,        // (2)  account_address must differ from owner
  TooManyAccounts: 409,       // (3)  per-owner 64-account cap reached
  MetadataNotFound: 404,      // (4)  no metadata stored for the account
  MetadataTooLarge: 400,      // (5)  metadata field exceeds size limit

  // ── MuxBatcher ──────────────────────────────────────────────────────────────
  EmptyBatch: 400,            // (1)  batch contains no operations
  BatchTooLarge: 400,         // (2)  batch exceeds 50-operation cap
  RequiredOperationFailed: 500, // (3) required operation failed; batch aborted
  MetadataAlreadySet: 409,    // (6)  metadata already set for this batcher

  // ── MuxDelegation ───────────────────────────────────────────────────────────
  NotADelegate: 404,          // (6001) no grant for (owner, delegate) pair
  TooManyPermissions: 400,    // (6002) permission list exceeds 64-entry cap
  EmptyPermissions: 400,      // (6003) empty permission list provided
  ContractIdAlreadySet: 409,  // (6005) link_contract_id is write-once

  // ── MuxPermissions ──────────────────────────────────────────────────────────
  RoleNotFound: 404,          // (4)  role does not exist
  AccountNotInRole: 404,      // (5)  account is not a member of the role
  PermissionNotFound: 404,    // (6)  permission does not exist
  TooManyMembers: 409,        // (7)  role at MAX_ROLE_MEMBERS (256)
  TooManyRoles: 409,          // (8)  account at MAX_ROLES_PER_ACCOUNT (32)
  AdminNotFound: 404,         // (9)  pending admin not found
  AlreadyApproved: 409,       // (10) approver already approved this candidate
  TooManyPendingAdmins: 409,  // (11) too many pending admin approvals

  // ── MuxPolicy ───────────────────────────────────────────────────────────────
  LimitNotFound: 404,         // (4)  no daily limit configured for the wallet
  LimitExceeded: 400,         // (5)  spend would exceed the daily limit
  TooManyWallets: 409,        // (8)  wallet cap (256) reached

  // ── MuxRecovery ─────────────────────────────────────────────────────────────
  RecoveryAlreadyPending: 409, // (4)  recovery request already pending
  NoActiveRecovery: 404,       // (5)  no recovery request found
  TimelockNotExpired: 400,     // (6)  recovery timelock has not elapsed
  TooManyGuardians: 409,       // (7)  guardian cap (16) reached
  GuardianAlreadyExists: 409,  // (8)  address already a registered guardian
  GuardianNotFound: 404,       // (9)  address is not a registered guardian
  MinGuardiansRequired: 400,   // (10) cannot remove the last guardian
  RecoveryExpired: 400,        // (11) recovery execution window has elapsed

  // ── MuxRegistry ─────────────────────────────────────────────────────────────
  ContractNotFound: 404,      // (4)  no contract registered under the given name
  TooManyContracts: 409,      // (5)  registry cap (128) reached

  // ── SpendingPolicy ──────────────────────────────────────────────────────────
  PolicyNotFound: 404,        // (4)  no spend policy for the account/asset pair
  InvalidInput: 400,          // (6)  limit not positive or spend amount negative

  // ── MuxWalletRegistry ───────────────────────────────────────────────────────
  WalletNotFound: 404,        // (4)  no wallet registered under the given name

  // ── Batching DoS caps (bindings/src/batcher.ts) ─────────────────────────────
  BatchAggregateTooLarge: 400,  // aggregate operation count exceeds on-chain cap
  BatchEmpty: 400,              // batch contains no operations (client-side guard)
  BatchCapConfigMissing: 403,   // no batching cap configured (deny-by-default)
  BatchCapConfigInvalid: 400,   // malformed batching cap configuration

  // ── Cross-network invoke guard (bindings/src/network.ts) ────────────────────
  CrossNetworkInvokeBlocked: 403, // target network does not match resolved network
  NetworkConfigMissing: 403,      // no network configured (deny-by-default)
  NetworkConfigInvalid: 400,      // malformed/unknown network configuration

  // ── Relayer fee sponsorship limits ──────────────────────────────────────────
  RelayerNotAuthorized: 403,       // relayer is not an authorized sponsor
  RelayerSponsorshipLimit: 400,    // per-relayer sponsorship cap exceeded
  AccountSponsorshipLimit: 400,    // per-account sponsorship cap exceeded
  SponsorshipWindowExceeded: 400,  // sponsorship window/period cap exceeded
  SponsorshipDisabled: 403,        // sponsorship kill-switch engaged
  InvalidSponsorshipConfig: 400,   // malformed sponsorship limit config
  DuplicateSponsorship: 409,       // replayed/idempotent sponsorship request
};

export const MuxErrorCode = {
  NETWORK_NOT_CONFIGURED: "NETWORK_NOT_CONFIGURED",
  NETWORK_UNKNOWN: "NETWORK_UNKNOWN",
  CROSS_NETWORK_BLOCKED: "CROSS_NETWORK_BLOCKED",
} as const;
export type MuxErrorCode = (typeof MuxErrorCode)[keyof typeof MuxErrorCode];

export class MuxError extends Error {
  constructor(public readonly code: MuxErrorCode, message: string) {
    super(message);
    this.name = "MuxError";
  }
}

/** Convert a contract or SDK error into a stable HTTP-shaped response. */
export function contractErrorToHttp(error: unknown): HttpErrorResponse {
  const value = error as { code?: string; message?: string; name?: string };
  const errorType = value.code ?? value.name ?? "UnknownError";
  return {
    statusCode: ERROR_HTTP_MAP[errorType] ?? 500,
    message: value.message ?? "Unknown contract error",
    errorType,
  };
}
