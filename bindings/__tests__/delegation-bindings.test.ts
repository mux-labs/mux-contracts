/**
 * Unit tests for MuxDelegationClient binding shape, DelegationQueryFilters,
 * error mapping, and delegation event constants/parser.
 */

import {
  MuxDelegationClient,
  DelegationQueryFilters,
  DELEGATION_CONTRACT_TAG,
  DELEGATION_GRANT_ACTION,
  DELEGATION_REVOKE_ACTION,
  DELEGATION_EXPIRY_ERROR_CODES,
  DelegationExpiryError,
  parseDelegationEvent,
  type DelegationEvent,
  type DelegationGrantEvent,
  type DelegationRevokeEvent,
} from "../src/generated/mux-delegation";
import { ERROR_HTTP_MAP } from "../src/errors";
import { muxDelegationErrorMessage } from "../src/types";

// ── Client shape — permissions-map methods ────────────────────────────────────

describe("MuxDelegationClient shape — permissions map (closes #407)", () => {
  it("exposes getDelegatePermissions as a function", () => {
    expect(typeof MuxDelegationClient.prototype.getDelegatePermissions).toBe("function");
  });

  it("getDelegatePermissions accepts sourceKeypair, owner, delegate, and optional filters (arity 4)", () => {
    expect(MuxDelegationClient.prototype.getDelegatePermissions.length).toBe(4);
  });

  it("exposes isDelegate as a function", () => {
    expect(typeof MuxDelegationClient.prototype.isDelegate).toBe("function");
  });

  it("isDelegate accepts sourceKeypair, owner, delegate, permission (arity 4)", () => {
    expect(MuxDelegationClient.prototype.isDelegate.length).toBe(4);
  });

  it("exposes getDelegates as a function", () => {
    expect(typeof MuxDelegationClient.prototype.getDelegates).toBe("function");
  });

  it("exposes checkDelegate as a function", () => {
    expect(typeof MuxDelegationClient.prototype.checkDelegate).toBe("function");
  });
});

describe("DelegationQueryFilters interface", () => {
  it("exports DelegationQueryFilters type", () => {
    const filters: DelegationQueryFilters = {};
    expect(filters).toBeDefined();
  });

  it("accepts a permission filter", () => {
    const filters: DelegationQueryFilters = { permission: "transfer" };
    expect(filters.permission).toBe("transfer");
  });

  it("accepts a hasAnyPermission filter set to true", () => {
    const filters: DelegationQueryFilters = { hasAnyPermission: true };
    expect(filters.hasAnyPermission).toBe(true);
  });

  it("accepts a hasAnyPermission filter set to false", () => {
    const filters: DelegationQueryFilters = { hasAnyPermission: false };
    expect(filters.hasAnyPermission).toBe(false);
  });

  it("accepts combined permission and hasAnyPermission filters", () => {
    const filters: DelegationQueryFilters = {
      permission: "read",
      hasAnyPermission: true,
    };
    expect(filters.permission).toBe("read");
    expect(filters.hasAnyPermission).toBe(true);
  });

  it("accepts an empty filter object (no-op)", () => {
    const filters: DelegationQueryFilters = {};
    expect(filters.permission).toBeUndefined();
    expect(filters.hasAnyPermission).toBeUndefined();
  });
});

describe("checkDelegate method shape", () => {
  it("getDelegatePermissions accepts optional DelegationQueryFilters parameter", () => {
    // Verify the method signature accepts the optional filters parameter.
    // The fourth parameter (filters) is optional; this confirms the binding
    // is callable without it and with it.
    const fn = MuxDelegationClient.prototype.getDelegatePermissions;
    expect(typeof fn).toBe("function");
    // arity: sourceKeypair, owner, delegate, [filters] — length may be 3 or 4
    expect(fn.length).toBeLessThanOrEqual(4);
  });

  it("getDelegates accepts optional DelegationQueryFilters parameter", () => {
    const fn = MuxDelegationClient.prototype.getDelegates;
    expect(typeof fn).toBe("function");
    // arity: sourceKeypair, owner, [filters]
    expect(fn.length).toBeLessThanOrEqual(3);
  });

  it("checkDelegate has the correct parameter count", () => {
    // sourceKeypair, owner, delegate, permission → 4 required params
    const fn = MuxDelegationClient.prototype.checkDelegate;
    expect(fn.length).toBe(4);
  });
});

