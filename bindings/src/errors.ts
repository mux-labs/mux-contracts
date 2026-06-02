import type {
  MuxAccountError,
  MuxBatcherError,
  MuxPermissionsError,
  MuxAccountFactoryError,
  MuxRegistryError,
} from "./types";

export interface HttpErrorResponse {
  statusCode: number;
  message: string;
  errorType: string;
}

type ContractError =
  | MuxAccountError
  | MuxBatcherError
  | MuxPermissionsError
  | MuxAccountFactoryError
  | MuxRegistryError;

/**
 * Maps contract error variants to HTTP status codes.
 * - 401: Unauthorized (authentication/permission issues)
 * - 404: Not Found (missing resources)
 * - 400: Bad Request (invalid input, constraint violations)
 * - 409: Conflict (state conflicts)
 * - 500: Internal Server Error (initialization or unknown errors)
 */
export const ERROR_HTTP_MAP: Record<string, number> = {
  // Authentication/Authorization errors → 401
  Unauthorized: 401,

  // Not Found errors → 404
  DelegateNotFound: 404,
  RoleNotFound: 404,
  AccountNotInRole: 404,
  PermissionNotFound: 404,
  ContractNotFound: 404,

  // Validation/Constraint errors → 400
  InvalidAmount: 400,
  InvalidPeriod: 400,
  SpendLimitExceeded: 400,
  DelegateExpired: 400,
  EmptyBatch: 400,
  BatchTooLarge: 400,
  InvalidAccount: 400,

  // State conflict → 409
  AlreadyInitialized: 409,

  // Security guard violations → 409 Conflict (concurrent/reentrant call)
  ReentrancyDetected: 409,

  // Capacity limits → 409 Conflict
  TooManyAccounts: 409,
  TooManyContracts: 409,

  // Internal/Uninitialized → 500
  NotInitialized: 500,
  RequiredOperationFailed: 500,
  ArithmeticOverflow: 500,
};

/**
 * Converts a contract error to an HTTP error response.
 * Unknown errors default to 500 Internal Server Error.
 */
export function contractErrorToHttp(error: ContractError | string): HttpErrorResponse {
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
  return error in ERROR_HTTP_MAP || true; // Conservative: treat any string as potential error
}
