/**
 * Regression tests for the simulate-batch CLI network handling (issue #994).
 *
 * The CLI reads `SOROBAN_NETWORK` and must fail closed on an unknown value
 * instead of silently falling back to the testnet RPC. These tests assert on
 * the known-network set the script validates against (mirrors the
 * `Unknown network` fail-closed pattern used by `bindings/src/local-invoke.ts`),
 * and lock in that `simulate-batch.ts` rejects an unrecognized
 * `SOROBAN_NETWORK` instead of defaulting to testnet.
 */

import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";

const SOURCE = fs.readFileSync(
  path.join(__dirname, "..", "src", "simulate-batch.ts"),
  "utf8"
);

const KNOWN_NETWORKS = ["localnet", "testnet", "mainnet"];

describe("simulate-batch CLI network validation", () => {
  it("defines passphrases only for the known network set", () => {
    for (const network of KNOWN_NETWORKS) {
      expect(SOURCE).toMatch(new RegExp(`${network}:`));
    }
  });

  it("fails closed (usage/exit) when the resolved network has no passphrase entry", () => {
    // `main()` must reject before constructing a client when
    // `PASSPHRASES[network]` is undefined, rather than silently resolving a
    // testnet RPC URL for an unrecognized SOROBAN_NETWORK value.
    expect(SOURCE).toMatch(/!PASSPHRASES\[network\]\)\s*usage\(\)/);
  });

  it("usage() exits with a non-zero code instead of continuing", () => {
    expect(SOURCE).toMatch(/function usage\(\): never \{[\s\S]*?process\.exit\(2\)/);
  });
});
