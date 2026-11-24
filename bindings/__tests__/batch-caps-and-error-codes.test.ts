import { describe, it, expect } from "vitest";
import * as fs from "node:fs";
import * as path from "node:path";
import { BATCH_CAPS, BatchCapErrorCode } from "../src/batcher";

/**
 * Pins the client-side batching constants (#992).
 *
 * `docs/batching-limits.md` documents the on-chain/client-side caps, and
 * `docs/bindings-error-mapping.md` documents the `MuxBatcherError` contract
 * error codes. This suite fails — naming the drifted value — whenever
 * `BATCH_CAPS` or `BatchCapErrorCode` in `bindings/src/batcher.ts` drift from
 * those documented values, so a contract/doc change that isn't reflected in
 * the bindings is caught in CI instead of at a rejected batch submission.
 */

const REPO_ROOT = path.resolve(__dirname, "../..");
const LIMITS_DOC = path.join(REPO_ROOT, "docs", "batching-limits.md");
const ERROR_MAPPING_DOC = path.join(
  REPO_ROOT,
  "docs",
  "bindings-error-mapping.md",
);

function documentedLimits(): Map<string, number> {
  const content = fs.readFileSync(LIMITS_DOC, "utf8");
  const limits = new Map<string, number>();
  const row = /^\|\s*`([A-Z][A-Z0-9_]*)`\s*\|\s*\*\*(\d+)\*\*\s*\|/gm;
  for (const match of content.matchAll(row)) {
    limits.set(match[1], Number(match[2]));
  }
  return limits;
}

/**
 * Parse the `MuxBatcherError` variant/code rows out of the "`MuxBatcherError`
 * (8 variants)" table section of docs/bindings-error-mapping.md.
 */
function documentedBatcherErrorCodes(): Map<string, number> {
  const content = fs.readFileSync(ERROR_MAPPING_DOC, "utf8");
  const sectionStart = content.indexOf("### `MuxBatcherError`");
  if (sectionStart === -1) {
    throw new Error(
      "docs/bindings-error-mapping.md no longer has a `MuxBatcherError` section",
    );
  }
  const nextSection = content.indexOf("\n### `", sectionStart + 1);
  const section = content.slice(
    sectionStart,
    nextSection === -1 ? undefined : nextSection,
  );
  const codes = new Map<string, number>();
  const row = /^\|\s*`([A-Za-z][A-Za-z0-9]*)`\s*\|\s*(\d+)\s*\|/gm;
  for (const match of section.matchAll(row)) {
    codes.set(match[1], Number(match[2]));
  }
  return codes;
}

describe("BATCH_CAPS stays in sync with docs/batching-limits.md (#992)", () => {
  const documented = documentedLimits();

  it("maxBatchSize matches the documented MAX_BATCH_SIZE", () => {
    const expected = documented.get("MAX_BATCH_SIZE");
    expect(
      BATCH_CAPS.maxBatchSize,
      `docs/batching-limits.md documents MAX_BATCH_SIZE=${expected} but ` +
        `BATCH_CAPS.maxBatchSize=${BATCH_CAPS.maxBatchSize}`,
    ).toBe(expected);
  });

  it("maxAggregateOps matches the documented MAX_AGGREGATE_OPS", () => {
    const expected = documented.get("MAX_AGGREGATE_OPS");
    expect(
      BATCH_CAPS.maxAggregateOps,
      `docs/batching-limits.md documents MAX_AGGREGATE_OPS=${expected} but ` +
        `BATCH_CAPS.maxAggregateOps=${BATCH_CAPS.maxAggregateOps}`,
    ).toBe(expected);
  });
});

describe("BatchCapErrorCode is pinned (#992)", () => {
  it("has exactly the four expected stable string values", () => {
    expect(BatchCapErrorCode).toEqual({
      BatchTooLarge: "BATCH_TOO_LARGE",
      AggregateLimitExceeded: "BATCH_AGGREGATE_LIMIT_EXCEEDED",
      Unauthorized: "BATCH_UNAUTHORIZED",
      DuplicateBatch: "BATCH_DUPLICATE",
    });
  });

  it("BatchTooLarge and Unauthorized correspond to the documented MuxBatcherError codes", () => {
    const contractCodes = documentedBatcherErrorCodes();
    // These two BatchCapErrorCode members mirror a real on-chain
    // MuxBatcherError variant (the other two — AggregateLimitExceeded and
    // DuplicateBatch — are client-side-only pre-validation concepts with no
    // on-chain counterpart, so they aren't checked here).
    expect(
      contractCodes.get("BatchTooLarge"),
      "docs/bindings-error-mapping.md no longer lists a BatchTooLarge code for MuxBatcherError",
    ).toBe(2);
    expect(
      contractCodes.get("Unauthorized"),
      "docs/bindings-error-mapping.md no longer lists an Unauthorized code for MuxBatcherError",
    ).toBe(4);
  });
});
