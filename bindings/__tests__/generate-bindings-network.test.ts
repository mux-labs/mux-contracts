import { spawnSync } from "child_process";
import * as path from "path";

const REPO_ROOT = path.resolve(__dirname, "../..");
const BINDINGS_DIR = path.join(REPO_ROOT, "bindings");
const SCRIPT_PATH = path.join(REPO_ROOT, "scripts", "generate-bindings.ts");

function runGenerator(...args: string[]) {
  return spawnSync(
    process.execPath,
    ["-r", "ts-node/register", SCRIPT_PATH, ...args],
    {
      cwd: BINDINGS_DIR,
      encoding: "utf8",
    },
  );
}

describe("generate-bindings network validation", () => {
  it.each(["testnet", "mainnet", "localnet"])(
    "accepts %s",
    (network) => {
      const result = runGenerator(
        "--network",
        network,
        "--contract",
        "mux-account",
        "--skip-build",
        "--dry-run",
      );

      expect(result.status).toBe(0);
      expect(`${result.stdout}${result.stderr}`).toContain(`Network:    ${network}`);
    },
  );

  it("rejects an unknown network before generation starts", () => {
    const result = runGenerator(
      "--network",
      "bogus",
      "--contract",
      "mux-account",
      "--dry-run",
    );
    const output = `${result.stdout}${result.stderr}`;

    expect(result.status).not.toBe(0);
    expect(output).toContain("Unknown network: bogus");
    expect(output).toContain("Valid: testnet, mainnet, localnet");
  });
});
