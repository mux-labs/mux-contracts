#!/usr/bin/env npx ts-node
/**
 * scripts/generate-addresses.ts
 *
 * Reads `config/addresses.json` and regenerates the `DEFAULT_ADDRESSES`
 * constant in `bindings/src/addresses-config.ts`.
 *
 * The generated block is delimited by sentinel comments so that the rest
 * of `addresses-config.ts` (type definitions, validation functions, etc.)
 * is preserved verbatim.
 *
 * Usage:
 *   npx ts-node scripts/generate-addresses.ts
 *   # or
 *   node -r ts-node/register scripts/generate-addresses.ts
 *
 * Options:
 *   --dry-run   Print the generated block without writing the file.
 *   --help      Show this help.
 *
 * CI integration:
 *   Run this script in CI after editing config/addresses.json to ensure
 *   bindings/src/addresses-config.ts stays in sync.  Add the output file
 *   to the set of files checked for unexpected diffs.
 *
 * Invariants:
 *   - All networks listed in addresses.json are emitted.
 *   - Contract keys are emitted in deterministic alphabetical order.
 *   - Only the sentinel-delimited block in addresses-config.ts is touched.
 *   - If no sentinel block is found, the generated block is appended at the
 *     top of the file (after the codegen header comment) and the existing
 *     content is preserved below it.
 */

import * as fs from "fs";
import * as path from "path";

const REPO_ROOT = path.resolve(__dirname, "..");
const ADDRESSES_JSON = path.join(REPO_ROOT, "config", "addresses.json");
const ADDRESSES_CONFIG_TS = path.join(
  REPO_ROOT,
  "bindings",
  "src",
  "addresses-config.ts"
);

// Sentinels that delimit the generated DEFAULT_ADDRESSES block in
// addresses-config.ts.  The generator rewrites everything between
// (and including) these two lines.
const BEGIN_SENTINEL = "// [codegen:begin] DEFAULT_ADDRESSES — generated from config/addresses.json";
const END_SENTINEL = "// [codegen:end] DEFAULT_ADDRESSES";

interface AddressBook {
  [key: string]: string;
}

interface AddressesJson {
  $schema?: string;
  _review?: unknown;
  localnet: AddressBook;
  testnet: AddressBook;
  mainnet: AddressBook;
  [network: string]: unknown;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

function die(msg: string): never {
  console.error(`\x1b[31m[ERROR]\x1b[0m ${msg}`);
  process.exit(1);
}

function parseArgs(argv: string[]): { dryRun: boolean } {
  const args = argv.slice(2);
  let dryRun = false;
  for (const arg of args) {
    if (arg === "--dry-run") {
      dryRun = true;
    } else if (arg === "--help" || arg === "-h") {
      console.log(
        "Usage: npx ts-node scripts/generate-addresses.ts [--dry-run] [--help]"
      );
      process.exit(0);
    } else {
      die(`Unknown argument: ${arg}`);
    }
  }
  return { dryRun };
}

/** Format a single network's address map as TypeScript source. */
function formatNetwork(name: string, addresses: AddressBook, indent: string): string {
  const keys = Object.keys(addresses).sort();
  const lines = keys.map((key) => {
    const val = addresses[key] ?? "";
    return `${indent}  ${key}: "${val}",`;
  });
  return [`${indent}${name}: {`, ...lines, `${indent}},`].join("\n");
}

/** Build the full generated block (including sentinels). */
function buildBlock(data: AddressesJson): string {
  const networks = ["localnet", "testnet", "mainnet"].filter(
    (n) => typeof data[n] === "object" && data[n] !== null
  );

  const networkLines = networks
    .map((n) => formatNetwork(n, data[n] as AddressBook, "  "))
    .join("\n");

  return [
    BEGIN_SENTINEL,
    "// DO NOT EDIT — run `npx ts-node scripts/generate-addresses.ts` to regenerate.",
    "export const DEFAULT_ADDRESSES = {",
    networkLines,
    "};",
    END_SENTINEL,
  ].join("\n");
}

/** Splice the generated block into the existing file content. */
function spliceBlock(existing: string, block: string): string {
  const beginIdx = existing.indexOf(BEGIN_SENTINEL);
  const endIdx = existing.indexOf(END_SENTINEL);

  if (beginIdx !== -1 && endIdx !== -1 && endIdx > beginIdx) {
    // Replace the existing sentineled block.
    return (
      existing.slice(0, beginIdx) +
      block +
      existing.slice(endIdx + END_SENTINEL.length)
    );
  }

  // No existing sentinels — replace the old `export const DEFAULT_ADDRESSES`
  // declaration if present, otherwise prepend.
  const oldDeclPattern = /^export const DEFAULT_ADDRESSES\s*=[\s\S]*?^};/m;
  if (oldDeclPattern.test(existing)) {
    return existing.replace(oldDeclPattern, block);
  }

  // Fallback: prepend the block followed by the existing content.
  return block + "\n\n" + existing;
}

// ── Main ─────────────────────────────────────────────────────────────────────

function main(): void {
  const { dryRun } = parseArgs(process.argv);

  // Read addresses.json
  if (!fs.existsSync(ADDRESSES_JSON)) {
    die(`addresses.json not found at ${ADDRESSES_JSON}`);
  }
  let data: AddressesJson;
  try {
    data = JSON.parse(fs.readFileSync(ADDRESSES_JSON, "utf8")) as AddressesJson;
  } catch (err) {
    die(`Failed to parse addresses.json: ${String(err)}`);
  }

  const block = buildBlock(data);

  if (dryRun) {
    console.log("\x1b[36m[DRY-RUN]\x1b[0m Generated DEFAULT_ADDRESSES block:\n");
    console.log(block);
    console.log("\n\x1b[36m[DRY-RUN]\x1b[0m No files written.");
    return;
  }

  // Read existing addresses-config.ts
  if (!fs.existsSync(ADDRESSES_CONFIG_TS)) {
    die(`addresses-config.ts not found at ${ADDRESSES_CONFIG_TS}`);
  }
  const existing = fs.readFileSync(ADDRESSES_CONFIG_TS, "utf8");

  const updated = spliceBlock(existing, block);

  if (updated === existing) {
    console.log("\x1b[32m[OK]\x1b[0m   addresses-config.ts is already up-to-date.");
    return;
  }

  fs.writeFileSync(ADDRESSES_CONFIG_TS, updated, "utf8");
  console.log(
    `\x1b[32m[OK]\x1b[0m   Updated ${path.relative(REPO_ROOT, ADDRESSES_CONFIG_TS)}`
  );
}

main();
