import {
  RECOVERY_CONTRACT_TAG,
  RECOVERY_EVENT_TOPICS,
  RecoveryEventDecodeError,
  parseRecoveryEvent,
  type RecoveryEvent,
  type RecoveryRecInitEvent,
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

  it("decodes u64 fields above Number.MAX_SAFE_INTEGER without precision loss (#937)", () => {
    const initiatedAt = "9007199254740993"; // 2^53 + 1
    const executableAt = "9007199254741000"; // initiatedAt + timelock-ish
    const expiresAt = "18446744073709551615"; // u64::MAX
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      GUARDIAN,
      NEW_OWNER,
      initiatedAt,
      executableAt,
      expiresAt,
    ]);
    const result = parseRecoveryEvent(event) as RecoveryRecInitEvent | null;
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_init");
    expect(result?.initiatedAt).toBe(initiatedAt);
    expect(result?.executableAt).toBe(executableAt);
    expect(result?.expiresAt).toBe(expiresAt);
  });

  it("decodes { u64 } object-shaped u64 fields above Number.MAX_SAFE_INTEGER (#937)", () => {
    const initiatedAt = "9007199254740993";
    const executableAt = "9007199254741000";
    const expiresAt = "9007199254750000";
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      GUARDIAN,
      NEW_OWNER,
      { u64: initiatedAt },
      { u64: executableAt },
      { u64: expiresAt },
    ]);
    const result = parseRecoveryEvent(event) as RecoveryRecInitEvent | null;
    expect(result?.initiatedAt).toBe(initiatedAt);
    expect(result?.executableAt).toBe(executableAt);
    expect(result?.expiresAt).toBe(expiresAt);
  });

  it("throws RecoveryEventDecodeError for an undecodable u64 field (#937)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      GUARDIAN,
      NEW_OWNER,
      "not-a-u64",
      EXECUTABLE_AT,
      EXPIRES_AT,
    ]);
    expect(() => parseRecoveryEvent(event)).toThrow(RecoveryEventDecodeError);
  });

  it("throws RecoveryEventDecodeError for an unsafe number u64 field (#937)", () => {
    // A JS number above 2^53 cannot represent the u64 exactly; it must fail
    // loudly instead of yielding a silently truncated ledger timestamp.
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      GUARDIAN,
      NEW_OWNER,
      2 ** 60,
      EXECUTABLE_AT,
      EXPIRES_AT,
    ]);
    expect(() => parseRecoveryEvent(event)).toThrow(RecoveryEventDecodeError);
  });

  it("throws RecoveryEventDecodeError when guardian is not a well-formed address (regression)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      "not-an-address",
      NEW_OWNER,
      EXECUTABLE_AT,
      EXECUTABLE_AT,
      EXPIRES_AT,
    ]);
    expect(() => parseRecoveryEvent(event)).toThrow(RecoveryEventDecodeError);
  });

  it("throws RecoveryEventDecodeError when newOwner is not a well-formed address (regression)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      GUARDIAN,
      "not-an-address",
      EXECUTABLE_AT,
      EXECUTABLE_AT,
      EXPIRES_AT,
    ]);
    expect(() => parseRecoveryEvent(event)).toThrow(RecoveryEventDecodeError);
  });

  it("throws RecoveryEventDecodeError for a missing payload (#937)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, null);
    expect(() => parseRecoveryEvent(event)).toThrow(RecoveryEventDecodeError);
  });

  it("throws RecoveryEventDecodeError for a short payload (#937)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      GUARDIAN,
      NEW_OWNER,
      INITIATED_AT,
    ]);
    expect(() => parseRecoveryEvent(event)).toThrow(RecoveryEventDecodeError);
  });

  it("throws RecoveryEventDecodeError for an invalid guardian address (#937)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, [
      "not-an-address",
      NEW_OWNER,
      INITIATED_AT,
      EXECUTABLE_AT,
      EXPIRES_AT,
    ]);
    expect(() => parseRecoveryEvent(event)).toThrow(RecoveryEventDecodeError);
  });

  it("names the action on the thrown decode error (#937)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_init, []);
    try {
      parseRecoveryEvent(event);
      expect.unreachable("parseRecoveryEvent should have thrown");
    } catch (err) {
      expect(err).toBeInstanceOf(RecoveryEventDecodeError);
      expect((err as RecoveryEventDecodeError).action).toBe(RECOVERY_EVENT_TOPICS.rec_init);
      expect((err as Error).message).toMatch(/rec_init/);
    }
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

  it("rejects rec_exec when guardian is not a well-formed address (regression)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_exec, [
      "not-an-address",
      NEW_OWNER,
    ]);
    expect(parseRecoveryEvent(event)).toBeNull();
  });

  it("rejects rec_exec when newOwner is not a well-formed address (regression)", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_exec, [
      GUARDIAN,
      "not-an-address",
    ]);
    expect(parseRecoveryEvent(event)).toBeNull();
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

describe("parseRecoveryEvent — cancel/advance audit semantics (#758)", () => {
  it("rec_cncl carries the cancelling actor for audit correlation", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_cncl, [GUARDIAN]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_cncl");
    expect((result as any)?.guardian).toBe(GUARDIAN);
  });

  it("rec_exec (advance) carries guardian and new owner for audit correlation", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_exec, [
      GUARDIAN,
      NEW_OWNER,
    ]);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_exec");
    expect((result as any)?.guardian).toBe(GUARDIAN);
    expect((result as any)?.newOwner).toBe(NEW_OWNER);
  });

  it("rec_cncl with no actor is still a valid audit event", () => {
    const event = makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_cncl, []);
    const result = parseRecoveryEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("rec_cncl");
  });

  it("rec_cncl and rec_exec are distinct audit actions", () => {
    const cancel = parseRecoveryEvent(
      makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_cncl, [GUARDIAN]),
    );
    const advance = parseRecoveryEvent(
      makeRawEvent(RECOVERY_CONTRACT_TAG, RECOVERY_EVENT_TOPICS.rec_exec, [GUARDIAN, NEW_OWNER]),
    );
    expect(cancel?.action).not.toBe(advance?.action);
  });
});
