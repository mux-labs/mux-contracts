import {
  POLICY_CONTRACT_TAG,
  POLICY_EVENT_TOPICS,
  parsePolicyEvent,
  type PolicyEvent,
} from "../src/policy-events";
import { makeRawEvent } from "./event-fixtures";

describe("POLICY_CONTRACT_TAG", () => {
  it("equals 'mux_pol'", () => {
    expect(POLICY_CONTRACT_TAG).toBe("mux_pol");
  });
});

describe("POLICY_EVENT_TOPICS", () => {
  it("contains all documented actions", () => {
    expect(POLICY_EVENT_TOPICS.init).toBe("init");
    expect(POLICY_EVENT_TOPICS.lmt_set).toBe("lmt_set");
    expect(POLICY_EVENT_TOPICS.spent).toBe("spent");
    expect(POLICY_EVENT_TOPICS.ctr_rst).toBe("ctr_rst");
  });

  it("all topic values are ≤8 characters", () => {
    for (const v of Object.values(POLICY_EVENT_TOPICS)) {
      expect(v.length).toBeLessThanOrEqual(8);
    }
  });
});

describe("parsePolicyEvent — init", () => {
  it("parses init event", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.init, [ADMIN]);
    const result = parsePolicyEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("init");
    expect((result as any)?.admin).toBe(ADMIN);
  });
});

describe("parsePolicyEvent — lmt_set", () => {
  it("parses lmt_set event", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.lmt_set, [
      WALLET,
      LIMIT,
      DAY_LEDGERS,
    ]);
    const result = parsePolicyEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("lmt_set");
    expect((result as any)?.wallet).toBe(WALLET);
  });
});

describe("parsePolicyEvent — spent", () => {
  it("parses spent event", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.spent, [
      WALLET,
      AMOUNT,
    ]);
    const result = parsePolicyEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("spent");
  });
});

describe("parsePolicyEvent — ctr_rst", () => {
  it("parses ctr_rst event", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.ctr_rst, [WALLET]);
    const result = parsePolicyEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("ctr_rst");
  });
});

describe("parsePolicyEvent — fail-closed validation", () => {
  it("returns null for lmt_set with non-address wallet", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.lmt_set, [
      "not-an-address",
      LIMIT,
      DAY_LEDGERS,
    ]);
    expect(parsePolicyEvent(event)).toBeNull();
  });

  it("returns null for spent with non-numeric amount", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.spent, [
      WALLET,
      "abc",
    ]);
    expect(parsePolicyEvent(event)).toBeNull();
  });
});

// ── Stable-ABI literal-value assertions (closes #918) ─────────────────────────

describe("POLICY_CONTRACT_TAG literal value (closes #918)", () => {
  it("equals exactly 'mux_pol'", () => {
    expect(POLICY_CONTRACT_TAG).toBe("mux_pol");
  });
});

describe("POLICY_EVENT_TOPICS literal values (closes #918)", () => {
  it("deep-equals the four documented key/value pairs", () => {
    expect(POLICY_EVENT_TOPICS).toEqual({
      init: "init",
      lmt_set: "lmt_set",
      spent: "spent",
      ctr_rst: "ctr_rst",
    });
  });
});

// ── decodePolicyEvent strict decoder (closes #914) ────────────────────────────

import {
  decodePolicyEvent,
  PolicyEventDecodeError,
  type PolicyRawEvent,
} from "../src/policy-events";

describe("decodePolicyEvent — valid events (closes #914)", () => {
  it("decodes a valid init event with metadata", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "init"],
      value: [ADMIN],
      id: "evt-1",
      ledger: 100,
    };
    const decoded = decodePolicyEvent(raw, { correlationId: "corr-1" });
    expect(decoded.action).toBe("init");
    expect((decoded as any).admin).toBe(ADMIN);
    expect(decoded.correlationId).toBe("corr-1");
    expect(decoded.eventId).toBe("evt-1");
    expect(decoded.ledger).toBe(100);
  });

  it("decodes a valid lmt_set event", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "lmt_set"],
      value: [WALLET, LIMIT, DAY_LEDGERS],
    };
    const decoded = decodePolicyEvent(raw);
    expect(decoded.action).toBe("lmt_set");
    expect((decoded as any).wallet).toBe(WALLET);
    expect((decoded as any).limit).toBe(LIMIT);
    expect((decoded as any).dayLedgers).toBe(DAY_LEDGERS);
  });

  it("decodes a valid spent event", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "spent"],
      value: [WALLET, AMOUNT],
    };
    const decoded = decodePolicyEvent(raw);
    expect(decoded.action).toBe("spent");
    expect((decoded as any).wallet).toBe(WALLET);
    expect((decoded as any).amount).toBe(AMOUNT);
  });

  it("decodes a valid ctr_rst event", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "ctr_rst"],
      value: [WALLET],
    };
    const decoded = decodePolicyEvent(raw);
    expect(decoded.action).toBe("ctr_rst");
    expect((decoded as any).wallet).toBe(WALLET);
  });
});

describe("decodePolicyEvent — error cases (closes #914)", () => {
  it("throws PolicyEventDecodeError for an unknown action under mux_pol", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "fee_charged"],
      value: [ADMIN],
    };
    expect(() => decodePolicyEvent(raw)).toThrow(PolicyEventDecodeError);
    try {
      decodePolicyEvent(raw);
    } catch (err: any) {
      expect(err.name).toBe("PolicyEventDecodeError");
      expect(err.action).toBe("fee_charged");
    }
  });

  it("throws PolicyEventDecodeError for a malformed payload on init (null data)", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "init"],
      value: null,
    };
    expect(() => decodePolicyEvent(raw)).toThrow(PolicyEventDecodeError);
  });

  it("throws PolicyEventDecodeError for a malformed payload on lmt_set (missing fields)", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "lmt_set"],
      value: [WALLET], // missing limit and dayLedgers
    };
    expect(() => decodePolicyEvent(raw)).toThrow(PolicyEventDecodeError);
  });

  it("throws PolicyEventDecodeError when topics[0] is not mux_pol", () => {
    const raw: PolicyRawEvent = {
      topic: ["mux_bat", "init"],
      value: [ADMIN],
    };
    expect(() => decodePolicyEvent(raw)).toThrow(PolicyEventDecodeError);
    try {
      decodePolicyEvent(raw);
    } catch (err: any) {
      expect(err.name).toBe("PolicyEventDecodeError");
    }
  });

  it("propagates correlationId on thrown errors", () => {
    const raw: PolicyRawEvent = {
      topic: [POLICY_CONTRACT_TAG, "bad_action"],
      value: [],
    };
    try {
      decodePolicyEvent(raw, { correlationId: "trace-xyz" });
    } catch (err: any) {
      expect(err.correlationId).toBe("trace-xyz");
    }
  });
});

describe("parsePolicyEvent unchanged behaviour after adding decodePolicyEvent (closes #914)", () => {
  it("still returns null for unknown action (never throws)", () => {
    const raw = makeRawEvent(POLICY_CONTRACT_TAG, "fee_charged", [ADMIN]);
    expect(parsePolicyEvent(raw)).toBeNull();
  });

  it("still returns null for malformed payload (never throws)", () => {
    const raw = makeRawEvent(POLICY_CONTRACT_TAG, "lmt_set", [WALLET]);
    expect(parsePolicyEvent(raw)).toBeNull();
  });

  it("still returns null for non-policy tag (never throws)", () => {
    const raw = makeRawEvent("mux_bat", "init", [ADMIN]);
    expect(parsePolicyEvent(raw)).toBeNull();
  });
});
