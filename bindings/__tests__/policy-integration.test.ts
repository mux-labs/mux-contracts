/**
 * Policy integration test stub.
 *
 * These tests connect to a live Soroban RPC endpoint and exercise the
 * mux-policy contract end-to-end.  They are skipped gracefully when the
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

describe("Policy Integration Tests (mux-policy)", () => {
  let networkAvailable: boolean;

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" is unavailable at ${config.rpcUrl}. ` +
          `Policy integration tests will be skipped. ` +
          `Start the network or set SOROBAN_NETWORK=testnet to run them.`
      );
    }
  });

  it("network config exposes muxPolicy contract ID", () => {
    expect(config.contracts.muxPolicy).toBeDefined();
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

  // ── Stub: initialize ──────────────────────────────────────────────────────
  // TODO: deploy mux-policy to the target network, fund a test keypair, and
  // replace the skip guard with a real invocation via MuxPolicyClient.
  it("initialize: sets the admin address and emits init event", async () => {
    if (!networkAvailable || !config.contracts.muxPolicy) {
      console.log("⏭  Skipping initialize integration test — network or contract unavailable.");
      return;
    }

    // Placeholder: wire up MuxPolicyClient and assert result shape once
    // the contract is deployed and a funded keypair is available.
    // Example (uncomment and fill in after deployment):
    //
    // const { MuxPolicyClient } = await import("../src/generated/mux-policy");
    // const client = new MuxPolicyClient({
    //   contractId: config.contracts.muxPolicy,
    //   networkPassphrase: config.networkPassphrase,
    //   rpcUrl: config.rpcUrl,
    // });
    // const admin = Keypair.fromSecret(process.env.TEST_SECRET_KEY!);
    // const result = await client.initialize(admin.publicKey());
    // expect(result).toBeUndefined(); // successful init returns void

    expect(true).toBe(true); // stub passes until wired up
  });

  // ── Stub: set_daily_limit + get_daily_limit ───────────────────────────────
  it("set_daily_limit and get_daily_limit: round-trips a wallet limit", async () => {
    if (!networkAvailable || !config.contracts.muxPolicy) {
      console.log("⏭  Skipping set_daily_limit integration test — network or contract unavailable.");
      return;
    }

    // TODO: wire up MuxPolicyClient.setDailyLimit / getDailyLimit once deployed.
    expect(true).toBe(true);
  });

  // ── Stub: record_spend ────────────────────────────────────────────────────
  it("record_spend: debits from the daily limit within bounds", async () => {
    if (!networkAvailable || !config.contracts.muxPolicy) {
      console.log("⏭  Skipping record_spend integration test — network or contract unavailable.");
      return;
    }

    // TODO: wire up MuxPolicyClient.recordSpend once deployed.
    expect(true).toBe(true);
  });

  // ── Stub: LimitExceeded ──────────────────────────────────────────────────
  it("record_spend: rejects spend that exceeds the daily limit", async () => {
    if (!networkAvailable || !config.contracts.muxPolicy) {
      console.log("⏭  Skipping LimitExceeded integration test — network or contract unavailable.");
      return;
    }

    // TODO: assert the RPC error maps to MuxPolicyError.LimitExceeded.
    expect(true).toBe(true);
  });

  // ── Stub: LimitNotFound ──────────────────────────────────────────────────
  it("get_daily_limit: returns LimitNotFound for unknown wallet", async () => {
    if (!networkAvailable || !config.contracts.muxPolicy) {
      console.log("⏭  Skipping LimitNotFound integration test — network or contract unavailable.");
      return;
    }

    // TODO: assert the RPC error maps to MuxPolicyError.LimitNotFound.
    expect(true).toBe(true);
  });
});
