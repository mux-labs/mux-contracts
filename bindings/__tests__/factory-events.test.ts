/**
 * Unit tests for factory-events.ts helpers.
 *
 * Tests cover:
 *  - FACTORY_CONTRACT_TAG and FACTORY_EVENT_TOPICS constants
 *  - parseFactoryEvent: deployed event (array-style and object-style data)
 *  - parseFactoryEvent: meta_set event
 *  - parseFactoryEvent: returns null for unknown tags, unknown actions, bad data
 *  - Event catalog completeness: all documented actions are present in FACTORY_EVENT_TOPICS
 */

import {
  FACTORY_CONTRACT_TAG,
  FACTORY_EVENT_TOPICS,
  parseFactoryEvent,
  type FactoryDeployedEvent,
  type FactoryMetaSetEvent,
  type FactoryEvent,
  type RawSorobanEvent,
} from "../src/factory-events";

// ── Fixtures ────────────────────────────────────────────────────────────────

const OWNER = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";
const ACCOUNT = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M";
const VERSION = "1.0.0";

/** Helper: build a raw event in plain-array style (SDK v11+). */
function makeRawEvent(
  tag: string,
  action: string,
  data: unknown,
): RawSorobanEvent {
  return { topic: [tag, action], value: data };
}

/** `deployed` event with array-style data. */
const DEPLOYED_ARRAY: RawSorobanEvent = makeRawEvent(
  FACTORY_CONTRACT_TAG,
  FACTORY_EVENT_TOPICS.deployed,
  [OWNER, ACCOUNT],
);

/** `deployed` event with object-style data (XDR-decoded struct). */
const DEPLOYED_OBJECT: RawSorobanEvent = makeRawEvent(
  FACTORY_CONTRACT_TAG,
  FACTORY_EVENT_TOPICS.deployed,
  { vec: [{ address: OWNER }, { address: ACCOUNT }] },
);

/** `meta_set` event with array-style data. */
const META_SET_ARRAY: RawSorobanEvent = makeRawEvent(
  FACTORY_CONTRACT_TAG,
  FACTORY_EVENT_TOPICS.meta_set,
  [OWNER, ACCOUNT, VERSION],
);

/** `meta_set` event with object-style data. */
const META_SET_OBJECT: RawSorobanEvent = makeRawEvent(
  FACTORY_CONTRACT_TAG,
  FACTORY_EVENT_TOPICS.meta_set,
  { vec: [{ address: OWNER }, { address: ACCOUNT }, { string: VERSION }] },
);

// ── Constants ────────────────────────────────────────────────────────────────

describe("FACTORY_CONTRACT_TAG", () => {
  it("equals 'mux_fac'", () => {
    expect(FACTORY_CONTRACT_TAG).toBe("mux_fac");
  });

  it("is a string literal (no accidental widening)", () => {
    expect(typeof FACTORY_CONTRACT_TAG).toBe("string");
  });
});

describe("FACTORY_EVENT_TOPICS", () => {
  it("contains 'deployed'", () => {
    expect(FACTORY_EVENT_TOPICS.deployed).toBe("deployed");
  });

  it("contains 'meta_set'", () => {
    expect(FACTORY_EVENT_TOPICS.meta_set).toBe("meta_set");
  });

  it("exposes exactly the two documented action topics", () => {
    const keys = Object.keys(FACTORY_EVENT_TOPICS).sort();
    expect(keys).toEqual(["deployed", "meta_set"]);
  });

  it("all topic values are strings of ≤8 characters (Soroban symbol_short limit)", () => {
    for (const v of Object.values(FACTORY_EVENT_TOPICS)) {
      expect(v.length).toBeLessThanOrEqual(8);
    }
  });

  it("FACTORY_CONTRACT_TAG is ≤8 characters", () => {
    expect(FACTORY_CONTRACT_TAG.length).toBeLessThanOrEqual(8);
  });
});

// ── parseFactoryEvent: deployed ──────────────────────────────────────────────

describe("parseFactoryEvent — deployed (array-style data)", () => {
  let result: FactoryEvent | null;

  beforeEach(() => {
    result = parseFactoryEvent(DEPLOYED_ARRAY);
  });

  it("returns a non-null event", () => {
    expect(result).not.toBeNull();
  });

  it("sets action to 'deployed'", () => {
    expect(result?.action).toBe("deployed");
  });

  it("extracts owner address", () => {
    expect((result as FactoryDeployedEvent).owner).toBe(OWNER);
  });

  it("extracts accountAddress", () => {
    expect((result as FactoryDeployedEvent).accountAddress).toBe(ACCOUNT);
  });
});

describe("parseFactoryEvent — deployed (object/XDR-style data)", () => {
  let result: FactoryEvent | null;

  beforeEach(() => {
    result = parseFactoryEvent(DEPLOYED_OBJECT);
  });

  it("returns a non-null event", () => {
    expect(result).not.toBeNull();
  });

  it("sets action to 'deployed'", () => {
    expect(result?.action).toBe("deployed");
  });

  it("extracts owner from object-style address wrapper", () => {
    expect((result as FactoryDeployedEvent).owner).toBe(OWNER);
  });

  it("extracts accountAddress from object-style address wrapper", () => {
    expect((result as FactoryDeployedEvent).accountAddress).toBe(ACCOUNT);
  });
});

