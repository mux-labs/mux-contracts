import { afterEach, describe, expect, it, vi } from "vitest";
import { SorobanRpc } from "@stellar/stellar-sdk";
import {
  DEFAULT_MAX_ATTEMPTS,
  TransactionFailedError,
  TransactionTimeoutError,
  pollTransaction,
} from "../src/horizon";

const { GetTransactionStatus } = SorobanRpc.Api;
const success = {
  status: GetTransactionStatus.SUCCESS,
  returnValue: null,
} as unknown as SorobanRpc.Api.GetSuccessfulTransactionResponse;
const pending = {
  status: GetTransactionStatus.NOT_FOUND,
} as unknown as SorobanRpc.Api.GetTransactionResponse;
const failed = {
  status: GetTransactionStatus.FAILED,
  resultXdr: { toXDR: () => "deadbeef" },
} as unknown as SorobanRpc.Api.GetTransactionResponse;

type MockServer = SorobanRpc.Server & { getTransaction: ReturnType<typeof vi.fn> };
function makeServer(responses: unknown[]): MockServer {
  let index = 0;
  return {
    getTransaction: vi.fn(async () => responses[Math.min(index++, responses.length - 1)]),
  } as unknown as MockServer;
}

describe("pollTransaction Horizon/RPC reliability", () => {
  afterEach(() => vi.restoreAllMocks());

  it("resolves immediately on SUCCESS", async () => {
    const server = makeServer([success]);
    await expect(pollTransaction(server, "abc123", { intervalMs: 0 })).resolves.toBe(success);
    expect(server.getTransaction).toHaveBeenCalledTimes(1);
  });

  it("retries NOT_FOUND and PENDING before success", async () => {
    const server = makeServer([pending, pending, success]);
    await expect(
      pollTransaction(server, "abc123", { intervalMs: 0, maxAttempts: 5 }),
    ).resolves.toBe(success);
    expect(server.getTransaction).toHaveBeenCalledTimes(3);
  });

  it("retries a transient RPC exception", async () => {
    const server = makeServer([new Error("gateway unavailable"), success]);
    await expect(
      pollTransaction(server, "abc123", { intervalMs: 0, maxAttempts: 3 }),
    ).resolves.toBe(success);
    expect(server.getTransaction).toHaveBeenCalledTimes(2);
  });

  it("surfaces FAILED with hash and result XDR", async () => {
    const server = makeServer([failed]);
    const error = await pollTransaction(server, "myhash", { intervalMs: 0 }).catch((e) => e);
    expect(error).toBeInstanceOf(TransactionFailedError);
    expect(error).toMatchObject({ hash: "myhash", resultXdr: "deadbeef" });
  });

  it("times out after exactly maxAttempts without an extra delay", async () => {
    const server = makeServer([pending]);
    const timeout = vi.spyOn(globalThis, "setTimeout").mockImplementation((callback) => {
      callback();
      return 0 as unknown as ReturnType<typeof setTimeout>;
    });
    const error = await pollTransaction(server, "myhash", {
      intervalMs: 1,
      maxAttempts: 3,
    }).catch((e) => e);
    expect(error).toBeInstanceOf(TransactionTimeoutError);
    expect(error).toMatchObject({ hash: "myhash", attempts: 3 });
    expect(server.getTransaction).toHaveBeenCalledTimes(3);
    expect(timeout).toHaveBeenCalledTimes(2);
  });

  it("uses the documented default retry bound", async () => {
    const server = makeServer([pending]);
    const timeout = vi.spyOn(globalThis, "setTimeout").mockImplementation((callback) => {
      callback();
      return 0 as unknown as ReturnType<typeof setTimeout>;
    });
    await expect(pollTransaction(server, "myhash")).rejects.toBeInstanceOf(TransactionTimeoutError);
    expect(server.getTransaction).toHaveBeenCalledTimes(DEFAULT_MAX_ATTEMPTS);
    expect(timeout).toHaveBeenCalledTimes(DEFAULT_MAX_ATTEMPTS - 1);
  });

  it("rejects empty hashes and invalid retry options", async () => {
    const server = makeServer([success]);
    await expect(pollTransaction(server, "  ")).rejects.toThrow("hash must not be empty");
    await expect(pollTransaction(server, "hash", { maxAttempts: 0 })).rejects.toThrow(
      "maxAttempts must be a positive integer",
    );
    await expect(pollTransaction(server, "hash", { intervalMs: -1 })).rejects.toThrow(
      "intervalMs must be a non-negative",
    );
  });
});
