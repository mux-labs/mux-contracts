/**
 * Smoke-tests for the frontend-usage example module.
 *
 * These tests verify that all exported helpers are importable and have the
 * expected shape without connecting to a live network.
 */

import * as fs from "fs";
import * as path from "path";
import { spawnSync } from "child_process";
import { Keypair } from "@stellar/stellar-sdk";
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

describe("backend-invoke example address resolution", () => {
  const bindingsDir = path.resolve(__dirname, "..");
  const esbuildBin = path.join(bindingsDir, "node_modules", ".bin", "esbuild");

  const created: string[] = [];

  afterEach(() => {
    while (created.length > 0) {
      fs.rmSync(created.pop()!, { recursive: true, force: true });
    }
  });

  /** Stage the example + a config/addresses.json under bindings/ so module
   *  resolution reaches bindings/node_modules, transpile with esbuild, and
   *  run the emitted JS with node. */
  function runExample(
    addresses: Record<string, unknown>,
    env: Record<string, string>
  ) {
    const tmp = fs.mkdtempSync(
      path.join(bindingsDir, ".tmp-backend-invoke-")
    );
    created.push(tmp);
    fs.mkdirSync(path.join(tmp, "examples"), { recursive: true });
    fs.mkdirSync(path.join(tmp, "config"), { recursive: true });
    fs.copyFileSync(
      path.join(bindingsDir, "..", "examples", "backend-invoke.ts"),
      path.join(tmp, "examples", "backend-invoke.ts")
    );
    fs.writeFileSync(
      path.join(tmp, "config", "addresses.json"),
      JSON.stringify(addresses, null, 2)
    );

    const jsPath = path.join(tmp, "examples", "backend-invoke.cjs");
    const build = spawnSync(
      esbuildBin,
      [path.join(tmp, "examples", "backend-invoke.ts"), "--format=cjs", "--platform=node", `--outfile=${jsPath}`],
      { cwd: bindingsDir, encoding: "utf-8", env: { PATH: process.env.PATH } }
    );
    expect(build.stderr || "").not.toContain("ERROR");

    return spawnSync(process.execPath, [jsPath], {
      cwd: bindingsDir,
      encoding: "utf-8",
      env: {
        PATH: process.env.PATH,
        HOME: process.env.HOME,
        ...env,
      },
    });
  }

  const localnet = { muxAccount: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA7I" };
  const validSecret = Keypair.random().secret();

  it("exits non-zero and names network + muxAccount when the key is missing", () => {
    const result = runExample(
      { localnet, testnet: {} },
      { SOROBAN_NETWORK: "testnet", SERVER_SECRET_KEY: validSecret }
    );
    expect(result.status).not.toBe(0);
    const output = `${result.stdout}${result.stderr}`;
    expect(output).toContain("testnet.muxAccount");
    expect(output).toContain("is missing");
  });

  it("exits non-zero for an empty-string muxAccount value", () => {
    const result = runExample(
      { localnet, testnet: { muxAccount: "" } },
      { SOROBAN_NETWORK: "testnet", SERVER_SECRET_KEY: validSecret }
    );
    expect(result.status).not.toBe(0);
    const output = `${result.stdout}${result.stderr}`;
    expect(output).toContain("testnet.muxAccount");
    expect(output).toContain("must be a non-empty string");
  });

  it("exits non-zero and names the network when the network entry is absent", () => {
    const result = runExample(
      { localnet },
      { SOROBAN_NETWORK: "testnet", SERVER_SECRET_KEY: validSecret }
    );
    expect(result.status).not.toBe(0);
    const output = `${result.stdout}${result.stderr}`;
    expect(output).toContain('"testnet"');
  });

  it("CONTRACT_ADDRESS env override still takes precedence over the config", () => {
    const address = "CTESTCONTRACTADDRESSOVERRIDE";
    const result = runExample(
      { localnet },
      {
        SOROBAN_NETWORK: "testnet",
        SERVER_SECRET_KEY: validSecret,
        CONTRACT_ADDRESS: address,
      }
    );
    const output = `${result.stdout}${result.stderr}`;
    // Resolution succeeded: the config-loaded banner shows the env address
    // and none of the config validation errors fired.
    expect(result.stdout).toContain("Configuration loaded");
    expect(result.stdout).toContain(address);
    expect(output).not.toContain("muxAccount");
    expect(output).not.toContain("config/addresses.json entry");
  });
});
