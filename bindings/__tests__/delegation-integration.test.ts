/**
 * Integration test stub for mux-delegation contract.
 *
 * These tests verify that the delegation bindings can interact with a
 * live or local Soroban network. They are skipped when the network is
 * unavailable, matching the pattern in integration.test.ts.
 */

import { NETWORK_CONFIGS } from "../src/network";

const NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const config = NETWORK_CONFIGS[NETWORK];

async function isNetworkAvailable(): Promise<boolean> {
  try {
    const response = await globalThis.fetch(config.rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "getNetwork",
        params: [],
      }),
    });
    return response.ok;
  } catch {
    return false;
  }
}

describe("Delegation Integration Tests", () => {
  let networkAvailable: boolean;

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" is unavailable at ${config.rpcUrl}. ` +
        `Delegation integration tests will be skipped.`
      );
    }
  });

  it("should have valid network configuration for delegation", () => {
    expect(config).toBeDefined();
    expect(config.rpcUrl).toBeTruthy();
    expect(config.networkPassphrase).toBeTruthy();
  });

  it("should be able to attempt connection for delegation tests", async () => {
    const available = await isNetworkAvailable();
    expect(typeof available).toBe("boolean");
    if (!available) {
      console.log(
        `ℹ️  Network ${NETWORK} at ${config.rpcUrl} is not currently available. ` +
        `To enable delegation integration tests, start the network or use SOROBAN_NETWORK=testnet.`
      );
    }
  });

  it("should expose delegation contract ID in network config", () => {
    expect(config.contracts).toBeDefined();
    expect(config.contracts.muxDelegation).toBeDefined();
  });

  it("stub: grant_delegate round-trip via bindings", () => {
    if (!networkAvailable) {
      console.log("Skipped — network unavailable");
      return;
    }
    // TODO: instantiate MuxDelegationClient, grant a delegate, and verify
    // with get_delegate_permissions. Requires a funded keypair on the
    // target network.
    expect(true).toBe(true);
  });

  it("stub: revoke_delegate round-trip via bindings", () => {
    if (!networkAvailable) {
      console.log("Skipped — network unavailable");
      return;
    }
    // TODO: grant then revoke a delegate and assert is_delegate returns false.
    expect(true).toBe(true);
  });

  it("stub: is_delegate query via bindings", () => {
    if (!networkAvailable) {
      console.log("Skipped — network unavailable");
      return;
    }
    // TODO: query is_delegate for an unknown delegate and expect false.
    expect(true).toBe(true);
  });
});
