import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const directory = dirname(fileURLToPath(import.meta.url));
const source = readFileSync(join(directory, "authorize-flow.ts"), "utf8");

describe("authorize-flow example", () => {
  it("covers registration, direct execution, sponsorship, and revocation", () => {
    expect(source).toContain("registerSessionKey");
    expect(source).toContain("executeWithSession");
    expect(source).toContain("executeSponsored");
    expect(source).toContain("revokeSessionKey");
  });

  it("fails closed when configuration or revoked authorization is invalid", () => {
    expect(source).toContain("refusing to run without explicit configuration");
    expect(source).toContain("revoked session unexpectedly executed");
    expect(source).toContain("process.exitCode = 1");
  });
});
