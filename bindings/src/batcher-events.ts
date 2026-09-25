/**
 * batcher-events.ts — TypeScript helpers for consuming mux-batcher
 * Soroban events.
 *
 * Every state-mutating batcher entrypoint emits structured contract events
 * with a two-element topic vector:
 *
 *   topics[0]  "mux_bat"   — contract-family tag (Symbol)
 *   topics[1]  action name  — one of BATCHER_EVENT_TOPICS (Symbol)
 *
 * Data payloads per action:
 *
 *   init        →  admin: Address
 *   bat_start   →  (caller: Address, op_count: u32)
 *   executed    →  (caller: Address, success_count: u32, failure_count: u32)
 *   bat_ok      →  (caller: Address, success_count: u32)
 *   bat_abort   →  caller: Address
 *   sim_done    →  (caller: Address, success_count: u32)
 *
 * Two decoding entrypoints are provided:
 *
 *   - {@link decodeBatcherEvent} — strict, fail-closed. Throws a typed
 *     {@link BatcherEventDecodeError} for any malformed topic or payload and
 *     attaches the transaction hash and correlation ID to the result. Use this
 *     for indexers and anything that must not silently drop events.
 *   - {@link parseBatcherEvent} — lenient. Returns `null` instead of throwing,
 *     for callers that only want to filter a mixed event stream.
 *
 * See docs/audit-events.md and docs/event-topic-conventions.md for the
 * canonical event catalog.
 *
 * @module batcher-events
 */

// ── Topic constants ────────────────────────────────────────────────────────────

/**
 * The contract-family tag emitted as `topics[0]` in every mux-batcher
 * event. Use this when constructing Soroban RPC `getEvents` filters.
 */
export const BATCHER_CONTRACT_TAG = "mux_bat" as const;

/**
 * Action names emitted as `topics[1]` in mux-batcher events.
 * Values are stable ABI — renaming requires a breaking-change doc update.
 */
export const BATCHER_EVENT_TOPICS = {
  /** Emitted by `initialize`. */
  init: "init",
  /** Emitted by `execute_batch` at start. */
  bat_start: "bat_start",
  /** Emitted by `execute_batch` on completion. */
  executed: "executed",
  /** Emitted by `execute_batch` with zero failures. */
  bat_ok: "bat_ok",
  /** Emitted by `execute_batch` on require_success failure. */
  bat_abort: "bat_abort",
  /** Emitted by `simulate_batch` on completion. */
  sim_done: "sim_done",
} as const;

export type BatcherEventAction = (typeof BATCHER_EVENT_TOPICS)[keyof typeof BATCHER_EVENT_TOPICS];

// ── Parsed event types ─────────────────────────────────────────────────────────

export interface BatcherInitEvent {
  action: "init";
  admin: string;
}

export interface BatcherBatStartEvent {
  action: "bat_start";
  caller: string;
  opCount: string;
}

export interface BatcherExecutedEvent {
  action: "executed";
  caller: string;
  successCount: string;
  failureCount: string;
}

export interface BatcherBatOkEvent {
  action: "bat_ok";
  caller: string;
  successCount: string;
}

export interface BatcherBatAbortEvent {
  action: "bat_abort";
  caller: string;
}

export interface BatcherSimDoneEvent {
  action: "sim_done";
  caller: string;
  successCount: string;
}

export type BatcherEvent =
  | BatcherInitEvent
  | BatcherBatStartEvent
  | BatcherExecutedEvent
  | BatcherBatOkEvent
  | BatcherBatAbortEvent
  | BatcherSimDoneEvent;

/**
 * Transaction-level metadata attached by {@link decodeBatcherEvent}.
 *
 * A Soroban transaction carries exactly one `InvokeHostFunction` operation,
 * so every event from one `execute_batch` / `simulate_batch` call shares a
 * transaction hash. `correlationId` defaults to that hash, which lets
 * `bat_start` be joined with its `executed` / `bat_ok` / `bat_abort`
 * outcome without scanning contract storage.
 */