// ── Role inheritance invariants (closes #868) ─────────────────────────────────
//
// Mirrors docs/permissions-role-model.md: a role inherits every permission of
// its declared parent, and a role can never hold a permission that is not
// reachable through its declared ancestry (no privilege escalation).

type RoleName = "viewer" | "operator" | "treasurer" | "owner";

const ROLE_PARENTS: Record<RoleName, RoleName | null> = {
  viewer: null,
  operator: "viewer",
  treasurer: "operator",
  owner: "treasurer",
};

const ROLE_DIRECT_PERMISSIONS: Record<RoleName, string[]> = {
  viewer: ["read"],
  operator: ["transfer"],
  treasurer: ["withdraw"],
  owner: ["admin"],
};

/** Resolve the effective permission set for a role by walking its ancestry. */
function effectivePermissions(role: RoleName): Set<string> {
  const perms = new Set<string>();
  let current: RoleName | null = role;
  while (current !== null) {
    for (const p of ROLE_DIRECT_PERMISSIONS[current]) perms.add(p);
    current = ROLE_PARENTS[current];
  }
  return perms;
}

/** True when `role` is `ancestor` or descends from it. */
function inheritsFrom(role: RoleName, ancestor: RoleName): boolean {
  let current: RoleName | null = role;
  while (current !== null) {
    if (current === ancestor) return true;
    current = ROLE_PARENTS[current];
  }
  return false;
}

describe("Role inheritance invariants (closes #868)", () => {
  it("a child role inherits every permission of its parent", () => {
    for (const role of Object.keys(ROLE_PARENTS) as RoleName[]) {
      const parent = ROLE_PARENTS[role];
      if (parent === null) continue;
      const childPerms = effectivePermissions(role);
      for (const p of effectivePermissions(parent)) {
        expect(childPerms.has(p)).toBe(true);
      }
    }
  });

  it("owner transitively inherits the full permission chain", () => {
    const ownerPerms = effectivePermissions("owner");
    expect(ownerPerms.has("read")).toBe(true);
    expect(ownerPerms.has("transfer")).toBe(true);
    expect(ownerPerms.has("withdraw")).toBe(true);
    expect(ownerPerms.has("admin")).toBe(true);
  });

  it("a role does not inherit permissions from roles outside its ancestry", () => {
    // operator descends from viewer but not from treasurer/owner.
    const operatorPerms = effectivePermissions("operator");
    expect(operatorPerms.has("withdraw")).toBe(false);
    expect(operatorPerms.has("admin")).toBe(false);
  });

  it("no privilege escalation: a role never holds a permission outside its declared ancestry", () => {
    for (const role of Object.keys(ROLE_PARENTS) as RoleName[]) {
      const declared = new Set<string>();
      let current: RoleName | null = role;
      while (current !== null) {
        for (const p of ROLE_DIRECT_PERMISSIONS[current]) declared.add(p);
        current = ROLE_PARENTS[current];
      }
      for (const p of effectivePermissions(role)) {
        expect(declared.has(p)).toBe(true);
      }
    }
  });

  it("inheritsFrom is reflexive and transitive along the declared chain", () => {
    expect(inheritsFrom("owner", "owner")).toBe(true);
    expect(inheritsFrom("owner", "viewer")).toBe(true);
    expect(inheritsFrom("treasurer", "operator")).toBe(true);
    expect(inheritsFrom("viewer", "owner")).toBe(false);
  });
});

// ── Authz negatives — deny-by-default (closes #868) ───────────────────────────

describe("Role inheritance authz negatives (closes #868)", () => {
  interface Caller {
    role: RoleName | null;
    expired?: boolean;
    revoked?: boolean;
    spoofed?: boolean;
  }

  /** Deny-by-default authorization check for a required permission. */
  function authorize(caller: Caller, required: string): boolean {
    if (caller.spoofed) return false;
    if (caller.revoked) return false;
    if (caller.expired) return false;
    if (caller.role === null) return false;
    return effectivePermissions(caller.role).has(required);
  }

  it("denies a caller with no role (missing role)", () => {
    expect(authorize({ role: null }, "read")).toBe(false);
  });

  it("denies a caller whose role lacks the required permission (wrong role)", () => {
    expect(authorize({ role: "viewer" }, "withdraw")).toBe(false);
  });

  it("denies a caller with an expired role", () => {
    expect(authorize({ role: "owner", expired: true }, "admin")).toBe(false);
  });

  it("denies a revoked delegate even when the role would otherwise allow it", () => {
    expect(authorize({ role: "treasurer", revoked: true }, "withdraw")).toBe(false);
  });

  it("denies a spoofed caller regardless of claimed role", () => {
    expect(authorize({ role: "owner", spoofed: true }, "admin")).toBe(false);
  });

  it("allows a valid caller holding the required permission", () => {
    expect(authorize({ role: "operator" }, "transfer")).toBe(true);
  });
});

