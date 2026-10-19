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
  MUX_BATCHER_ERROR_CODES,
  MuxBatcherCapError,
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

