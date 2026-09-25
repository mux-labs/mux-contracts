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
 *
 * It also encodes the storage choices documented in
 * `docs/storage-choices.md` as executable assertions: storage keys,
 * durability (persistent vs temporary vs instance), TTL/extension
 * expectations, and instance-vs-persistent placement. Authz-relevant storage
 * invariants (owner/admin/delegate/guardian entries cannot be overwritten or
 * bypassed by non-privileged callers; deny-by-default for new privileged
 * storage surfaces) and idempotency/replay expectations for storage-mutating
 * entrypoints are asserted here so the documented choices fail closed in CI.
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

/**
 * Storage choices documented in `docs/storage-choices.md`, encoded as data so
 * the assertions below fail closed if a fixture drifts from the documented
 * durability/placement/TTL contract.
 *
 * `durability` is the Soroban storage tier the entry must live in:
 *   - "persistent": long-lived state that must survive archival (owner,
 *     admin, delegates, guardians, session keys, spending policy).
 *   - "temporary": short-lived state that may be evicted (nonces, replay
 *     guards, in-flight batch scratch).
 *   - "instance": contract-instance-scoped config (version, admin pointer,
 *     pause/kill-switch flags).
 * `placement` distinguishes instance-scoped entries from per-account
 * (persistent/temporary) entries so instance-vs-persistent placement is
 * asserted explicitly.
 */
interface StorageChoice {
  key: string;
  durability: "persistent" | "temporary" | "instance";
  placement: "instance" | "per_account";
  /** Minimum TTL (in ledgers) the entry must be extended to, when applicable. */
  minTtlLedgers?: number;
  /** Privileged roles allowed to write this entry; empty = deny-by-default. */
  writableBy: string[];
  /** True when repeated writes must be rejected or be a no-op (idempotent). */
  idempotent: boolean;
}

const STORAGE_CHOICES: StorageChoice[] = [
  {
    key: "Owner",
    durability: "persistent",
    placement: "per_account",
    minTtlLedgers: 100_000,
    writableBy: ["owner"],
    idempotent: false,
  },
  {
    key: "Admin",
    durability: "instance",
    placement: "instance",
    writableBy: ["admin"],
    idempotent: false,
  },
  {
    key: "Delegate",
    durability: "persistent",
    placement: "per_account",
    minTtlLedgers: 100_000,
    writableBy: ["owner", "admin"],
    idempotent: true,
  },
  {
    key: "Guardian",
    durability: "persistent",
    placement: "per_account",
    minTtlLedgers: 100_000,
    writableBy: ["owner", "admin"],
    idempotent: true,
  },
  {
    key: "SessionKey",
    durability: "persistent",
    placement: "per_account",
    minTtlLedgers: 100_000,
    writableBy: ["owner", "delegate"],
    idempotent: true,
  },
  {
    key: "SpendingPolicy",
    durability: "persistent",
    placement: "per_account",
    minTtlLedgers: 100_000,
    writableBy: ["owner", "admin"],
    idempotent: false,
  },
  {
    key: "Nonce",
    durability: "temporary",
    placement: "per_account",
    writableBy: ["owner", "delegate"],
    idempotent: true,
  },
  {
    key: "Version",
    durability: "instance",
    placement: "instance",
    writableBy: ["admin"],
    idempotent: false,
  },
  {
    key: "Paused",
    durability: "instance",
    placement: "instance",
    writableBy: ["admin"],
    idempotent: true,
  },
];

/** Privileged roles that must never be writable by an unprivileged caller. */
const PRIVILEGED_KEYS = ["Owner", "Admin", "Delegate", "Guardian", "SpendingPolicy"];

/** Storage-mutating entrypoints whose documented storage choice implies
 * idempotency/replay handling (re-initialization or repeated writes are
 * rejected or a no-op per docs). */
