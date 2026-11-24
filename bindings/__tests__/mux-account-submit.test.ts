import { describe, expect, it, vi, beforeEach } from "vitest";
import { Keypair, Transaction } from "@stellar/stellar-sdk";

const { simulateTransaction, sendTransaction, assembleTransactionMock, pollTransactionMock } =
  vi.hoisted(() => ({
    simulateTransaction: vi.fn(),
    sendTransaction: vi.fn(),
    assembleTransactionMock: vi.fn(),
    pollTransactionMock: vi.fn(async () => ({ status: "SUCCESS" as const })),
  }));

vi.mock("@stellar/stellar-sdk", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@stellar/stellar-sdk")>();
  return {
    ...actual,
    SorobanRpc: {
      ...actual.SorobanRpc,
      assembleTransaction: (...args: unknown[]) => assembleTransactionMock(...args),
      Server: class {
        simulateTransaction = simulateTransaction;
        sendTransaction = sendTransaction;
        constructor(_url: string, _opts?: unknown) {}
      },
    },
  };
});

vi.mock("../src/horizon", () => ({
  pollTransaction: pollTransactionMock,
}));

import { MuxAccountClient } from "../src/generated/mux-account";

const CONTRACT_ID = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";
const NETWORK_PASSPHRASE = "Test SDF Network ; September 2015";
const RPC_URL = "https://soroban-testnet.stellar.org";

const TX_HASH = "abc123hash789";

function setupServerMocks() {
  simulateTransaction.mockResolvedValue({
    _parsed: true,
    transactionData: {},
    events: [],
    minResourceFee: "100000",
    result: { auth: [], retval: undefined },
    cost: { cpuInsns: "0", memBytes: "0" },
    latestLedger: 1,
  });
  sendTransaction.mockResolvedValue({
    status: "PENDING" as const,
    hash: TX_HASH,
  });
  assembleTransactionMock.mockReturnValue({ build: vi.fn(() => ({ sign: vi.fn() })) });
}

describe("MuxAccountClient.submit returns the tx hash", () => {
  let client: MuxAccountClient;
  let signer: Keypair;

  beforeEach(() => {
    vi.clearAllMocks();
    setupServerMocks();
    client = new MuxAccountClient({
      contractId: CONTRACT_ID,
      networkPassphrase: NETWORK_PASSPHRASE,
      rpcUrl: RPC_URL,
    });
    signer = Keypair.random();
  });

  it("returns Promise<string> resolving to sendTransaction's hash", async () => {
    const tx = { sign: vi.fn() } as unknown as Transaction;
    const submit = (client as unknown as {
      submit: (tx: Transaction, signer: Keypair) => Promise<string>;
    }).submit.bind(client);

    const hash = await submit(tx, signer);

    expect(hash).toBe(TX_HASH);
    expect(typeof hash).toBe("string");
    expect(sendTransaction).toHaveBeenCalledOnce();
  });

  it("still awaits pollTransaction before resolving", async () => {
    const tx = { sign: vi.fn() } as unknown as Transaction;
    const submit = (client as unknown as {
      submit: (tx: Transaction, signer: Keypair) => Promise<string>;
    }).submit.bind(client);

    await submit(tx, signer);

    expect(pollTransactionMock).toHaveBeenCalledOnce();
    expect(pollTransactionMock.mock.calls[0]?.[1]).toBe(TX_HASH);
  });

  it("initialize/setDelegate/removeDelegate/setSpendLimit are typed Promise<string>", () => {
    // Type-level assertion: the public mutating methods must expose the hash.
    const initFn: (client: MuxAccountClient) => Promise<string> = (c) =>
      c.initialize(signer, null as never, []);
    const setDelFn: (client: MuxAccountClient) => Promise<string> = (c) =>
      c.setDelegate(signer, null as never, 1n, false);
    const remDelFn: (client: MuxAccountClient) => Promise<string> = (c) =>
      c.removeDelegate(signer, null as never);
    const setLimitFn: (client: MuxAccountClient) => Promise<string> = (c) =>
      c.setSpendLimit(signer, null as never, 1n, 1);

    expect(typeof initFn).toBe("function");
    expect(typeof setDelFn).toBe("function");
    expect(typeof remDelFn).toBe("function");
    expect(typeof setLimitFn).toBe("function");
  });

  it("throws when sendTransaction returns ERROR (no hash to return)", async () => {
    sendTransaction.mockResolvedValueOnce({
      status: "ERROR" as const,
      errorResult: { result: "txFailed" },
    });
    const tx = { sign: vi.fn() } as unknown as Transaction;
    const submit = (client as unknown as {
      submit: (tx: Transaction, signer: Keypair) => Promise<string>;
    }).submit.bind(client);

    await expect(submit(tx, signer)).rejects.toThrow("Transaction failed");
  });
});
