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
