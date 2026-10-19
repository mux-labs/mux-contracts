/**
 * Smoke-tests for the frontend-usage example module.
 *
 * These tests verify that all exported helpers are importable and have the
 * expected shape without connecting to a live network.
 */

import * as fs from "fs";
import * as path from "path";
import * as accountExamples from "../src/examples/account-invoke";
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

  it("bootstrapNetwork throws when mainnet addresses are not review-approved", () => {
    // Mainnet resolution is fail-closed: without the review marker,
    // bootstrapNetwork must throw instead of returning unreviewed addresses.
    const originalNetwork = process.env.SOROBAN_NETWORK;
    const originalReview = process.env.MAINNET_ADDRESS_REVIEW_APPROVED;
    process.env.SOROBAN_NETWORK = "mainnet";
    delete process.env.MAINNET_ADDRESS_REVIEW_APPROVED;
    expect(() => frontendUsage.bootstrapNetwork()).toThrow(
      /Mainnet address change requires review approval/
    );
    process.env.SOROBAN_NETWORK = originalNetwork;
    if (originalReview === undefined) {
      delete process.env.MAINNET_ADDRESS_REVIEW_APPROVED;
    } else {
      process.env.MAINNET_ADDRESS_REVIEW_APPROVED = originalReview;
    }
  });

  it("includes the wallet registry invoke example file", () => {
    const repoRoot = path.resolve(__dirname, "..", "..");
    const examplePath = path.join(repoRoot, "examples", "wallet-registry-invoke.ts");
    expect(fs.existsSync(examplePath)).toBe(true);
  });
});

describe("account-invoke example module", () => {
  it("exports all expected functions from account-invoke", () => {
    expect(typeof accountExamples.initializeAccount).toBe("function");
    expect(typeof accountExamples.getOwner).toBe("function");
    expect(typeof accountExamples.grantDelegate).toBe("function");
    expect(typeof accountExamples.revokeDelegate).toBe("function");
    expect(typeof accountExamples.setSpendLimit).toBe("function");
  });
});
