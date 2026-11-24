import { describe, it, expect } from "vitest";
import * as fs from "node:fs";
import * as path from "node:path";
import {
  BATCH_CAPS,
  MAX_AGGREGATE_OPS,
  MAX_BATCH_SIZE,
} from "../src/batcher";

/**
 * Cross-language cap sync (closes #951).
 *
 * The Rust contract is the authority for batching caps; docs/batching-limits.md
 * documents them; the TypeScript bindings must agree with both. This test
 * fails — naming both values — whenever the documented limit and the exported
 * constant drift apart, so the mismatch is caught in CI rather than when the
 * contract rejects a batch the client happily accepted.
 */

const REPO_ROOT = path.resolve(__dirname, "../..");
const LIMITS_DOC = path.join(REPO_ROOT, "docs", "batching-limits.md");
const BATCHER_CONTRACT = path.join(
  REPO_ROOT,
  "contracts",
  "mux-batcher",
  "src",
  "lib.rs",
);

/** Parse `` | `NAME` | **N** | ... `` rows out of docs/batching-limits.md. */
function documentedLimits(): Map<string, number> {
  const content = fs.readFileSync(LIMITS_DOC, "utf8");
  const limits = new Map<string, number>();
  const row = /^\|\s*`([A-Z][A-Z0-9_]*)`\s*\|\s*\*\*(\d+)\*\*\s*\|/gm;
  for (const match of content.matchAll(row)) {
    limits.set(match[1], Number(match[2]));
  }
  return limits;
}

function documentedLimit(name: string): number {
  const value = documentedLimits().get(name);
  if (value === undefined) {
    throw new Error(
      `docs/batching-limits.md does not state a machine-readable value for ${name}. ` +
        `Add a table row like: | \`${name}\` | **N** | reason |`,
    );
  }
  return value;
}

describe("batcher cap constants stay in sync with docs/batching-limits.md (closes #951)", () => {
  it("MAX_BATCH_SIZE matches the documented limit", () => {
    const documented = documentedLimit("MAX_BATCH_SIZE");
    expect(
      MAX_BATCH_SIZE,
      `docs/batching-limits.md documents MAX_BATCH_SIZE=${documented} but ` +
        `bindings/src/batcher.ts exports MAX_BATCH_SIZE=${MAX_BATCH_SIZE}`,
    ).toBe(documented);
  });

  it("MAX_AGGREGATE_OPS matches the documented limit", () => {
    const documented = documentedLimit("MAX_AGGREGATE_OPS");
    expect(
      MAX_AGGREGATE_OPS,
      `docs/batching-limits.md documents MAX_AGGREGATE_OPS=${documented} but ` +
        `bindings/src/batcher.ts exports MAX_AGGREGATE_OPS=${MAX_AGGREGATE_OPS}`,
    ).toBe(documented);
  });

  it("named constants alias BATCH_CAPS", () => {
    expect(MAX_BATCH_SIZE).toBe(BATCH_CAPS.maxBatchSize);
    expect(MAX_AGGREGATE_OPS).toBe(BATCH_CAPS.maxAggregateOps);
  });

  it("the contract's MAX_BATCH_SIZE constant matches the exported one", () => {
    const source = fs.readFileSync(BATCHER_CONTRACT, "utf8");
    const match = /const\s+MAX_BATCH_SIZE:\s*u32\s*=\s*(\d+)\s*;/.exec(source);
    expect(
      match,
      "contracts/mux-batcher/src/lib.rs no longer declares `const MAX_BATCH_SIZE: u32 = N;`",
    ).not.toBeNull();
    const onChain = Number(match![1]);
    expect(
      MAX_BATCH_SIZE,
      `the contract pins MAX_BATCH_SIZE=${onChain} but bindings/src/batcher.ts ` +
        `exports MAX_BATCH_SIZE=${MAX_BATCH_SIZE}`,
    ).toBe(onChain);
  });
});
