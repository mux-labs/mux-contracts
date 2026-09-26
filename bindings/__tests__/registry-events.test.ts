import {
  REGISTRY_CONTRACT_TAG,
  REGISTRY_EVENT_TOPICS,
  parseRegistryEvent,
  type RegistryEvent,
} from "../src/registry-events";
import type { RawSorobanEvent } from "../src/factory-events";

const ADMIN = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";
const CONTRACT_NAME = "mux_acct";
const VERSION = "1.0.0";

function makeRawEvent(tag: string, action: string, data: unknown): RawSorobanEvent {
  return { topic: [tag, action], value: data };
}

describe("REGISTRY_CONTRACT_TAG", () => {
  it("equals 'mux_reg'", () => {
    expect(REGISTRY_CONTRACT_TAG).toBe("mux_reg");
  });
});

describe("REGISTRY_EVENT_TOPICS", () => {
  it("contains all documented actions", () => {
    expect(REGISTRY_EVENT_TOPICS.init).toBe("init");
    expect(REGISTRY_EVENT_TOPICS.reg).toBe("reg");
    expect(REGISTRY_EVENT_TOPICS.regmeta).toBe("regmeta");
  });

  it("all topic values are ≤8 characters", () => {
    for (const v of Object.values(REGISTRY_EVENT_TOPICS)) {
      expect(v.length).toBeLessThanOrEqual(8);
    }
  });
});

describe("parseRegistryEvent — init", () => {
  it("parses init event", () => {
    const event = makeRawEvent(REGISTRY_CONTRACT_TAG, REGISTRY_EVENT_TOPICS.init, [ADMIN]);
    const result = parseRegistryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("init");
    expect((result as any)?.admin).toBe(ADMIN);
  });
});

describe("parseRegistryEvent — reg", () => {
  it("parses reg event", () => {
    const event = makeRawEvent(REGISTRY_CONTRACT_TAG, REGISTRY_EVENT_TOPICS.reg, [
      CONTRACT_NAME,
      VERSION,
    ]);
    const result = parseRegistryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("reg");
    expect((result as any)?.name).toBe(CONTRACT_NAME);
    expect((result as any)?.version).toBe(VERSION);
  });
});

describe("parseRegistryEvent — regmeta", () => {
  it("parses regmeta event", () => {
    const event = makeRawEvent(REGISTRY_CONTRACT_TAG, REGISTRY_EVENT_TOPICS.regmeta, [
      CONTRACT_NAME,
      VERSION,
    ]);
    const result = parseRegistryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("regmeta");
  });
});

describe("parseRegistryEvent — returns null for non-registry events", () => {
  it("returns null for different contract tag", () => {
    const event = makeRawEvent("mux_other", REGISTRY_EVENT_TOPICS.init, [ADMIN]);
    expect(parseRegistryEvent(event)).toBeNull();
  });

  it("returns null for unknown action", () => {
    const event = makeRawEvent(REGISTRY_CONTRACT_TAG, "unknown", [ADMIN]);
    expect(parseRegistryEvent(event)).toBeNull();
  });

  it("returns null when data is null", () => {
    const event = makeRawEvent(REGISTRY_CONTRACT_TAG, REGISTRY_EVENT_TOPICS.init, null);
    expect(parseRegistryEvent(event)).toBeNull();
  });

  it("returns null when required fields are missing", () => {
    const event = makeRawEvent(REGISTRY_CONTRACT_TAG, REGISTRY_EVENT_TOPICS.reg, [
      CONTRACT_NAME,
    ]);
    expect(parseRegistryEvent(event)).toBeNull();
  });
});
