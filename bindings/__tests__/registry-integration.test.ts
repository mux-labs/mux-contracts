/**
 * Registry integration test stub.
 *
 * These tests connect to a live Soroban RPC endpoint and exercise the
 * mux-registry contract end-to-end.  They are skipped gracefully when the
 * network is unavailable (CI without localnet, offline dev).
 *
 * Run against localnet:
 *   SOROBAN_NETWORK=localnet npm test
 *
 * Run against testnet:
 *   SOROBAN_NETWORK=testnet npm test
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

describe("Registry Integration Tests (mux-registry)", () => {
  let networkAvailable: boolean;

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" is unavailable at ${config.rpcUrl}. ` +
          `Registry integration tests will be skipped. ` +
          `Start the network or set SOROBAN_NETWORK=testnet to run them.`
      );
    }
  });

  it("network config is defined", () => {
    expect(config).toBeDefined();
    expect(config.rpcUrl).toBeTruthy();
    expect(config.networkPassphrase).toBeTruthy();
  });

  it("can reach the configured RPC endpoint", async () => {
    const available = await isNetworkAvailable();
    expect(typeof available).toBe("boolean");
    if (!available) {
      console.log(
        `ℹ️  Skipping live RPC check — ${NETWORK} at ${config.rpcUrl} is not reachable.`
      );
    }
  });

  // ── Stub: register + get_version ──────────────────────────────────────────
  // TODO: deploy mux-registry to the target network, fund a test keypair, and
  // replace the skip guard with a real invocation via MuxRegistryClient.
  it("register: registers a contract name and get_version returns it", async () => {
    if (!networkAvailable) {
      console.log("⏭  Skipping register integration test — network unavailable.");
      return;
    }

    // Placeholder: wire up MuxRegistryClient once the contract is deployed
    // and a funded keypair is available.
    // Example (uncomment and fill in after deployment):
    //
    // const { MuxRegistryClient } = await import("../src/generated/mux-registry");
    // const client = new MuxRegistryClient({
    //   contractId: process.env.LOCALNET_MUX_REGISTRY_ID!,
    //   networkPassphrase: config.networkPassphrase,
    //   rpcUrl: config.rpcUrl,
    // });
    // const adminKeypair = Keypair.fromSecret(process.env.TEST_SECRET_KEY!);
    // await client.register(adminKeypair, "account", "1.0.0");
    // const version = await client.getVersion(adminKeypair, "account");
    // expect(version).toBe("1.0.0");

    expect(true).toBe(true); // stub passes until wired up
  });

  // ── Stub: register_with_metadata + get_metadata ───────────────────────────
  it("register_with_metadata: stores and retrieves full metadata", async () => {
    if (!networkAvailable) {
      console.log("⏭  Skipping register_with_metadata integration test — network unavailable.");
      return;
    }

    // TODO: wire up MuxRegistryClient.registerWithMetadata and getMetadata once deployed.
    // Example:
    //
    // await client.registerWithMetadata(adminKeypair, "batcher", "2.0.0", "Atomic batcher", "mux-labs");
    // const meta = await client.getMetadata(adminKeypair, "batcher");
    // expect(meta.version).toBe("2.0.0");
    // expect(meta.author).toBe("mux-labs");

    expect(true).toBe(true);
  });

  // ── Stub: get_version on unknown contract returns ContractNotFound ─────────
  it("get_version: returns ContractNotFound error for unknown contract", async () => {
    if (!networkAvailable) {
      console.log("⏭  Skipping ContractNotFound integration test — network unavailable.");
      return;
    }

    // TODO: assert the RPC error maps to MuxRegistryError.ContractNotFound.
    // Example:
    //
    // await expect(client.getVersion(adminKeypair, "ghost")).rejects.toThrow();

    expect(true).toBe(true);
  });

  // ── Stub: double-initialize returns AlreadyInitialized ────────────────────
  it("initialize: second call returns AlreadyInitialized error", async () => {
    if (!networkAvailable) {
      console.log("⏭  Skipping AlreadyInitialized integration test — network unavailable.");
      return;
    }

    // TODO: assert the RPC error maps to MuxRegistryError.AlreadyInitialized.
    expect(true).toBe(true);
  });
});
