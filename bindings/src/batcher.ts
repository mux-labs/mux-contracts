/**
 * Batcher namespace — re-exports the MuxBatcherClient together with all
 * batcher-specific types so consumers can import from a single module.
 *
 * Usage (named import):
 *   import { MuxBatcherClient, type Operation, type BatchResult } from "@mux-protocol/contracts";
 *
 * Usage (namespace import):
 *   import { batcher } from "@mux-protocol/contracts";
 *   const client = new batcher.MuxBatcherClient({ ... });
 */

export { MuxBatcherClient } from "./generated/mux-batcher";
export type { MuxBatcherClientOptions } from "./generated/mux-batcher";
export type {
  BatcherMeta,
  BatchOperationKind,
  BatchResult,
  MuxBatcherError,
  Operation,
} from "./types";

/**
 * On-chain batching DoS caps (issue #811).
 *
 * The mux-batcher contract enforces these limits fail-closed: any batch that
 * exceeds a cap is rejected before execution and no partial state is written.
 * These constants mirror the contract's `BatchCaps` so clients can pre-validate
 * and surface actionable errors instead of relying on a failed simulation.
 */
export const BATCH_CAPS = {
  /** Maximum number of operations allowed in a single batch. */
  maxBatchSize: 64,
  /** Maximum aggregate operation weight (sum of per-op weights) per batch. */
  maxAggregateOps: 256,
} as const;

export type BatchCaps = typeof BATCH_CAPS;

/**
 * Stable error codes emitted by the batcher contract for cap violations.
 * Kept in sync with the contract's `BatchError` enum so callers can branch on
 * a code rather than parsing messages.
 */
export const BatchCapErrorCode = {
  /** Batch contains more operations than `maxBatchSize`. */
  BatchTooLarge: "BATCH_TOO_LARGE",
  /** Aggregate operation weight exceeds `maxAggregateOps`. */
  AggregateLimitExceeded: "BATCH_AGGREGATE_LIMIT_EXCEEDED",
  /** Caller is not an authorized owner/delegate/guardian for this batch. */
  Unauthorized: "BATCH_UNAUTHORIZED",
  /** Batch id was already executed (replay / idempotency guard). */
  DuplicateBatch: "BATCH_DUPLICATE",
} as const;

export type BatchCapErrorCode =
  (typeof BatchCapErrorCode)[keyof typeof BatchCapErrorCode];

/**
 * Thrown by {@link assertBatchWithinCaps} when a batch violates an on-chain cap.
 * Carries a stable {@link BatchCapErrorCode} so callers can handle cap failures
 * deterministically (deny-by-default) without string matching.
 */
export class BatchCapError extends Error {
  readonly code: BatchCapErrorCode;
  readonly limit: number;
  readonly actual: number;

  constructor(code: BatchCapErrorCode, limit: number, actual: number) {
    super(`${code}: limit=${limit} actual=${actual}`);
    this.name = "BatchCapError";
    this.code = code;
    this.limit = limit;
    this.actual = actual;
  }
}

/**
 * Fail-closed client-side guard mirroring the on-chain batching caps.
 *
 * Rejects oversized or adversarial batches before they are submitted so the
 * contract's caps are never the only line of defense. The contract remains the
 * source of truth; this is a fast, deterministic pre-check.
 *
 * @param operations Operations about to be batched.
 * @param caps Optional cap overrides (e.g. testnet vs mainnet config).
 * @throws {BatchCapError} when the batch exceeds a cap.
 */
export function assertBatchWithinCaps(
  operations: readonly Operation[],
  caps: BatchCaps = BATCH_CAPS,
): void {
  if (operations.length > caps.maxBatchSize) {
    throw new BatchCapError(
      BatchCapErrorCode.BatchTooLarge,
      caps.maxBatchSize,
      operations.length,
    );
  }

  const aggregate = operations.reduce(
    (sum, op) => sum + (op.weight ?? 1),
    0,
  );
  if (aggregate > caps.maxAggregateOps) {
    throw new BatchCapError(
      BatchCapErrorCode.AggregateLimitExceeded,
      caps.maxAggregateOps,
      aggregate,
    );
  }
}
