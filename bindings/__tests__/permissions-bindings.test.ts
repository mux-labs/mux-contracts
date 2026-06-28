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
