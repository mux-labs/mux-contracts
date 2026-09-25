import { describe, it, expect } from "vitest";
import {
  BatcherError,
  BatcherErrorCode,
  MAX_BATCH_SIZE,
  MAX_AGGREGATE_OPS,
  validateBatch,
  encodeBatch,
  decodeBatch,
  type BatchRequest,
} from "../src/batcher";

function makeRequest(overrides: Partial<BatchRequest> = {}): BatchRequest {
  return {
    correlationId: "corr-1",
    operations: [{ kind: "transfer", amount: 1n }],
    ...overrides,
  };
}

describe("batching DoS caps", () => {
  it("accepts a batch at the max size boundary", () => {
    const operations = Array.from({ length: MAX_BATCH_SIZE }, () => ({
      kind: "transfer" as const,
      amount: 1n,
    }));
    const result = validateBatch(makeRequest({ operations }));
    expect(result.ok).toBe(true);
  });

  it("rejects an oversized batch fail-closed with a stable error code", () => {
    const operations = Array.from({ length: MAX_BATCH_SIZE + 1 }, () => ({
      kind: "transfer" as const,
      amount: 1n,
    }));
    const result = validateBatch(makeRequest({ operations }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.BatchTooLarge);
      expect(result.error.correlationId).toBe("corr-1");
    }
  });

  it("rejects a batch exceeding the aggregate operation cap", () => {
    const operations = Array.from({ length: MAX_BATCH_SIZE }, () => ({
      kind: "transfer" as const,
      amount: 1n,
    }));
    const result = validateBatch(
      makeRequest({ operations, aggregateOps: MAX_AGGREGATE_OPS + 1 }),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.AggregateLimitExceeded);
    }
  });

  it("rejects an empty batch", () => {
    const result = validateBatch(makeRequest({ operations: [] }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.EmptyBatch);
    }
  });

  it("denies privileged batch surfaces by default without an authorized role", () => {
    const result = validateBatch(
      makeRequest({ privileged: true, role: undefined }),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.Unauthorized);
    }
  });

  it("rejects a revoked delegate", () => {
    const result = validateBatch(
      makeRequest({ privileged: true, role: "delegate", revoked: true }),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.DelegateRevoked);
    }
  });

  it("rejects an expired authorization", () => {
    const result = validateBatch(
      makeRequest({ privileged: true, role: "guardian", expired: true }),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.AuthExpired);
    }
  });

  it("fails closed on dependency outage for writes", () => {
    const result = validateBatch(
      makeRequest({ dependencyAvailable: false, isWrite: true }),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.DependencyUnavailable);
    }
  });

  it("rejects a replayed batch via idempotency key", () => {
    const seen = new Set<string>();
    const first = validateBatch(makeRequest({ idempotencyKey: "idem-1" }), seen);
    expect(first.ok).toBe(true);
    const replay = validateBatch(makeRequest({ idempotencyKey: "idem-1" }), seen);
    expect(replay.ok).toBe(false);
    if (!replay.ok) {
      expect(replay.error.code).toBe(BatcherErrorCode.ReplayedRequest);
    }
  });

  it("rejects a mainnet batch when configured for testnet", () => {
    const result = validateBatch(
      makeRequest({ network: "mainnet", configuredNetwork: "testnet" }),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(BatcherErrorCode.NetworkMismatch);
    }
  });

  it("round-trips a valid batch through encode/decode", () => {
    const request = makeRequest();
    const encoded = encodeBatch(request);
    const decoded = decodeBatch(encoded);
    expect(decoded.correlationId).toBe(request.correlationId);
    expect(decoded.operations).toHaveLength(1);
  });

  it("surfaces cap violations as BatcherError instances", () => {
    const operations = Array.from({ length: MAX_BATCH_SIZE + 1 }, () => ({
      kind: "transfer" as const,
      amount: 1n,
    }));
    const result = validateBatch(makeRequest({ operations }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBeInstanceOf(BatcherError);
    }
  });
});
