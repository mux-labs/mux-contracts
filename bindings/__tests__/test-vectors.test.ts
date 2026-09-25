/**
 * Loads the shared JSON test vectors (`tests/fixtures/test_vectors.json`,
 * `tests/fixtures/account_limit_vectors.json`) and cross-checks every
 * `expect.err` / `expect.code` pair against the TypeScript error-message
 * helpers in `../src/types`.
 *
 * Before this file, the fixtures were hand-written "shared truth" for
 * Rust/TS tests but nothing on the TS side loaded or executed them — a
 * fixture and a binding could silently drift apart with no test failure to
 * catch it. `tests/fixture_vectors.rs` closes the same gap on the Rust side
 * by actually driving the contracts with these vectors.
 *
 * This suite is the committed, CI-checked gate for those vectors: it runs as
 * a required check in the bindings workflow, so any drift between the
 * committed fixtures and the generated bindings/ABI fails closed.
 */

import * as fs from "fs";
import * as path from "path";
import {
  muxAccountErrorMessage,
  muxAccountFactoryErrorMessage,
  muxBatcherErrorMessage,
  muxPermissionsErrorMessage,
  spendingPolicyErrorMessage,
} from "../src/types";

const FIXTURES_DIR = path.join(__dirname, "..", "..", "tests", "fixtures");

function loadFixture(name: string): any {
  const raw = fs.readFileSync(path.join(FIXTURES_DIR, name), "utf-8");
  return JSON.parse(raw);
}

const testVectors = loadFixture("test_vectors.json");
const accountLimitVectors = loadFixture("account_limit_vectors.json");

/** Maps a fixture's top-level contract key to its TS error-message helper. */
const ERROR_MESSAGE_FN: Record<string, (e: any) => string> = {
  mux_account: muxAccountErrorMessage,
  mux_account_factory: muxAccountFactoryErrorMessage,
  mux_batcher: muxBatcherErrorMessage,
  mux_permissions: muxPermissionsErrorMessage,
  mux_spending_policy: spendingPolicyErrorMessage,
};

/** Recursively collects every `{ expect: { err, code? } }` vector found
 * under `node`, tagging each with the top-level contract key it lives
 * under so the right error-message helper can be used to validate it. */
function collectErrorVectors(
  node: unknown,
  contract: string | null,
  out: Array<{ contract: string; id: string; err: string; code?: number }>
): void {
  if (Array.isArray(node)) {
    for (const item of node) collectErrorVectors(item, contract, out);
    return;
  }
  if (node === null || typeof node !== "object") return;

  const obj = node as Record<string, unknown>;
  if (
    contract &&
    typeof obj.id === "string" &&
    obj.expect &&
    typeof (obj.expect as any).err === "string"
  ) {
    out.push({
      contract,
      id: obj.id,
      err: (obj.expect as any).err,
      code: typeof (obj.expect as any).code === "number" ? (obj.expect as any).code : undefined,
    });
  }

  for (const [key, value] of Object.entries(obj)) {
    // Track which top-level contract section (mux_account, mux_batcher, ...)
    // we're under so nested vectors resolve to the right error-message fn.
    const nextContract = ERROR_MESSAGE_FN[key] ? key : contract;
    collectErrorVectors(value, nextContract, out);
  }
}

/** Recursively collects every vector that carries an `id`, regardless of
 * whether it asserts an error, so we can enforce determinism (unique ids)
 * across the committed fixtures. */
function collectIds(node: unknown, out: string[]): void {
  if (Array.isArray(node)) {
    for (const item of node) collectIds(item, out);
    return;
  }
  if (node === null || typeof node !== "object") return;
  const obj = node as Record<string, unknown>;
  if (typeof obj.id === "string") out.push(obj.id);
  for (const value of Object.values(obj)) collectIds(value, out);
}

