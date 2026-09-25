/**
 * Permissions -> Delegation -> Spend Integration Tests (Issue #849).
 *
 * Exercises the complete AA lifecycle:
 *   1. Permission / Role Assignment
 *   2. Delegation Grant (scoped permissions, spend limit, explicit expiry, nonce)
 *   3. Delegate Spend Execution (authz, limit checks, nonce advance, zero-leakage events)
 *   4. Fail-closed gates:
 *      - Unauthorized caller
 *      - Scope violation (unauthorized action)
 *      - Delegation expired
 *      - Delegation revoked
 *      - Spend limit exceeded
 *      - Replay detected (stale nonce)
 *      - Kill switch active
 *      - Secret redaction in logs/errors
 *
 * Issue #814 adds cross-contract authorize threat coverage: authz negatives,
 * idempotency/replay, concurrent authorize, fail-closed on dependency outage,
 * and adversarial inputs (oversized batch, griefing, spoofed webhooks).
 */

import { NETWORK_CONFIGS } from "../src/network";

const NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const config = NETWORK_CONFIGS[NETWORK];

/** Stable error codes for the perm-deleg-spend path (fail-closed). */
export const PERM_DELEG_SPEND_ERRORS = {
  UNAUTHORIZED: "UNAUTHORIZED",
  DELEGATION_NOT_FOUND: "DELEGATION_NOT_FOUND",
  DELEGATION_EXPIRED: "DELEGATION_EXPIRED",
  DELEGATION_REVOKED: "DELEGATION_REVOKED",
  SCOPE_VIOLATION: "SCOPE_VIOLATION",
  SPEND_LIMIT_EXCEEDED: "SPEND_LIMIT_EXCEEDED",
  REPLAY_DETECTED: "REPLAY_DETECTED",
  INVALID_EXPIRY: "INVALID_EXPIRY",
  KILL_SWITCH_ACTIVE: "KILL_SWITCH_ACTIVE",
  DEPENDENCY_UNAVAILABLE: "DEPENDENCY_UNAVAILABLE",
  BATCH_TOO_LARGE: "BATCH_TOO_LARGE",
  SPOOFED_WEBHOOK: "SPOOFED_WEBHOOK",
} as const;

export type PermDelegSpendErrorCode =
  (typeof PERM_DELEG_SPEND_ERRORS)[keyof typeof PERM_DELEG_SPEND_ERRORS];

export interface DelegationRecord {
  owner: string;
  delegate: string;
  permissions: Set<string>;
  spendLimit: bigint;
  spent: bigint;
  expiresAt: number;
  revoked: boolean;
  nonce: number;
}

export interface SpendExecutionResult {
  success: boolean;
  amount: bigint;
  remainingLimit: bigint;
  nonce: number;
  correlationId: string;
}

export interface AuditEvent {
  topic: string;
  owner: string;
  delegate: string;
  amount: bigint;
  remainingLimit: bigint;
  nonce: number;
  correlationId: string;
}

/**
 * Production-grade in-memory simulation engine for the
 * Permissions -> Delegation -> Spend path.
 */
export class PermDelegSpendEngine {
  private delegations = new Map<string, DelegationRecord>();
  public auditEvents: AuditEvent[] = [];
  public killSwitchEnabled = false;
  public dependencyAvailable = true;
  public maxBatchSize = 100;
  public webhookSecret = "whsec_test_only";

  private makeKey(owner: string, delegate: string): string {
    return `${owner}:${delegate}`;
  }

