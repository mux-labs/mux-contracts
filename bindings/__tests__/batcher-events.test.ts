import {
  BATCHER_CONTRACT_TAG,
  BATCHER_EVENT_DECODE_ERROR_CODES,
  BATCHER_EVENT_TOPICS,
  BatcherEventDecodeError,
  correlateBatcherEvents,
  decodeBatcherEvent,
  decodeBatcherEvents,
  parseBatcherEvent,
  type BatcherEvent,
  type BatcherEventDecodeErrorCode,
  type BatcherRawEvent,
} from "../src/batcher-events";
import type { RawSorobanEvent } from "../src/factory-events";

const ADMIN = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";
const CALLER = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M";
const OP_COUNT = "5";
const SUCCESS_COUNT = "4";
const FAILURE_COUNT = "1";

function makeRawEvent(tag: string, action: string, data: unknown): RawSorobanEvent {
  return { topic: [tag, action], value: data };
}

describe("BATCHER_CONTRACT_TAG", () => {
  it("equals 'mux_bat'", () => {
    expect(BATCHER_CONTRACT_TAG).toBe("mux_bat");
  });
});

describe("BATCHER_EVENT_TOPICS", () => {
  it("contains all documented actions", () => {
    expect(BATCHER_EVENT_TOPICS.init).toBe("init");
    expect(BATCHER_EVENT_TOPICS.bat_start).toBe("bat_start");
    expect(BATCHER_EVENT_TOPICS.executed).toBe("executed");
    expect(BATCHER_EVENT_TOPICS.bat_ok).toBe("bat_ok");
    expect(BATCHER_EVENT_TOPICS.bat_abort).toBe("bat_abort");
    expect(BATCHER_EVENT_TOPICS.sim_done).toBe("sim_done");
  });

  it("all topic values are valid Soroban action names", () => {
    for (const v of Object.values(BATCHER_EVENT_TOPICS)) {
      expect(typeof v).toBe("string");
      expect(v.length).toBeGreaterThan(0);
    }
  });
});

describe("parseBatcherEvent — init", () => {
  it("parses init event", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.init, [ADMIN]);
    const result = parseBatcherEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("init");
    expect((result as any)?.admin).toBe(ADMIN);
  });
});

describe("parseBatcherEvent — bat_start", () => {
  it("parses bat_start event", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.bat_start, [
      CALLER,
      OP_COUNT,
    ]);
    const result = parseBatcherEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("bat_start");
    expect((result as any)?.caller).toBe(CALLER);
  });
});

describe("parseBatcherEvent — executed", () => {
  it("parses executed event", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.executed, [
      CALLER,
      SUCCESS_COUNT,
      FAILURE_COUNT,
    ]);
    const result = parseBatcherEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("executed");
  });
});

describe("parseBatcherEvent — bat_ok", () => {
  it("parses bat_ok event", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.bat_ok, [
      CALLER,
      SUCCESS_COUNT,
    ]);
    const result = parseBatcherEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("bat_ok");
  });
});

describe("parseBatcherEvent — bat_abort", () => {
  it("parses bat_abort event", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.bat_abort, [CALLER]);
    const result = parseBatcherEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("bat_abort");
  });
});

describe("parseBatcherEvent — sim_done", () => {
  it("parses sim_done event", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.sim_done, [
      CALLER,
      SUCCESS_COUNT,
    ]);
    const result = parseBatcherEvent(event);
    expect(result).not.toBeNull();
    expect(result?.action).toBe("sim_done");
  });
});

