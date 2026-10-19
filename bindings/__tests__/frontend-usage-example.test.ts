/**
 * Tests for the frontend-usage example module (issue #941).
 *
 * These tests do not connect to a live network; the generated
 * mux-permissions client is mocked so the granted/denied paths of
 * `checkPermission` can be exercised deterministically.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { Address, Keypair } from "@stellar/stellar-sdk";
import { checkPermission } from "../src/examples/frontend-usage";
import { MuxPermissionsClient } from "../src/generated/mux-permissions";

const hasPermission = vi.fn();

// `index.ts` re-exports every generated client, including the committed
// (broken) `mux-delegation.ts`, which esbuild cannot transform. Mock it so
// this test file can load without pulling in the corrupt generated module.
vi.mock("../src/generated/mux-delegation", () => ({}));

vi.mock("../src/generated/mux-permissions", () => ({
  MuxPermissionsClient: vi.fn(() => ({
    hasPermission,
  })),
}));

const mockPermissionsClient = vi.mocked(MuxPermissionsClient);

describe("checkPermission", () => {
  const contractId = "C" + "A".repeat(55);
  let accountAddress: string;

  beforeEach(() => {
    accountAddress = Keypair.random().publicKey();
    // Only reset the per-call mock; resetting the constructor mock would wipe
    // the `vi.mock` factory implementation.
    hasPermission.mockReset();
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
