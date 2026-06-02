#!/usr/bin/env npx ts-node
/**
 * scripts/generate-bindings.ts
 *
 * TypeScript bindings generation script for Mux Protocol Soroban contracts.
 * Reads compiled WASM artifacts and invokes `stellar contract bindings typescript`
 * to produce typed TypeScript clients in bindings/src/generated/.
 *
 * Usage:
 *   npx ts-node scripts/generate-bindings.ts [options]
 *   npm run generate:bindings            (from repo root — uses bindings/package.json)
 *
 * Options:
 *   --network  <name>    Stellar network (testnet|mainnet|localnet, default: testnet)
 *   --skip-build         Skip cargo build; use pre-built WASMs from target/
 *   --contract <name>    Generate bindings for a single contract only
 *   --dry-run            Print commands without executing them
 *   --help               Show this help
 *
 * Prerequisites:
 *   - stellar CLI installed (https://developers.stellar.org/docs/tools/stellar-cli)
 *   - Rust + cargo installed (unless --skip-build)
 *   - npx / ts-node available (devDependency in bindings/package.json)
 */

import { execSync, ExecSyncOptions } from "child_process";
import * as fs from "fs";
import * as path from "path";

// ── Config ────────────────────────────────────────────────────────────────────

const REPO_ROOT = path.resolve(__dirname, "..");
const WASM_DIR = path.join(REPO_ROOT, "target", "wasm32-unknown-unknown", "release");
const BINDINGS_DIR = path.join(REPO_ROOT, "bindings", "src", "generated");

const ALL_CONTRACTS = [
  "mux-account",
  "mux-account-factory",
  "mux-batcher",
  "mux-permissions",
  "mux-registry",
] as const;

type ContractName = (typeof ALL_CONTRACTS)[number];

// ── CLI argument parsing ──────────────────────────────────────────────────────

interface Options {
  network: string;
  skipBuild: boolean;
  contract: ContractName | null;
  dryRun: boolean;
}

function parseArgs(argv: string[]): Options {
  const opts: Options = {
    network: "testnet",
    skipBuild: false,
    contract: null,
    dryRun: false,
  };

  const args = argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--network":
        opts.network = args[++i] ?? die("--network requires a value");
        break;
      case "--skip-build":
        opts.skipBuild = true;
        break;
      case "--contract":
        opts.contract = (args[++i] ?? die("--contract requires a value")) as ContractName;
        break;
      case "--dry-run":
        opts.dryRun = true;
        break;
      case "--help":
      case "-h":
        printHelp();
        process.exit(0);
        break;
      default:
        die(`Unknown argument: ${args[i]}`);
    }
  }

  if (opts.contract && !(ALL_CONTRACTS as readonly string[]).includes(opts.contract)) {
    die(`Unknown contract: ${opts.contract}. Valid: ${ALL_CONTRACTS.join(", ")}`);
  }

  return opts;
}

function die(msg: string): never {
  console.error(`\x1b[31m[ERROR]\x1b[0m ${msg}`);
  process.exit(2);
}

function printHelp(): void {
  console.log(`
Usage: npx ts-node scripts/generate-bindings.ts [options]

Options:
  --network  <name>    Stellar network passed to stellar CLI (default: testnet)
  --skip-build         Skip cargo build; use pre-built WASMs from target/
  --contract <name>    Generate bindings for a single contract
  --dry-run            Log commands without executing
  --help               Show this help

Contracts: ${ALL_CONTRACTS.join(", ")}
  `);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const DRY_PREFIX = "\x1b[36m[DRY-RUN]\x1b[0m";
const INFO_PREFIX = "\x1b[34m[INFO]\x1b[0m ";
const OK_PREFIX = "\x1b[32m[OK]\x1b[0m   ";
const WARN_PREFIX = "\x1b[33m[WARN]\x1b[0m ";

function log(prefix: string, msg: string): void {
  console.log(`${prefix} ${msg}`);
}

function run(cmd: string, dryRun: boolean, opts: ExecSyncOptions = {}): void {
  if (dryRun) {
    log(DRY_PREFIX, `Would run: ${cmd}`);
    return;
  }
  log(INFO_PREFIX, `Running: ${cmd}`);
  execSync(cmd, { stdio: "inherit", ...opts });
}

// ── Build ─────────────────────────────────────────────────────────────────────

function buildContracts(opts: Options): void {
  if (opts.skipBuild) {
    log(INFO_PREFIX, `Skipping build (--skip-build); using WASMs from ${WASM_DIR}`);
    return;
  }
  log(INFO_PREFIX, "Building Soroban contracts (wasm32, release)...");
  run(
    "cargo build --target wasm32-unknown-unknown --release --workspace",
    opts.dryRun,
    { cwd: REPO_ROOT }
  );
  log(OK_PREFIX, "Build complete");
}

// ── Generate bindings for one contract ───────────────────────────────────────

function generateBindings(contractName: string, opts: Options): void {
  const wasmName = contractName.replace(/-/g, "_") + ".wasm";
  const wasmPath = path.join(WASM_DIR, wasmName);
  const outDir = path.join(BINDINGS_DIR, contractName);

  if (!opts.dryRun && !fs.existsSync(wasmPath)) {
    log(WARN_PREFIX, `WASM not found for ${contractName} at ${wasmPath} — skipping`);
    return;
  }

  log(INFO_PREFIX, `Generating bindings for ${contractName}...`);

  if (!opts.dryRun) {
    fs.mkdirSync(outDir, { recursive: true });
  }

  run(
    [
      "stellar contract bindings typescript",
      `--wasm "${wasmPath}"`,
      `--network "${opts.network}"`,
      `--output-dir "${outDir}"`,
      "--overwrite",
    ].join(" \\\n  "),
    opts.dryRun,
    { cwd: REPO_ROOT }
  );

  log(OK_PREFIX, `${contractName} → ${path.relative(REPO_ROOT, outDir)}`);
}

// ── Main ──────────────────────────────────────────────────────────────────────

function main(): void {
  const opts = parseArgs(process.argv);

  console.log("");
  log(INFO_PREFIX, "Mux Protocol — TypeScript Bindings Generation");
  log(INFO_PREFIX, `Network:    ${opts.network}`);
  log(INFO_PREFIX, `Skip build: ${opts.skipBuild}`);
  log(INFO_PREFIX, `Dry-run:    ${opts.dryRun}`);
  log(INFO_PREFIX, `Output:     ${path.relative(REPO_ROOT, BINDINGS_DIR)}`);
  console.log("");

  buildContracts(opts);

  const contracts: string[] = opts.contract ? [opts.contract] : [...ALL_CONTRACTS];

  log(INFO_PREFIX, `Generating bindings for ${contracts.length} contract(s)...`);
  console.log("");

  for (const contract of contracts) {
    generateBindings(contract, opts);
  }

  console.log("");
  if (opts.dryRun) {
    log(OK_PREFIX, "Dry-run complete — no files written");
  } else {
    log(OK_PREFIX, "Bindings generation complete");
  }
}

main();
