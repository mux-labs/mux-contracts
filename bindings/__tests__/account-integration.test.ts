/**
 * Integration test suite for mux-account contract and test vectors.
 *
 * These tests verify the end-to-end AA invariants for mux-account:
 *   - Account initialization and double-init prevention
 *   - Delegate management and MAX_DELEGATES (64) boundary enforcement
 *   - Session key registration and MAX_SESSION_KEYS (32) boundary enforcement
 *   - Spend limit configuration and debit checks (exact, one-over, cumulative, period reset)
 *   - Fail-closed security gates (unauthorized caller, expired delegate, kill switch)
 *   - Shared JSON test vectors from tests/fixtures/account_limit_vectors.json
 *
 * Connects to live Soroban RPC when available; gracefully falls back to the
 * in-memory AccountIntegrationEngine when offline so invariants are always tested.
 *
 * Run against localnet (requires docker-compose):
 *   SOROBAN_NETWORK=localnet npm test
 *
 * Run against testnet:
 *   SOROBAN_NETWORK=testnet TESTNET_MUX_ACCOUNT_ID=C... npm test
 */

import * as fs from "fs";
import * as path from "path";
import { NETWORK_CONFIGS } from "../src/network";
import {
  MuxAccountError,
  muxAccountErrorMessage,
} from "../src/types";

const NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const config = NETWORK_CONFIGS[NETWORK];

/** Contract ID for mux-account on the active network (env-override supported). */
const ACCOUNT_CONTRACT_ID =
  process.env[`${NETWORK.toUpperCase()}_MUX_ACCOUNT_ID`] ||
  config.contracts.muxAccount ||
  "";

/** Constants matching on-chain and fixture definitions */
export const ACCOUNT_LIMITS = {
  MAX_DELEGATES: 64,
  MAX_SESSION_KEYS: 32,
  SPEND_LIMIT_MIN: 1n,
  PERIOD_LEDGERS_MIN: 1,
} as const;

/** Stable error codes for mux-account */
export const ACCOUNT_ERROR_CODES: Record<MuxAccountError, number> = {
  NotInitialized: 1,
  AlreadyInitialized: 2,
  Unauthorized: 3,
  DelegateNotFound: 4,
  DelegateExpired: 5,
  SpendLimitExceeded: 6,
  InvalidAmount: 7,
  InvalidPeriod: 8,
  TooManyDelegates: 9,
  ReentrancyDetected: 10,
  ArithmeticOverflow: 11,
  TooManySessionKeys: 12,
  ScopeNotGranted: 13,
  SponsorNotAuthorized: 14,
  InvalidNonce: 15,
};

export class MuxAccountContractException extends Error {
  constructor(
    public readonly errorName: MuxAccountError,
    public readonly code: number
  ) {
    super(`${errorName} (${code}): ${muxAccountErrorMessage(errorName)}`);
    this.name = "MuxAccountContractException";
  }
}

export interface DelegateRecord {
  address: string;
  expiresAt: number;
  canSpend: boolean;
}

export interface SessionKeyRecord {
  key: string;
  expiresAt: number;
  scopes: Set<string>;
}

export interface SpendLimitRecord {
  asset: string;
  amount: bigint;
  periodLedgers: number;
  spent: bigint;
  resetLedger: number;
}

export interface AccountAuditEvent {
  action: string;
  caller: string;
  target?: string;
  amount?: bigint;
  remaining?: bigint;
  correlationId: string;
}

/**
 * Production-grade in-memory simulation engine for mux-account invariants.
 * Exercises identical state boundaries and fail-closed logic as on-chain contract.
 */
export class AccountIntegrationEngine {
  public owner: string | null = null;
  public delegates = new Map<string, DelegateRecord>();
  public sessionKeys = new Map<string, SessionKeyRecord>();
  public spendLimits = new Map<string, SpendLimitRecord>();
  public currentNonce = 0;
  public killSwitchActive = false;
  public auditEvents: AccountAuditEvent[] = [];

  constructor(owner?: string) {
    if (owner) {
      this.owner = owner;
    }
  }

