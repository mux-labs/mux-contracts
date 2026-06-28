/**
 * Tests for MuxWalletRegistryClient binding shape and integration stubs.
 */

import { MuxWalletRegistryClient } from "../src/generated/mux-wallet-registry";
import { NETWORK_CONFIGS } from "../src/network";
import { ERROR_HTTP_MAP } from "../src/errors";

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

describe("MuxWalletRegistryClient shape", () => {
  it("exposes initialize as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.initialize).toBe("function");
  });

  it("exposes registerWallet as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.registerWallet).toBe("function");
  });

  it("exposes getWallet as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.getWallet).toBe("function");
  });

  it("exposes registerWalletWithMetadata as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.registerWalletWithMetadata).toBe("function");
  });

  it("exposes getMetadata as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.getMetadata).toBe("function");
  });
});

describe("Wallet registry error HTTP mapping", () => {
  it("maps WalletNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.WalletNotFound).toBe(404);
  });

  it("maps Unauthorized to 401", () => {
    expect(ERROR_HTTP_MAP.Unauthorized).toBe(401);
  });

  it("maps AlreadyInitialized to 409", () => {
    expect(ERROR_HTTP_MAP.AlreadyInitialized).toBe(409);
  });

  it("maps NotInitialized to 500", () => {
    expect(ERROR_HTTP_MAP.NotInitialized).toBe(500);
  });
});

describe("Wallet registry integration stubs", () => {
  beforeAll(async () => {
    const available = await isNetworkAvailable();
    if (!available) {
      console.warn(
        `⚠️  Network "${NETWORK}" is unavailable at ${config.rpcUrl}. ` +
        `Wallet registry integration tests require a running Soroban network.`
      );
    }
  });

  it("should have valid network configuration", () => {
    expect(config).toBeDefined();
    expect(config.rpcUrl).toBeTruthy();
    expect(config.networkPassphrase).toBeTruthy();
  });

  it("can attempt connection to configured network", async () => {
    const available = await isNetworkAvailable();
    expect(typeof available).toBe("boolean");
    if (!available) {
      console.log(
        `ℹ️  Network ${NETWORK} at ${config.rpcUrl} not available. ` +
        `Set SOROBAN_NETWORK=testnet and deploy mux-wallet-registry to run live tests.`
      );
    }
  });

  it.todo("initialize registry and verify owner is set");
  it.todo("register a wallet and retrieve it by name");
  it.todo("register wallet with metadata and verify label and description");
  it.todo("update an existing wallet entry and confirm address changes");
  it.todo("get_wallet returns WalletNotFound for unknown name");
  it.todo("get_metadata returns WalletNotFound for entry with no metadata");
  it.todo("non-owner register_wallet call is rejected with Unauthorized");
  it.todo("double initialize returns AlreadyInitialized");
});