describe("parseBatcherEvent — returns null for non-batcher events", () => {
  it("returns null for different contract tag", () => {
    const event = makeRawEvent("mux_other", BATCHER_EVENT_TOPICS.init, [ADMIN]);
    expect(parseBatcherEvent(event)).toBeNull();
  });

  it("returns null for unknown action", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, "unknown", [ADMIN]);
    expect(parseBatcherEvent(event)).toBeNull();
  });

  it("returns null when data is null", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.init, null);
    expect(parseBatcherEvent(event)).toBeNull();
  });

  it("returns null when required fields are missing", () => {
    const event = makeRawEvent(BATCHER_CONTRACT_TAG, BATCHER_EVENT_TOPICS.executed, [CALLER]);
    expect(parseBatcherEvent(event)).toBeNull();
  });
});

// ── Strict decoder (#793) ──────────────────────────────────────────────────────

const ACCOUNT_CALLER = `G${"A".repeat(55)}`;
const TX_A = "a".repeat(64);
const TX_B = "B".repeat(64);
const TX_B_LOWER = "b".repeat(64);

function rpcEvent(
  action: unknown,
  value: unknown,
  extra: Partial<BatcherRawEvent> = {},
): BatcherRawEvent {
  return { topic: [BATCHER_CONTRACT_TAG, action], value, txHash: TX_A, ...extra };
}

function expectDecodeError(
  fn: () => unknown,
  code: BatcherEventDecodeErrorCode,
  fields: { action?: string; field?: string } = {},
): BatcherEventDecodeError {
  let caught: unknown;
  try {
    fn();
  } catch (err) {
    caught = err;
  }
  expect(caught).toBeInstanceOf(BatcherEventDecodeError);
  const err = caught as BatcherEventDecodeError;
  expect(err.name).toBe("BatcherEventDecodeError");
  expect(err.code).toBe(code);
  expect(err.message.length).toBeGreaterThan(0);
  if (fields.action !== undefined) expect(err.action).toBe(fields.action);
  if (fields.field !== undefined) expect(err.field).toBe(fields.field);
  return err;
}

describe("decodeBatcherEvent — happy paths", () => {
  it("decodes every action with typed fields and tx metadata", () => {
    const cases: Array<[string, unknown, BatcherEvent]> = [
      ["init", ADMIN, { action: "init", admin: ADMIN }],
      ["bat_start", [CALLER, 5], { action: "bat_start", caller: CALLER, opCount: "5" }],
      [
        "executed",
        [CALLER, 4, 1],
        { action: "executed", caller: CALLER, successCount: "4", failureCount: "1" },
      ],
      ["bat_ok", [CALLER, 4], { action: "bat_ok", caller: CALLER, successCount: "4" }],
      ["bat_abort", CALLER, { action: "bat_abort", caller: CALLER }],
      ["sim_done", [CALLER, 3], { action: "sim_done", caller: CALLER, successCount: "3" }],
    ];
    for (const [action, value, expected] of cases) {
      expect(decodeBatcherEvent(rpcEvent(action, value))).toEqual({
        ...expected,
        txHash: TX_A,
        correlationId: TX_A,
      });
    }
  });

  it("accepts bare scalar and single-element payloads for single-value events", () => {
    expect(decodeBatcherEvent(rpcEvent("init", ADMIN))).toMatchObject({ admin: ADMIN });
    expect(decodeBatcherEvent(rpcEvent("bat_abort", [CALLER]))).toMatchObject({ caller: CALLER });
  });

  it("accepts ScVal-style topics and payload wrappers", () => {
    const event: BatcherRawEvent = {
      topic: [{ symbol: BATCHER_CONTRACT_TAG }, { symbol: "executed" }],
      value: { vec: [{ address: CALLER }, { u32: 2 }, { u32: "0" }] },
      txHash: TX_A,
    };
    expect(decodeBatcherEvent(event)).toMatchObject({
      action: "executed",
      caller: CALLER,
      successCount: "2",
      failureCount: "0",
    });
  });

  it("accepts account (G…) callers and bigint / u32-max counts", () => {
    const decoded = decodeBatcherEvent(rpcEvent("bat_start", [ACCOUNT_CALLER, 4294967295n]));
    expect(decoded).toMatchObject({ caller: ACCOUNT_CALLER, opCount: "4294967295" });
  });

  it("carries event id, ledger, and contract id from the RPC record", () => {
    const decoded = decodeBatcherEvent(
      rpcEvent("bat_ok", [CALLER, 1], { id: "0000123-0001", ledger: 123, contractId: ADMIN }),
    );
    expect(decoded).toMatchObject({ eventId: "0000123-0001", ledger: 123, contractId: ADMIN });
  });

  it("normalises the tx hash to lowercase and lets context override it", () => {
    expect(decodeBatcherEvent(rpcEvent("bat_abort", CALLER, { txHash: TX_B })).txHash).toBe(
      TX_B_LOWER,
    );
    const decoded = decodeBatcherEvent(rpcEvent("bat_abort", CALLER), {
      txHash: TX_B,
      correlationId: "batch-42",
    });
    expect(decoded.txHash).toBe(TX_B_LOWER);
    expect(decoded.correlationId).toBe("batch-42");
  });
});