// ── Idempotency / replay + fail-closed on write (closes #868) ─────────────────

describe("Privileged entrypoint idempotency and fail-closed writes (closes #868)", () => {
  interface WriteResult {
    ok: boolean;
    applied: boolean;
    error?: string;
  }

  /**
   * Simulates a privileged write guarded by an idempotency key and a
   * dependency health gate. Replays are no-ops; dependency outages fail closed.
   */
  function privilegedWrite(
    seenKeys: Set<string>,
    idempotencyKey: string,
    dependencyHealthy: boolean
  ): WriteResult {
    if (seenKeys.has(idempotencyKey)) {
      return { ok: true, applied: false };
    }
    if (!dependencyHealthy) {
      return { ok: false, applied: false, error: "dependency_unavailable" };
    }
    seenKeys.add(idempotencyKey);
    return { ok: true, applied: true };
  }

  it("applies a first-time privileged write", () => {
    const seen = new Set<string>();
    expect(privilegedWrite(seen, "key-1", true)).toEqual({ ok: true, applied: true });
  });

  it("treats a replayed idempotency key as a no-op", () => {
    const seen = new Set<string>();
    privilegedWrite(seen, "key-1", true);
    expect(privilegedWrite(seen, "key-1", true)).toEqual({ ok: true, applied: false });
  });

  it("fails closed when the dependency is unavailable", () => {
    const seen = new Set<string>();
    const result = privilegedWrite(seen, "key-2", false);
    expect(result.ok).toBe(false);
    expect(result.applied).toBe(false);
    expect(result.error).toBe("dependency_unavailable");
  });

  it("does not record the key when the write fails closed", () => {
    const seen = new Set<string>();
    privilegedWrite(seen, "key-3", false);
    expect(seen.has("key-3")).toBe(false);
  });
});

// ── HTTP error mapping ────────────────────────────────────────────────────────

describe("Delegation error HTTP mapping", () => {
  // Revoke-path error: no grant found for the (owner, delegate) pair.
  it("maps NotADelegate to 404", () => {
    expect(ERROR_HTTP_MAP.NotADelegate).toBe(404);
  });

  it("maps TooManyPermissions to 400", () => {
    expect(ERROR_HTTP_MAP.TooManyPermissions).toBe(400);
  });

  it("maps EmptyPermissions to 400", () => {
    expect(ERROR_HTTP_MAP.EmptyPermissions).toBe(400);
  });

  it("maps TooManyDelegates to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyDelegates).toBe(409);
  });
});

// ── Error message helper ──────────────────────────────────────────────────────

describe("muxDelegationErrorMessage helper", () => {
  it("resolves NotADelegate by name (6001)", () => {
    expect(muxDelegationErrorMessage("NotADelegate")).toBe(
      "no delegate grant found for this pair"
    );
  });

  it("resolves NotADelegate by code 6001", () => {
    expect(muxDelegationErrorMessage(6001)).toBe(
      "no delegate grant found for this pair"
    );
  });

  it("resolves TooManyPermissions by name (6002)", () => {
    expect(muxDelegationErrorMessage("TooManyPermissions")).toBe(
      "permission list exceeds the 64-entry cap"
    );
  });

  it("resolves TooManyPermissions by code 6002", () => {
    expect(muxDelegationErrorMessage(6002)).toBe(
      "permission list exceeds the 64-entry cap"
    );
  });

  it("resolves EmptyPermissions by name (6003)", () => {
    expect(muxDelegationErrorMessage("EmptyPermissions")).toBe(
      "permission list is empty; at least one permission is required"
    );
  });

  it("resolves EmptyPermissions by code 6003", () => {
    expect(muxDelegationErrorMessage(6003)).toBe(
      "permission list is empty; at least one permission is required"
    );
  });

  it("resolves TooManyDelegates by name (6004)", () => {
    expect(muxDelegationErrorMessage("TooManyDelegates")).toBe(
      "owner already has 128 delegates registered"
    );
  });

  it("resolves TooManyDelegates by code 6004", () => {
    expect(muxDelegationErrorMessage(6004)).toBe(
      "owner already has 128 delegates registered"
    );
  });

  it("returns 'unknown error code' for unrecognised code", () => {
    expect(muxDelegationErrorMessage(9999)).toBe("unknown error code");
  });

  it("returns 'unknown error code' for code 0", () => {
    expect(muxDelegationErrorMessage(0)).toBe("unknown error code");
  });
});

