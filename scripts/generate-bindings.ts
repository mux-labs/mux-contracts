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
 *   npm run generate:bindings            (from repo root or bindings/)
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
 *   - ts-node and typescript in devDependencies (already present in bindings/package.json)
 */

import { execSync, ExecSyncOptions } from "child_process";
import * as fs from "fs";
import * as path from "path";

// ── Config ────────────────────────────────────────────────────────────────────

const REPO_ROOT = path.resolve(__dirname, "..");
const WASM_DIR = path.join(
  REPO_ROOT,
  "target",
  "wasm32-unknown-unknown",
  "release",
);
const BINDINGS_DIR = path.join(REPO_ROOT, "bindings", "src", "generated");

const ALL_CONTRACTS = [
  "mux-account",
  "mux-account-factory",
  "mux-batcher",
  "mux-permissions",
] as const;

type Contract = (typeof ALL_CONTRACTS)[number];

// ── Argument parsing ──────────────────────────────────────────────────────────

interface Options {
  network: string;
  skipBuild: boolean;
  contract: Contract | null;
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
        opts.network = args[++i] ?? fail("--network requires a value");
        break;
      case "--skip-build":
        opts.skipBuild = true;
        break;
      case "--contract":
        opts.contract = (args[++i] ?? fail("--contract requires a value")) as Contract;
        if (!ALL_CONTRACTS.includes(opts.contract)) {
          fail(`Unknown contract '${opts.contract}'. Valid: ${ALL_CONTRACTS.join(", ")}`);
        }
        break;
      case "--dry-run":
        opts.dryRun = true;
        break;
      case "--help":
        printHelp();
        process.exit(0);
        break;
      default:
        fail(`Unknown argument: ${args[i]}`);
    }
  }
  return opts;
}

function fail(msg: string): never {
  console.error(`\x1b[31m✗\x1b[0m ${msg}`);
  process.exit(1);
}

function printHelp(): void {
  console.log(`
Usage: npx ts-node scripts/generate-bindings.ts [options]

Options:
  --network <name>    Stellar network: testnet|mainnet|localnet (default: testnet)
  --skip-build        Skip cargo build; use pre-built WASMs
  --contract <name>   Generate bindings for a single contract only
  --dry-run           Print commands without executing them
  --help              Show this help
`);
}

// ── Logging ───────────────────────────────────────────────────────────────────

const DRY = "\x1b[36m\x1b[1m[DRY RUN]\x1b[0m";
const OK = "\x1b[32m✓\x1b[0m";
const INFO = "\x1b[34mℹ️ \x1b[0m";
const WARN = "\x1b[33m⚠️ \x1b[0m";

// ── Command runner ────────────────────────────────────────────────────────────

/** Execute a shell command, or print it if dry-run is active. */
function run(cmd: string, dryRun: boolean, opts: ExecSyncOptions = {}): void {
  if (dryRun) {
    console.log(`${DRY} Would run: ${cmd}`);
    return;
  }
  execSync(cmd, { stdio: "inherit", ...opts });
}

// ── Main ──────────────────────────────────────────────────────────────────────

function main(): void {
  const opts = parseArgs(process.argv);
  const contracts: readonly string[] = opts.contract
    ? [opts.contract]
    : ALL_CONTRACTS;

  console.log(`${INFO} Mux Protocol — TypeScript Bindings Generator`);
  console.log(`${INFO} Network:    ${opts.network}`);
  console.log(`${INFO} Contracts:  ${contracts.join(", ")}`);
  console.log(`${INFO} Skip build: ${opts.skipBuild}`);
  console.log(`${INFO} Dry run:    ${opts.dryRun}`);
  console.log("");

  // ── Step 1: Build contracts ──────────────────────────────────────────────

  if (!opts.skipBuild) {
    console.log(`${INFO} Step 1/2: Building Soroban contracts...`);
    run(
      `cargo build --target wasm32-unknown-unknown --release --workspace`,
      opts.dryRun,
      { cwd: REPO_ROOT },
    );
    if (!opts.dryRun) console.log(`${OK} Build complete`);
  } else {
    console.log(
      `${INFO} Step 1/2: Skipping build (--skip-build); using pre-built WASMs from ${WASM_DIR}`,
    );
  }
  console.log("");

  // ── Step 2: Generate bindings ────────────────────────────────────────────

  console.log(
    `${INFO} Step 2/2: Generating TypeScript bindings into ${BINDINGS_DIR}...`,
  );

  if (!opts.dryRun) {
    fs.mkdirSync(BINDINGS_DIR, { recursive: true });
  } else {
    console.log(`${DRY} Would mkdir -p ${BINDINGS_DIR}`);
  }

  let generated = 0;
  let skipped = 0;

  for (const contract of contracts) {
    const wasmName = contract.replace(/-/g, "_") + ".wasm";
    const wasmPath = path.join(WASM_DIR, wasmName);
    const outDir = path.join(BINDINGS_DIR, contract);

    console.log(`  → ${contract}`);

    if (!opts.dryRun && !fs.existsSync(wasmPath)) {
      console.log(`${WARN}  WASM not found at ${wasmPath} — skipping.`);
      skipped++;
      continue;
    }

    const cmd = [
      "stellar contract bindings typescript",
      `--wasm "${wasmPath}"`,
      `--network "${opts.network}"`,
      `--output-dir "${outDir}"`,
      "--overwrite",
    ].join(" \\\n      ");

    run(cmd, opts.dryRun);

    if (!opts.dryRun) {
      console.log(`${OK}  Generated → ${outDir}`);
      generated++;
    }
  }

  console.log("");
  if (opts.dryRun) {
    console.log(
      `${DRY} Dry-run complete — no files written, no stellar CLI invoked.`,
    );
  } else {
    console.log(
      `${OK} Bindings generation complete. ` +
        `Generated: ${generated}, Skipped: ${skipped}`,
    );
  }
}

main();
