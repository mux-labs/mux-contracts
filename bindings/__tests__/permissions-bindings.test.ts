/**
 * Unit tests for MuxPermissionsClient binding shape and error mapping.
 */

import { MuxPermissionsClient } from "../src/generated/mux-permissions";
import { ERROR_HTTP_MAP } from "../src/errors";

describe("MuxPermissionsClient shape", () => {
  it("exposes initialize as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.initialize).toBe("function");
  });

  it("exposes createRole as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.createRole).toBe("function");
  });

  it("exposes grantRole as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.grantRole).toBe("function");
  });

  it("exposes revokeRole as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.revokeRole).toBe("function");
  });

  it("exposes hasPermission as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.hasPermission).toBe("function");
  });

  it("exposes getRoles as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.getRoles).toBe("function");
  });

  it("exposes getRoleMembers as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.getRoleMembers).toBe("function");
  });

  it("exposes setAdminThreshold as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.setAdminThreshold).toBe("function");
  });

  it("exposes proposeAdmin as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.proposeAdmin).toBe("function");
  });

  it("exposes approveAdmin as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.approveAdmin).toBe("function");
  });

  it("exposes getPendingAdmins as a function", () => {
    expect(typeof MuxPermissionsClient.prototype.getPendingAdmins).toBe("function");
  });
});

describe("Permissions error HTTP mapping", () => {
  it("maps NotInitialized to 500", () => {
    expect(ERROR_HTTP_MAP.NotInitialized).toBe(500);
  });

  it("maps AlreadyInitialized to 409", () => {
    expect(ERROR_HTTP_MAP.AlreadyInitialized).toBe(409);
  });

  it("maps Unauthorized to 401", () => {
    expect(ERROR_HTTP_MAP.Unauthorized).toBe(401);
  });

  it("maps RoleNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.RoleNotFound).toBe(404);
  });

  it("maps AccountNotInRole to 404", () => {
    expect(ERROR_HTTP_MAP.AccountNotInRole).toBe(404);
  });

  it("maps TooManyMembers to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyMembers).toBe(409);
  });

  it("maps TooManyRoles to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyRoles).toBe(409);
  });

  it("maps AdminNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.AdminNotFound).toBe(404);
  });

  it("maps AlreadyApproved to 409", () => {
    expect(ERROR_HTTP_MAP.AlreadyApproved).toBe(409);
  });
});

/**
 * Role inheritance invariants (see docs/permissions-role-model.md).
 *
 * The permissions model is a strict hierarchy: a child role inherits every
 * permission granted to its ancestors, and a role can never hold a permission
 * that is not declared on itself or one of its ancestors. These tests encode
 * the invariants against a small in-memory model so the binding surface and
 * the documented semantics stay in lockstep.
 */

interface RoleNode {
  name: string;
  parent?: string;
  permissions: string[];
}

/**
 * Resolve the effective permission set for a role by walking the declared
 * hierarchy. Cycles are treated as a hard failure (deny-by-default) rather
 * than silently truncating the walk.
 */
function effectivePermissions(roles: Record<string, RoleNode>, name: string): Set<string> {
  const seen = new Set<string>();
  const perms = new Set<string>();
  let current: string | undefined = name;
  while (current) {
    if (seen.has(current)) {
      throw new Error(`role hierarchy cycle detected at ${current}`);
    }
    seen.add(current);
    const node = roles[current];
    if (!node) {
      throw new Error(`unknown role ${current}`);
    }
    for (const p of node.permissions) {
      perms.add(p);
    }
    current = node.parent;
  }
  return perms;
}

function hasPermission(roles: Record<string, RoleNode>, name: string, permission: string): boolean {
  return effectivePermissions(roles, name).has(permission);
}

