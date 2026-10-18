import { describe, it, expect } from "vitest";
import {
  BATCH_CAPS,
  BatchCapError,
  BatchCapErrorCode,
  MAX_AGGREGATE_OPS,
  MAX_BATCH_SIZE,
  assertBatchWithinCaps,
} from "../src/batcher";
import type { Operation } from "../src/types";

/**
 * Batching DoS cap guards (issues #811 / #951).
 *
 * Note: an earlier revision of this file exercised a `validateBatch` /
 * `encodeBatch` API that was removed from `../src/batcher` in a prior
 * refactor, so every test failed at import time. These tests target the
 * public surface that actually exists today: `assertBatchWithinCaps` plus
 * the `BATCH_CAPS` / `MAX_BATCH_SIZE` / `MAX_AGGREGATE_OPS` constants
 * (kept in sync with the contract and docs by batcher-limits-sync.test.ts).
 */

/** Minimal ops — `assertBatchWithinCaps` only inspects the batch shape. */
function makeOps(count: number): Operation[] {
  return Array.from({ length: count }, () => ({
    kind: "Transfer",
  })) as unknown as Operation[];
}

describe("batching DoS caps", () => {
  it("exports MAX_BATCH_SIZE / MAX_AGGREGATE_OPS as aliases of BATCH_CAPS", () => {
    expect(MAX_BATCH_SIZE).toBe(BATCH_CAPS.maxBatchSize);
    expect(MAX_AGGREGATE_OPS).toBe(BATCH_CAPS.maxAggregateOps);
  });

  it("accepts a batch at the max size boundary", () => {
    expect(() => assertBatchWithinCaps(makeOps(MAX_BATCH_SIZE))).not.toThrow();
  });

  it("rejects an oversized batch fail-closed with a stable error code", () => {
    let caught: unknown;
    try {
      assertBatchWithinCaps(makeOps(MAX_BATCH_SIZE + 1));
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(BatchCapError);
    const err = caught as BatchCapError;
    expect(err.code).toBe(BatchCapErrorCode.BatchTooLarge);
    expect(err.limit).toBe(MAX_BATCH_SIZE);
    expect(err.actual).toBe(MAX_BATCH_SIZE + 1);
  });

  it("rejects a batch exceeding the aggregate operation cap", () => {
    // With a looser size cap than aggregate cap, the aggregate check is
    // reachable: 5 ops pass the size check but exceed an aggregate cap of 3.
    const caps = { maxBatchSize: 10, maxAggregateOps: 3 };
    let caught: unknown;
    try {
      assertBatchWithinCaps(makeOps(5), caps);
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(BatchCapError);
    const err = caught as BatchCapError;
    expect(err.code).toBe(BatchCapErrorCode.AggregateLimitExceeded);
    expect(err.limit).toBe(3);
    expect(err.actual).toBe(5);
  });

  it("does not throw for an empty batch", () => {
    expect(() => assertBatchWithinCaps([])).not.toThrow();
  });

  it("surfaces cap violations as BatchCapError instances", () => {
    expect(() => assertBatchWithinCaps(makeOps(MAX_BATCH_SIZE + 1))).toThrow(
      BatchCapError,
    );
  });

  it("includes the stable error code in the message", () => {
    try {
      assertBatchWithinCaps(makeOps(MAX_BATCH_SIZE + 1));
      expect.unreachable("must throw");
    } catch (err) {
      expect((err as Error).message).toContain(BatchCapErrorCode.BatchTooLarge);
    }
  });
});

