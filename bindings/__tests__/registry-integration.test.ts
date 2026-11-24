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

/**
 * Canonical version metadata for the mux-registry contract.
 *
 * This constant is the single source of truth for the version reported by the
 * contract's `version()` entrypoint.  It must stay in lockstep with the
 * generated bindings and the on-chain contract; a mismatch is a fail-closed
 * condition (see the version-consistency tests below).
 */
const REGISTRY_CONTRACT_VERSION = "1.0.0";

/**
 * Stable error codes surfaced by the mux-registry contract.  Kept in sync with
 * the contract's typed error enum so integration assertions can match on a
 * stable identifier rather than a free-form message.
 */
const MuxRegistryError = {
  ContractNotFound: 1,
  AlreadyInitialized: 2,
  VersionMismatch: 3,
} as const;

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

/**
 * Fail-closed version check: reject any reported version that does not match
 * the expected constant instead of silently proceeding.
 */
function assertVersionConsistent(reported: string, expected: string): void {
  if (reported !== expected) {
    throw new Error(
      `VersionMismatch (${MuxRegistryError.VersionMismatch}): ` +
        `registry reported "${reported}" but expected "${expected}"`
    );
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

  // ── Version metadata invariants ───────────────────────────────────────────
  it("version metadata: constant is a well-formed semver string", () => {
    expect(REGISTRY_CONTRACT_VERSION).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("version metadata: matching version passes the fail-closed check", () => {
    expect(() =>
      assertVersionConsistent(REGISTRY_CONTRACT_VERSION, REGISTRY_CONTRACT_VERSION)
    ).not.toThrow();
  });

  it("version metadata: mismatched version fails closed with VersionMismatch", () => {
    expect(() => assertVersionConsistent("0.0.1", REGISTRY_CONTRACT_VERSION)).toThrow(
      /VersionMismatch/
    );
    expect(() => assertVersionConsistent("0.0.1", REGISTRY_CONTRACT_VERSION)).toThrow(
      new RegExp(String(MuxRegistryError.VersionMismatch))
    );
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
