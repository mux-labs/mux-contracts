/**
 * Batcher Invoke Example Tests
 *
 * Verifies the batcher-invoke example validates contract IDs before building
 * the client, failing fast on malformed values instead of surfacing a raw SDK
 * stack trace.
 */

import { describe, it, expect } from "vitest";
import * as fs from "fs";
import * as path from "path";

describe("batcher-invoke example", () => {
  const examplePath = path.join(__dirname, "batcher-invoke.ts");
  const content = fs.readFileSync(examplePath, "utf8");

  it("example file exists", () => {
    expect(fs.existsSync(examplePath)).toBe(true);
  });

  it("example imports MuxBatcherClient", () => {
    expect(content).toContain("MuxBatcherClient");
  });

  it("example validates BATCHER_CONTRACT_ID presence", () => {
    expect(content).toContain("BATCHER_CONTRACT_ID is required");
  });

  it("example validates TARGET_CONTRACT_ID presence", () => {
    expect(content).toContain("TARGET_CONTRACT_ID is required");
  });

  it("example defines assertContractId validation helper", () => {
    expect(content).toContain("assertContractId");
    expect(content).toContain("Address.fromString");
    expect(content).toContain("process.exit(1)");
  });

  it("example validates contract IDs are contract StrKey (C...)", () => {
    expect(content).toContain("is not a valid contract address");
    expect(content).toContain("BATCHER_CONTRACT_ID");
    expect(content).toContain("TARGET_CONTRACT_ID");
  });

  it("example keeps network passphrase validation untouched", () => {
    expect(content).toContain("Unknown network");
    expect(content).toContain("PASSPHRASES");
  });
});
