/**
 * Unit tests for MuxDelegationClient binding shape and error mapping.
 */

import { MuxDelegationClient } from "../src/generated/mux-delegation";
import { ERROR_HTTP_MAP } from "../src/errors";

describe("MuxDelegationClient shape", () => {
  it("exposes grantDelegate as a function", () => {
    expect(typeof MuxDelegationClient.prototype.grantDelegate).toBe("function");
  });

  it("exposes revokeDelegate as a function", () => {
    expect(typeof MuxDelegationClient.prototype.revokeDelegate).toBe("function");
  });

  it("exposes getDelegatePermissions as a function", () => {
    expect(typeof MuxDelegationClient.prototype.getDelegatePermissions).toBe("function");
  });

  it("exposes isDelegate as a function", () => {
    expect(typeof MuxDelegationClient.prototype.isDelegate).toBe("function");
  });

  it("exposes getDelegates as a function", () => {
    expect(typeof MuxDelegationClient.prototype.getDelegates).toBe("function");
  });
});

describe("Delegation error HTTP mapping", () => {
  it("maps NotADelegate to 404", () => {
    expect(ERROR_HTTP_MAP.NotADelegate).toBe(404);
  });

  it("maps TooManyPermissions to 400", () => {
    expect(ERROR_HTTP_MAP.TooManyPermissions).toBe(400);
  });

  it("maps EmptyPermissions to 400", () => {
    expect(ERROR_HTTP_MAP.EmptyPermissions).toBe(400);
  });
});
