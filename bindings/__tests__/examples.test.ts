/**
 * Smoke-tests for the frontend-usage example module.
 *
 * These tests verify that all exported helpers are importable and have the
 * expected shape without connecting to a live network.
 */

import * as frontendUsage from "../src/examples/frontend-usage";

describe("frontend-usage example module", () => {
  it("exports bootstrapNetwork as a function", () => {
    expect(typeof frontendUsage.bootstrapNetwork).toBe("function");
  });

  it("exports fetchAccountOwner as a function", () => {
    expect(typeof frontendUsage.fetchAccountOwner).toBe("function");
  });

  it("exports grantDelegate as a function", () => {
    expect(typeof frontendUsage.grantDelegate).toBe("function");
  });

  it("exports checkPermission as a function", () => {
    expect(typeof frontendUsage.checkPermission).toBe("function");
  });

  it("exports executeBatch as a function", () => {
    expect(typeof frontendUsage.executeBatch).toBe("function");
  });

  it("exports REACT_HOOK_EXAMPLE as a string", () => {
    expect(typeof frontendUsage.REACT_HOOK_EXAMPLE).toBe("string");
  });

  it("bootstrapNetwork throws when addresses are not configured", () => {
    // Default addresses are empty strings; validation should reject them.
    const originalNetwork = process.env.SOROBAN_NETWORK;
    process.env.SOROBAN_NETWORK = "testnet";
    expect(() => frontendUsage.bootstrapNetwork()).toThrow();
    process.env.SOROBAN_NETWORK = originalNetwork;
  });
});