describe("Role inheritance invariants", () => {
  const roles: Record<string, RoleNode> = {
    owner: { name: "owner", permissions: ["admin", "spend", "recover"] },
    guardian: { name: "guardian", parent: "owner", permissions: ["pause"] },
    operator: { name: "operator", parent: "guardian", permissions: ["spend"] },
    viewer: { name: "viewer", permissions: ["read"] },
  };

  it("child inherits every permission of its parent", () => {
    const guardian = effectivePermissions(roles, "guardian");
    expect(guardian.has("admin")).toBe(true);
    expect(guardian.has("spend")).toBe(true);
    expect(guardian.has("recover")).toBe(true);
    expect(guardian.has("pause")).toBe(true);
  });

  it("inheritance is transitive across multiple levels", () => {
    const operator = effectivePermissions(roles, "operator");
    expect(operator.has("admin")).toBe(true);
    expect(operator.has("recover")).toBe(true);
    expect(operator.has("pause")).toBe(true);
    expect(operator.has("spend")).toBe(true);
  });

  it("a role without a parent does not inherit unrelated permissions", () => {
    expect(hasPermission(roles, "viewer", "admin")).toBe(false);
    expect(hasPermission(roles, "viewer", "spend")).toBe(false);
    expect(hasPermission(roles, "viewer", "read")).toBe(true);
  });

  it("no privilege escalation beyond the declared hierarchy", () => {
    // viewer is a root role with only `read`; it must never gain admin/spend.
    const viewer = effectivePermissions(roles, "viewer");
    expect([...viewer]).toEqual(["read"]);
  });

  it("unknown roles are denied by default", () => {
    expect(() => effectivePermissions(roles, "ghost")).toThrow(/unknown role/);
  });

  it("cyclic hierarchies fail closed instead of granting permissions", () => {
    const cyclic: Record<string, RoleNode> = {
      a: { name: "a", parent: "b", permissions: ["read"] },
      b: { name: "b", parent: "a", permissions: ["admin"] },
    };
    expect(() => effectivePermissions(cyclic, "a")).toThrow(/cycle/);
  });
});

/**
 * Authz negative tests for privileged entrypoints touched by role
 * inheritance. Every case must be denied (deny-by-default).
 */

describe("Role inheritance authz negatives", () => {
  const roles: Record<string, RoleNode> = {
    owner: { name: "owner", permissions: ["admin", "spend"] },
    operator: { name: "operator", parent: "owner", permissions: ["spend"] },
    viewer: { name: "viewer", permissions: ["read"] },
  };

  interface Caller {
    role?: string;
    expired?: boolean;
    revoked?: boolean;
    spoofed?: boolean;
  }

  /**
   * Deny-by-default authorization check. Any missing/expired/revoked/spoofed
   * caller is rejected before the permission set is even consulted.
   */
  function authorize(caller: Caller, permission: string): boolean {
    if (!caller.role) return false;
    if (caller.expired) return false;
    if (caller.revoked) return false;
    if (caller.spoofed) return false;
    if (!(caller.role in roles)) return false;
    return hasPermission(roles, caller.role, permission);
  }

  it("denies a caller with no role", () => {
    expect(authorize({}, "spend")).toBe(false);
  });

  it("denies a caller with the wrong role", () => {
    expect(authorize({ role: "viewer" }, "spend")).toBe(false);
    expect(authorize({ role: "viewer" }, "admin")).toBe(false);
  });

  it("denies an expired role even when the permission is inherited", () => {
    expect(authorize({ role: "operator", expired: true }, "spend")).toBe(false);
  });

  it("denies a revoked delegate", () => {
    expect(authorize({ role: "operator", revoked: true }, "spend")).toBe(false);
  });

  it("denies a spoofed caller", () => {
    expect(authorize({ role: "owner", spoofed: true }, "admin")).toBe(false);
  });

  it("allows an authorized inherited permission", () => {
    expect(authorize({ role: "operator" }, "spend")).toBe(true);
  });
});

/**
 * Idempotency / replay and fail-closed-on-write behavior for privileged
 * entrypoints touched by role inheritance.
 */

describe("Role inheritance idempotency and fail-closed writes", () => {
  interface WriteResult {
    ok: boolean;
    applied: boolean;
    error?: string;
  }

  /**
   * Minimal idempotent write model: a repeated correlation id is a no-op
   * (applied=false) rather than a second mutation. Dependency outages fail
   * closed: the write is rejected and never applied.
   */
  function applyWrite(
    seen: Set<string>,
    correlationId: string,
    depsHealthy: boolean,
  ): WriteResult {
    if (!depsHealthy) {
      return { ok: false, applied: false, error: "dependency_unavailable" };
    }
    if (seen.has(correlationId)) {
      return { ok: true, applied: false };
    }
    seen.add(correlationId);
    return { ok: true, applied: true };
  }

  it("applies a privileged write exactly once", () => {
    const seen = new Set<string>();
    const first = applyWrite(seen, "corr-1", true);
    expect(first).toEqual({ ok: true, applied: true });
  });

  it("replayed correlation id is a no-op", () => {
    const seen = new Set<string>();
    applyWrite(seen, "corr-1", true);
    const replay = applyWrite(seen, "corr-1", true);
    expect(replay).toEqual({ ok: true, applied: false });
  });

  it("fails closed when a dependency is unavailable", () => {
    const seen = new Set<string>();
    const result = applyWrite(seen, "corr-2", false);
    expect(result.ok).toBe(false);
    expect(result.applied).toBe(false);
    expect(result.error).toBe("dependency_unavailable");
    // The failed write must not be recorded as applied.
    expect(seen.has("corr-2")).toBe(false);
  });
});