describe("shared JSON test vectors", () => {
  it("both fixtures parse and cross-reference each other", () => {
    expect(typeof testVectors.description).toBe("string");
    expect(typeof accountLimitVectors.description).toBe("string");
    expect(testVectors._see_also.account_limit_vectors).toBe(
      "tests/fixtures/account_limit_vectors.json"
    );
  });

  it("committed vectors are deterministic (unique ids, no duplicates)", () => {
    const ids: string[] = [];
    collectIds(testVectors, ids);
    collectIds(accountLimitVectors, ids);
    expect(ids.length).toBeGreaterThan(0);
    const seen = new Set<string>();
    const duplicates = ids.filter((id) => (seen.has(id) ? true : (seen.add(id), false)));
    expect(duplicates).toEqual([]);
  });

  describe("test_vectors.json error vectors match TS error-message helpers", () => {
    const vectors: Array<{ contract: string; id: string; err: string; code?: number }> = [];
    collectErrorVectors(testVectors, null, vectors);

    it("found at least one error vector per known contract", () => {
      const contracts = new Set(vectors.map((v) => v.contract));
      expect(contracts.has("mux_account")).toBe(true);
      expect(contracts.has("mux_batcher")).toBe(true);
      expect(contracts.has("mux_permissions")).toBe(true);
    });

    it.each(vectors.map((v) => [v.id, v] as const))(
      "%s: expect.err is a recognized error name",
      (_id, v) => {
        const fn = ERROR_MESSAGE_FN[v.contract];
        expect(fn(v.err)).not.toBe("unknown error code");
        if (v.code !== undefined) {
          // Name and numeric code must resolve to the same message — proof
          // the fixture's code and the binding's nameMap agree.
          expect(fn(v.err)).toBe(fn(v.code));
        }
      }
    );
  });

  describe("account_limit_vectors.json error vectors match TS error-message helpers", () => {
    const vectors: Array<{ contract: string; id: string; err: string; code?: number }> = [];
    collectErrorVectors(accountLimitVectors, null, vectors);

    it("found at least one error vector", () => {
      expect(vectors.length).toBeGreaterThan(0);
    });

    it.each(vectors.map((v) => [v.id, v] as const))(
      "%s: expect.err/code are internally consistent and recognized",
      (_id, v) => {
        const fn = ERROR_MESSAGE_FN[v.contract];
        expect(fn(v.err)).not.toBe("unknown error code");
        if (v.code !== undefined) {
          expect(fn(v.err)).toBe(fn(v.code));
        }
      }
    );
  });

  describe("account_limit_vectors.json constants stay internally consistent", () => {
    const { constants, mux_batcher } = accountLimitVectors;

    it("MAX_BATCH_SIZE matches the batch_size_limits boundary vectors", () => {
      const atCap = mux_batcher.batch_size_limits.find(
        (v: any) => v.id === "bat-size-at-cap"
      );
      const oneOver = mux_batcher.batch_size_limits.find(
        (v: any) => v.id === "bat-size-one-over"
      );
      expect(atCap.input.ops_count).toBe(constants.mux_batcher.MAX_BATCH_SIZE);
      expect(oneOver.input.ops_count).toBe(constants.mux_batcher.MAX_BATCH_SIZE + 1);
    });

    it("MAX_DELEGATES matches the delegate_limits boundary vectors", () => {
      const { delegate_limits } = accountLimitVectors.mux_account;
      const underCap = delegate_limits.find((v: any) => v.id === "acct-dlg-under-cap");
      const atCapReject = delegate_limits.find((v: any) => v.id === "acct-dlg-at-cap-reject");
      expect(underCap.input.pre_existing_delegates + 1).toBe(
        constants.mux_account.MAX_DELEGATES
      );
      expect(atCapReject.input.pre_existing_delegates).toBe(
        constants.mux_account.MAX_DELEGATES
      );
    });

    it("MAX_SESSION_KEYS matches the session_key_limits boundary vectors", () => {
      const { session_key_limits } = accountLimitVectors.mux_account;
      const underCap = session_key_limits.find((v: any) => v.id === "acct-sk-under-cap");
      const atCapReject = session_key_limits.find((v: any) => v.id === "acct-sk-at-cap-reject");
      expect(underCap.input.pre_existing_keys + 1).toBe(
        constants.mux_account.MAX_SESSION_KEYS
      );
      expect(atCapReject.input.pre_existing_keys).toBe(
        constants.mux_account.MAX_SESSION_KEYS
      );
    });

    it("factory metadata size limits match boundary vectors", () => {
      const { metadata_limits } = accountLimitVectors.mux_account_factory;
      const atLimit = metadata_limits.find((v: any) => v.id === "fac-meta-at-limit");
      const verOver = metadata_limits.find((v: any) => v.id === "fac-meta-version-over-limit");
      const descOver = metadata_limits.find((v: any) => v.id === "fac-meta-desc-over-limit");
      const authorOver = metadata_limits.find((v: any) => v.id === "fac-meta-author-over-limit");

      expect(atLimit.input.version_len).toBe(constants.mux_account_factory.MAX_VERSION_LENGTH);
      expect(verOver.input.version_len).toBe(constants.mux_account_factory.MAX_VERSION_LENGTH + 1);

      expect(atLimit.input.description_len).toBe(constants.mux_account_factory.MAX_DESCRIPTION_LENGTH);
      expect(descOver.input.description_len).toBe(constants.mux_account_factory.MAX_DESCRIPTION_LENGTH + 1);

      expect(atLimit.input.author_len).toBe(constants.mux_account_factory.MAX_AUTHOR_LENGTH);
      expect(authorOver.input.author_len).toBe(constants.mux_account_factory.MAX_AUTHOR_LENGTH + 1);
    });

    it("spending policy constants match boundary vectors", () => {
      const { boundary_limits } = accountLimitVectors.mux_spending_policy;
      const zeroLmt = boundary_limits.find((v: any) => v.id === "sp-lmt-zero-limit-reject");
      const zeroPeriod = boundary_limits.find((v: any) => v.id === "sp-lmt-zero-period-reject");

      expect(zeroLmt.input.limit).toBeLessThan(constants.mux_spending_policy.SPEND_LIMIT_MIN);
      expect(zeroPeriod.input.period_ledgers).toBeLessThan(constants.mux_spending_policy.PERIOD_LEDGERS_MIN);
    });
  });
});
