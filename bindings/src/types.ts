import { Address, xdr } from "@stellar/stellar-sdk";

export type NetworkPassphrase = string;

export interface MuxContractIds {
  muxAccount: string;
  muxBatcher: string;
  muxPermissions: string;
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

export type MuxPermissionsError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "RoleNotFound"
  | "AccountNotInRole"
  | "PermissionNotFound";

export type MuxAccountFactoryError =
  | "Unauthorized"
  | "InvalidAccount"
  | "TooManyAccounts";

export type MuxRegistryError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "ContractNotFound"
  | "TooManyContracts";

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
