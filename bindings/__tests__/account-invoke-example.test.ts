/**
 * Tests for the account-invoke example module (#936).
 *
 * `getOwner` must validate that the simulated `owner()` return value is a
 * well-formed Stellar address and fail loudly (naming the contract) when the
 * client returns an empty string or a malformed value. The generated client is
 * stubbed with vi.mock so no network access is required.
 */

import { vi } from "vitest";
import { Address, Keypair } from "@stellar/stellar-sdk";

const { ownerMock } = vi.hoisted(() => ({
  ownerMock: vi.fn(),
}));

vi.mock("../src/generated/mux-account", () => ({
  MuxAccountClient: vi.fn(() => ({
    owner: ownerMock,
  })),
}));

import { getOwner } from "../src/examples/account-invoke";

const CONTRACT_ID = "CMUXACCOUNT0000000000000000000000000000000000000000000AA";

describe("account-invoke example — getOwner (#936)", () => {
  beforeEach(() => {
    ownerMock.mockReset();
  });

  it("returns the address string unchanged for a valid address", async () => {
    const valid = Keypair.random().publicKey();
    ownerMock.mockResolvedValue(Address.fromString(valid));

    await expect(getOwner(CONTRACT_ID)).resolves.toBe(valid);
    expect(ownerMock).toHaveBeenCalledTimes(1);
  });

  it("throws a descriptive error naming the contract when the client returns an empty string", async () => {
    ownerMock.mockResolvedValue("");

    await expect(getOwner(CONTRACT_ID)).rejects.toThrow(
      `mux-account ${CONTRACT_ID} owner() returned an empty value`
    );
  });

  it("throws a descriptive error naming the contract when the client returns a malformed address", async () => {
    ownerMock.mockResolvedValue("not-a-stellar-address");

    await expect(getOwner(CONTRACT_ID)).rejects.toThrow(
      `mux-account ${CONTRACT_ID} owner() returned a value that is not a valid Stellar address: not-a-stellar-address`
    );
  });

  it("throws when the client returns a non-address object", async () => {
    ownerMock.mockResolvedValue({ garbage: true });

    await expect(getOwner(CONTRACT_ID)).rejects.toThrow(/not a valid Stellar address/);
  });
});
