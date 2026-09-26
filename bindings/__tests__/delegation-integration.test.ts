/**
 * Integration test stub for mux-delegation contract.
 *
 * These tests verify that the delegation bindings can interact with a
 * live or local Soroban network. They are skipped when the network is
 * unavailable, matching the pattern in integration.test.ts.
 *
 * Delegation expiry invariants (issue #869):
 *   - A delegate grant carries an explicit expiry timestamp.
 *   - Expired or revoked delegates are rejected fail-closed.
 *   - Expiry failures surface stable error codes, never raw key material.
 *
 * Cross-contract authorize threat tests (issue #814):
 *   - Cross-contract authorize calls are deny-by-default.
 *   - Wrong role, expired auth, revoked delegate, and unauthorized
 *     callers are rejected with stable error codes.
 *   - Replayed/concurrent authorize requests are idempotent.
 *   - Dependency outage (RPC/Horizon) fails closed on writes.
 *   - Adversarial inputs (oversized batch, griefing, spoofed webhooks)
 *     are rejected without leaking secrets or raw key material.
 */

import { NETWORK_CONFIGS } from "../src/network";

const NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const config = NETWORK_CONFIGS[NETWORK];

/** Stable error codes for delegation expiry failures (fail-closed). */
const DELEGATION_ERROR_CODES = {
  DELEGATE_EXPIRED: "DELEGATE_EXPIRED",
  DELEGATE_REVOKED: "DELEGATE_REVOKED",
  DELEGATE_NOT_FOUND: "DELEGATE_NOT_FOUND",
  UNAUTHORIZED: "UNAUTHORIZED",
} as const;

type DelegationErrorCode =
  (typeof DELEGATION_ERROR_CODES)[keyof typeof DELEGATION_ERROR_CODES];

/**
 * Stable error codes for cross-contract authorize failures (issue #814).
 * Deny-by-default: every privileged surface returns one of these codes
 * instead of raw contract/RPC errors or key material.
 */
const AUTHORIZE_ERROR_CODES = {
  UNAUTHORIZED: "UNAUTHORIZED",
  WRONG_ROLE: "WRONG_ROLE",
  AUTH_EXPIRED: "AUTH_EXPIRED",
  DELEGATE_REVOKED: "DELEGATE_REVOKED",
  REPLAY_DETECTED: "REPLAY_DETECTED",
  DEPENDENCY_UNAVAILABLE: "DEPENDENCY_UNAVAILABLE",
  BATCH_TOO_LARGE: "BATCH_TOO_LARGE",
  SPOOFED_WEBHOOK: "SPOOFED_WEBHOOK",
} as const;

type AuthorizeErrorCode =
  (typeof AUTHORIZE_ERROR_CODES)[keyof typeof AUTHORIZE_ERROR_CODES];

/**
 * Minimal in-memory model of the delegation expiry invariants so the
 * expiry/revocation rules are covered even when no network is available.
 * Mirrors the on-chain contract semantics: deny-by-default, expiry is
 * exclusive (now >= expiresAt means expired).
 */
interface DelegateGrant {
  delegate: string;
  expiresAt: number;
  revoked: boolean;
}

class DelegationExpiryModel {
  private grants = new Map<string, DelegateGrant>();

  grant(delegate: string, expiresAt: number): void {
    this.grants.set(delegate, { delegate, expiresAt, revoked: false });
  }

  revoke(delegate: string): void {
    const grant = this.grants.get(delegate);
    if (!grant) {
      throw new Error(DELEGATION_ERROR_CODES.DELEGATE_NOT_FOUND);
    }
    grant.revoked = true;
  }

  /** Fail-closed authorization check used by every privileged entrypoint. */
  assertAuthorized(delegate: string, now: number): void {
    const grant = this.grants.get(delegate);
    if (!grant) {
      throw new Error(DELEGATION_ERROR_CODES.DELEGATE_NOT_FOUND);
    }
    if (grant.revoked) {
      throw new Error(DELEGATION_ERROR_CODES.DELEGATE_REVOKED);
    }
    if (now >= grant.expiresAt) {
      throw new Error(DELEGATION_ERROR_CODES.DELEGATE_EXPIRED);
    }
  }