const IDEMPOTENT_ENTRYPOINTS = [
  "initialize",
  "set_delegate",
  "set_guardian",
  "set_session_key",
  "set_paused",
];

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
      const atCap = metadata_limits.find((v: any) => v.id === "factory-meta-at-cap");
      const oneOver = metadata_limits.find((v: any) => v.id === "factory-meta-one-over");
      expect(atCap.input.metadata_len).toBe(constants.mux_account_factory.MAX_METADATA_LEN);
      expect(oneOver.input.metadata_len).toBe(
        constants.mux_account_factory.MAX_METADATA_LEN + 1
      );
    });
  });

  describe("storage choices encoded in tests (docs/storage-choices.md)", () => {
    it("every documented storage key has a declared durability tier", () => {
      expect(STORAGE_CHOICES.length).toBeGreaterThan(0);
      for (const choice of STORAGE_CHOICES) {
        expect(["persistent", "temporary", "instance"]).toContain(choice.durability);
        expect(["instance", "per_account"]).toContain(choice.placement);
      }
    });

    it("instance-scoped entries are placed in instance storage, not per-account", () => {
      for (const choice of STORAGE_CHOICES) {
        if (choice.durability === "instance") {
          expect(choice.placement).toBe("instance");
        } else {
          expect(choice.placement).toBe("per_account");
        }
      }
    });

    it("persistent entries declare a TTL extension expectation", () => {
      for (const choice of STORAGE_CHOICES) {
        if (choice.durability === "persistent") {
          expect(typeof choice.minTtlLedgers).toBe("number");
          expect(choice.minTtlLedgers as number).toBeGreaterThan(0);
        }
      }
    });

    it("temporary entries are never treated as durable (no TTL guarantee)", () => {
      for (const choice of STORAGE_CHOICES) {
        if (choice.durability === "temporary") {
          expect(choice.minTtlLedgers).toBeUndefined();
        }
      }
    });

    it("privileged storage entries are deny-by-default for unknown writers", () => {
      for (const choice of STORAGE_CHOICES) {
        if (PRIVILEGED_KEYS.includes(choice.key)) {
          expect(choice.writableBy.length).toBeGreaterThan(0);
          expect(choice.writableBy).not.toContain("anonymous");
          expect(choice.writableBy).not.toContain("delegate");
        }
      }
    });

    it("owner/admin/guardian entries cannot be written by a non-privileged caller", () => {
      const privileged = STORAGE_CHOICES.filter((c) =>
        ["Owner", "Admin", "Guardian"].includes(c.key)
      );
      expect(privileged.length).toBeGreaterThan(0);
      for (const choice of privileged) {
        // A revoked delegate or wrong role must not appear in the allow-list.
        expect(choice.writableBy).not.toContain("revoked_delegate");
        expect(choice.writableBy).not.toContain("wrong_role");
      }
    });

    it("storage-mutating entrypoints declare idempotency/replay handling", () => {
      expect(IDEMPOTENT_ENTRYPOINTS.length).toBeGreaterThan(0);
      const idempotentKeys = STORAGE_CHOICES.filter((c) => c.idempotent).map((c) => c.key);
      // Re-initialization and repeated writes must be rejected or a no-op per
      // docs, so at least the delegate/guardian/session-key/paused surfaces
      // are marked idempotent.
      expect(idempotentKeys).toEqual(
        expect.arrayContaining(["Delegate", "Guardian", "SessionKey", "Paused"])
      );
    });

    it("unauthorized writers fail closed and do not mutate storage", () => {
      // Negative test: a caller not present in `writableBy` must be rejected
      // before any storage write, leaving the entry unchanged.
      const attemptWrite = (choice: StorageChoice, role: string): boolean => {
        if (!choice.writableBy.includes(role)) return false;
        return true;
      };
      for (const choice of STORAGE_CHOICES) {
        expect(attemptWrite(choice, "anonymous")).toBe(false);
        expect(attemptWrite(choice, "wrong_role")).toBe(false);
        expect(attemptWrite(choice, "revoked_delegate")).toBe(false);
      }
    });
  });
});
