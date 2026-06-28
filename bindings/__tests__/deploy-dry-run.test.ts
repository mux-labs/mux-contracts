/**
 * Unit tests for deploy.sh --dry-run flag (#114)
 *
 * Verifies that dry-run mode logs expected messages and never invokes
 * `stellar contract upload` or `stellar contract deploy` (i.e. no on-chain
 * transactions are submitted).
 */

import { execSync } from "child_process";
import * as path from "path";
import * as fs from "fs";

const REPO_ROOT = path.resolve(__dirname, "../..");
const DEPLOY_SCRIPT = path.join(REPO_ROOT, "scripts", "deploy.sh");

describe("deploy.sh --dry-run flag", () => {
  it("deploy script exists and is executable", () => {
    expect(fs.existsSync(DEPLOY_SCRIPT)).toBe(true);
    const stat = fs.statSync(DEPLOY_SCRIPT);
    // Owner execute bit set
    expect(stat.mode & 0o100).toBeTruthy();
  });

  it("--dry-run flag produces [DRY-RUN] prefix on simulated steps", () => {
    const output = execSync(
      `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build`,
      { encoding: "utf8", env: { ...process.env } },
    );

    expect(output).toMatch(/\[DRY-RUN\]/);
  });

  it("--dry-run does not invoke stellar contract upload", () => {
    const output = execSync(
      `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build`,
      { encoding: "utf8", env: { ...process.env } },
    );

    expect(output).not.toMatch(/^stellar contract upload/m);
    expect(output).toMatch(/stellar contract upload/i);
  });

  it("--dry-run does not invoke stellar contract deploy", () => {
    const output = execSync(
      `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build`,
      { encoding: "utf8", env: { ...process.env } },
    );

    expect(output).not.toMatch(/^stellar contract deploy/m);
    expect(output).toMatch(/stellar contract deploy/i);
  });

  it("--dry-run exits 0 even without DEPLOYER_SECRET_KEY set", () => {
    // Unset DEPLOYER_SECRET_KEY — live mode would fail; dry-run must not
    const env = { ...process.env };
    delete env.DEPLOYER_SECRET_KEY;

    expect(() =>
      execSync(
        `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build`,
        { encoding: "utf8", env },
      ),
    ).not.toThrow();
  });

  it("--dry-run prints dry-run summary at the end", () => {
    const output = execSync(
      `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build`,
      { encoding: "utf8", env: { ...process.env } },
    );

    expect(output).toMatch(/no on-chain transactions were submitted/i);
    expect(output).toMatch(/Dry-run complete/i);
  });

  it("--dry-run respects --contract flag and only simulates the specified contract", () => {
    const output = execSync(
      `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build --contract mux-account`,
      { encoding: "utf8", env: { ...process.env } },
    );

    expect(output).toMatch(/mux-account/);
    // Other contracts should not appear in the output
    expect(output).not.toMatch(/mux-batcher/);
    expect(output).not.toMatch(/mux-permissions/);
  });

  it("--dry-run simulates mux-wallet-registry when requested", () => {
    const output = execSync(
      `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build --contract mux-wallet-registry`,
      { encoding: "utf8", env: { ...process.env } },
    );

    expect(output).toMatch(/mux-wallet-registry/);
    expect(output).toMatch(/stellar contract upload/i);
    expect(output).toMatch(/stellar contract deploy/i);
  });
});