export interface BatcherEventMeta {
  /** 64-char lowercase hex transaction hash that emitted the event. */
  txHash: string;
  /** Groups all events of one batch invocation. Defaults to `txHash`. */
  correlationId: string;
  /** Soroban RPC event id (`getEvents` → `id`), when available. */
  eventId?: string;
  /** Ledger sequence the event was emitted in, when available. */
  ledger?: number;
  /** Emitting contract id, when available. */
  contractId?: string;
}

/** A batcher event plus its transaction and correlation metadata. */
export type DecodedBatcherEvent = BatcherEvent & BatcherEventMeta;

// ── Raw Soroban event shape (minimal — avoids importing the full SDK) ──────────

// Import shared RawSorobanEvent from factory-events to avoid duplication
import type { RawSorobanEvent } from "./factory-events";

/**
 * Raw event accepted by {@link decodeBatcherEvent}. A superset of
 * {@link RawSorobanEvent}: topics may be plain strings or ScVal-style
 * `{ symbol: "..." }` objects, and the optional RPC fields are used for
 * correlation.
 */
export interface BatcherRawEvent {
  topic: readonly unknown[];
  value: unknown;
  id?: string;
  txHash?: string;
  ledger?: number;
  contractId?: string;
}

/**
 * Caller-supplied metadata. Values here take precedence over the matching
 * fields on the raw event.
 */
export interface BatcherEventContext {
  txHash?: string;
  correlationId?: string;
  eventId?: string;
  ledger?: number;
  contractId?: string;
}

// ── Errors ─────────────────────────────────────────────────────────────────────

/**
 * Stable error codes raised by {@link decodeBatcherEvent}.
 *
 *   NotBatcherEvent   topics[0] is not `mux_bat` — the event belongs to
 *                     another contract; stream decoders skip it.
 *   MalformedTopics   topic vector is not exactly `[Symbol, Symbol]`.
 *   UnknownAction     topics[1] is not a documented batcher action.
 *   MalformedPayload  data is missing or has the wrong arity for the action.
 *   InvalidAddress    an Address field is not a G…/C… strkey.
 *   InvalidU32        a u32 field is not an integer in [0, 2^32 - 1].
 *   MissingTxHash     no transaction hash on the event or in the context.
 *   InvalidTxHash     transaction hash is not 64 hex characters.
 */
export const BATCHER_EVENT_DECODE_ERROR_CODES = {
  NotBatcherEvent: "NotBatcherEvent",
  MalformedTopics: "MalformedTopics",
  UnknownAction: "UnknownAction",
  MalformedPayload: "MalformedPayload",
  InvalidAddress: "InvalidAddress",
  InvalidU32: "InvalidU32",
  MissingTxHash: "MissingTxHash",
  InvalidTxHash: "InvalidTxHash",
} as const;

export type BatcherEventDecodeErrorCode =
  (typeof BATCHER_EVENT_DECODE_ERROR_CODES)[keyof typeof BATCHER_EVENT_DECODE_ERROR_CODES];

/**
 * Typed error thrown when a batcher event cannot be decoded.
 *
 * Fail-closed: a corrupted or unexpected event log is surfaced to the caller
 * rather than being dropped or partially decoded.
 */
export class BatcherEventDecodeError extends Error {
  readonly code: BatcherEventDecodeErrorCode;
  /** Action from topics[1], when it could be read. */
  readonly action?: string;
  /** Payload field that failed validation, e.g. `"caller"`. */
  readonly field?: string;
  readonly txHash?: string;
  readonly eventId?: string;
  readonly correlationId?: string;

