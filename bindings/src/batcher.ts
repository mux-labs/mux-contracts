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
import type { Operation } from "./types";

/**
 * On-chain batching DoS caps (issue #811).
 *
 * The mux-batcher contract enforces these limits fail-closed: any batch that
 * exceeds a cap is rejected before execution and no partial state is written.
 * These constants mirror the contract's `BatchCaps` so clients can pre-validate
 * and surface actionable errors instead of relying on a failed simulation.
 *
 * `maxBatchSize` mirrors the contract's `MAX_BATCH_SIZE` constant
 * (`contracts/mux-batcher/src/lib.rs`, pinned to 50 by
 * `test_max_batch_size_constant_is_50`) and is documented in
 * `docs/batching-limits.md`. `maxAggregateOps` is a client-side
 * pre-validation cap (the contract currently enforces only the operation
 * count on-chain). Both values are asserted against the docs by
 * `__tests__/batcher-limits-sync.test.ts` so drift fails CI.
 */
export const BATCH_CAPS = {
  /** Maximum number of operations allowed in a single batch. */
  maxBatchSize: 50,
  /** Maximum aggregate operation weight (sum of per-op weights) per batch. */
  maxAggregateOps: 256,
} as const;

export type BatchCaps = typeof BATCH_CAPS;

/**
 * Maximum operations per batch — named alias for `BATCH_CAPS.maxBatchSize`,
 * mirroring the on-chain `MAX_BATCH_SIZE` constant so callers can import the
 * familiar name directly.
 */
export const MAX_BATCH_SIZE = BATCH_CAPS.maxBatchSize;

/**
 * Maximum aggregate operation weight per batch — named alias for
 * `BATCH_CAPS.maxAggregateOps` (client-side pre-validation cap).
 */