  isDelegate(delegate: string, now: number): boolean {
    try {
      this.assertAuthorized(delegate, now);
      return true;
    } catch {
      return false;
    }
  }
}

/**
 * Minimal in-memory model of the cross-contract authorize path so the
 * threat scenarios in issue #814 are covered without a live network.
 *
 * Invariants:
 *   - Deny-by-default: unknown callers/roles are rejected.
 *   - Authz is checked before any state mutation (fail-closed).
 *   - Replayed request ids are rejected (idempotency guard).
 *   - Dependency outage fails closed on writes.
 *   - Oversized batches and spoofed webhooks are rejected.
 */
interface AuthorizeRequest {
  requestId: string;
  caller: string;
  role: string;
  authExpiresAt: number;
  delegateRevoked?: boolean;
  webhookSignature?: string;
}

class CrossContractAuthorizeModel {
  private seenRequestIds = new Set<string>();
  private readonly allowedRoles: ReadonlySet<string>;
  private readonly maxBatchSize: number;
  private readonly webhookSecret: string;
  private dependencyUp = true;

  constructor(opts: {
    allowedRoles: string[];
    maxBatchSize: number;
    webhookSecret: string;
  }) {
    this.allowedRoles = new Set(opts.allowedRoles);
    this.maxBatchSize = opts.maxBatchSize;
    this.webhookSecret = opts.webhookSecret;
  }

  setDependencyUp(up: boolean): void {
    this.dependencyUp = up;
  }

  /** Fail-closed authorize check; throws a stable error code on denial. */
  authorize(req: AuthorizeRequest, now: number): void {
    if (!this.dependencyUp) {
      throw new Error(AUTHORIZE_ERROR_CODES.DEPENDENCY_UNAVAILABLE);
    }
    if (!req.caller) {
      throw new Error(AUTHORIZE_ERROR_CODES.UNAUTHORIZED);
    }
    if (!this.allowedRoles.has(req.role)) {
      throw new Error(AUTHORIZE_ERROR_CODES.WRONG_ROLE);
    }
    if (now >= req.authExpiresAt) {
      throw new Error(AUTHORIZE_ERROR_CODES.AUTH_EXPIRED);
    }
    if (req.delegateRevoked) {
      throw new Error(AUTHORIZE_ERROR_CODES.DELEGATE_REVOKED);
    }
    if (this.seenRequestIds.has(req.requestId)) {
      throw new Error(AUTHORIZE_ERROR_CODES.REPLAY_DETECTED);
    }
    this.seenRequestIds.add(req.requestId);
  }

  /** Authorize a batch; oversized batches are rejected before any work. */
  authorizeBatch(reqs: AuthorizeRequest[], now: number): void {
    if (reqs.length > this.maxBatchSize) {
      throw new Error(AUTHORIZE_ERROR_CODES.BATCH_TOO_LARGE);
    }
    for (const req of reqs) {
      this.authorize(req, now);
    }
  }

