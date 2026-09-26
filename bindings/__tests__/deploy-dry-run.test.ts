/**
 * Unit tests for deploy.sh --dry-run flag (#114)
 *
 * Verifies that dry-run mode logs expected messages and never invokes
 * `stellar contract upload` or `stellar contract deploy` (i.e. no on-chain
 * transactions are submitted).
 *
 * Also verifies the shape and behaviour of the TypeScript-level dry-run API
 * on MuxAccountFactoryClient (simulateDeploy / simulateDeployWithMetadata).
 *
 * Additionally covers the immutable mainnet flag (#809): mainnet-affecting
 * deploys are deny-by-default and require an explicit opt-in, and testnet vs
 * mainnet misconfiguration fails closed with a stable, actionable error.
 */

import { execSync } from "child_process";
import * as path from "path";
import * as fs from "fs";
import { MuxAccountFactoryClient } from "../src/generated/mux-account-factory";

const REPO_ROOT = path.resolve(__dirname, "../..");
const DEPLOY_SCRIPT = path.join(REPO_ROOT, "scripts", "deploy.sh");

/**
 * Run deploy.sh capturing stdout+stderr and the exit status without throwing,
 * so negative (fail-closed) cases can be asserted.
 */
function runDeploy(
  args: string,
  env: NodeJS.ProcessEnv = {},
): { status: number; output: string } {
  try {
    const output = execSync(`bash "${DEPLOY_SCRIPT}" ${args}`, {
      encoding: "utf8",
      env: { ...process.env, ...env },
      stdio: ["ignore", "pipe", "pipe"],
    });
    return { status: 0, output };
  } catch (err: any) {
    const stdout = err.stdout ? err.stdout.toString() : "";
    const stderr = err.stderr ? err.stderr.toString() : "";
    return { status: err.status ?? 1, output: `${stdout}${stderr}` };
  }
}

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

  it("--dry-run shows mux-account init params with --owner and --guardians", () => {
    const output = execSync(
      `bash "${DEPLOY_SCRIPT}" --dry-run --network testnet --skip-build --contract mux-account`,
      { encoding: "utf8", env: { ...process.env } },
    );

    expect(output).toMatch(/--owner/);
    expect(output).toMatch(/--guardians/);
    expect(output).toMatch(/mux-account/);
    expect(output).toMatch(/stellar contract invoke/);
  });
});

describe("deploy.sh immutable mainnet flag (#809)", () => {
  it("aborts a mainnet deploy when the immutable flag is not set (deny-by-default)", () => {
    const env = { ...process.env };
    delete env.MUX_ALLOW_MAINNET_DEPLOY;

    const { status, output } = runDeploy(
      "--network mainnet --skip-build --contract mux-account",
      env,
    );

    expect(status).not.toBe(0);
    expect(output).toMatch(/MUX_MAINNET_FLAG_REQUIRED/);
  });

  it("aborts a mainnet deploy when the immutable flag is set to a non-affirmative value", () => {
    const { status, output } = runDeploy(
      "--network mainnet --skip-build --contract mux-account",
      { MUX_ALLOW_MAINNET_DEPLOY: "false" },
    );

    expect(status).not.toBe(0);
    expect(output).toMatch(/MUX_MAINNET_FLAG_REQUIRED/);
  });

  it("does not require the immutable flag for testnet deploys", () => {
    const env = { ...process.env };
    delete env.MUX_ALLOW_MAINNET_DEPLOY;

    const { status, output } = runDeploy(
      "--dry-run --network testnet --skip-build --contract mux-account",
      env,
    );

    expect(status).toBe(0);
    expect(output).not.toMatch(/MUX_MAINNET_FLAG_REQUIRED/);
  });

  it("fails closed on an unknown network rather than defaulting to mainnet", () => {
    const { status, output } = runDeploy(
      "--dry-run --network mainnnet --skip-build --contract mux-account",
      { MUX_ALLOW_MAINNET_DEPLOY: "true" },
    );

    expect(status).not.toBe(0);
    expect(output).toMatch(/MUX_NETWORK_INVALID/);
  });

  it("the immutable flag cannot be silently overridden by a CLI argument", () => {
    const env = { ...process.env };
    delete env.MUX_ALLOW_MAINNET_DEPLOY;

    const { status, output } = runDeploy(
      "--network mainnet --skip-build --contract mux-account --allow-mainnet",
      env,
    );

    expect(status).not.toBe(0);
    expect(output).toMatch(/MUX_MAINNET_FLAG_REQUIRED/);
  });
});

describe("MuxAccountFactoryClient — dry-run API shape", () => {
  it("exposes simulateDeploy as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.simulateDeploy).toBe("function");
  });

  it("exposes simulateDeployWithMetadata as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.simulateDeployWithMetadata).toBe("function");
  });

  it("simulateDeploy is distinct from deployAccount", () => {
    // Ensures the dry-run path is a separate method that never submits
    expect(MuxAccountFactoryClient.prototype.simulateDeploy).not.toBe(
      MuxAccountFactoryClient.prototype.deployAccount
    );
  });

  it("simulateDeployWithMetadata is distinct from deployAccountWithMetadata", () => {
    expect(MuxAccountFactoryClient.prototype.simulateDeployWithMetadata).not.toBe(
      MuxAccountFactoryClient.prototype.deployAccountWithMetadata
    );
  });
});