describe("decodeBatcherEvent — negative paths (fail-closed)", () => {
  it("rejects events from another contract", () => {
    expectDecodeError(
      () => decodeBatcherEvent({ topic: ["mux_pol", "init"], value: ADMIN, txHash: TX_A }),
      BATCHER_EVENT_DECODE_ERROR_CODES.NotBatcherEvent,
    );
    expectDecodeError(
      () => decodeBatcherEvent({ topic: [], value: ADMIN, txHash: TX_A }),
      BATCHER_EVENT_DECODE_ERROR_CODES.NotBatcherEvent,
    );
  });

  it("rejects wrong topic arity", () => {
    expectDecodeError(
      () => decodeBatcherEvent({ topic: [BATCHER_CONTRACT_TAG], value: ADMIN, txHash: TX_A }),
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedTopics,
    );
    expectDecodeError(
      () =>
        decodeBatcherEvent({
          topic: [BATCHER_CONTRACT_TAG, "init", "extra"],
          value: ADMIN,
          txHash: TX_A,
        }),
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedTopics,
    );
  });

  it("rejects a non-symbol action topic", () => {
    expectDecodeError(
      () => decodeBatcherEvent(rpcEvent(42, ADMIN)),
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedTopics,
    );
  });

  it("rejects undocumented actions", () => {
    expectDecodeError(
      () => decodeBatcherEvent(rpcEvent("fee_paid", [CALLER, 1])),
      BATCHER_EVENT_DECODE_ERROR_CODES.UnknownAction,
      { action: "fee_paid" },
    );
  });

  it("rejects missing or wrong-arity payloads", () => {
    for (const value of [null, undefined, [], [CALLER], [CALLER, 1, 2, 3], "not-json"]) {
      expectDecodeError(
        () => decodeBatcherEvent(rpcEvent("executed", value)),
        BATCHER_EVENT_DECODE_ERROR_CODES.MalformedPayload,
        { action: "executed" },
      );
    }
    expectDecodeError(
      () => decodeBatcherEvent(rpcEvent("init", [ADMIN, ADMIN])),
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedPayload,
    );
  });

  it("rejects corrupted addresses", () => {
    const bad: unknown[] = ["", "C123", CALLER.toLowerCase(), `M${CALLER.slice(1)}`, 7, { address: 1 }];
    for (const caller of bad) {
      expectDecodeError(
        () => decodeBatcherEvent(rpcEvent("bat_abort", [caller])),
        BATCHER_EVENT_DECODE_ERROR_CODES.InvalidAddress,
        { action: "bat_abort", field: "caller" },
      );
    }
    expectDecodeError(
      () => decodeBatcherEvent(rpcEvent("init", "garbage")),
      BATCHER_EVENT_DECODE_ERROR_CODES.InvalidAddress,
      { field: "admin" },
    );
  });

  it("rejects out-of-range or non-integer u32 values", () => {
    const bad: unknown[] = [
      -1,
      1.5,
      4294967296,
      "4294967296",
      "12abc",
      "-3",
      NaN,
      2n ** 40n,
      { u32: -1 },
      null,
    ];
    for (const count of bad) {
      expectDecodeError(
        () => decodeBatcherEvent(rpcEvent("sim_done", [CALLER, count])),
        BATCHER_EVENT_DECODE_ERROR_CODES.InvalidU32,
        { action: "sim_done", field: "successCount" },
      );
    }
    expectDecodeError(
      () => decodeBatcherEvent(rpcEvent("executed", [CALLER, 1, "x"])),
      BATCHER_EVENT_DECODE_ERROR_CODES.InvalidU32,
      { field: "failureCount" },
    );
  });

  it("requires a transaction hash for correlation", () => {
    expectDecodeError(
      () => decodeBatcherEvent({ topic: [BATCHER_CONTRACT_TAG, "bat_abort"], value: CALLER }),
      BATCHER_EVENT_DECODE_ERROR_CODES.MissingTxHash,
      { action: "bat_abort" },
    );
  });

  it("rejects malformed transaction hashes", () => {
    for (const txHash of ["abc", "g".repeat(64), `${TX_A}0`]) {
      expectDecodeError(
        () => decodeBatcherEvent(rpcEvent("bat_abort", CALLER, { txHash })),
        BATCHER_EVENT_DECODE_ERROR_CODES.InvalidTxHash,
      );
    }
  });

  it("includes event id and tx hash on errors for triage", () => {
    const err = expectDecodeError(
      () => decodeBatcherEvent(rpcEvent("bat_ok", [CALLER, "bad"], { id: "evt-9" })),
      BATCHER_EVENT_DECODE_ERROR_CODES.InvalidU32,
    );
    expect(err.eventId).toBe("evt-9");
    expect(err.txHash).toBe(TX_A);
  });

  it("parseBatcherEvent stays lenient for the same malformed inputs", () => {
    expect(
      parseBatcherEvent({ topic: [BATCHER_CONTRACT_TAG, "sim_done"], value: [CALLER, -1] }),
    ).toBeNull();
    expect(
      parseBatcherEvent({ topic: [BATCHER_CONTRACT_TAG, "bat_abort"], value: "garbage" }),
    ).toBeNull();
  });
});