  constructor(
    code: BatcherEventDecodeErrorCode,
    message: string,
    options: {
      action?: string;
      field?: string;
      txHash?: string;
      eventId?: string;
      correlationId?: string;
    } = {},
  ) {
    super(message);
    this.name = "BatcherEventDecodeError";
    this.code = code;
    this.action = options.action;
    this.field = options.field;
    this.txHash = options.txHash;
    this.eventId = options.eventId;
    this.correlationId = options.correlationId;
  }
}

// ── Strict decoder ─────────────────────────────────────────────────────────────

/**
 * Decode a raw Soroban RPC event into a typed {@link DecodedBatcherEvent}.
 *
 * Fail-closed: throws {@link BatcherEventDecodeError} when the event is not a
 * batcher event, the topics or payload are malformed, or no valid
 * transaction hash is available.
 *
 * @example
 * ```ts
 * import { decodeBatcherEvent, BatcherEventDecodeError } from "./batcher-events";
 *
 * for (const raw of rpcResponse.events) {
 *   try {
 *     const event = decodeBatcherEvent(raw);
 *     index.add(event.correlationId, event);
 *   } catch (err) {
 *     if (err instanceof BatcherEventDecodeError && err.code === "NotBatcherEvent") continue;
 *     throw err;
 *   }
 * }
 * ```
 */
export function decodeBatcherEvent(
  event: BatcherRawEvent,
  context: BatcherEventContext = {},
): DecodedBatcherEvent {
  const eventId = context.eventId ?? event.id;
  const rawTxHash = context.txHash ?? event.txHash;
  const errCtx = { eventId, txHash: rawTxHash, correlationId: context.correlationId };

  const decoded = decodeBatcherPayload(event, errCtx);

  if (rawTxHash === undefined || rawTxHash === null || rawTxHash === "") {
    throw new BatcherEventDecodeError(
      BATCHER_EVENT_DECODE_ERROR_CODES.MissingTxHash,
      `mux_bat.${decoded.action}: no transaction hash on the event or in the decode context; ` +
        "pass { txHash } so the event can be correlated",
      { ...errCtx, action: decoded.action },
    );
  }
  if (typeof rawTxHash !== "string" || !TX_HASH_RE.test(rawTxHash)) {
    throw new BatcherEventDecodeError(
      BATCHER_EVENT_DECODE_ERROR_CODES.InvalidTxHash,
      `mux_bat.${decoded.action}: transaction hash ${describe(rawTxHash)} is not 64 hex characters`,
      { ...errCtx, action: decoded.action },
    );
  }
  const txHash = rawTxHash.toLowerCase();

  const meta: BatcherEventMeta = {
    txHash,
    correlationId: context.correlationId ?? txHash,
  };
  if (eventId !== undefined) meta.eventId = eventId;
  const ledger = context.ledger ?? event.ledger;
  if (ledger !== undefined) meta.ledger = ledger;
  const contractId = context.contractId ?? event.contractId;
  if (contractId !== undefined) meta.contractId = contractId;

  return { ...decoded, ...meta };
}

/**
 * Decode every batcher event in a mixed RPC event stream.
 *
 * Events from other contracts (`NotBatcherEvent`) are skipped; any malformed
 * batcher event throws. Per-event metadata is taken from each raw event, so
 * pass Soroban RPC `getEvents` records directly.
 */
export function decodeBatcherEvents(
  events: readonly BatcherRawEvent[],
): DecodedBatcherEvent[] {
  const out: DecodedBatcherEvent[] = [];
  for (const event of events) {
    try {
      out.push(decodeBatcherEvent(event));
    } catch (err) {
      if (
        err instanceof BatcherEventDecodeError &&
        err.code === BATCHER_EVENT_DECODE_ERROR_CODES.NotBatcherEvent
      ) {
        continue;
      }
      throw err;
    }
  }
  return out;
}

/**
 * Group decoded events by `correlationId`, preserving input order within each
 * group. A complete `execute_batch` group reads `bat_start` → `executed`
 * (→ `bat_ok`), or `bat_start` → `bat_abort`.
 */
