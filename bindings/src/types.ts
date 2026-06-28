import { Address, xdr } from "@stellar/stellar-sdk";

export type NetworkPassphrase = string;

export interface MuxContractIds {
  muxAccount: string;
  muxBatcher: string;
  muxDelegation: string;
  muxPermissions: string;
  muxWalletRegistry: string;
}

export interface SpendLimit {
  asset: Address;
  amount: bigint;
  periodLedgers: number;
  spent: bigint;
  resetLedger: number;
}

export interface DelegateInfo {
  address: Address;
  expiryLedger: number;
  canSpend: boolean;
}

export interface Operation {
  target: Address;
  fnName: string;
  args: xdr.ScVal[];
  requireSuccess: boolean;
  /** Classifies the operation intent for indexers and UI. */
  kind: BatchOperationKind;
}

/** Mirrors the on-chain `BatchOperationKind` enum. */
export type BatchOperationKind = "Invoke" | "Transfer" | "Approve";

export interface BatchResult {
  successCount: number;
  failureCount: number;
}

export type MuxAccountError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "DelegateNotFound"
  | "DelegateExpired"
  | "SpendLimitExceeded"
  | "InvalidAmount"
  | "InvalidPeriod"
  | "ReentrancyDetected"
  | "ArithmeticOverflow";

export type MuxBatcherError =
  | "EmptyBatch"
  | "BatchTooLarge"
  | "RequiredOperationFailed"
  | "Unauthorized"
  | "ReentrancyDetected";

export type MuxDelegationError =
  | "Unauthorized"
  | "DelegateNotFound"
  | "DelegateExpired"
  | "TooManyDelegates";

export type MuxPermissionsError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "RoleNotFound"
  | "AccountNotInRole"
  | "PermissionNotFound"
  | "TooManyMembers"
  | "TooManyRoles"
  | "AdminNotFound"
  | "AlreadyApproved";

/**
 * Maps a `MuxPermissionsError` variant or its raw `u32` contract error code to
 * a human-readable description.
 *
 * Mirrors the on-chain `error_message` function so that clients can resolve
 * error codes without an extra RPC call.
 *
 * @example
 * ```ts
 * import { muxPermissionsErrorMessage } from "./types";
 * console.log(muxPermissionsErrorMessage("RoleNotFound")); // "role not found"
 * console.log(muxPermissionsErrorMessage(4));              // "role not found"
 * ```
 */
export function muxPermissionsErrorMessage(
  error: MuxPermissionsError | number
): string {
  const codeMap: Record<number, string> = {
    1: "contract not initialized",
    2: "contract already initialized",
    3: "caller is not authorized",
    4: "role not found",
    5: "account is not a member of the role",
    6: "permission not found",
    7: "role has too many members",
    8: "account holds too many roles",
    9: "pending admin not found",
    10: "approver has already approved this candidate",
  };

  const nameMap: Record<MuxPermissionsError, number> = {
    NotInitialized: 1,
    AlreadyInitialized: 2,
    Unauthorized: 3,
    RoleNotFound: 4,
    AccountNotInRole: 5,
    PermissionNotFound: 6,
    TooManyMembers: 7,
    TooManyRoles: 8,
    AdminNotFound: 9,
    AlreadyApproved: 10,
  };

  const code =
    typeof error === "number" ? error : nameMap[error] ?? -1;
  return codeMap[code] ?? "unknown error code";
}

export interface SpendingPolicyLimit {
  asset: Address;
  limit: bigint;
}

export type SpendingPolicyError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "PolicyNotFound"
  | "SpendLimitExceeded";
