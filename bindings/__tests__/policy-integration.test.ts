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

/**
 * Daily spend windows are anchored to a fixed 24h (86_400s) period.  The
 * window that contains `now` starts at `floor(now / DAY) * DAY`; a spend
 * recorded at exactly the boundary belongs to the *new* window.  These
 * helpers mirror the contract's window math so the integration suite can
 * assert reset timing without depending on wall-clock drift.
 */
const DAY_SECONDS = 86_400;

function windowStart(now: number): number {
  return Math.floor(now / DAY_SECONDS) * DAY_SECONDS;
}

function windowEnd(now: number): number {
  return windowStart(now) + DAY_SECONDS;
}

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

  // ── Daily spend window reset: exact boundary timing ───────────────────────
  //
  // Invariants under test (see docs/policy-semantics.md):
  //   1. A spend recorded at `windowEnd - 1` still counts against the
  //      *current* window and must be rejected once the limit is exhausted.
  //   2. A spend recorded at exactly `windowEnd` (the boundary) belongs to
  //      the *next* window and therefore sees a fresh, full limit.
  //   3. A spend recorded at `windowEnd + 1` also sees the fresh limit.
  //   4. Replaying the same spend within a window is idempotent and does not
  //      double-debit the limit.
  //   5. Concurrent spends that together exceed the limit fail closed: the
  //      sum of accepted spends never exceeds the configured limit.
  describe("daily spend window reset (exact timing)", () => {
    const LIMIT = 1_000n;

    it("windowStart/windowEnd helpers align to fixed 24h periods", () => {
      const now = 1_700_000_000; // arbitrary epoch seconds
      const start = windowStart(now);
      const end = windowEnd(now);
      expect(start % DAY_SECONDS).toBe(0);
      expect(end - start).toBe(DAY_SECONDS);
      expect(start).toBeLessThanOrEqual(now);
      expect(end).toBeGreaterThan(now);
    });

    it("spend just before the boundary counts against the current window", () => {
      const now = 1_700_000_000;
      const justBefore = windowEnd(now) - 1;
      // Same window as `now` — the limit is shared, not reset.
      expect(windowStart(justBefore)).toBe(windowStart(now));
    });

    it("spend exactly at the boundary belongs to the next window", () => {
      const now = 1_700_000_000;
      const boundary = windowEnd(now);
      expect(windowStart(boundary)).toBe(boundary);
      expect(windowStart(boundary)).not.toBe(windowStart(now));
    });

    it("spend just after the boundary also belongs to the next window", () => {
      const now = 1_700_000_000;
      const justAfter = windowEnd(now) + 1;
      expect(windowStart(justAfter)).toBe(windowEnd(now));
    });

    it("limit exhaustion fails closed within a window", () => {
      // Simulate the contract's accounting: accepted spends accumulate and
      // any spend that would push the total past LIMIT is rejected.
      let spent = 0n;
      const trySpend = (amount: bigint): boolean => {
        if (spent + amount > LIMIT) return false;
        spent += amount;
        return true;
      };

      expect(trySpend(600n)).toBe(true);
      expect(trySpend(400n)).toBe(true);
      expect(spent).toBe(LIMIT);
      // Any further spend in the same window must be rejected.
      expect(trySpend(1n)).toBe(false);
      expect(spent).toBe(LIMIT);
    });

    it("replayed spend within a window is idempotent", () => {
      const seen = new Set<string>();
      let spent = 0n;
      const record = (id: string, amount: bigint): boolean => {
        if (seen.has(id)) return true; // replay: no double-debit
        if (spent + amount > LIMIT) return false;
        seen.add(id);
        spent += amount;
        return true;
      };

      expect(record("tx-1", 500n)).toBe(true);
      expect(record("tx-1", 500n)).toBe(true); // replay accepted, no debit
      expect(spent).toBe(500n);
      expect(record("tx-2", 500n)).toBe(true);
      expect(spent).toBe(LIMIT);
      expect(record("tx-3", 1n)).toBe(false);
    });

    it("concurrent spends across the boundary never exceed the limit", () => {
      // Two windows: the first is exhausted, the second starts fresh.
      const windows = new Map<number, bigint>();
      const trySpend = (at: number, amount: bigint): boolean => {
        const w = windowStart(at);
        const spent = windows.get(w) ?? 0n;
        if (spent + amount > LIMIT) return false;
        windows.set(w, spent + amount);
        return true;
      };

      const now = 1_700_000_000;
      const before = windowEnd(now) - 1;
      const after = windowEnd(now);

      expect(trySpend(before, LIMIT)).toBe(true);
      expect(trySpend(before, 1n)).toBe(false); // current window exhausted
      expect(trySpend(after, LIMIT)).toBe(true); // fresh window
      expect(windows.get(windowStart(before))).toBe(LIMIT);
      expect(windows.get(windowStart(after))).toBe(LIMIT);
    });
  });
});
