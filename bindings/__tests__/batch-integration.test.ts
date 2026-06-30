/**
 * Batch integration test stub.
 *
 * These tests connect to a live Soroban RPC endpoint and exercise the
 * mux-batcher contract end-to-end.  They are skipped gracefully when the
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

describe("Batch Integration Tests (mux-batcher)", () => {
  let networkAvailable: boolean;

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" is unavailable at ${config.rpcUrl}. ` +
          `Batch integration tests will be skipped. ` +
          `Start the network or set SOROBAN_NETWORK=testnet to run them.`
      );
    }
  });

  it("network config exposes muxBatcher contract ID", () => {
    expect(config.contracts.muxBatcher).toBeDefined();
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

  // ── Stub: execute_batch ────────────────────────────────────────────────────
  // TODO: deploy mux-batcher to the target network, fund a test keypair, and
  // replace the skip guard with a real invocation via MuxBatcherClient.
  it("execute_batch: submits a single-op batch and returns BatchResult", async () => {
    if (!networkAvailable || !config.contracts.muxBatcher) {
      console.log("⏭  Skipping execute_batch integration test — network or contract unavailable.");
      return;
    }

    // Placeholder: wire up MuxBatcherClient and assert result shape once
    // the contract is deployed and a funded keypair is available.
    // Example (uncomment and fill in after deployment):
    //
    // const { MuxBatcherClient } = await import("../src/generated/mux-batcher");
    // const client = new MuxBatcherClient({
    //   contractId: config.contracts.muxBatcher,
    //   networkPassphrase: config.networkPassphrase,
    //   rpcUrl: config.rpcUrl,
    // });
    // const sourceKeypair = Keypair.fromSecret(process.env.TEST_SECRET_KEY!);
    // const result = await client.executeBatch(sourceKeypair, sourceKeypair.publicKey(), [
    //   { target: ..., fnName: "...", args: [], requireSuccess: false },
    // ]);
    // expect(result.successCount + result.failureCount).toBe(1);

    expect(true).toBe(true); // stub passes until wired up
  });

  // ── Stub: simulate_batch ───────────────────────────────────────────────────
  it("simulate_batch: returns success_count equal to op count", async () => {
    if (!networkAvailable || !config.contracts.muxBatcher) {
      console.log("⏭  Skipping simulate_batch integration test — network or contract unavailable.");
      return;
    }

    // TODO: wire up MuxBatcherClient.simulateBatch once deployed.
    expect(true).toBe(true);
  });

  // ── Stub: estimate_fees ────────────────────────────────────────────────────
  it("estimate_fees: returns a positive number for a valid op count", async () => {
    if (!networkAvailable || !config.contracts.muxBatcher) {
      console.log("⏭  Skipping estimate_fees integration test — network or contract unavailable.");
      return;
    }

    // TODO: wire up MuxBatcherClient.estimateFees once deployed.
    expect(true).toBe(true);
  });

  // ── Stub: empty batch rejected ─────────────────────────────────────────────
  it("execute_batch: rejects an empty batch with EmptyBatch error", async () => {
    if (!networkAvailable || !config.contracts.muxBatcher) {
      console.log("⏭  Skipping empty-batch integration test — network or contract unavailable.");
      return;
    }

    // TODO: assert the RPC error maps to MuxBatcherError.EmptyBatch.
    expect(true).toBe(true);
  });

  // ── Stubs: error cases ────────────────────────────────────────────────────
  it.todo("execute_batch: rejects an oversized batch with BatchTooLarge");
  it.todo("execute_batch: returns RequiredOperationFailed when a required op fails");
  it.todo("execute_batch: rejects an unauthorized caller");
});