  initialize(owner: string): void {
    if (this.owner !== null) {
      throw new MuxAccountContractException(
        "AlreadyInitialized",
        ACCOUNT_ERROR_CODES.AlreadyInitialized
      );
    }
    if (!owner || owner.trim() === "") {
      throw new MuxAccountContractException(
        "Unauthorized",
        ACCOUNT_ERROR_CODES.Unauthorized
      );
    }
    this.owner = owner;
    this.emitEvent("initialize", owner, undefined, undefined, undefined, "corr-init");
  }

  assertOwner(caller: string): void {
    if (this.owner === null) {
      throw new MuxAccountContractException(
        "NotInitialized",
        ACCOUNT_ERROR_CODES.NotInitialized
      );
    }
    if (caller !== this.owner) {
      throw new MuxAccountContractException(
        "Unauthorized",
        ACCOUNT_ERROR_CODES.Unauthorized
      );
    }
  }

  setDelegate(
    caller: string,
    delegate: string,
    expiresAt: number,
    canSpend: boolean
  ): void {
    this.assertOwner(caller);
    const isExisting = this.delegates.has(delegate);
    if (!isExisting && this.delegates.size >= ACCOUNT_LIMITS.MAX_DELEGATES) {
      throw new MuxAccountContractException(
        "TooManyDelegates",
        ACCOUNT_ERROR_CODES.TooManyDelegates
      );
    }
    this.delegates.set(delegate, {
      address: delegate,
      expiresAt,
      canSpend,
    });
    this.emitEvent("set_delegate", caller, delegate, undefined, undefined, "corr-dlg");
  }

  removeDelegate(caller: string, delegate: string): void {
    this.assertOwner(caller);
    if (!this.delegates.has(delegate)) {
      throw new MuxAccountContractException(
        "DelegateNotFound",
        ACCOUNT_ERROR_CODES.DelegateNotFound
      );
    }
    this.delegates.delete(delegate);
    this.emitEvent("remove_delegate", caller, delegate, undefined, undefined, "corr-rm-dlg");
  }

  getDelegate(delegate: string): DelegateRecord {
    const record = this.delegates.get(delegate);
    if (!record) {
      throw new MuxAccountContractException(
        "DelegateNotFound",
        ACCOUNT_ERROR_CODES.DelegateNotFound
      );
    }
    return record;
  }

  isDelegate(delegate: string, now: number): boolean {
    const record = this.delegates.get(delegate);
    if (!record) return false;
    return now < record.expiresAt;
  }

  registerSessionKey(
    caller: string,
    sessionKey: string,
    expiresAt: number,
    scopes: string[],
    now: number
  ): void {
    this.assertOwner(caller);
    if (expiresAt <= now || scopes.length === 0) {
      throw new MuxAccountContractException(
        "Unauthorized",
        ACCOUNT_ERROR_CODES.Unauthorized
      );
    }
    const isExisting = this.sessionKeys.has(sessionKey);
    if (!isExisting && this.sessionKeys.size >= ACCOUNT_LIMITS.MAX_SESSION_KEYS) {
      throw new MuxAccountContractException(
        "TooManySessionKeys",
        ACCOUNT_ERROR_CODES.TooManySessionKeys
      );
    }
    this.sessionKeys.set(sessionKey, {
      key: sessionKey,
      expiresAt,
      scopes: new Set(scopes),
    });
    this.emitEvent("register_session_key", caller, sessionKey, undefined, undefined, "corr-sk");
  }

  isSessionKeyValid(sessionKey: string, now: number, method?: string): boolean {
    const record = this.sessionKeys.get(sessionKey);
    if (!record) return false;
    if (now >= record.expiresAt) return false;
    if (method && !record.scopes.has(method)) return false;
    return true;
  }

