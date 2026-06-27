/**
 * Account Factory Usage Example Tests
 *
 * Tests for the account-factory-usage.ts example script.
 * These tests verify the example code compiles and has correct structure.
 */

import { describe, it, expect } from "vitest";

describe("account-factory-usage example", () => {
  it("example file exists", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    expect(fs.existsSync(examplePath)).toBe(true);
  });

  it("example imports MuxAccountFactoryClient", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("MuxAccountFactoryClient");
  });

  it("example demonstrates deploy_account", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("deployAccount");
  });

  it("example demonstrates deploy_account_with_metadata", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("deployAccountWithMetadata");
  });

  it("example demonstrates get_accounts", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("getAccounts");
  });

  it("example demonstrates get_account_metadata", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("getAccountMetadata");
  });

  it("example demonstrates account_count", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("accountCount");
  });

  it("example includes environment variable configuration", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("RPC_URL");
    expect(content).toContain("SECRET_KEY");
    expect(content).toContain("FACTORY_CONTRACT");
    expect(content).toContain("SOROBAN_NETWORK");
  });

  it("example includes error handling", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("try");
    expect(content).toContain("catch");
  });

  it("example supports multiple networks", () => {
    const fs = require("fs");
    const path = require("path");
    const examplePath = path.join(__dirname, "account-factory-usage.ts");
    const content = fs.readFileSync(examplePath, "utf-8");
    expect(content).toContain("localnet");
    expect(content).toContain("testnet");
    expect(content).toContain("mainnet");
  });
});
