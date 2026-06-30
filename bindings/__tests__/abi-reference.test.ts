/**
 * Verifies that docs/abi_reference.md documents the mux-account-factory
 * public interface (#221).
 */

import * as fs from "fs";
import * as path from "path";

const ABI_DOC = path.resolve(__dirname, "../../docs/abi_reference.md");
const content = fs.readFileSync(ABI_DOC, "utf-8");

describe("docs/abi_reference.md — mux-account-factory", () => {
  it("has a mux-account-factory section", () => {
    expect(content).toMatch(/##\s+mux-account-factory/);
  });

  it("documents deploy_account", () => {
    expect(content).toContain("deploy_account");
  });

  it("documents deploy_account_with_metadata", () => {
    expect(content).toContain("deploy_account_with_metadata");
  });

  it("documents get_accounts", () => {
    expect(content).toContain("get_accounts");
  });

  it("documents get_account_metadata", () => {
    expect(content).toContain("get_account_metadata");
  });

  it("documents account_count", () => {
    expect(content).toContain("account_count");
  });

  it("documents AccountMetadata type", () => {
    expect(content).toContain("AccountMetadata");
  });

  it("documents all four error variants", () => {
    expect(content).toContain("Unauthorized");
    expect(content).toContain("InvalidAccount");
    expect(content).toContain("TooManyAccounts");
    expect(content).toContain("MetadataNotFound");
  });

  it("documents the deployed event", () => {
    expect(content).toContain("deployed");
  });

  it("documents MAX_ACCOUNTS_PER_OWNER cap", () => {
    expect(content).toContain("MAX_ACCOUNTS_PER_OWNER");
  });
});
