import { describe, it, expect } from "vitest";
import { resolveContractAddressesFromEnv } from "../src/env-resolve";

/**
 * Shared env-var resolution helper (#988), extracted out of
 * getContractAddresses (network.ts) and loadContractAddresses
 * (addresses.ts) so both stop maintaining their own copy of this loop.
 */
describe("resolveContractAddressesFromEnv (#988)", () => {
  it("falls back to the provided default when no env var is set", () => {
    const result = resolveContractAddressesFromEnv("localnet", {
      muxAccount: "DEFAULT_ACCOUNT",
    });
    expect(result.muxAccount).toBe("DEFAULT_ACCOUNT");
  });

  it("prefers the <NETWORK>_MUX_<NAME>_ID env var over the default", () => {
    process.env.TESTNET_MUX_BATCHER_ID = "ENV_BATCHER";
    try {
      const result = resolveContractAddressesFromEnv("testnet", {
        muxBatcher: "DEFAULT_BATCHER",
      });
      expect(result.muxBatcher).toBe("ENV_BATCHER");
    } finally {
      delete process.env.TESTNET_MUX_BATCHER_ID;
    }
  });

  it("falls back to an empty string when neither env var nor default is set", () => {
    const result = resolveContractAddressesFromEnv("mainnet", {
      muxRecovery: undefined,
    });
    expect(result.muxRecovery).toBe("");
  });

  it("only resolves the keys present on the defaults object", () => {
    const result = resolveContractAddressesFromEnv("localnet", {
      muxAccount: "A",
      muxBatcher: "B",
    });
    expect(Object.keys(result).sort()).toEqual(["muxAccount", "muxBatcher"]);
  });

  it("uppercases the network name for the env-var prefix", () => {
    process.env.MAINNET_MUX_ACCOUNT_ID = "ENV_MAINNET_ACCOUNT";
    try {
      const result = resolveContractAddressesFromEnv("mainnet", {
        muxAccount: "DEFAULT",
      });
      expect(result.muxAccount).toBe("ENV_MAINNET_ACCOUNT");
    } finally {
      delete process.env.MAINNET_MUX_ACCOUNT_ID;
    }
  });
});