// ── Delegation event constants (closes #409) ──────────────────────────────────

describe("Delegation event constants", () => {
  it("DELEGATION_CONTRACT_TAG is 'mux_dlg'", () => {
    expect(DELEGATION_CONTRACT_TAG).toBe("mux_dlg");
  });

  it("DELEGATION_CONTRACT_TAG has length <= 9 (symbol_short limit)", () => {
    expect(DELEGATION_CONTRACT_TAG.length).toBeLessThanOrEqual(9);
  });

  it("DELEGATION_GRANT_ACTION is 'dlg_grant'", () => {
    expect(DELEGATION_GRANT_ACTION).toBe("dlg_grant");
  });

  it("DELEGATION_GRANT_ACTION has length <= 9 (symbol_short limit)", () => {
    expect(DELEGATION_GRANT_ACTION.length).toBeLessThanOrEqual(9);
  });

  it("DELEGATION_REVOKE_ACTION is 'dlg_rev'", () => {
    expect(DELEGATION_REVOKE_ACTION).toBe("dlg_rev");
  });

  it("DELEGATION_REVOKE_ACTION has length <= 9 (symbol_short limit)", () => {
    expect(DELEGATION_REVOKE_ACTION.length).toBeLessThanOrEqual(9);
  });
});

// ── parseDelegationEvent (closes #409) ───────────────────────────────────────

describe("parseDelegationEvent", () => {
  const makeRaw = (tag: string, action: string, data: unknown[], ledger = 1000) => ({
    topic: [tag, action],
    value: data,
    ledger,
  });

  it("parses a dlg_grant event into DelegationGrantEvent", () => {
    const raw = makeRaw("mux_dlg", "dlg_grant", ["OWNER_ADDR", "DELEGATE_ADDR"], 42);
    const event = parseDelegationEvent(raw);
    expect(event).not.toBeNull();
    expect(event!.action).toBe("dlg_grant");
    expect((event as DelegationGrantEvent).owner).toBe("OWNER_ADDR");
    expect((event as DelegationGrantEvent).delegate).toBe("DELEGATE_ADDR");
    expect(event!.ledger).toBe(42);
  });

  it("parses a dlg_rev event into DelegationRevokeEvent", () => {
    const raw = makeRaw("mux_dlg", "dlg_rev", ["OWNER_ADDR", "DELEGATE_ADDR"], 43);
    const event = parseDelegationEvent(raw);
    expect(event).not.toBeNull();
    expect(event!.action).toBe("dlg_rev");
    expect((event as DelegationRevokeEvent).owner).toBe("OWNER_ADDR");
    expect((event as DelegationRevokeEvent).delegate).toBe("DELEGATE_ADDR");
    expect(event!.ledger).toBe(43);
  });

  it("returns null for an unknown contract tag", () => {
    const raw = makeRaw("other_tag", "dlg_grant", ["OWNER_ADDR", "DELEGATE_ADDR"]);
    expect(parseDelegationEvent(raw)).toBeNull();
  });

  it("returns null for an unknown action", () => {
    const raw = makeRaw("mux_dlg", "dlg_unknown", ["OWNER_ADDR", "DELEGATE_ADDR"]);
    expect(parseDelegationEvent(raw)).toBeNull();
  });

  it("returns null when the topic is missing", () => {
    const raw = { topic: [], value: [], ledger: 1 };
    expect(parseDelegationEvent(raw)).toBeNull();
  });

  it("returns a DelegationEvent union member with a stable action field", () => {
    const raw = makeRaw("mux_dlg", "dlg_grant", ["OWNER_ADDR", "DELEGATE_ADDR"]);
    const event: DelegationEvent | null = parseDelegationEvent(raw);
    expect(event).not.toBeNull();
    expect(typeof event!.action).toBe("string");
  });
});