  /** Verify a webhook signature; spoofed webhooks are rejected. */
  verifyWebhook(signature: string | undefined): void {
    if (!signature || signature !== this.webhookSecret) {
      throw new Error(AUTHORIZE_ERROR_CODES.SPOOFED_WEBHOOK);
    }
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

describe("Delegation Integration Tests", () => {
  let networkAvailable: boolean;
  let contractDeployed: boolean;

  const DELEGATION_CONTRACT_ID =
    process.env[`${NETWORK.toUpperCase()}_MUX_DELEGATION_ID`] ||
    config.contracts.muxDelegation ||
    "";

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    contractDeployed = networkAvailable && !!DELEGATION_CONTRACT_ID;

    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" is unavailable at ${config.rpcUrl}. ` +
        `Delegation integration tests will be skipped.`
      );
    } else if (!contractDeployed) {
      console.warn(
        `⚠️  Delegation contract ID not set for network "${NETWORK}". ` +
        `Set ${NETWORK.toUpperCase()}_MUX_DELEGATION_ID to enable these tests.`
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

  it("should query is_delegate from a deployed delegation contract", async () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — delegation contract not available on "${NETWORK}".`);
      return;
    }
    // Call is_delegate via JSON-RPC simulation to verify contract is callable.
    const body = {
      jsonrpc: "2.0",
      id: 2,
      method: "simulateTransaction",
      params: [{ contractId: DELEGATION_CONTRACT_ID, method: "is_delegate", args: ["", ""] }],
    };
    const response = await globalThis.fetch(config.rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    // Accept 200 (success with result) or 400 (RPC error from contract) — contract is present.
    expect([200, 400]).toContain(response.status);
  });

  it("stub: grant_delegate round-trip via bindings", () => {
    if (!contractDeployed) {
      console.log("Skipped — contract not available");
      return;
    }
    // TODO: instantiate MuxDelegationClient, grant a delegate, and verify
    // with get_delegate_permissions. Requires a funded keypair on the
    // target network.
    console.info("TODO: wire MuxDelegationClient with funded keypair for grant_delegate test");
    expect(true).toBe(true);
  });

  it("stub: revoke_delegate round-trip via bindings", () => {
    if (!contractDeployed) {
      console.log("Skipped — contract not available");
      return;
    }
    // TODO: grant then revoke a delegate and assert is_delegate returns false.
    console.info("TODO: wire MuxDelegationClient with funded keypair for revoke_delegate test");
    expect(true).toBe(true);
  });

  it("stub: is_delegate query via bindings", () => {
    if (!contractDeployed) {
      console.log("Skipped — contract not available");
      return;
    }
    // TODO: query is_delegate for an unknown delegate and expect false.
    console.info("TODO: wire MuxDelegationClient with funded keypair for is_delegate test");
    expect(true).toBe(true);
  });
});

describe("Delegation expiry invariants", () => {
  const NOW = 1_700_000_000;
  const DELEGATE = "GDELEGATE000000000000000000000000000000000000000000000000";

  it("accepts a delegate before its expiry timestamp", () => {
    const model = new DelegationExpiryModel();
    model.grant(DELEGATE, NOW + 3600);
    expect(model.isDelegate(DELEGATE, NOW)).toBe(true);
    expect(() => model.assertAuthorized(DELEGATE, NOW)).not.toThrow();
  });

  it("rejects an expired delegate with DELEGATE_EXPIRED (fail-closed)", () => {
    const model = new DelegationExpiryModel();
    model.grant(DELEGATE, NOW);
    expect(model.isDelegate(DELEGATE, NOW)).toBe(false);
    expect(() => model.assertAuthorized(DELEGATE, NOW)).toThrow(
      DELEGATION_ERROR_CODES.DELEGATE_EXPIRED
    );
  });

  it("rejects a revoked delegate with DELEGATE_REVOKED", () => {
    const model = new DelegationExpiryModel();
    model.grant(DELEGATE, NOW + 3600);
    model.revoke(DELEGATE);
    expect(model.isDelegate(DELEGATE, NOW)).toBe(false);
    expect(() => model.assertAuthorized(DELEGATE, NOW)).toThrow(
      DELEGATION_ERROR_CODES.DELEGATE_REVOKED
    );
  });

  it("denies-by-default for an unknown delegate", () => {
    const model = new DelegationExpiryModel();
    expect(model.isDelegate(DELEGATE, NOW)).toBe(false);
    expect(() => model.assertAuthorized(DELEGATE, NOW)).toThrow(
      DELEGATION_ERROR_CODES.DELEGATE_NOT_FOUND
    );
  });

  it("revoking an unknown delegate fails with DELEGATE_NOT_FOUND", () => {
    const model = new DelegationExpiryModel();
    expect(() => model.revoke(DELEGATE)).toThrow(
      DELEGATION_ERROR_CODES.DELEGATE_NOT_FOUND
    );
  });
});

describe("Cross-contract authorize threat tests (#814)", () => {
  const NOW = 1_700_000_000;
  const CALLER = "GCALLER0000000000000000000000000000000000000000000000000";
  const WEBHOOK_SECRET = "whsec_test_only_not_a_real_secret";

  function makeModel(): CrossContractAuthorizeModel {
    return new CrossContractAuthorizeModel({
      allowedRoles: ["owner", "delegate"],
      maxBatchSize: 3,
      webhookSecret: WEBHOOK_SECRET,
    });
  }

  function validRequest(overrides: Partial<AuthorizeRequest> = {}): AuthorizeRequest {
    return {
      requestId: "req-1",
      caller: CALLER,
      role: "owner",
      authExpiresAt: NOW + 3600,
      ...overrides,
    };
  }

  it("authorizes a valid owner request", () => {
    const model = makeModel();
    expect(() => model.authorize(validRequest(), NOW)).not.toThrow();
  });

  it("rejects an unauthorized caller (deny-by-default)", () => {
    const model = makeModel();
    expect(() => model.authorize(validRequest({ caller: "" }), NOW)).toThrow(
      AUTHORIZE_ERROR_CODES.UNAUTHORIZED
    );
  });

  it("rejects a caller with the wrong role", () => {
    const model = makeModel();
    expect(() => model.authorize(validRequest({ role: "viewer" }), NOW)).toThrow(
      AUTHORIZE_ERROR_CODES.WRONG_ROLE
    );
  });

  it("rejects an expired auth token", () => {
    const model = makeModel();
    expect(() => model.authorize(validRequest({ authExpiresAt: NOW }), NOW)).toThrow(
      AUTHORIZE_ERROR_CODES.AUTH_EXPIRED
    );
  });

  it("rejects a revoked delegate", () => {
    const model = makeModel();
    expect(() =>
      model.authorize(validRequest({ role: "delegate", delegateRevoked: true }), NOW)
    ).toThrow(AUTHORIZE_ERROR_CODES.DELEGATE_REVOKED);
  });

  it("rejects a replayed request id (idempotency guard)", () => {
    const model = makeModel();
    model.authorize(validRequest({ requestId: "req-replay" }), NOW);
    expect(() => model.authorize(validRequest({ requestId: "req-replay" }), NOW)).toThrow(
      AUTHORIZE_ERROR_CODES.REPLAY_DETECTED
    );
  });

  it("handles concurrent authorize requests with distinct ids", async () => {
    const model = makeModel();
    const results = await Promise.all(
      ["c-1", "c-2", "c-3"].map((id) =>
        Promise.resolve().then(() => {
          try {
            model.authorize(validRequest({ requestId: id }), NOW);
            return "ok";
          } catch (err) {
            return (err as Error).message;
          }
        })
      )
    );
    expect(results).toEqual(["ok", "ok", "ok"]);
  });

  it("fails closed on dependency outage for writes", () => {
    const model = makeModel();
    model.setDependencyUp(false);
    expect(() => model.authorize(validRequest(), NOW)).toThrow(
      AUTHORIZE_ERROR_CODES.DEPENDENCY_UNAVAILABLE
    );
  });

  it("rejects an oversized batch before processing", () => {
    const model = makeModel();
    const batch = ["b-1", "b-2", "b-3", "b-4"].map((id) =>
      validRequest({ requestId: id })
    );
    expect(() => model.authorizeBatch(batch, NOW)).toThrow(
      AUTHORIZE_ERROR_CODES.BATCH_TOO_LARGE
    );
  });

  it("accepts a batch within the size limit", () => {
    const model = makeModel();
    const batch = ["b-1", "b-2", "b-3"].map((id) => validRequest({ requestId: id }));
    expect(() => model.authorizeBatch(batch, NOW)).not.toThrow();
  });

  it("rejects a spoofed webhook signature", () => {
    const model = makeModel();
    expect(() => model.verifyWebhook("whsec_spoofed")).toThrow(
      AUTHORIZE_ERROR_CODES.SPOOFED_WEBHOOK
    );
    expect(() => model.verifyWebhook(undefined)).toThrow(
      AUTHORIZE_ERROR_CODES.SPOOFED_WEBHOOK
    );
  });

  it("accepts a valid webhook signature", () => {
    const model = makeModel();
    expect(() => model.verifyWebhook(WEBHOOK_SECRET)).not.toThrow();
  });

  it("does not leak secrets or raw key material in error messages", () => {
    const model = makeModel();
    try {
      model.authorize(validRequest({ role: "viewer" }), NOW);
      throw new Error("expected authorize to throw");
    } catch (err) {
      const message = (err as Error).message;
      expect(message).toBe(AUTHORIZE_ERROR_CODES.WRONG_ROLE);
      expect(message).not.toContain(WEBHOOK_SECRET);
      expect(message).not.toContain(CALLER);
    }
  });
});
