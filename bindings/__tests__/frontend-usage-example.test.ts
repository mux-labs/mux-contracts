/**
 * Tests for the frontend-usage example module (issue #941, #995).
 *
 * These tests do not connect to a live network; the generated clients are
 * mocked so every exported helper in this example (not just `checkPermission`)
 * is exercised against the shape of the real generated clients: constructor
 * options, argument marshalling (e.g. `Address.fromString`), and the
 * resolved return value / error mapping.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { Address, Keypair } from "@stellar/stellar-sdk";
import {
  checkPermission,
  fetchAccountOwner,
  grantDelegate,
  executeBatch,
  bootstrapNetwork,
} from "../src/examples/frontend-usage";
import { MuxPermissionsClient } from "../src/generated/mux-permissions";
import { MuxAccountClient } from "../src/generated/mux-account";
import { MuxBatcherClient } from "../src/generated/mux-batcher";
import { DEFAULT_ADDRESSES } from "../src/addresses-config";

const hasPermission = vi.fn();
const owner = vi.fn();
const setDelegate = vi.fn();
const executeBatchFn = vi.fn();

// `index.ts` re-exports every generated client, including the committed
// (broken) `mux-delegation.ts`, which esbuild cannot transform. Mock it so
// this test file can load without pulling in the corrupt generated module.
vi.mock("../src/generated/mux-delegation", () => ({}));

vi.mock("../src/generated/mux-permissions", () => ({
  MuxPermissionsClient: vi.fn(() => ({
    hasPermission,
  })),
}));

vi.mock("../src/generated/mux-account", () => ({
  MuxAccountClient: vi.fn(() => ({
    owner,
    setDelegate,
  })),
}));

vi.mock("../src/generated/mux-batcher", () => ({
  MuxBatcherClient: vi.fn(() => ({
    executeBatch: executeBatchFn,
  })),
}));

const mockPermissionsClient = vi.mocked(MuxPermissionsClient);
const mockAccountClient = vi.mocked(MuxAccountClient);
const mockBatcherClient = vi.mocked(MuxBatcherClient);

describe("checkPermission", () => {
  const contractId = "C" + "A".repeat(55);
  let accountAddress: string;

  beforeEach(() => {
    accountAddress = Keypair.random().publicKey();
    // Only reset the per-call mocks; resetting the constructor mocks would
    // wipe the `vi.mock` factory implementations.
    hasPermission.mockReset();
    owner.mockReset();
    setDelegate.mockReset();
    executeBatchFn.mockReset();
  });

  it("returns true when the permissions client grants the permission", async () => {
    hasPermission.mockResolvedValue(true);

    const result = await checkPermission(contractId, accountAddress, "deposit");

    expect(result).toBe(true);
    expect(hasPermission).toHaveBeenCalledTimes(1);
    const [signer, address, permission] = hasPermission.mock.calls[0] as [
      Keypair,
      ReturnType<typeof Address.fromString>,
      string
    ];
    expect(signer).toBeInstanceOf(Keypair);
    expect(address.toString()).toBe(accountAddress);
    expect(permission).toBe("deposit");
    expect(mockPermissionsClient).toHaveBeenCalledTimes(1);
  });

  it("returns false when the permissions client denies the permission", async () => {
    hasPermission.mockResolvedValue(false);

    const result = await checkPermission(
      contractId,
      accountAddress,
      "withdraw"
    );

    expect(result).toBe(false);
    expect(hasPermission).toHaveBeenCalledTimes(1);
  });

  it("constructs the mux-permissions client from the resolved network config", async () => {
    hasPermission.mockResolvedValue(true);
    mockPermissionsClient.mockClear();

    await checkPermission(contractId, accountAddress, "deposit");

    const options = mockPermissionsClient.mock.calls[0][0] as {
      contractId: string;
      networkPassphrase: string;
      rpcUrl: string;
    };
    expect(options.contractId).toBe(contractId);
    expect(typeof options.networkPassphrase).toBe("string");
    expect(typeof options.rpcUrl).toBe("string");
  });
});

describe("fetchAccountOwner", () => {
  const contractId = "C" + "B".repeat(55);

  beforeEach(() => {
    owner.mockReset();
    mockAccountClient.mockClear();
  });

  it("returns the owner address resolved by the generated mux-account client", async () => {
    const ownerAddress = Keypair.random().publicKey();
    owner.mockResolvedValue(Address.fromString(ownerAddress));

    const result = await fetchAccountOwner(contractId);

    expect(result).toBe(ownerAddress);
    expect(owner).toHaveBeenCalledTimes(1);
    const [signer] = owner.mock.calls[0] as [Keypair];
    expect(signer).toBeInstanceOf(Keypair);

    const options = mockAccountClient.mock.calls[0][0] as {
      contractId: string;
      networkPassphrase: string;
      rpcUrl: string;
    };
    expect(options.contractId).toBe(contractId);
    expect(typeof options.networkPassphrase).toBe("string");
    expect(typeof options.rpcUrl).toBe("string");
  });
});

describe("grantDelegate", () => {
  const contractId = "C" + "C".repeat(55);
  let ownerKeypair: Keypair;
  let delegateAddress: string;

  beforeEach(() => {
    ownerKeypair = Keypair.random();
    delegateAddress = Keypair.random().publicKey();
    setDelegate.mockReset();
    mockAccountClient.mockClear();
  });

  it("calls setDelegate on the generated mux-account client with marshalled args", async () => {
    setDelegate.mockResolvedValue(undefined);
    const expiresAt = 1234567890n;

    await grantDelegate(contractId, ownerKeypair, delegateAddress, expiresAt, true);

    expect(setDelegate).toHaveBeenCalledTimes(1);
    const [signer, address, exp, canSpend] = setDelegate.mock.calls[0] as [
      Keypair,
      ReturnType<typeof Address.fromString>,
      bigint,
      boolean
    ];
    expect(signer).toBe(ownerKeypair);
    expect(address.toString()).toBe(delegateAddress);
    expect(exp).toBe(expiresAt);
    expect(canSpend).toBe(true);
  });

  it("maps a contract error to an HTTP-style error and rethrows", async () => {
    setDelegate.mockRejectedValue("MuxAccountDelegateExpired");

    await expect(
      grantDelegate(contractId, ownerKeypair, delegateAddress, 1n, false)
    ).rejects.toThrow(/Contract error/);
  });
});

describe("executeBatch", () => {
  const batcherContractId = "C" + "D".repeat(55);
  let callerKeypair: Keypair;

  beforeEach(() => {
    callerKeypair = Keypair.random();
    executeBatchFn.mockReset();
    mockBatcherClient.mockClear();
  });

  it("marshals operations and calls executeBatch on the generated mux-batcher client", async () => {
    executeBatchFn.mockResolvedValue({ successCount: 1, failureCount: 0 });
    const target = Keypair.random().publicKey();

    const result = await executeBatch(batcherContractId, callerKeypair, [
      { target, fnName: "noop", args: [], requireSuccess: true, kind: "Invoke" as any },
    ]);

    expect(result).toEqual({ successCount: 1, failureCount: 0 });
    expect(executeBatchFn).toHaveBeenCalledTimes(1);
    const [signer, callerAddress, ops] = executeBatchFn.mock.calls[0] as [
      Keypair,
      ReturnType<typeof Address.fromString>,
      Array<{ target: ReturnType<typeof Address.fromString> }>
    ];
    expect(signer).toBe(callerKeypair);
    expect(callerAddress.toString()).toBe(callerKeypair.publicKey());
    expect(ops[0].target.toString()).toBe(target);
  });

  it("maps a batch contract error to an HTTP-style error and rethrows", async () => {
    executeBatchFn.mockRejectedValue("MuxBatcherOperationFailed");

    await expect(
      executeBatch(batcherContractId, callerKeypair, [
        {
          target: Keypair.random().publicKey(),
          fnName: "noop",
          args: [],
          requireSuccess: true,
          kind: "Invoke" as any,
        },
      ])
    ).rejects.toThrow(/Batch error/);
  });
});

describe("bootstrapNetwork", () => {
  const originalNetwork = process.env.SOROBAN_NETWORK;

  afterEach(() => {
    if (originalNetwork === undefined) {
      delete process.env.SOROBAN_NETWORK;
    } else {
      process.env.SOROBAN_NETWORK = originalNetwork;
    }
  });

  it("resolves validated addresses for the configured network", () => {
    process.env.SOROBAN_NETWORK = "testnet";

    const result = bootstrapNetwork();

    expect(result.networkName).toBe("testnet");
    expect(result.addresses.muxAccount).toBe(DEFAULT_ADDRESSES.testnet.muxAccount);
    expect(result.network.networkPassphrase).toBeTypeOf("string");
  });

  it("defaults to localnet when SOROBAN_NETWORK is unset", () => {
    delete process.env.SOROBAN_NETWORK;

    const result = bootstrapNetwork();

    expect(result.networkName).toBe("localnet");
    expect(result.addresses.muxAccount).toBe(DEFAULT_ADDRESSES.localnet.muxAccount);
  });
});
