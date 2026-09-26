import {
  POLICY_CONTRACT_TAG,
  POLICY_EVENT_TOPICS,
  parsePolicyEvent,
  type PolicyEvent,
} from "../src/policy-events";
import type { RawSorobanEvent } from "../src/factory-events";

const ADMIN = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";
const WALLET = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M";
const LIMIT = "1000000000";
const AMOUNT = "500000000";
const DAY_LEDGERS = "17280";

function makeRawEvent(tag: string, action: string, data: unknown): RawSorobanEvent {
  return { topic: [tag, action], value: data };
}

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

describe("parsePolicyEvent — returns null for non-policy events", () => {
  it("returns null for different contract tag", () => {
    const event = makeRawEvent("mux_other", POLICY_EVENT_TOPICS.init, [ADMIN]);
    expect(parsePolicyEvent(event)).toBeNull();
  });

  it("returns null for unknown action", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, "unknown", [ADMIN]);
    expect(parsePolicyEvent(event)).toBeNull();
  });

  it("returns null when data is null", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.init, null);
    expect(parsePolicyEvent(event)).toBeNull();
  });

  it("returns null when required fields are missing", () => {
    const event = makeRawEvent(POLICY_CONTRACT_TAG, POLICY_EVENT_TOPICS.lmt_set, [WALLET]);
    expect(parsePolicyEvent(event)).toBeNull();
  });
});