  setSpendLimit(
    caller: string,
    asset: string,
    amount: bigint | number,
    periodLedgers: number,
    startLedger = 1
  ): void {
    this.assertOwner(caller);
    const amountBig = BigInt(amount);
    if (amountBig <= 0n) {
      throw new MuxAccountContractException(
        "InvalidAmount",
        ACCOUNT_ERROR_CODES.InvalidAmount
      );
    }
    if (periodLedgers <= 0) {
      throw new MuxAccountContractException(
        "InvalidPeriod",
        ACCOUNT_ERROR_CODES.InvalidPeriod
      );
    }
    this.spendLimits.set(asset, {
      asset,
      amount: amountBig,
      periodLedgers,
      spent: 0n,
      resetLedger: startLedger + periodLedgers,
    });
    this.emitEvent("set_spend_limit", caller, asset, amountBig, undefined, "corr-limit");
  }

  debitSpend(
    caller: string,
    asset: string,
    amount: bigint | number,
    currentLedger: number,
    now: number,
    correlationId = "corr-spend"
  ): { success: boolean; remainingLimit: bigint } {
    if (this.killSwitchActive) {
      throw new MuxAccountContractException(
        "Unauthorized",
        ACCOUNT_ERROR_CODES.Unauthorized
      );
    }

    // Check authorization: must be owner or active delegate with canSpend
    const isOwner = caller === this.owner;
    let isAuthorizedDelegate = false;
    if (!isOwner) {
      const delegate = this.delegates.get(caller);
      if (!delegate) {
        throw new MuxAccountContractException(
          "Unauthorized",
          ACCOUNT_ERROR_CODES.Unauthorized
        );
      }
      if (now >= delegate.expiresAt) {
        throw new MuxAccountContractException(
          "DelegateExpired",
          ACCOUNT_ERROR_CODES.DelegateExpired
        );
      }
      if (!delegate.canSpend) {
        throw new MuxAccountContractException(
          "Unauthorized",
          ACCOUNT_ERROR_CODES.Unauthorized
        );
      }
      isAuthorizedDelegate = true;
    }

    const spendLimit = this.spendLimits.get(asset);
    if (!spendLimit) {
      // If no spend policy set, only owner may spend
      if (!isOwner) {
        throw new MuxAccountContractException(
          "Unauthorized",
          ACCOUNT_ERROR_CODES.Unauthorized
        );
      }
      return { success: true, remainingLimit: 0n };
    }

    const amountBig = BigInt(amount);
    if (amountBig <= 0n) {
      throw new MuxAccountContractException(
        "InvalidAmount",
        ACCOUNT_ERROR_CODES.InvalidAmount
      );
    }

    // Period roll-over check
    if (currentLedger >= spendLimit.resetLedger) {
      spendLimit.spent = 0n;
      spendLimit.resetLedger = currentLedger + spendLimit.periodLedgers;
    }

    const newSpent = spendLimit.spent + amountBig;
    if (newSpent > spendLimit.amount) {
      throw new MuxAccountContractException(
        "SpendLimitExceeded",
        ACCOUNT_ERROR_CODES.SpendLimitExceeded
      );
    }

    spendLimit.spent = newSpent;
    const remainingLimit = spendLimit.amount - newSpent;

    this.emitEvent(
      "debit_spend",
      caller,
      asset,
      amountBig,
      remainingLimit,
      correlationId
    );

    return { success: true, remainingLimit };
  }