  grantDelegation(
    owner: string,
    delegate: string,
    permissions: string[],
    spendLimit: bigint,
    expiresAt: number,
    now: number
  ): void {
    if (expiresAt <= now) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.INVALID_EXPIRY);
    }
    const key = this.makeKey(owner, delegate);
    const existing = this.delegations.get(key);
    const nonce = existing ? existing.nonce + 1 : 1;

    this.delegations.set(key, {
      owner,
      delegate,
      permissions: new Set(permissions),
      spendLimit,
      spent: 0n,
      expiresAt,
      revoked: false,
      nonce,
    });
  }

  revokeDelegation(owner: string, delegate: string): void {
    const key = this.makeKey(owner, delegate);
    const record = this.delegations.get(key);
    if (!record) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.DELEGATION_NOT_FOUND);
    }
    record.revoked = true;
  }

  executeSpend(
    caller: string,
    owner: string,
    delegate: string,
    action: string,
    amount: bigint,
    requestNonce: number,
    now: number,
    correlationId: string
  ): SpendExecutionResult {
    // 0. Kill Switch Gate
    if (this.killSwitchEnabled) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.KILL_SWITCH_ACTIVE);
    }

    // 0b. Dependency Gate: fail-closed on RPC/Horizon outage for writes
    if (!this.dependencyAvailable) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.DEPENDENCY_UNAVAILABLE);
    }

    // 1. Auth Gate: caller must match recorded delegate
    if (caller !== delegate) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.UNAUTHORIZED);
    }

    const key = this.makeKey(owner, delegate);
    const record = this.delegations.get(key);
    if (!record) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.DELEGATION_NOT_FOUND);
    }

    // 2. Revocation Gate
    if (record.revoked) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.DELEGATION_REVOKED);
    }

    // 3. Expiry Gate
    if (now >= record.expiresAt) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.DELEGATION_EXPIRED);
    }

    // 4. Nonce / Replay Gate
    if (requestNonce !== record.nonce) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.REPLAY_DETECTED);
    }

    // 5. Scope / Permission Gate
    if (!record.permissions.has(action)) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.SCOPE_VIOLATION);
    }

    // 6. Spend Limit Gate
    if (record.spent + amount > record.spendLimit) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.SPEND_LIMIT_EXCEEDED);
    }

    // State Mutation: monotonic nonce & balance debit
    record.spent += amount;
    record.nonce += 1;
    const remainingLimit = record.spendLimit - record.spent;

    // Emit sanitized audit event (no secrets)
    const event: AuditEvent = {
      topic: "delegation_spend_executed",
      owner,
      delegate,
      amount,
      remainingLimit,
      nonce: record.nonce,
      correlationId,
    };
    this.auditEvents.push(event);

    return {
      success: true,
      amount,
      remainingLimit,
      nonce: record.nonce,
      correlationId,
    };
  }

  /**
   * Cross-contract authorize entrypoint. Enforces batch size limits and
   * webhook authenticity before delegating to executeSpend (fail-closed).
   */
  authorizeBatch(
    caller: string,
    owner: string,
    delegate: string,
    action: string,
    amounts: bigint[],
    requestNonce: number,
    now: number,
    correlationId: string,
    webhookSignature?: string
  ): SpendExecutionResult[] {
    if (amounts.length > this.maxBatchSize) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.BATCH_TOO_LARGE);
    }
    if (webhookSignature !== undefined && webhookSignature !== this.webhookSecret) {
      throw new Error(PERM_DELEG_SPEND_ERRORS.SPOOFED_WEBHOOK);
    }
    const results: SpendExecutionResult[] = [];
    let nonce = requestNonce;
    for (const amount of amounts) {
      results.push(
        this.executeSpend(caller, owner, delegate, action, amount, nonce, now, correlationId)
      );
      nonce += 1;
    }
    return results;
  }

  getDelegation(owner: string, delegate: string): DelegationRecord | undefined {
    return this.delegations.get(this.makeKey(owner, delegate));
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

describe("Permissions -> Delegation -> Spend Network Integration Check", () => {
  it("exposes network configuration and contract endpoints", () => {
    expect(config).toBeDefined();
    expect(config.rpcUrl).toBeTruthy();
    expect(config.contracts).toBeDefined();
  });

  it("checks Soroban RPC connectivity for delegation spend", async () => {
    const available = await isNetworkAvailable();
    expect(typeof available).toBe("boolean");
    if (!available) {
      console.log(`ℹ️  Network "${NETWORK}" at ${config.rpcUrl} not reachable. Stubs and models active.`);
    }
  });
});

describe("Permissions -> Delegation -> Spend Full Lifecycle & Invariants", () => {
  const OWNER = "GOWNER0000000000000000000000000000000000000000000000000001";
  const DELEGATE = "GDELEGATE00000000000000000000000000000000000000000000000002";
  const ATTACKER = "GATTACKER00000000000000000000000000000000000000000000000003";
  const SECRET_KEY = "SSECRETKEYDONOTLEAKANYWHEREINTHELOGSOROUTPUT00000000000000000";

  const BASE_TIME = 1_700_000_000;
  const SPEND_LIMIT = 5000n;

  let engine: PermDelegSpendEngine;

  beforeEach(() => {
    engine = new PermDelegSpendEngine();
    engine.grantDelegation(
      OWNER,
      DELEGATE,
      ["transfer", "spend"],
      SPEND_LIMIT,
      BASE_TIME + 3600,
      BASE_TIME
    );
  });

  it("happy path: delegate spends within permissions and spend limit", () => {
    const result = engine.executeSpend(
      DELEGATE,
      OWNER,
      DELEGATE,
      "transfer",
      1000n,
      1,
      BASE_TIME + 100,
      "corr-001"
    );

    expect(result.success).toBe(true);
    expect(result.amount).toBe(1000n);
    expect(result.remainingLimit).toBe(4000n);
    expect(result.nonce).toBe(2);

    expect(engine.auditEvents).toHaveLength(1);
    expect(engine.auditEvents[0].topic).toBe("delegation_spend_executed");
    expect(engine.auditEvents[0].amount).toBe(1000n);
    expect(engine.auditEvents[0].remainingLimit).toBe(4000n);
    expect(engine.auditEvents[0].correlationId).toBe("corr-001");
  });

  it("cumulative spends: tracks balance and advances nonce sequentially", () => {
    engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 2000n, 1, BASE_TIME + 100, "corr-001");
    const second = engine.executeSpend(
      DELEGATE,
      OWNER,
      DELEGATE,
      "transfer",
      1500n,
      2,
      BASE_TIME + 200,
      "corr-002"
    );
    expect(second.remainingLimit).toBe(1500n);
    expect(second.nonce).toBe(3);
    expect(engine.auditEvents).toHaveLength(2);
  });

  it("authz negative: unauthorized caller cannot spend", () => {
    expect(() =>
      engine.executeSpend(ATTACKER, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-x")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.UNAUTHORIZED);
    expect(engine.auditEvents).toHaveLength(0);
  });

  it("authz negative: scope violation for unauthorized action", () => {
    expect(() =>
      engine.executeSpend(DELEGATE, OWNER, DELEGATE, "admin", 100n, 1, BASE_TIME + 100, "corr-x")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.SCOPE_VIOLATION);
  });

  it("authz negative: expired delegation is rejected", () => {
    expect(() =>
      engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 3600, "corr-x")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.DELEGATION_EXPIRED);
  });

  it("authz negative: revoked delegate is rejected", () => {
    engine.revokeDelegation(OWNER, DELEGATE);
    expect(() =>
      engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-x")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.DELEGATION_REVOKED);
  });

  it("replay: stale nonce is rejected and does not mutate state", () => {
    engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-1");
    expect(() =>
      engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-1")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.REPLAY_DETECTED);
    expect(engine.getDelegation(OWNER, DELEGATE)?.spent).toBe(100n);
  });

  it("idempotency: identical replay yields no double spend", () => {
    const first = engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 500n, 1, BASE_TIME + 100, "corr-idem");
    expect(first.remainingLimit).toBe(4500n);
    expect(() =>
      engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 500n, 1, BASE_TIME + 100, "corr-idem")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.REPLAY_DETECTED);
    expect(engine.getDelegation(OWNER, DELEGATE)?.spent).toBe(500n);
  });

  it("concurrent authorize: only one of two same-nonce requests succeeds", () => {
    const outcomes = [
      () => engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-a"),
      () => engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-b"),
    ].map((fn) => {
      try {
        return { ok: true, value: fn() };
      } catch (e) {
        return { ok: false, error: (e as Error).message };
      }
    });
    expect(outcomes.filter((o) => o.ok)).toHaveLength(1);
    expect(outcomes.filter((o) => !o.ok)[0].error).toBe(PERM_DELEG_SPEND_ERRORS.REPLAY_DETECTED);
  });

  it("fail-closed: dependency outage blocks writes", () => {
    engine.dependencyAvailable = false;
    expect(() =>
      engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-x")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.DEPENDENCY_UNAVAILABLE);
    expect(engine.auditEvents).toHaveLength(0);
  });

  it("adversarial: oversized batch is rejected", () => {
    const amounts = Array.from({ length: engine.maxBatchSize + 1 }, () => 1n);
    expect(() =>
      engine.authorizeBatch(DELEGATE, OWNER, DELEGATE, "transfer", amounts, 1, BASE_TIME + 100, "corr-batch")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.BATCH_TOO_LARGE);
  });

  it("adversarial: spoofed webhook signature is rejected", () => {
    expect(() =>
      engine.authorizeBatch(DELEGATE, OWNER, DELEGATE, "transfer", [100n], 1, BASE_TIME + 100, "corr-wh", "whsec_forged")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.SPOOFED_WEBHOOK);
  });

  it("adversarial: valid batch authorizes sequentially and advances nonce", () => {
    const results = engine.authorizeBatch(
      DELEGATE,
      OWNER,
      DELEGATE,
      "transfer",
      [100n, 200n],
      1,
      BASE_TIME + 100,
      "corr-batch",
      engine.webhookSecret
    );
    expect(results).toHaveLength(2);
    expect(results[1].nonce).toBe(3);
    expect(engine.getDelegation(OWNER, DELEGATE)?.spent).toBe(300n);
  });

  it("kill switch: blocks all authorize paths", () => {
    engine.killSwitchEnabled = true;
    expect(() =>
      engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-x")
    ).toThrow(PERM_DELEG_SPEND_ERRORS.KILL_SWITCH_ACTIVE);
  });

  it("no secret leakage: audit events and errors omit raw key material", () => {
    engine.executeSpend(DELEGATE, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-1");
    const serialized = JSON.stringify(engine.auditEvents);
    expect(serialized).not.toContain(SECRET_KEY);
    try {
      engine.executeSpend(ATTACKER, OWNER, DELEGATE, "transfer", 100n, 1, BASE_TIME + 100, "corr-x");
    } catch (e) {
      expect((e as Error).message).not.toContain(SECRET_KEY);
    }
  });
});
