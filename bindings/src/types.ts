import { Address, xdr } from "@stellar/stellar-sdk";

export type NetworkPassphrase = string;

export interface MuxContractIds {
  muxAccount: string;
  muxBatcher: string;
  muxDelegation: string;
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
}

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
  | "PermissionNotFound";