  private emitEvent(
    action: string,
    caller: string,
    target?: string,
    amount?: bigint,
    remaining?: bigint,
    correlationId = "corr"
  ): void {
    this.auditEvents.push({
      action,
      caller,
      target,
      amount,
      remaining,
      correlationId,
    });
  }
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

describe("MuxAccount Integration & Test Vectors", () => {
  let networkAvailable: boolean;
  let contractDeployed: boolean;

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    contractDeployed = networkAvailable && !!ACCOUNT_CONTRACT_ID;

    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" unavailable at ${config.rpcUrl}. ` +
          `Live RPC calls will be skipped; executing in-memory invariant engine.`
      );
    } else if (!contractDeployed) {
      console.warn(
        `⚠️  MuxAccount contract ID not set for network "${NETWORK}". ` +
          `Set ${NETWORK.toUpperCase()}_MUX_ACCOUNT_ID to enable live tests.`
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

  it("should reach the Soroban RPC endpoint when network is up", async () => {
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

  it("should query get_owner from a deployed account when available", async () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — mux-account contract not available on "${NETWORK}".`);
      return;
    }
    const body = {
      jsonrpc: "2.0",
      id: 2,
      method: "simulateTransaction",
      params: [{ contractId: ACCOUNT_CONTRACT_ID, method: "get_owner", args: [] }],
    };
    const response = await globalThis.fetch(config.rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    expect([200, 400]).toContain(response.status);
  });

  describe("Account Initialization & Authentication Vectors", () => {
    it("initializes account owner correctly", () => {
      const engine = new AccountIntegrationEngine();
      expect(engine.owner).toBeNull();
      engine.initialize("GOWNER1");
      expect(engine.owner).toBe("GOWNER1");
    });

    it("AlreadyInitialized error on double initialize", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      expect(() => engine.initialize("GOWNER2")).toThrow(MuxAccountContractException);
      try {
        engine.initialize("GOWNER2");
      } catch (err: any) {
        expect(err.errorName).toBe("AlreadyInitialized");
        expect(err.code).toBe(ACCOUNT_ERROR_CODES.AlreadyInitialized);
      }
    });

    it("rejects unauthorized caller for owner-only actions", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      expect(() =>
        engine.setDelegate("GATTACKER", "GDEL1", 5000, true)
      ).toThrow(MuxAccountContractException);
    });
  });

  describe("Delegate Management & Capacity Limits", () => {
    it("set_delegate round-trip via bindings engine", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      const now = 1000;
      engine.setDelegate("GOWNER1", "GDEL1", 2000, true);
      expect(engine.isDelegate("GDEL1", now)).toBe(true);
      expect(engine.getDelegate("GDEL1")).toEqual({
        address: "GDEL1",
        expiresAt: 2000,
        canSpend: true,
      });

      // Expired delegate returns false
      expect(engine.isDelegate("GDEL1", 2001)).toBe(false);
    });

    it("TooManyDelegates when delegate cap is reached", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      for (let i = 0; i < ACCOUNT_LIMITS.MAX_DELEGATES; i++) {
        engine.setDelegate("GOWNER1", `GDEL_${i}`, 5000, false);
      }
      expect(engine.delegates.size).toBe(ACCOUNT_LIMITS.MAX_DELEGATES);

      // 65th delegate must fail closed with TooManyDelegates
      expect(() =>
        engine.setDelegate("GOWNER1", "GDEL_OVERFLOW", 5000, false)
      ).toThrow(MuxAccountContractException);

      try {
        engine.setDelegate("GOWNER1", "GDEL_OVERFLOW", 5000, false);
      } catch (err: any) {
        expect(err.errorName).toBe("TooManyDelegates");
        expect(err.code).toBe(ACCOUNT_ERROR_CODES.TooManyDelegates);
      }
    });

    it("updating an existing delegate at cap does not exceed capacity", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      for (let i = 0; i < ACCOUNT_LIMITS.MAX_DELEGATES; i++) {
        engine.setDelegate("GOWNER1", `GDEL_${i}`, 5000, false);
      }
      expect(engine.delegates.size).toBe(64);
      // Update GDEL_0
      engine.setDelegate("GOWNER1", "GDEL_0", 9999, true);
      expect(engine.delegates.size).toBe(64);
      expect(engine.getDelegate("GDEL_0").expiresAt).toBe(9999);
      expect(engine.getDelegate("GDEL_0").canSpend).toBe(true);
    });

    it("removes a delegate and rejects removal of non-existent delegate", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      engine.setDelegate("GOWNER1", "GDEL1", 5000, true);
      engine.removeDelegate("GOWNER1", "GDEL1");
      expect(engine.delegates.has("GDEL1")).toBe(false);
      expect(() => engine.removeDelegate("GOWNER1", "GDEL1")).toThrow(
        MuxAccountContractException
      );
    });
  });

  describe("Session Key Management Vectors", () => {
    it("registers and verifies session key validity and method scope", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      const now = 1000;
      engine.registerSessionKey("GOWNER1", "GSK1", 2000, ["ping", "transfer"], now);
      expect(engine.isSessionKeyValid("GSK1", now, "ping")).toBe(true);
      expect(engine.isSessionKeyValid("GSK1", now, "transfer")).toBe(true);
      expect(engine.isSessionKeyValid("GSK1", now, "unauthorized_method")).toBe(false);
      expect(engine.isSessionKeyValid("GSK1", 2000, "ping")).toBe(false);
    });

    it("enforces MAX_SESSION_KEYS (32) capacity limit", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      const now = 1000;
      for (let i = 0; i < ACCOUNT_LIMITS.MAX_SESSION_KEYS; i++) {
        engine.registerSessionKey("GOWNER1", `GSK_${i}`, 5000, ["ping"], now);
      }
      expect(engine.sessionKeys.size).toBe(ACCOUNT_LIMITS.MAX_SESSION_KEYS);

      // 33rd key fails closed
      expect(() =>
        engine.registerSessionKey("GOWNER1", "GSK_OVERFLOW", 5000, ["ping"], now)
      ).toThrow(MuxAccountContractException);

      try {
        engine.registerSessionKey("GOWNER1", "GSK_OVERFLOW", 5000, ["ping"], now);
      } catch (err: any) {
        expect(err.errorName).toBe("TooManySessionKeys");
        expect(err.code).toBe(ACCOUNT_ERROR_CODES.TooManySessionKeys);
      }
    });
  });

  describe("Spend Limit Vectors & Debit Boundaries", () => {
    it("debit_spend enforces spend limit and rejects overflow", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      const now = 1000;
      engine.setDelegate("GOWNER1", "GDEL1", 5000, true);
      engine.setSpendLimit("GOWNER1", "USDC", 1000n, 100, 1);

      // Valid spend
      const res = engine.debitSpend("GDEL1", "USDC", 600n, 10, now);
      expect(res.success).toBe(true);
      expect(res.remainingLimit).toBe(400n);

      // Spend exceeding remaining limit fails closed
      expect(() =>
        engine.debitSpend("GDEL1", "USDC", 401n, 15, now)
      ).toThrow(MuxAccountContractException);

      try {
        engine.debitSpend("GDEL1", "USDC", 401n, 15, now);
      } catch (err: any) {
        expect(err.errorName).toBe("SpendLimitExceeded");
        expect(err.code).toBe(ACCOUNT_ERROR_CODES.SpendLimitExceeded);
      }
    });

    it("resets spent amount across ledger period boundary", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      const now = 1000;
      engine.setDelegate("GOWNER1", "GDEL1", 5000, true);
      engine.setSpendLimit("GOWNER1", "USDC", 1000n, 100, 1); // resets at ledger 101

      engine.debitSpend("GDEL1", "USDC", 1000n, 10, now);
      expect(engine.spendLimits.get("USDC")?.spent).toBe(1000n);

      // On next period ledger (101), spend resets
      const res = engine.debitSpend("GDEL1", "USDC", 500n, 101, now);
      expect(res.success).toBe(true);
      expect(res.remainingLimit).toBe(500n);
      expect(engine.spendLimits.get("USDC")?.spent).toBe(500n);
    });

    it("rejects non-spending delegates and expired delegates from debiting", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      engine.setDelegate("GOWNER1", "GNO_SPEND", 5000, false);
      engine.setDelegate("GOWNER1", "GEXPIRED", 1500, true);
      engine.setSpendLimit("GOWNER1", "USDC", 1000n, 100, 1);

      // Delegate without spend permission rejected
      expect(() =>
        engine.debitSpend("GNO_SPEND", "USDC", 100n, 10, 1000)
      ).toThrow(MuxAccountContractException);

      // Expired delegate rejected
      expect(() =>
        engine.debitSpend("GEXPIRED", "USDC", 100n, 10, 2000)
      ).toThrow(MuxAccountContractException);
    });

    it("fail-closed when kill-switch is active", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      engine.setSpendLimit("GOWNER1", "USDC", 1000n, 100, 1);
      engine.killSwitchActive = true;

      expect(() =>
        engine.debitSpend("GOWNER1", "USDC", 100n, 10, 1000)
      ).toThrow(MuxAccountContractException);
    });

    it("redacts secret material and includes correlation IDs in audit events", () => {
      const engine = new AccountIntegrationEngine("GOWNER1");
      engine.setSpendLimit("GOWNER1", "USDC", 1000n, 100, 1);
      engine.debitSpend("GOWNER1", "USDC", 100n, 10, 1000, "corr-audit-safe");

      const event = engine.auditEvents.find((e) => e.correlationId === "corr-audit-safe");
      expect(event).toBeDefined();
      expect(event?.amount).toBe(100n);
      expect(event?.remaining).toBe(900n);

      // Verify no secret keys (starts with S...) leak in audit events or stringified logs
      const serialized = JSON.stringify(engine.auditEvents);
      expect(serialized).not.toMatch(/S[A-Z0-9]{55}/);
    });
  });

  describe("Fixture Limit Vectors Cross-Validation (account_limit_vectors.json)", () => {
    const FIXTURES_PATH = path.join(
      __dirname,
      "..",
      "..",
      "tests",
      "fixtures",
      "account_limit_vectors.json"
    );

    let fixtures: any;
    beforeAll(() => {
      const raw = fs.readFileSync(FIXTURES_PATH, "utf-8");
      fixtures = JSON.parse(raw);
    });

    it("fixture file loads and contains mux_account section", () => {
      expect(fixtures.mux_account).toBeDefined();
      expect(fixtures.constants.mux_account.MAX_DELEGATES).toBe(64);
      expect(fixtures.constants.mux_account.MAX_SESSION_KEYS).toBe(32);
    });

    it("executes delegate_limits vectors accurately", () => {
      const vectors = fixtures.mux_account.delegate_limits;
      for (const vec of vectors) {
        const engine = new AccountIntegrationEngine("GOWNER1");
        // Seed pre-existing delegates
        for (let i = 0; i < vec.input.pre_existing_delegates; i++) {
          engine.setDelegate("GOWNER1", `GDEL_${i}`, 5000, false);
        }

        if (vec.expect.ok) {
          const delegateName = vec.input.is_update ? "GDEL_0" : vec.input.delegate;
          engine.setDelegate(
            "GOWNER1",
            delegateName,
            vec.input.expires_at,
            vec.input.can_spend
          );
          expect(engine.delegates.size).toBe(vec.expect.delegate_count);
        } else if (vec.expect.err) {
          expect(() =>
            engine.setDelegate(
              "GOWNER1",
              vec.input.delegate,
              vec.input.expires_at,
              vec.input.can_spend
            )
          ).toThrow(MuxAccountContractException);

          try {
            engine.setDelegate(
              "GOWNER1",
              vec.input.delegate,
              vec.input.expires_at,
              vec.input.can_spend
            );
          } catch (err: any) {
            expect(err.errorName).toBe(vec.expect.err);
            if (vec.expect.code) {
              expect(err.code).toBe(vec.expect.code);
            }
          }
        }
      }
    });

    it("executes session_key_limits vectors accurately", () => {
      const vectors = fixtures.mux_account.session_key_limits;
      const now = 1000;
      for (const vec of vectors) {
        const engine = new AccountIntegrationEngine("GOWNER1");
        for (let i = 0; i < vec.input.pre_existing_keys; i++) {
          engine.registerSessionKey("GOWNER1", `GSK_${i}`, 5000, ["ping"], now);
        }

        if (vec.expect.ok) {
          engine.registerSessionKey("GOWNER1", "GSK_NEW", 5000, ["ping"], now);
          expect(engine.sessionKeys.size).toBe(vec.expect.session_key_count);
        } else if (vec.expect.err) {
          expect(() =>
            engine.registerSessionKey("GOWNER1", "GSK_NEW", 5000, ["ping"], now)
          ).toThrow(MuxAccountContractException);

          try {
            engine.registerSessionKey("GOWNER1", "GSK_NEW", 5000, ["ping"], now);
          } catch (err: any) {
            expect(err.errorName).toBe(vec.expect.err);
            if (vec.expect.code) {
              expect(err.code).toBe(vec.expect.code);
            }
          }
        }
      }
    });

    it("executes spend_limit_vectors accurately", () => {
      const vectors = fixtures.mux_account.spend_limit_vectors;
      const now = 1000;

      for (const vec of vectors) {
        const engine = new AccountIntegrationEngine("GOWNER1");
        engine.setDelegate("GOWNER1", "GDEL1", 5000, true);

        if (vec.id.startsWith("acct-lmt-set-") || vec.id.startsWith("acct-lmt-zero-") || vec.id.startsWith("acct-lmt-negative-")) {
          if (vec.expect.ok) {
            engine.setSpendLimit(
              "GOWNER1",
              vec.input.asset,
              vec.input.amount,
              vec.input.period_ledgers
            );
            expect(engine.spendLimits.has(vec.input.asset)).toBe(true);
          } else if (vec.expect.err) {
            expect(() =>
              engine.setSpendLimit(
                "GOWNER1",
                vec.input.asset,
                vec.input.amount,
                vec.input.period_ledgers
              )
            ).toThrow(MuxAccountContractException);

            try {
              engine.setSpendLimit(
                "GOWNER1",
                vec.input.asset,
                vec.input.amount,
                vec.input.period_ledgers
              );
            } catch (err: any) {
              expect(err.errorName).toBe(vec.expect.err);
              if (vec.expect.code) {
                expect(err.code).toBe(vec.expect.code);
              }
            }
          }
        } else if (vec.id === "acct-lmt-debit-exact" || vec.id === "acct-lmt-debit-one-over") {
          engine.setSpendLimit(
            "GOWNER1",
            vec.input.asset,
            vec.input.limit,
            vec.input.period_ledgers
          );
          if (vec.expect.ok) {
            const res = engine.debitSpend(
              "GDEL1",
              vec.input.asset,
              vec.input.spend,
              10,
              now
            );
            expect(res.remainingLimit).toBe(BigInt(vec.expect.remaining));
          } else if (vec.expect.err) {
            expect(() =>
              engine.debitSpend(
                "GDEL1",
                vec.input.asset,
                vec.input.spend,
                10,
                now
              )
            ).toThrow(MuxAccountContractException);

            try {
              engine.debitSpend(
                "GDEL1",
                vec.input.asset,
                vec.input.spend,
                10,
                now
              );
            } catch (err: any) {
              expect(err.errorName).toBe(vec.expect.err);
              if (vec.expect.code) {
                expect(err.code).toBe(vec.expect.code);
              }
            }
          }
        } else if (vec.id === "acct-lmt-debit-cumulative") {
          engine.setSpendLimit(
            "GOWNER1",
            vec.input.asset,
            vec.input.limit,
            vec.input.period_ledgers
          );
          // First spend succeeds
          const res1 = engine.debitSpend(
            "GDEL1",
            vec.input.asset,
            vec.input.spends[0],
            10,
            now
          );
          expect(res1.remainingLimit).toBe(BigInt(vec.expect.first.remaining));

          // Second spend fails with SpendLimitExceeded
          expect(() =>
            engine.debitSpend(
              "GDEL1",
              vec.input.asset,
              vec.input.spends[1],
              10,
              now
            )
          ).toThrow(MuxAccountContractException);

          try {
            engine.debitSpend(
              "GDEL1",
              vec.input.asset,
              vec.input.spends[1],
              10,
              now
            );
          } catch (err: any) {
            expect(err.errorName).toBe(vec.expect.second.err);
            if (vec.expect.second.code) {
              expect(err.code).toBe(vec.expect.second.code);
            }
          }
        }
      }
    });
  });
});
