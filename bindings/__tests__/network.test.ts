import { describe, it, expect } from "vitest";
import {
  assertSameNetwork,
  NETWORK_CONFIGS,
  resolveNetworkConfig,
} from "../src/network";
import { MuxError, MuxErrorCode } from "../src/errors";

/**
 * assertSameNetwork must check resolved contract ids, not just the network
 * passphrase (#989). Two configs can share a passphrase (same chain) while
 * one of them points at a stale/wrong contract address — that must be
 * rejected rather than silently allowed through.
 */
describe("assertSameNetwork (#989)", () => {
  it("passes when target and configured network are identical", () => {
    expect(() => assertSameNetwork("testnet", "testnet")).not.toThrow();
  });

  it("rejects when passphrases differ (pre-existing behavior)", () => {
    expect(() => assertSameNetwork("testnet", "mainnet")).toThrow(MuxError);
  });

  it("rejects a stale contract address on the same chain/passphrase", () => {
    // Clone "testnet" into a second network entry that shares the exact same
    // passphrase (same chain) but has a stale muxBatcher address — this is
    // the scenario the old passphrase-only check let through.
    const testnet = NETWORK_CONFIGS.testnet;
    const staleKey = "testnet-stale-batcher";
    NETWORK_CONFIGS[staleKey] = {
      ...testnet,
      contracts: {
        ...testnet.contracts,
        muxBatcher: "STALE_BATCHER_CONTRACT_ID",
      },
    };

    try {
      expect(() => assertSameNetwork(staleKey, "testnet")).toThrow(MuxError);
      try {
        assertSameNetwork(staleKey, "testnet");
        throw new Error("expected assertSameNetwork to throw");
      } catch (e) {
        expect((e as MuxError).code).toBe(MuxErrorCode.CROSS_NETWORK_BLOCKED);
      }
    } finally {
      delete NETWORK_CONFIGS[staleKey];
    }
  });

  it("resolveNetworkConfig still fails closed on unknown/missing networks", () => {
    expect(() => resolveNetworkConfig(undefined)).toThrow(MuxError);
    expect(() => resolveNetworkConfig("does-not-exist")).toThrow(MuxError);
  });
});