export function correlateBatcherEvents(
  events: readonly DecodedBatcherEvent[],
): Map<string, DecodedBatcherEvent[]> {
  const groups = new Map<string, DecodedBatcherEvent[]>();
  for (const event of events) {
    const group = groups.get(event.correlationId);
    if (group) group.push(event);
    else groups.set(event.correlationId, [event]);
  }
  return groups;
}

// ── Lenient parser ─────────────────────────────────────────────────────────────

/**
 * Parse a raw Soroban RPC event into a typed {@link BatcherEvent}.
 *
 * Returns `null` when the event does not match the batcher's contract tag,
 * when the action is unrecognised, or when the payload is malformed. Use
 * {@link decodeBatcherEvent} when malformed events must not be dropped.
 *
 * @example
 * ```ts
 * import { parseBatcherEvent, BATCHER_CONTRACT_TAG } from "./batcher-events";
 *
 * const rawEvents = await server.getEvents({ startLedger, filters: [...] });
 * const batcherEvents = rawEvents.records
 *   .map(parseBatcherEvent)
 *   .filter((e): e is BatcherEvent => e !== null);
 * ```
 */
export function parseBatcherEvent(event: RawSorobanEvent): BatcherEvent | null {
  try {
    return decodeBatcherPayload(event, {});
  } catch (err) {
    if (err instanceof BatcherEventDecodeError) return null;
    throw err;
  }
}

// ── Internal helpers ───────────────────────────────────────────────────────────

type ErrCtx = { eventId?: string; txHash?: string; correlationId?: string };

/** Payload arity per action, mirroring contracts/mux-batcher emit sites. */
const PAYLOAD_ARITY: Record<BatcherEventAction, number> = {
  init: 1,
  bat_start: 2,
  executed: 3,
  bat_ok: 2,
  bat_abort: 1,
  sim_done: 2,
};

const KNOWN_ACTIONS = new Set<string>(Object.values(BATCHER_EVENT_TOPICS));
const U32_MAX = 0xffff_ffff;
// Soroban `Address` is an account (G…) or contract (C…) strkey.
const ADDRESS_RE = /^[GC][A-Z2-7]{55}$/;
const TX_HASH_RE = /^[0-9a-fA-F]{64}$/;

function decodeBatcherPayload(
  event: { topic?: readonly unknown[] | null; value: unknown },
  errCtx: ErrCtx,
): BatcherEvent {
  const topics: readonly unknown[] = Array.isArray(event.topic) ? event.topic : [];
  const tag = resolveSymbol(topics[0]);

  if (tag !== BATCHER_CONTRACT_TAG) {
    throw new BatcherEventDecodeError(
      BATCHER_EVENT_DECODE_ERROR_CODES.NotBatcherEvent,
      `event topic[0] ${describe(topics[0])} is not "${BATCHER_CONTRACT_TAG}"`,
      errCtx,
    );
  }
  if (topics.length !== 2) {
    throw new BatcherEventDecodeError(
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedTopics,
      `mux_bat event must have exactly 2 topics [tag, action]; got ${topics.length}`,
      errCtx,
    );
  }

  const action = resolveSymbol(topics[1]);
  if (action === null) {
    throw new BatcherEventDecodeError(
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedTopics,
      `mux_bat event topic[1] ${describe(topics[1])} is not a Symbol`,
      errCtx,
    );
  }
  if (!KNOWN_ACTIONS.has(action)) {
    throw new BatcherEventDecodeError(
      BATCHER_EVENT_DECODE_ERROR_CODES.UnknownAction,
      `mux_bat action "${action}" is not one of: ${[...KNOWN_ACTIONS].join(", ")}`,
      { ...errCtx, action },
    );
  }

  const known = action as BatcherEventAction;
  const data = normaliseData(event.value, PAYLOAD_ARITY[known]);
  if (data === null || data.length !== PAYLOAD_ARITY[known]) {
    throw new BatcherEventDecodeError(
      BATCHER_EVENT_DECODE_ERROR_CODES.MalformedPayload,
      `mux_bat.${action} expects ${PAYLOAD_ARITY[known]} payload value(s); got ` +
        (data === null ? describe(event.value) : `${data.length}`),
      { ...errCtx, action },
    );
  }

  const address = (i: number, field: string) => requireAddress(data[i], action, field, errCtx);
  const u32 = (i: number, field: string) => requireU32(data[i], action, field, errCtx);

  switch (known) {
    case "init":
      return { action: known, admin: address(0, "admin") };
    case "bat_start":
      return { action: known, caller: address(0, "caller"), opCount: u32(1, "opCount") };
    case "executed":
      return {
        action: known,
        caller: address(0, "caller"),
        successCount: u32(1, "successCount"),
        failureCount: u32(2, "failureCount"),
      };
    case "bat_ok":
      return { action: known, caller: address(0, "caller"), successCount: u32(1, "successCount") };
    case "bat_abort":
      return { action: known, caller: address(0, "caller") };
    case "sim_done":
      return { action: known, caller: address(0, "caller"), successCount: u32(1, "successCount") };
  }
}