export const MAX_AGGREGATE_OPS = BATCH_CAPS.maxAggregateOps;

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
 *
 * Also carries an optional `correlationId` (issue #990) — when the caller
 * passes one through to {@link assertBatchWithinCaps}, it is attached here so
 * the cap failure can be tied back to the request that caused it (e.g. in
 * logs/traces that key off the same correlation id).
 */
export class BatchCapError extends Error {
  readonly code: BatchCapErrorCode;
  readonly limit: number;
  readonly actual: number;
  readonly correlationId?: string;

  constructor(
    code: BatchCapErrorCode,
    limit: number,
    actual: number,
    correlationId?: string,
  ) {
    super(
      `${code}: limit=${limit} actual=${actual}` +
        (correlationId ? ` correlationId=${correlationId}` : ""),
    );
    this.name = "BatchCapError";
    this.code = code;
    this.limit = limit;
    this.actual = actual;
    this.correlationId = correlationId;
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
 * @param correlationId Optional id identifying the request that produced
 *   `operations` (issue #990). When provided, it is attached to the thrown
 *   {@link BatchCapError} so the cap failure can be tied back to the request
 *   that caused it.
 * @throws {BatchCapError} when the batch exceeds a cap.
 */
export function assertBatchWithinCaps(
  operations: readonly Operation[],
  caps: BatchCaps = BATCH_CAPS,
  correlationId?: string,
): void {
  if (operations.length > caps.maxBatchSize) {
    throw new BatchCapError(
      BatchCapErrorCode.BatchTooLarge,
      caps.maxBatchSize,
      operations.length,
      correlationId,
    );
  }

  // Sum each operation's weight (defaulting to 1 when unset) rather than
  // just counting operations, so a batch of a few high-weight operations is
  // rejected even when it stays under maxBatchSize.
  const aggregate = operations.reduce((sum, op) => sum + (op.weight ?? 1), 0);
  if (aggregate > caps.maxAggregateOps) {
    throw new BatchCapError(
      BatchCapErrorCode.AggregateLimitExceeded,
      caps.maxAggregateOps,
      aggregate,
      correlationId,
    );
  }
}

// ── Issue #1374: Transaction signing optimization + batch support ─────────────

/** Single operation within a {@link BatchRequest}. */
export type BatchOperation = {
  kind: "transfer" | "invoke" | "approve";
  amount?: bigint;
};

/**
 * A client-side batch request descriptor used by {@link validateBatch},
 * {@link encodeBatch}, and {@link decodeBatch}.
 */
export interface BatchRequest {
  correlationId: string;
  operations: BatchOperation[];
  aggregateOps?: number;
  privileged?: boolean;
  role?: "owner" | "delegate" | "guardian";
  revoked?: boolean;
  expired?: boolean;
  dependencyAvailable?: boolean;
  isWrite?: boolean;
  idempotencyKey?: string;
  network?: string;
  configuredNetwork?: string;
}

/** Maximum number of operations in a single {@link BatchRequest}. */
export const MAX_BATCH_SIZE = 64;

/** Maximum aggregate operation weight in a single {@link BatchRequest}. */
export const MAX_AGGREGATE_OPS = 256;

/**
 * Stable error codes emitted by the batcher validation layer for all
 * structural and authorization rejections.
 */
export const BatcherErrorCode = {
  BatchTooLarge: "BATCH_TOO_LARGE",
  AggregateLimitExceeded: "BATCH_AGGREGATE_LIMIT_EXCEEDED",
  EmptyBatch: "BATCH_EMPTY",
  Unauthorized: "BATCH_UNAUTHORIZED",
  DelegateRevoked: "BATCH_DELEGATE_REVOKED",
  AuthExpired: "BATCH_AUTH_EXPIRED",
  DependencyUnavailable: "BATCH_DEPENDENCY_UNAVAILABLE",
  ReplayedRequest: "BATCH_REPLAYED_REQUEST",
  NetworkMismatch: "BATCH_NETWORK_MISMATCH",
} as const;

export type BatcherErrorCode =
  (typeof BatcherErrorCode)[keyof typeof BatcherErrorCode];

/**
 * Error produced by {@link validateBatch} when a {@link BatchRequest}
 * violates a cap or authorization rule. Carries a stable
 * {@link BatcherErrorCode} and optional `correlationId` for tracing.
 */
export class BatcherError extends Error {
  readonly code: BatcherErrorCode;
  readonly correlationId?: string;

  constructor(
    code: BatcherErrorCode,
    message: string,
    correlationId?: string,
  ) {
    super(message);
    this.name = "BatcherError";
    this.code = code;
    this.correlationId = correlationId;
  }
}

/** Discriminated union returned by {@link validateBatch}. */
export type ValidationResult =
  | { ok: true }
  | { ok: false; error: BatcherError };

/**
 * Validate a {@link BatchRequest} fail-closed. Returns a
 * {@link ValidationResult} rather than throwing so callers can branch on the
 * result without wrapping in try/catch.
 *
 * Rules are checked in a fixed order so the first violation is always
 * reported:
 * 1. Empty batch
 * 2. Batch size cap
 * 3. Aggregate ops cap
 * 4. Idempotency / replay guard
 * 5. Privileged surface requires a role
 * 6. Revoked delegate
 * 7. Expired authorization
 * 8. Dependency unavailable for a write
 * 9. Network mismatch
 *
 * @param request The batch request to validate.
 * @param seen    Optional set used to track idempotency keys across calls.
 *                Pass the same `Set` instance across multiple validations to
 *                detect replays.
 */
export function validateBatch(
  request: BatchRequest,
  seen?: Set<string>,
): ValidationResult {
  const fail = (code: BatcherErrorCode, message: string): ValidationResult => ({
    ok: false,
    error: new BatcherError(code, message, request.correlationId),
  });

  if (request.operations.length === 0) {
    return fail(BatcherErrorCode.EmptyBatch, "Batch must contain at least one operation");
  }

  if (request.operations.length > MAX_BATCH_SIZE) {
    return fail(
      BatcherErrorCode.BatchTooLarge,
      `Batch size ${request.operations.length} exceeds MAX_BATCH_SIZE (${MAX_BATCH_SIZE})`,
    );
  }

  if (
    request.aggregateOps !== undefined &&
    request.aggregateOps > MAX_AGGREGATE_OPS
  ) {
    return fail(
      BatcherErrorCode.AggregateLimitExceeded,
      `Aggregate ops ${request.aggregateOps} exceeds MAX_AGGREGATE_OPS (${MAX_AGGREGATE_OPS})`,
    );
  }

  if (request.idempotencyKey !== undefined) {
    if (seen?.has(request.idempotencyKey)) {
      return fail(
        BatcherErrorCode.ReplayedRequest,
        `Idempotency key "${request.idempotencyKey}" has already been processed`,
      );
    }
    seen?.add(request.idempotencyKey);
  }

  if (request.privileged && !request.role) {
    return fail(
      BatcherErrorCode.Unauthorized,
      "Privileged batch requires an authorized role",
    );
  }

  if (request.privileged && request.revoked) {
    return fail(
      BatcherErrorCode.DelegateRevoked,
      "Delegate has been revoked",
    );
  }

  if (request.privileged && request.expired) {
    return fail(
      BatcherErrorCode.AuthExpired,
      "Authorization has expired",
    );
  }

  if (request.dependencyAvailable === false && request.isWrite === true) {
    return fail(
      BatcherErrorCode.DependencyUnavailable,
      "Dependency is unavailable; write operations are denied",
    );
  }

  if (
    request.network &&
    request.configuredNetwork &&
    request.network !== request.configuredNetwork
  ) {
    return fail(
      BatcherErrorCode.NetworkMismatch,
      `Network "${request.network}" does not match configured network "${request.configuredNetwork}"`,
    );
  }

  return { ok: true };
}

/**
 * Encode a {@link BatchRequest} to a JSON string suitable for transport.
 * `bigint` amounts are serialized as decimal strings so the output is
 * valid JSON.
 */
export function encodeBatch(request: BatchRequest): string {
  return JSON.stringify({
    ...request,
    operations: request.operations.map((op) => ({
      ...op,
      amount: op.amount?.toString(),
    })),
  });
}

/**
 * Decode a JSON string produced by {@link encodeBatch} back into a
 * {@link BatchRequest}. Decimal string amounts are converted back to
 * `bigint`.
 */
export function decodeBatch(encoded: string): BatchRequest {
  const raw = JSON.parse(encoded) as {
    operations: Array<{ kind: "transfer" | "invoke" | "approve"; amount?: string }>;
    [key: string]: unknown;
  };
  return {
    ...(raw as unknown as Omit<BatchRequest, "operations">),
    operations: raw.operations.map((op) => ({
      ...op,
      amount: op.amount !== undefined ? BigInt(op.amount) : undefined,
    })),
  };
}
