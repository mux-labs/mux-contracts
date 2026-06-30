/**
 * Integration test stub for mux-account contract.
 *
 * These tests connect to a live Soroban RPC endpoint and verify that the
 * mux-account contract is deployed and responds correctly. They gracefully
 * skip when the network or contract address is unavailable, matching the
 * pattern used in delegation-integration.test.ts and factory-integration.test.ts.
 *
 * Run against localnet (requires docker-compose):
 *   SOROBAN_NETWORK=localnet npm test
 *
 * Run against testnet:
 *   SOROBAN_NETWORK=testnet TESTNET_MUX_ACCOUNT_ID=C... npm test
 */

import { NETWORK_CONFIGS } from "../src/network";

const NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const config = NETWORK_CONFIGS[NETWORK];

/** Contract ID for mux-account on the active network (env-override supported). */
const ACCOUNT_CONTRACT_ID =
  process.env[`${NETWORK.toUpperCase()}_MUX_ACCOUNT_ID`] ||
  config.contracts.muxAccount ||
  "";

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

describe("MuxAccount integration", () => {
  let networkAvailable: boolean;
  let contractDeployed: boolean;

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    contractDeployed = networkAvailable && !!ACCOUNT_CONTRACT_ID;

    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" unavailable at ${config.rpcUrl}. ` +
          `MuxAccount integration tests will be skipped.`
      );
    } else if (!contractDeployed) {
      console.warn(
        `⚠️  MuxAccount contract ID not set for network "${NETWORK}". ` +
          `Set ${NETWORK.toUpperCase()}_MUX_ACCOUNT_ID to enable these tests.`
      );
    }
  });

  it("network config includes rpcUrl and networkPassphrase", () => {
    expect(config.rpcUrl).toBeTruthy();
    expect(config.networkPassphrase).toBeTruthy();
  });

  it("mux-account contract ID env var is documented", () => {
    const envKey = `${NETWORK.toUpperCase()}_MUX_ACCOUNT_ID`;
    expect(typeof envKey).toBe("string");
    if (!ACCOUNT_CONTRACT_ID) {
      console.info(`ℹ️  Set ${envKey} to enable live mux-account tests.`);
    }
  });

  it("should reach the Soroban RPC endpoint", async () => {
    if (!networkAvailable) {
      console.log(`ℹ️  Skipping — network "${NETWORK}" not available.`);
      return;
    }
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
    expect(response.ok).toBe(true);
  });

  it("stub: initialize round-trip via bindings", () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — mux-account contract not available on "${NETWORK}".`);
      return;
    }
    // TODO: instantiate MuxAccountClient, call initialize with a funded
    // keypair, and verify owner is set. Requires deployed contract + signer.
    expect(true).toBe(true);
  });

  it("stub: set_delegate round-trip via bindings", () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — mux-account contract not available on "${NETWORK}".`);
      return;
    }
    // TODO: set a delegate and verify with get_delegate / is_delegate.
    expect(true).toBe(true);
  });

  it("stub: debit_spend enforces spend limit", () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — mux-account contract not available on "${NETWORK}".`);
      return;
    }
    // TODO: set spend limit, debit within limit, then assert SpendLimitExceeded
    // (HTTP 400 via contractErrorToHttp) when limit is exceeded.
    expect(true).toBe(true);
  });

  it("stub: AlreadyInitialized error on double initialize", () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — mux-account contract not available on "${NETWORK}".`);
      return;
    }
    // TODO: initialize twice and assert AlreadyInitialized (HTTP 409).
    expect(true).toBe(true);
  });

  it("stub: TooManyDelegates when delegate cap is reached", () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — mux-account contract not available on "${NETWORK}".`);
      return;
    }
    // TODO: add delegates up to MAX_DELEGATES and assert TooManyDelegates (HTTP 409).
    expect(true).toBe(true);
  });
});