/**
 * Normalise the event data into a value list. Tuple payloads arrive as a
 * `Vec`; single-value payloads (`init`, `bat_abort`) arrive as a bare
 * scalar, which is wrapped when `arity` is 1.
 */
function normaliseData(raw: unknown, arity: number): unknown[] | null {
  if (raw === null || raw === undefined) return null;
  if (Array.isArray(raw)) return raw as unknown[];

  if (typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (Array.isArray(obj["vec"])) return obj["vec"] as unknown[];
    if (Array.isArray(obj["_value"])) return obj["_value"] as unknown[];
  }

  if (typeof raw === "string") {
    try {
      const parsed: unknown = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed as unknown[];
    } catch {
      // Not JSON — fall through to scalar handling.
    }
  }

  return arity === 1 ? [raw] : null;
}

function resolveSymbol(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["symbol"] === "string") return obj["symbol"];
    if (typeof obj["sym"] === "string") return obj["sym"];
  }
  return null;
}

function requireAddress(raw: unknown, action: string, field: string, errCtx: ErrCtx): string {
  let value: unknown = raw;
  if (raw && typeof raw === "object") value = (raw as Record<string, unknown>)["address"];
  if (typeof value === "string" && ADDRESS_RE.test(value)) return value;
  throw new BatcherEventDecodeError(
    BATCHER_EVENT_DECODE_ERROR_CODES.InvalidAddress,
    `mux_bat.${action}.${field}: ${describe(raw)} is not a G…/C… strkey Address`,
    { ...errCtx, action, field },
  );
}

function requireU32(raw: unknown, action: string, field: string, errCtx: ErrCtx): string {
  let value: unknown = raw;
  if (raw && typeof raw === "object") value = (raw as Record<string, unknown>)["u32"];

  let n: number | null = null;
  if (typeof value === "number" && Number.isInteger(value)) n = value;
  else if (typeof value === "bigint" && value <= BigInt(U32_MAX)) n = Number(value);
  else if (typeof value === "string" && /^\d{1,10}$/.test(value)) n = Number(value);

  if (n !== null && n >= 0 && n <= U32_MAX) return String(n);
  throw new BatcherEventDecodeError(
    BATCHER_EVENT_DECODE_ERROR_CODES.InvalidU32,
    `mux_bat.${action}.${field}: ${describe(raw)} is not a u32`,
    { ...errCtx, action, field },
  );
}

function describe(value: unknown): string {
  if (value === undefined) return "undefined";
  if (typeof value === "bigint") return `${value}n`;
  try {
    const json = JSON.stringify(value);
    if (json === undefined) return String(value);
    return json.length > 80 ? `${json.slice(0, 77)}...` : json;
  } catch {
    return String(value);
  }
}
