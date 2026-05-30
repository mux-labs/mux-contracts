import { SorobanRpc } from "@stellar/stellar-sdk";
import {
  pollTransaction,
  TransactionFailedError,
  TransactionTimeoutError,
} from "../src/horizon";

// Make sleep() a no-op so timeout tests run instantly
// eslint-disable-next-line @typescript-eslint/no-explicit-any
jest.spyOn(global, "setTimeout").mockImplementation(((fn: () => void) => { fn(); return 0; }) as any);

const { GetTransactionStatus } = SorobanRpc.Api;

function makeServer(responses: SorobanRpc.Api.GetTransactionResponse[]): SorobanRpc.Server {
  let call = 0;
  return {
    getTransaction: jest.fn(async () => responses[Math.min(call++, responses.length - 1)]),
  } as unknown as SorobanRpc.Server;
}

const SUCCESS_RESPONSE = {
  status: GetTransactionStatus.SUCCESS,
  returnValue: null,
} as unknown as SorobanRpc.Api.GetSuccessfulTransactionResponse;

const PENDING_RESPONSE = {
  status: GetTransactionStatus.NOT_FOUND,
} as SorobanRpc.Api.GetTransactionResponse;

const FAILED_RESPONSE = {
  status: GetTransactionStatus.FAILED,
  resultXdr: { toXDR: () => "deadbeef" },
} as unknown as SorobanRpc.Api.GetTransactionResponse;

describe("pollTransaction", () => {
  it("resolves immediately on SUCCESS", async () => {
    const server = makeServer([SUCCESS_RESPONSE]);
    const result = await pollTransaction(server, "abc123");
    expect(result.status).toBe(GetTransactionStatus.SUCCESS);
    expect((server.getTransaction as jest.Mock).mock.calls.length).toBe(1);
  });

  it("retries on NOT_FOUND then resolves on SUCCESS", async () => {
    const server = makeServer([PENDING_RESPONSE, PENDING_RESPONSE, SUCCESS_RESPONSE]);
    const result = await pollTransaction(server, "abc123");
    expect(result.status).toBe(GetTransactionStatus.SUCCESS);
    expect((server.getTransaction as jest.Mock).mock.calls.length).toBe(3);
  });

  it("throws TransactionFailedError on FAILED", async () => {
    const server = makeServer([FAILED_RESPONSE]);
    await expect(pollTransaction(server, "abc123")).rejects.toBeInstanceOf(
      TransactionFailedError
    );
  });

  it("throws TransactionTimeoutError after MAX_ATTEMPTS", async () => {
    // Always returns PENDING — will exhaust retries
    const server = {
      getTransaction: jest.fn(async () => PENDING_RESPONSE),
    } as unknown as SorobanRpc.Server;

    await expect(pollTransaction(server, "abc123")).rejects.toBeInstanceOf(
      TransactionTimeoutError
    );
  });

  it("TransactionFailedError carries the hash", async () => {
    const server = makeServer([FAILED_RESPONSE]);
    const err = await pollTransaction(server, "myhash").catch((e) => e);
    expect(err).toBeInstanceOf(TransactionFailedError);
    expect((err as TransactionFailedError).hash).toBe("myhash");
  });

  it("TransactionTimeoutError carries the hash", async () => {
    const server = {
      getTransaction: jest.fn(async () => PENDING_RESPONSE),
    } as unknown as SorobanRpc.Server;
    const err = await pollTransaction(server, "myhash").catch((e) => e);
    expect(err).toBeInstanceOf(TransactionTimeoutError);
    expect((err as TransactionTimeoutError).hash).toBe("myhash");
  });
});