// ── parseFactoryEvent: meta_set ──────────────────────────────────────────────

describe("parseFactoryEvent — meta_set (array-style data)", () => {
  let result: FactoryEvent | null;

  beforeEach(() => {
    result = parseFactoryEvent(META_SET_ARRAY);
  });

  it("returns a non-null event", () => {
    expect(result).not.toBeNull();
  });

  it("sets action to 'meta_set'", () => {
    expect(result?.action).toBe("meta_set");
  });

  it("extracts owner", () => {
    expect((result as FactoryMetaSetEvent).owner).toBe(OWNER);
  });

  it("extracts accountAddress", () => {
    expect((result as FactoryMetaSetEvent).accountAddress).toBe(ACCOUNT);
  });

  it("extracts version string", () => {
    expect((result as FactoryMetaSetEvent).version).toBe(VERSION);
  });
});

describe("parseFactoryEvent — meta_set (object/XDR-style data)", () => {
  let result: FactoryEvent | null;

  beforeEach(() => {
    result = parseFactoryEvent(META_SET_OBJECT);
  });

  it("returns a non-null event", () => {
    expect(result).not.toBeNull();
  });

  it("sets action to 'meta_set'", () => {
    expect(result?.action).toBe("meta_set");
  });

  it("extracts owner from nested object", () => {
    expect((result as FactoryMetaSetEvent).owner).toBe(OWNER);
  });

  it("extracts accountAddress from nested object", () => {
    expect((result as FactoryMetaSetEvent).accountAddress).toBe(ACCOUNT);
  });

  it("extracts version from { string: '...' } wrapper", () => {
    expect((result as FactoryMetaSetEvent).version).toBe(VERSION);
  });
});

// ── parseFactoryEvent: null / unknown inputs ─────────────────────────────────

describe("parseFactoryEvent — returns null for non-factory events", () => {
  it("returns null when topics[0] is a different contract tag", () => {
    const e = makeRawEvent("mux_acct", "init", [OWNER]);
    expect(parseFactoryEvent(e)).toBeNull();
  });

  it("returns null when topics[0] is empty", () => {
    const e: RawSorobanEvent = { topic: [], value: [OWNER, ACCOUNT] };
    expect(parseFactoryEvent(e)).toBeNull();
  });

  it("returns null for an unrecognised action with the correct tag", () => {
    const e = makeRawEvent(FACTORY_CONTRACT_TAG, "unknown_action", [OWNER, ACCOUNT]);
    expect(parseFactoryEvent(e)).toBeNull();
  });

  it("returns null when deployed data has fewer than two elements", () => {
    const e = makeRawEvent(FACTORY_CONTRACT_TAG, FACTORY_EVENT_TOPICS.deployed, [OWNER]);
    expect(parseFactoryEvent(e)).toBeNull();
  });

  it("returns null when meta_set data is missing the version field", () => {
    const e = makeRawEvent(FACTORY_CONTRACT_TAG, FACTORY_EVENT_TOPICS.meta_set, [OWNER, ACCOUNT]);
    expect(parseFactoryEvent(e)).toBeNull();
  });

  it("returns null when data is null", () => {
    const e = makeRawEvent(FACTORY_CONTRACT_TAG, FACTORY_EVENT_TOPICS.deployed, null);
    expect(parseFactoryEvent(e)).toBeNull();
  });

  it("returns null when data is an empty array", () => {
    const e = makeRawEvent(FACTORY_CONTRACT_TAG, FACTORY_EVENT_TOPICS.deployed, []);
    expect(parseFactoryEvent(e)).toBeNull();
  });

  it("returns null when data is an unparseable string", () => {
    const e = makeRawEvent(FACTORY_CONTRACT_TAG, FACTORY_EVENT_TOPICS.deployed, "not-json");
    expect(parseFactoryEvent(e)).toBeNull();
  });
});

// ── Event catalog completeness ────────────────────────────────────────────────

describe("event catalog completeness", () => {
  it("all entries in FACTORY_EVENT_TOPICS are handled by parseFactoryEvent (deployed)", () => {
    // If deployed is unhandled, parseFactoryEvent returns null for a valid event.
    expect(parseFactoryEvent(DEPLOYED_ARRAY)).not.toBeNull();
  });

  it("all entries in FACTORY_EVENT_TOPICS are handled by parseFactoryEvent (meta_set)", () => {
    expect(parseFactoryEvent(META_SET_ARRAY)).not.toBeNull();
  });

  it("read-only / simulate entrypoints emit no events — they have no topic entry", () => {
    // simulate_deploy, simulate_deploy_with_metadata, get_accounts,
    // account_count, get_account_metadata, max_accounts_per_owner must NOT
    // appear in FACTORY_EVENT_TOPICS.
    const simulateAndRead = [
      "simulate_deploy",
      "simulate_deploy_with_metadata",
      "get_accounts",
      "account_count",
      "get_account_metadata",
      "max_accounts_per_owner",
    ];
    for (const name of simulateAndRead) {
      expect(Object.values(FACTORY_EVENT_TOPICS)).not.toContain(name);
    }
  });
});
