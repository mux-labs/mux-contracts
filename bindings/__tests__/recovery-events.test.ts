import {
  RECOVERY_CONTRACT_TAG,
  RECOVERY_EVENT_TOPICS,
  parseRecoveryEvent,
  type RecoveryEvent,
} from "../src/recovery-events";
import type { RawSorobanEvent } from "../src/factory-events";

const ADMIN = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";
const GUARDIAN = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M";
const NEW_OWNER = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAYG6C";
const REGISTRY_ID = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJPQE";
const INITIATED_AT = "1234567890";
const EXECUTABLE_AT = "1234571490";
const EXPIRES_AT = "1234671490";

function makeRawEvent(tag: string, action: string, data: unknown): RawSorobanEvent {
  return { topic: [tag, action], value: data };
}

describe("RECOVERY_CONTRACT_TAG", () => {
  it("equals 'mux_recv'", () => {
    expect(RECOVERY_CONTRACT_TAG).toBe("mux_recv");
  });
});

describe("RECOVERY_EVENT_TOPICS", () => {
  it("contains all documented actions", () => {
    expect(RECOVERY_EVENT_TOPICS.init).toBe("init");
    expect(RECOVERY_EVENT_TOPICS.rec_init).toBe("rec_init");
    expect(RECOVERY_EVENT_TOPICS.rec_exec).toBe("rec_exec");
    expect(RECOVERY_EVENT_TOPICS.rec_adm).toBe("rec_adm");
    expect(RECOVERY_EVENT_TOPICS.rec_cncl).toBe("rec_cncl");
    expect(RECOVERY_EVENT_TOPICS.grd_add).toBe("grd_add");
    expect(RECOVERY_EVENT_TOPICS.grd_rm).toBe("grd_rm");
    expect(RECOVERY_EVENT_TOPICS.reg_link).toBe("reg_link");
  });

  it("all topic values are ≤8 characters", () => {
    for (const v of Object.values(RECOVERY_EVENT_TOPICS)) {
      expect(v.length).toBeLessThanOrEqual(8);
    }
  });
});

describe("parseRecoveryEvent — init", () => {
  it("parses init event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.init, [ADMIN]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("init");
    expect((result as any)?.admin).toBe(ADMIN);
  });
});

describe("parseRecoveryEvent — rec_init", () => {
  it("parses rec_init event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      GUARDIAN,
      NEW_OWNER,
      INITIATED_AT,
      EXECUTABLE_AT,
      EXPIRES_AT,
    ]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_init");
    expect((result as any)?.guardian).toBe(GUARDIAN);
    expect((result as any)?.newOwner).toBe(NEW_OWNER);
  });
});

describe("parseRecoveryEvent — rec_exec", () => {
  it("parses rec_exec event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_exec, [
      GUARDIAN,
      NEW_OWNER,
    ]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_exec");
  });
});

describe("parseRecoveryEvent — rec_adm", () => {
  it("parses rec_adm event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_adm, [NEW_OWNER]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_adm");
  });
});

describe("parseRecoveryEvent — rec_cncl", () => {
  it("parses rec_cncl event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_cncl, []);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_cncl");
  });
});

describe("parseRecoveryEvent — grd_add", () => {
  it("parses grd_add event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.grd_add, [GUARDIAN]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("grd_add");
  });
});

describe("parseRecoveryEvent — grd_rm", () => {
  it("parses grd_rm event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.grd_rm, [GUARDIAN]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("grd_rm");
  });
});

describe("parseRecoveryEvent — reg_link", () => {
  it("parses reg_link event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.reg_link, [
      REGISTRY_ID,
    ]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("reg_link");
  });
});

describe("parseRecoveryEvent — returns null for non-recovery events", () => {
  it("returns null for different contract tag", () => {
    const event = makeRawEvent("mux_other", RECOVERY_EVENT_TOPICS.init, [ADMIN]);
    expect(parseRecoveryEvent(event)).toBeNull();
  });

  it("returns null for unknown action", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, "unknown", [ADMIN]);
    expect(parseRecoveryEvent(event)).toBeNull();
  });

  it("returns null when data is null", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.init, null);
    expect(parseRecoveryEvent(event)).toBeNull();
  });

  it("returns null when address is missing", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.init, []);
    expect(parseRecoveryEvent(event)).toBeNull();
  });
});
