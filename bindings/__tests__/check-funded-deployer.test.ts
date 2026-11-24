import { describe, it, expect } from "vitest";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const SCRIPT = resolve(__dirname, "../scripts/check-funded-deployer.mjs");

interface RunResult {
  status: number;
  stdout: string;
  stderr: string;
  output: string;
}

function runScript(
  args: string[],
  env: Record<string, string | undefined>,
): RunResult {
  const result = spawnSync(process.execPath, [SCRIPT, ...args], {
    env: {
      PATH: process.env.PATH,
      NODE_ENV: "test",
      ...env,
    },
    encoding: "utf-8",
    timeout: 10_000,
  });
  return {
    status: result.status ?? -1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    output: (result.stdout ?? "") + (result.stderr ?? ""),
  };
}

describe("check-funded-deployer.mjs", () => {
  describe("dry-run mode", () => {
    it("exits 0 and prints the dry-run banner without DEPLOYER_PRIVATE_KEY", () => {
      const result = runScript(["--dry-run"], {
        DEPLOYER_PRIVATE_KEY: undefined,
        HORIZON_URL: "https://horizon-testnet.stellar.org",
        MIN_DEPLOYER_BALANCE_XLM: "2",
      });

      expect(result.status).toBe(0);
      expect(result.output).toContain("dry run");
      expect(result.output).toContain("horizon-testnet.stellar.org");
    });

    it("prints the minimum balance in dry-run mode", () => {
      const result = runScript(["--dry-run"], {
        DEPLOYER_PRIVATE_KEY: undefined,
        MIN_DEPLOYER_BALANCE_XLM: "5",
      });

      expect(result.status).toBe(0);
      expect(result.output).toContain("minimum native balance 5 XLM");
    });
  });

  describe("invalid MIN_DEPLOYER_BALANCE_XLM", () => {
    it("exits non-zero for a negative balance", () => {
      const result = runScript(["--dry-run"], {
        MIN_DEPLOYER_BALANCE_XLM: "-1",
        DEPLOYER_PRIVATE_KEY: undefined,
      });

      expect(result.status).not.toBe(0);
      expect(result.output).toContain("non-negative");
    });

    it("exits non-zero for a non-numeric balance", () => {
      const result = runScript(["--dry-run"], {
        MIN_DEPLOYER_BALANCE_XLM: "not-a-number",
        DEPLOYER_PRIVATE_KEY: undefined,
      });

      expect(result.status).not.toBe(0);
      expect(result.output).toContain("non-negative");
    });
  });

  describe("missing DEPLOYER_PRIVATE_KEY outside dry-run", () => {
    it("exits non-zero with the required-outside-dry-run message", () => {
      const result = runScript([], {
        DEPLOYER_PRIVATE_KEY: undefined,
        HORIZON_URL: "https://horizon-testnet.stellar.org",
        MIN_DEPLOYER_BALANCE_XLM: "2",
      });

      expect(result.status).not.toBe(0);
      expect(result.output).toContain("required outside --dry-run mode");
    });

    it("does not set or require a real DEPLOYER_PRIVATE_KEY value", () => {
      // Explicitly ensure no real secret is present.
      const result = runScript([], {
        DEPLOYER_PRIVATE_KEY: undefined,
      });

      expect(result.status).not.toBe(0);
      expect(result.output).not.toMatch(/SC[A-Z0-9]{55}/);
    });
  });
});