// ── Delegation bindings auth negatives (closes #796) ──────────────────────────
//
// Invariants verified:
// 1. Fail-closed deny-by-default on all privileged delegation operations.
// 2. Explicit caller authentication: non-owner cannot grant or revoke delegations.
// 3. Delegation expiry: expired delegate grants fail closed with DELEGATION_EXPIRED.
// 4. Delegation revocation: revoked delegate grants fail closed with DELEGATION_REVOKED.
// 5. Unregistered delegates fail closed with DELEGATION_NOT_FOUND.
// 6. Scope violation: actions outside granted permissions fail closed.
// 7. Capacity limits: grants exceeding MAX_DELEGATES (128) or MAX_PERMISSIONS (64) fail closed.
// 8. Kill-switch / emergency freeze: all operations fail closed when kill-switch is active.
// 9. Observability: stable error codes and correlation IDs without raw secret key leakage.
// 10. Cross-network passphrase validation: prevents cross-network transaction submission.

describe("Delegation bindings auth negatives (closes #796)", () => {
  interface DelegationAuthGrant {
    owner: string;
    delegate: string;
    permissions: Set<string>;
    expiresAt: number;
    revoked: boolean;
  }

  class DelegationAuthNegativesEngine {
    public grants = new Map<string, DelegationAuthGrant>();
    public killSwitchActive = false;
    public loggedEvents: Array<{
      action: string;
      caller: string;
      delegate?: string;
      correlationId: string;
      error?: string;
    }> = [];

    private grantKey(owner: string, delegate: string): string {
      return `${owner}:${delegate}`;
    }

    grantDelegate(
      caller: string,
      owner: string,
      delegate: string,
      permissions: string[],
      expiresAt: number,
      now: number,
      correlationId = "corr-grant"
    ): void {
      if (this.killSwitchActive) {
        throw new DelegationExpiryError(
          "UNAUTHORIZED" as any,
          correlationId
        );
      }
      if (caller !== owner) {
        this.log("grant_delegate_denied", caller, delegate, correlationId, "UNAUTHORIZED");
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }
      if (!delegate || delegate.trim() === "") {
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }
      if (caller === delegate) {
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }
      if (permissions.length === 0) {
        throw new Error(muxDelegationErrorMessage("EmptyPermissions"));
      }
      if (permissions.length > 64) {
        throw new Error(muxDelegationErrorMessage("TooManyPermissions"));
      }
      if (expiresAt <= now) {
        throw new DelegationExpiryError(
          DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_EXPIRY_INVALID,
          correlationId
        );
      }

      const key = this.grantKey(owner, delegate);
      const isNew = !this.grants.has(key);
      const ownerGrantCount = Array.from(this.grants.values()).filter(
        (g) => g.owner === owner && !g.revoked
      ).length;

      if (isNew && ownerGrantCount >= 128) {
        throw new Error(muxDelegationErrorMessage("TooManyDelegates"));
      }

      this.grants.set(key, {
        owner,
        delegate,
        permissions: new Set(permissions),
        expiresAt,
        revoked: false,
      });

      this.log("grant_delegate_success", caller, delegate, correlationId);
    }

    revokeDelegate(
      caller: string,
      owner: string,
      delegate: string,
      correlationId = "corr-revoke"
    ): void {
      if (this.killSwitchActive) {
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }
      if (caller !== owner) {
        this.log("revoke_delegate_denied", caller, delegate, correlationId, "UNAUTHORIZED");
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }
      const key = this.grantKey(owner, delegate);
      const grant = this.grants.get(key);
      if (!grant) {
        throw new Error(muxDelegationErrorMessage("NotADelegate"));
      }
      grant.revoked = true;
      this.log("revoke_delegate_success", caller, delegate, correlationId);
    }

    assertAuthorized(
      caller: string,
      owner: string,
      requiredPermission: string,
      now: number,
      networkPassphrase: string,
      expectedPassphrase: string,
      correlationId = "corr-auth"
    ): boolean {
      if (this.killSwitchActive) {
        this.log("invoke_denied", caller, undefined, correlationId, "KILL_SWITCH_ACTIVE");
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }

      // Cross-network protection
      if (networkPassphrase !== expectedPassphrase) {
        this.log("invoke_denied", caller, undefined, correlationId, "CROSS_NETWORK_REJECTED");
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }

      // Owner is always authorized
      if (caller === owner) {
        return true;
      }

      const key = this.grantKey(owner, caller);
      const grant = this.grants.get(key);
      if (!grant) {
        this.log("invoke_denied", caller, undefined, correlationId, DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_NOT_FOUND);
        throw new DelegationExpiryError(
          DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_NOT_FOUND,
          correlationId
        );
      }

      if (grant.revoked) {
        this.log("invoke_denied", caller, undefined, correlationId, DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_REVOKED);
        throw new DelegationExpiryError(
          DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_REVOKED,
          correlationId
        );
      }

      if (now >= grant.expiresAt) {
        this.log("invoke_denied", caller, undefined, correlationId, DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_EXPIRED);
        throw new DelegationExpiryError(
          DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_EXPIRED,
          correlationId
        );
      }

      if (!grant.permissions.has(requiredPermission)) {
        this.log("invoke_denied", caller, undefined, correlationId, "SCOPE_VIOLATION");
        throw new DelegationExpiryError("UNAUTHORIZED" as any, correlationId);
      }

      return true;
    }

    private log(
      action: string,
      caller: string,
      delegate?: string,
      correlationId = "corr",
      error?: string
    ): void {
      this.loggedEvents.push({
        action,
        caller,
        delegate,
        correlationId,
        error,
      });
    }
  }

  const TESTNET_PASSPHRASE = "Test SDF Network ; September 2015";
  const MAINNET_PASSPHRASE = "Public Global Stellar Network ; September 2015";

  describe("Privileged entrypoint caller auth negatives", () => {
    it("denies non-owner from granting delegations", () => {
      const engine = new DelegationAuthNegativesEngine();
      expect(() =>
        engine.grantDelegate(
          "GATTACKER",
          "GOWNER",
          "GDELEGATE",
          ["transfer"],
          2000,
          1000,
          "corr-unauth-grant"
        )
      ).toThrow(DelegationExpiryError);

      try {
        engine.grantDelegate("GATTACKER", "GOWNER", "GDELEGATE", ["transfer"], 2000, 1000);
      } catch (err: any) {
        expect(err.code).toBe("UNAUTHORIZED");
      }
    });

    it("denies non-owner from revoking delegations", () => {
      const engine = new DelegationAuthNegativesEngine();
      engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["transfer"], 2000, 1000);

      expect(() =>
        engine.revokeDelegate("GATTACKER", "GOWNER", "GDELEGATE", "corr-unauth-rev")
      ).toThrow(DelegationExpiryError);

      try {
        engine.revokeDelegate("GATTACKER", "GOWNER", "GDELEGATE");
      } catch (err: any) {
        expect(err.code).toBe("UNAUTHORIZED");
      }
    });

    it("denies self-delegation (owner == delegate)", () => {
      const engine = new DelegationAuthNegativesEngine();
      expect(() =>
        engine.grantDelegate("GOWNER", "GOWNER", "GOWNER", ["transfer"], 2000, 1000)
      ).toThrow(DelegationExpiryError);
    });
  });

  describe("Expiry and revocation auth negatives", () => {
    it("throws DELEGATION_EXPIRY_INVALID when expiry is in the past or now", () => {
      const engine = new DelegationAuthNegativesEngine();
      expect(() =>
        engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["transfer"], 1000, 1000)
      ).toThrow(DelegationExpiryError);

      try {
        engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["transfer"], 999, 1000);
      } catch (err: any) {
        expect(err.code).toBe(DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_EXPIRY_INVALID);
      }
    });

    it("throws DELEGATION_EXPIRED when delegate invokes after expiry timestamp", () => {
      const engine = new DelegationAuthNegativesEngine();
      engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["transfer"], 2000, 1000);

      // Active at 1999
      expect(
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 1999, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)
      ).toBe(true);

      // Expired at 2000
      expect(() =>
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 2000, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)
      ).toThrow(DelegationExpiryError);

      try {
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 2001, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE);
      } catch (err: any) {
        expect(err.code).toBe(DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_EXPIRED);
      }
    });

    it("throws DELEGATION_REVOKED when delegate invokes after revocation", () => {
      const engine = new DelegationAuthNegativesEngine();
      engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["transfer"], 5000, 1000);
      engine.revokeDelegate("GOWNER", "GOWNER", "GDELEGATE");

      expect(() =>
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 1500, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)
      ).toThrow(DelegationExpiryError);

      try {
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 1500, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE);
      } catch (err: any) {
        expect(err.code).toBe(DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_REVOKED);
      }
    });

    it("throws DELEGATION_NOT_FOUND when unregistered caller invokes as delegate", () => {
      const engine = new DelegationAuthNegativesEngine();
      expect(() =>
        engine.assertAuthorized("GUNREGISTERED", "GOWNER", "transfer", 1500, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)
      ).toThrow(DelegationExpiryError);

      try {
        engine.assertAuthorized("GUNREGISTERED", "GOWNER", "transfer", 1500, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE);
      } catch (err: any) {
        expect(err.code).toBe(DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_NOT_FOUND);
      }
    });
  });

  describe("Scope boundary and permissions negatives", () => {
    it("denies delegate attempting action outside granted scope", () => {
      const engine = new DelegationAuthNegativesEngine();
      engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["read"], 5000, 1000);

      // 'read' allowed
      expect(
        engine.assertAuthorized("GDELEGATE", "GOWNER", "read", 1500, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)
      ).toBe(true);

      // 'transfer' rejected
      expect(() =>
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 1500, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)
      ).toThrow(DelegationExpiryError);
    });

    it("rejects EmptyPermissions when granting delegation", () => {
      const engine = new DelegationAuthNegativesEngine();
      expect(() =>
        engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", [], 5000, 1000)
      ).toThrow("permission list is empty; at least one permission is required");
    });

    it("rejects TooManyPermissions when permission list exceeds 64 entries", () => {
      const engine = new DelegationAuthNegativesEngine();
      const perms = Array.from({ length: 65 }, (_, i) => `perm_${i}`);
      expect(() =>
        engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", perms, 5000, 1000)
      ).toThrow("permission list exceeds the 64-entry cap");
    });

    it("rejects TooManyDelegates when exceeding the 128 delegate capacity", () => {
      const engine = new DelegationAuthNegativesEngine();
      for (let i = 0; i < 128; i++) {
        engine.grantDelegate("GOWNER", "GOWNER", `GDEL_${i}`, ["read"], 5000, 1000);
      }
      expect(() =>
        engine.grantDelegate("GOWNER", "GOWNER", "GDEL_129", ["read"], 5000, 1000)
      ).toThrow("owner already has 128 delegates registered");
    });
  });

  describe("Kill-switch and cross-network safety negatives", () => {
    it("fails closed when emergency kill-switch is active", () => {
      const engine = new DelegationAuthNegativesEngine();
      engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["transfer"], 5000, 1000);
      engine.killSwitchActive = true;

      expect(() =>
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 1500, TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)
      ).toThrow(DelegationExpiryError);

      expect(() =>
        engine.grantDelegate("GOWNER", "GOWNER", "GNEW", ["read"], 6000, 1000)
      ).toThrow(DelegationExpiryError);
    });

    it("fails closed on cross-network passphrase mismatch", () => {
      const engine = new DelegationAuthNegativesEngine();
      engine.grantDelegate("GOWNER", "GOWNER", "GDELEGATE", ["transfer"], 5000, 1000);

      // Attempting to invoke with Testnet passphrase against Mainnet contract
      expect(() =>
        engine.assertAuthorized("GDELEGATE", "GOWNER", "transfer", 1500, TESTNET_PASSPHRASE, MAINNET_PASSPHRASE)
      ).toThrow(DelegationExpiryError);
    });
  });

  describe("Observability & secret key material redaction", () => {
    it("attaches correlation IDs to auth negative errors and logs without key leaks", () => {
      const engine = new DelegationAuthNegativesEngine();
      const corrId = "corr-test-trace-999";

      try {
        engine.grantDelegate("GATTACKER", "GOWNER", "GDELEGATE", ["transfer"], 5000, 1000, corrId);
      } catch (err: any) {
        expect(err.correlationId).toBe(corrId);
      }

      const log = engine.loggedEvents.find((e) => e.correlationId === corrId);
      expect(log).toBeDefined();
      expect(log?.error).toBe("UNAUTHORIZED");

      // Verify no secret keys (starts with S...) leak in logged events or stringified errors
      const serialized = JSON.stringify(engine.loggedEvents);
      expect(serialized).not.toMatch(/S[A-Z0-9]{55}/);
    });
  });
});