describe("decodeBatcherEvents / correlateBatcherEvents", () => {
  it("skips foreign events, decodes batcher events, and groups by correlation id", () => {
    const stream: BatcherRawEvent[] = [
      rpcEvent("bat_start", [CALLER, 2]),
      { topic: ["mux_pol", "spent"], value: [CALLER, "1"], txHash: TX_A },
      rpcEvent("bat_start", [ACCOUNT_CALLER, 1], { txHash: TX_B }),
      rpcEvent("executed", [CALLER, 2, 0]),
      rpcEvent("bat_ok", [CALLER, 2]),
      rpcEvent("bat_abort", ACCOUNT_CALLER, { txHash: TX_B }),
    ];
    const decoded = decodeBatcherEvents(stream);
    expect(decoded.map((e) => e.action)).toEqual([
      "bat_start",
      "bat_start",
      "executed",
      "bat_ok",
      "bat_abort",
    ]);

    const groups = correlateBatcherEvents(decoded);
    expect([...groups.keys()]).toEqual([TX_A, TX_B_LOWER]);
    expect(groups.get(TX_A)?.map((e) => e.action)).toEqual(["bat_start", "executed", "bat_ok"]);
    expect(groups.get(TX_B_LOWER)?.map((e) => e.action)).toEqual(["bat_start", "bat_abort"]);
  });

  it("throws on the first malformed batcher event instead of dropping it", () => {
    expectDecodeError(
      () =>
        decodeBatcherEvents([rpcEvent("bat_start", [CALLER, 1]), rpcEvent("executed", [CALLER])]),
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedPayload,
    );
  });
});
