import { describe, it, expect, vi } from "vitest";
import { Keypair, StrKey } from "@stellar/stellar-sdk";
import {
  BATCH_CAPS,
  BatchCapError,
  BatchCapErrorCode,
  MAX_AGGREGATE_OPS,
  MAX_BATCH_SIZE,
  assertBatchWithinCaps,
} from "../src/batcher";
import {
  MuxBatcherClient,
  MAX_BATCH_SIZE as CLIENT_MAX_BATCH_SIZE,
  MAX_BATCH_WEIGHT,
  MUX_BATCHER_ERROR_CODES,
  MuxBatcherCapError,
  operationWeight,
  batchWeight,
  assertBatchWithinCaps as assertBatchWithinWeightCaps,
} from "../src/generated/mux-batcher";
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

  it("attaches the correlationId to BatchCapError when provided (#990)", () => {
    let caught: unknown;
    try {
      assertBatchWithinCaps(
        makeOps(MAX_BATCH_SIZE + 1),
        BATCH_CAPS,
        "req-abc-123",
      );
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(BatchCapError);
    const err = caught as BatchCapError;
    expect(err.correlationId).toBe("req-abc-123");
    expect(err.message).toContain("req-abc-123");
  });

  it("leaves correlationId undefined when not provided (#990)", () => {
    let caught: unknown;
    try {
      assertBatchWithinCaps(makeOps(MAX_BATCH_SIZE + 1));
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(BatchCapError);
    expect((caught as BatchCapError).correlationId).toBeUndefined();
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

  /**
   * Regression suite for #985: `assertBatchWithinCaps` must sum per-operation
   * `weight` and reject once the aggregate exceeds `maxAggregateOps`, not
   * just count operations (which made the aggregate check unreachable for
   * any batch within `maxBatchSize`, since count <= maxBatchSize <=
   * maxAggregateOps in practice).
   */
  describe("per-operation weight (#985)", () => {
    function makeWeightedOps(weights: number[]): Operation[] {
      return weights.map((weight) => ({ kind: "Transfer", weight })) as unknown as Operation[];
    }

    it("rejects a small batch whose aggregate weight exceeds maxAggregateOps", () => {
      // Only 4 operations (well under maxBatchSize), but weight 100 each.
      const caps = { maxBatchSize: 10, maxAggregateOps: 256 };
      const ops = makeWeightedOps([100, 100, 100, 100]);
      let caught: unknown;
      try {
        assertBatchWithinCaps(ops, caps);
      } catch (err) {
        caught = err;
      }
      expect(caught).toBeInstanceOf(BatchCapError);
      const err = caught as BatchCapError;
      expect(err.code).toBe(BatchCapErrorCode.AggregateLimitExceeded);
      expect(err.limit).toBe(256);
      expect(err.actual).toBe(400);
    });

    it("accepts a batch whose weighted aggregate sits exactly at the cap", () => {
      const caps = { maxBatchSize: 10, maxAggregateOps: 10 };
      const ops = makeWeightedOps([5, 5]);
      expect(() => assertBatchWithinCaps(ops, caps)).not.toThrow();
    });

    it("treats operations with no weight field as weight 1 (backward compatible)", () => {
      // makeOps() produces operations with no `weight` property at all.
      const caps = { maxBatchSize: 10, maxAggregateOps: 5 };
      expect(() => assertBatchWithinCaps(makeOps(5), caps)).not.toThrow();
      expect(() => assertBatchWithinCaps(makeOps(6), caps)).toThrow(BatchCapError);
    });

    it("mixes weighted and unweighted operations when computing the aggregate", () => {
      const caps = { maxBatchSize: 10, maxAggregateOps: 10 };
      const ops = [
        { kind: "Transfer", weight: 8 },
        { kind: "Transfer" }, // defaults to weight 1
      ] as unknown as Operation[];
      expect(() => assertBatchWithinCaps(ops, caps)).not.toThrow();

      const overLimitOps = [
        { kind: "Transfer", weight: 9 },
        { kind: "Transfer" }, // defaults to weight 1, total = 10, still within cap
        { kind: "Transfer" }, // total = 11, now over cap
      ] as unknown as Operation[];
      expect(() => assertBatchWithinCaps(overLimitOps, caps)).toThrow(BatchCapError);
    });
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

/**
 * `MuxBatcherClient.estimateFees` client-side caps (issue #942).
 * The guard must fail closed before any RPC call is attempted.
 */
describe("MuxBatcherClient.estimateFees bounds", () => {
  const CONTRACT_ID = StrKey.encodeContract(new Uint8Array(32));
  const NETWORK_PASSPHRASE = "Test SDF Network ; September 2015";
  const RPC_URL = "https://127.0.0.1:8000";

  /** Account shape accepted by `TransactionBuilder` without a live RPC. */
  function mockAccount(signer: Keypair) {
    return {
      accountId: () => signer.publicKey(),
      sequence: "0",
      sequenceNumber: () => 0,
      incrementSequenceNumber: () => {},
    };
  }

  function buildClient() {
    const client = new MuxBatcherClient({
      contractId: CONTRACT_ID,
      networkPassphrase: NETWORK_PASSPHRASE,
      rpcUrl: RPC_URL,
    });
    const signer = Keypair.random();
    const server = {
      getAccount: vi.fn().mockResolvedValue(mockAccount(signer)),
      simulateTransaction: vi.fn().mockResolvedValue({
        result: { retval: { value: () => 4200 } },
      }),
    };
    (client as unknown as { server: unknown }).server = server;
    return { client, server };
  }

  it("throws MuxBatcherCapError with code EmptyBatch for opCount 0, without submitting a transaction", async () => {
    const { client, server } = buildClient();
    await expect(client.estimateFees(Keypair.random(), 0)).rejects.toMatchObject(
      {
        name: "MuxBatcherCapError",
        code: MUX_BATCHER_ERROR_CODES.EmptyBatch,
      }
    );
    expect(server.getAccount).not.toHaveBeenCalled();
    expect(server.simulateTransaction).not.toHaveBeenCalled();
  });

  it("throws MuxBatcherCapError with code BatchTooLarge for opCount above MAX_BATCH_SIZE, without submitting a transaction", async () => {
    const { client, server } = buildClient();
    await expect(
      client.estimateFees(Keypair.random(), CLIENT_MAX_BATCH_SIZE + 1)
    ).rejects.toMatchObject({
      name: "MuxBatcherCapError",
      code: MUX_BATCHER_ERROR_CODES.BatchTooLarge,
    });
    expect(server.getAccount).not.toHaveBeenCalled();
    expect(server.simulateTransaction).not.toHaveBeenCalled();
  });

  it("does not throw the cap error for opCount 1 and proceeds to simulate", async () => {
    const { client, server } = buildClient();
    const fee = await client.estimateFees(Keypair.random(), 1);
    expect(fee).toBe(4200);
    expect(server.simulateTransaction).toHaveBeenCalledTimes(1);
  });

  it("does not throw the cap error for opCount equal to MAX_BATCH_SIZE", async () => {
    const { client, server } = buildClient();
    const fee = await client.estimateFees(
      Keypair.random(),
      CLIENT_MAX_BATCH_SIZE
    );
    expect(fee).toBe(4200);
    expect(server.simulateTransaction).toHaveBeenCalledTimes(1);
  });

  it("client MAX_BATCH_SIZE is the on-chain cap (100)", () => {
    expect(CLIENT_MAX_BATCH_SIZE).toBe(100);
  });
});


/**
 * batchWeight / operationWeight boundary coverage against MAX_BATCH_WEIGHT
 * (issue #943).
 *
 * The weight arithmetic is the only thing standing between a client and a
 * wasted round-trip that the contract will reject, so the exact boundary must
 * be pinned: `MAX_BATCH_WEIGHT` passes, `MAX_BATCH_WEIGHT + 1` fails with
 * `BatchWeightExceeded`, and `operationWeight` accounts for `args.length`.
 *
 * Batches are sized so the weight — not the op count — is the limiting
 * factor: `MAX_BATCH_SIZE` (100) ops × weight 100 = exactly MAX_BATCH_WEIGHT.
 */
describe("batchWeight / MAX_BATCH_WEIGHT boundaries (issue #943)", () => {
  /** An op with `argCount` args — only `args.length` feeds the weight. */
  function makeWeightedOp(argCount: number): Operation {
    return {
      kind: "Transfer",
      args: Array.from({ length: argCount }, () => ({})),
    } as unknown as Operation;
  }

  /** `count` ops whose weights sum to exactly `totalWeight` (count | totalWeight). */
  function makeBatchAtWeight(totalWeight: number, count: number): Operation[] {
    expect(totalWeight % count).toBe(0);
    const weightPerOp = totalWeight / count; // = 1 + argsPerOp
    return Array.from({ length: count }, () => makeWeightedOp(weightPerOp - 1));
  }

  it("operationWeight is 1 for a zero-arg op and grows by one per argument", () => {
    expect(operationWeight({ args: [] } as unknown as Operation)).toBe(1);
    expect(operationWeight(makeWeightedOp(0))).toBe(1);
    expect(operationWeight(makeWeightedOp(1))).toBe(2);
    expect(operationWeight(makeWeightedOp(5))).toBe(6);
    // One more argument → exactly one more weight.
    for (const n of [0, 1, 2, 10, 99]) {
      expect(operationWeight(makeWeightedOp(n + 1))).toBe(
        operationWeight(makeWeightedOp(n)) + 1
      );
    }
  });

  it("batchWeight sums operationWeight across the batch", () => {
    const ops = [makeWeightedOp(0), makeWeightedOp(3), makeWeightedOp(10)];
    expect(batchWeight(ops)).toBe(1 + 4 + 11);
    expect(batchWeight([])).toBe(0);
  });

  it("accepts a batch whose batchWeight is exactly MAX_BATCH_WEIGHT", () => {
    // 100 ops × weight 100 (99 args each) = 10_000 = MAX_BATCH_WEIGHT.
    // The op-count cap (MAX_BATCH_SIZE = 100) is satisfied, so weight is
    // the limiting factor at the boundary.
    const ops = makeBatchAtWeight(MAX_BATCH_WEIGHT, 100);
    expect(ops.length).toBe(CLIENT_MAX_BATCH_SIZE);
    expect(batchWeight(ops)).toBe(MAX_BATCH_WEIGHT);
    expect(() => assertBatchWithinWeightCaps(ops)).not.toThrow();
  });

  it("throws BatchWeightExceeded at MAX_BATCH_WEIGHT + 1", () => {
    // 99 ops × weight 100 (99 args) + 1 op × weight 101 (100 args)
    // = 9_900 + 101 = 10_001, still within the 100-op cap.
    const ops = Array.from({ length: 99 }, () => makeWeightedOp(99));
    ops.push(makeWeightedOp(100));
    const expectedWeight = MAX_BATCH_WEIGHT + 1;
    expect(batchWeight(ops)).toBe(expectedWeight);
    expect(ops.length).toBeLessThanOrEqual(CLIENT_MAX_BATCH_SIZE);

    let caught: unknown;
    try {
      assertBatchWithinWeightCaps(ops);
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(MuxBatcherCapError);
    const err = caught as MuxBatcherCapError;
    expect(err.code).toBe(MUX_BATCHER_ERROR_CODES.BatchWeightExceeded);
    expect(err.message).toContain(String(expectedWeight));
    expect(err.message).toContain(String(MAX_BATCH_WEIGHT));
  });
});
