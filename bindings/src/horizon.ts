import { SorobanRpc } from "@stellar/stellar-sdk";

/** Default milliseconds between poll attempts. */
export const DEFAULT_POLL_INTERVAL_MS = 1_500;
/** Default maximum number of RPC attempts before giving up (~30 seconds). */
export const DEFAULT_MAX_ATTEMPTS = 20;

export interface PollTransactionOptions {
  /** Delay between attempts; set to 0 in tests or latency-sensitive callers. */
  intervalMs?: number;
  /** Maximum calls to getTransaction, including the first call. */
  maxAttempts?: number;
}

export class TransactionTimeoutError extends Error {
  constructor(
    public readonly hash: string,
    public readonly attempts: number = DEFAULT_MAX_ATTEMPTS,
  ) {
    super(`Transaction ${hash} not confirmed after ${attempts} attempts`);
    this.name = "TransactionTimeoutError";
  }
}

export class TransactionFailedError extends Error {
  constructor(
    public readonly hash: string,
    public readonly resultXdr: string,
  ) {
    super(`Transaction ${hash} failed on-chain`);
    this.name = "TransactionFailedError";
  }
}

/**
 * Thrown by {@link pollTransaction} immediately (no retries consumed) when
 * the error raised by `getTransaction` is known to be permanent: a
 * malformed hash the RPC will never accept, or an RPC rejection code that
 * cannot resolve itself on retry. Distinguishing these from transient
 * transport/gateway errors keeps a bad hash or a permanent rejection from
 * burning all `maxAttempts` before failing.
 */
export class NonRetryableRpcError extends Error {
  constructor(
    public readonly hash: string,
    public readonly cause: unknown,
  ) {
    super(
      `Transaction ${hash} polling failed with a non-retryable RPC error: ${describeError(cause)}`
    );
    this.name = "NonRetryableRpcError";
  }
}

/** RPC error codes that indicate a permanent rejection, never transient. */
const NON_RETRYABLE_RPC_CODES = new Set([-32602, -32600, -32601]);

function describeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

/**
 * True when `error` represents a permanent RPC rejection rather than a
 * transient transport/gateway failure. Covers JSON-RPC error objects with a
 * known non-retryable `code` (invalid params, invalid request, method not
 * found) as well as HTTP-style 4xx status codes, which indicate the request
 * itself is invalid and will fail identically on every retry.
 */
function isNonRetryableRpcError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const err = error as { code?: unknown; status?: unknown };
  if (typeof err.code === "number" && NON_RETRYABLE_RPC_CODES.has(err.code)) {
    return true;
  }
  if (typeof err.status === "number" && err.status >= 400 && err.status < 500) {
    return true;
  }
  return false;
}

/**
 * Poll Soroban RPC until a transaction is confirmed or reaches a terminal
 * state. NOT_FOUND and PENDING are retried; FAILED is surfaced with its result
 * XDR. Transient RPC errors are retried so a short Horizon/RPC interruption
 * does not turn a submitted transaction into a false failure.
 */
export async function pollTransaction(
  server: SorobanRpc.Server,
  hash: string,
  options: PollTransactionOptions = {},
): Promise<SorobanRpc.Api.GetSuccessfulTransactionResponse> {
  if (!hash.trim()) {
    throw new TypeError("Transaction hash must not be empty");
  }

  const intervalMs = options.intervalMs ?? DEFAULT_POLL_INTERVAL_MS;
  const maxAttempts = options.maxAttempts ?? DEFAULT_MAX_ATTEMPTS;
  if (!Number.isFinite(intervalMs) || intervalMs < 0) {
    throw new RangeError("intervalMs must be a non-negative finite number");
  }
  if (!Number.isInteger(maxAttempts) || maxAttempts < 1) {
    throw new RangeError("maxAttempts must be a positive integer");
  }

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      const response = await server.getTransaction(hash);
      if (response.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
        return response as SorobanRpc.Api.GetSuccessfulTransactionResponse;
      }
      if (response.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
        const resultXdr =
          (response as SorobanRpc.Api.GetFailedTransactionResponse).resultXdr?.toXDR("base64") ?? "";
        throw new TransactionFailedError(hash, resultXdr);
      }
    } catch (error) {
      // A terminal contract failure is not transient and must be returned now.
      if (error instanceof TransactionFailedError) {
        throw error;
      }
      // A permanent RPC rejection (bad params/method, 4xx) will fail
      // identically on every retry, so fail fast instead of burning attempts.
      if (isNonRetryableRpcError(error)) {
        throw new NonRetryableRpcError(hash, error);
      }
      // Other errors (transport failures, malformed gateway responses, and
      // NOT_FOUND/PENDING responses) consume this attempt and are retried.
    }

    if (attempt < maxAttempts && intervalMs > 0) {
      await sleep(intervalMs);
    }
  }

  throw new TransactionTimeoutError(hash, maxAttempts);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
