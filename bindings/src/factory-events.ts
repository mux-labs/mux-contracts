/**
 * factory-events.ts — TypeScript helpers for consuming mux-account-factory
 * Soroban events.
 *
 * Every state-mutating factory entrypoint emits structured contract events
 * with a two-element topic vector:
 *
 *   topics[0]  "mux_fac"    — contract-family tag (Symbol)
 *   topics[1]  action name  — one of FACTORY_EVENT_TOPICS (Symbol)
 *
 * Data payloads per action:
 *
 *   deployed  →  [owner: Address, account_address: Address]
 *   meta_set  →  [owner: Address, account_address: Address, version: String]
 *
 * Read-only / simulate entrypoints emit NO events:
 *   get_accounts, account_count, get_account_metadata,
 *   simulate_deploy, simulate_deploy_with_metadata, max_accounts_per_owner
 *
 * See docs/audit-events.md and docs/event-topic-conventions.md for the
 * canonical event catalog.
 *
 * @module factory-events
 */

// ── Topic constants ────────────────────────────────────────────────────────────

/**
 * The contract-family tag emitted as `topics[0]` in every mux-account-factory
 * event. Use this when constructing Soroban RPC `getEvents` filters.
 *
 * @example
 * ```ts
 * const events = await server.getEvents({
 *   startLedger,
 *   filters: [{
 *     type: "contract",
 *     contractIds: [FACTORY_CONTRACT_ID],
 *     topics: [[FACTORY_CONTRACT_TAG], [FACTORY_EVENT_TOPICS.deployed]],
 *   }],
 * });
 * ```
 */
export const FACTORY_CONTRACT_TAG = "mux_fac" as const;

/**
 * Action names emitted as `topics[1]` in mux-account-factory events.
 * Values are stable ABI — renaming requires a breaking-change doc update.
 */
export const FACTORY_EVENT_TOPICS = {
  /** Emitted by `deploy_account` and `deploy_account_with_metadata`. */
  deployed: "deployed",
  /** Emitted only by `deploy_account_with_metadata`. */
  meta_set: "meta_set",
} as const;

export type FactoryEventAction = (typeof FACTORY_EVENT_TOPICS)[keyof typeof FACTORY_EVENT_TOPICS];

/**
 * Frozen schema version for the factory event catalog. Indexers MUST pin this
 * value and reject events whose {@link FactoryEventEnvelope.schemaVersion}
 * differs, so a future breaking change cannot be silently mis-decoded.
 *
 * Bump only alongside a docs/event-topic-conventions.md breaking-change note.
 */
export const FACTORY_EVENT_SCHEMA_VERSION = 1 as const;

/**
 * Hard maximum number of topic elements accepted for a factory event. The
 * contract emits exactly two (`tag`, `action`); anything larger is treated as
 * adversarial and rejected fail-closed.
 */
export const MAX_FACTORY_EVENT_TOPICS = 2 as const;

/**
 * Hard maximum byte length accepted for a single decoded event field. Guards
 * indexers against oversized/griefing payloads before they are parsed.
 */
export const MAX_FACTORY_EVENT_FIELD_BYTES = 1024 as const;

// ── getAccounts bounds ─────────────────────────────────────────────────────────

/**
 * Hard maximum page size accepted by the factory `get_accounts` entrypoint.
 *
 * The on-chain contract rejects any `limit` above this value with
 * {@link FactoryBoundsErrorCode.LimitTooLarge} rather than silently clamping,
 * so callers cannot accidentally request an unbounded result set. Keep this in
 * sync with the contract constant `MAX_ACCOUNTS_PAGE_SIZE`.
 */
export const MAX_ACCOUNTS_PAGE_SIZE = 100 as const;

/**
 * Stable, typed error codes returned by the factory `get_accounts` bounds
 * validation. These mirror the contract's `Error` enum discriminants so that
 * off-chain callers can branch on a stable code instead of parsing strings.
 */
export const FactoryBoundsErrorCode = {
  /** `offset` was negative or otherwise not a valid u32. */
  InvalidOffset: "InvalidOffset",
  /** `limit` was zero — callers must request at least one account. */
  InvalidLimit: "InvalidLimit",
  /** `limit` exceeded {@link MAX_ACCOUNTS_PAGE_SIZE}. */
  LimitTooLarge: "LimitTooLarge",
} as const;

export type FactoryBoundsErrorCode =
  (typeof FactoryBoundsErrorCode)[keyof typeof FactoryBoundsErrorCode];

/**
 * Typed error thrown by {@link validateGetAccountsBounds} when the requested
 * bounds are invalid or out of range. Carries a stable {@link FactoryBoundsErrorCode}
 * so callers can fail closed without string matching.
 */
export class FactoryBoundsError extends Error {
  readonly code: FactoryBoundsErrorCode;

  constructor(code: FactoryBoundsErrorCode, message?: string) {
    super(message ?? code);
    this.name = "FactoryBoundsError";
    this.code = code;
  }
}

/**
 * Validated, normalised bounds for a `get_accounts` call.
 */
export interface GetAccountsBounds {
  /** Zero-based offset into the owner's account list. */
  offset: number;
  /** Page size, guaranteed to be within `1..MAX_ACCOUNTS_PAGE_SIZE`. */
  limit: number;
}

/**
 * Validate and normalise `get_accounts` pagination bounds before they reach the
 * contract. Enforces a hard maximum page size and rejects invalid values with a
 * stable {@link FactoryBoundsErrorCode} instead of panicking or returning an
 * unbounded result set.
 *
 * @throws {FactoryBoundsError} when `offset` or `limit` are out of range.
 *
 * @example
 * ```ts
 * const bounds = validateGetAccountsBounds({ offset: 0, limit: 50 });
 * const accounts = await factory.get_accounts(owner, bounds.offset, bounds.limit);
 * ```
 */
export function validateGetAccountsBounds(input: {
  offset?: number;
  limit?: number;
}): GetAccountsBounds {
  const offset = input.offset ?? 0;
  const limit = input.limit ?? MAX_ACCOUNTS_PAGE_SIZE;

  if (!Number.isInteger(offset) || offset < 0) {
    throw new FactoryBoundsError(
      FactoryBoundsErrorCode.InvalidOffset,
      `offset must be a non-negative integer, received ${offset}`,
    );
  }

  if (!Number.isInteger(limit) || limit < 1) {
    throw new FactoryBoundsError(
      FactoryBoundsErrorCode.InvalidLimit,
      `limit must be a positive integer, received ${limit}`,
    );
  }

  if (limit > MAX_ACCOUNTS_PAGE_SIZE) {
    throw new FactoryBoundsError(
      FactoryBoundsErrorCode.LimitTooLarge,
      `limit ${limit} exceeds maximum page size ${MAX_ACCOUNTS_PAGE_SIZE}`,
    );
  }

  return { offset, limit };
}

// ── Parsed event types ─────────────────────────────────────────────────────────

/**
 * Decoded `deployed` event. Emitted every time `deploy_account` or
 * `deploy_account_with_metadata` succeeds.
 *
 * On-chain data: `(owner: Address, account_address: Address)`
 */
export interface FactoryDeployedEvent {
  action: "deployed";
  /** The owner who authorized the deploy. */
  owner: string;
  /** The account address that was registered. */
  accountAddress: string;
}

/**
 * Decoded `meta_set` event. Emitted only by `deploy_account_with_metadata`.
 * Always follows a `deployed` event in the same transaction.
 *
 * On-chain data: `(owner: Address, account_address: Address, version: String)`
 */
export interface FactoryMetaSetEvent {
  action: "meta_set";
  /** The owner who authorized the deploy. */
  owner: string;
  /** The account address for which metadata was stored. */
  accountAddress: string;
  /** The semantic version string stored with the metadata. */
  version: string;
}

export type FactoryEvent = FactoryDeployedEvent | FactoryMetaSetEvent;

/**
 * Frozen, indexer-facing envelope wrapping a decoded {@link FactoryEvent}.
 *
 * Indexers consume this shape rather than the bare event so that every record
 * carries a stable schema version, a deterministic correlation id, and the
 * originating ledger/transaction coordinates needed for idempotent replay.
 */
export interface FactoryEventEnvelope {
  /** Frozen schema version — see {@link FACTORY_EVENT_SCHEMA_VERSION}. */
  schemaVersion: typeof FACTORY_EVENT_SCHEMA_VERSION;
  /** Contract-family tag (`topics[0]`). */
  contractTag: typeof FACTORY_CONTRACT_TAG;
  /** Action name (`topics[1]`). */
  action: FactoryEventAction;
  /**
   * Deterministic correlation id for idempotent indexing. Derived from the
   * ledger, transaction hash, and event index so replays collapse to one row.
   */
  correlationId: string;
  /** Ledger sequence the event was emitted in. */
  ledger: number;
  /** Transaction hash the event belongs to. */
  txHash: string;
  /** Zero-based index of the event within its transaction. */
  eventIndex: number;
  /** The decoded, typed event payload. */
  event: FactoryEvent;
}

// ── Raw Soroban event shape (minimal — avoids importing the full SDK) ──────────

/**
 * Minimal shape of a Soroban RPC event entry as returned by `getEvents`.
 * The full type is `SorobanRpc.Api.EventResponse` from `@stellar/stellar-sdk`.
 */
export interface RawSorobanEvent {
  /** Decoded topic values, one per topic element. */
  topic: string[];
  /** Decoded data value (XDR-decoded or JSON string). */
  value: string | unknown;
  /** Ledger sequence the event was emitted in (optional for lenient callers). */
  ledger?: number;
  /** Transaction hash the event belongs to (optional for lenient callers). */
  txHash?: string;
  /** Zero-based index of the event within its transaction. */
  eventIndex?: number;
}

// ── Parser ─────────────────────────────────────────────────────────────────────

/**
 * Stable, typed error codes emitted by the factory event parser. Indexers can
 * branch on these codes to fail closed instead of silently dropping or
 * mis-decoding events.
 */
export const FactoryEventErrorCode = {
  /** Event did not carry the factory contract tag (`topics[0]`). */
  UnknownContractTag: "UnknownContractTag",
  /** Event carried an unrecognised action (`topics[1]`). */
  UnknownAction: "UnknownAction",
  /** Topic vector was missing, malformed, or exceeded the frozen maximum. */
  MalformedTopics: "MalformedTopics",
  /** Data payload was missing, malformed, or not decodable. */
  MalformedPayload: "MalformedPayload",
  /** A decoded field exceeded {@link MAX_FACTORY_EVENT_FIELD_BYTES}. */
  OversizedPayload: "OversizedPayload",
  /** Envelope schema version did not match the frozen version. */
  SchemaVersionMismatch: "SchemaVersionMismatch",
} as const;

export type FactoryEventErrorCode =
  (typeof FactoryEventErrorCode)[keyof typeof FactoryEventErrorCode];

/**
 * Typed error thrown by {@link decodeFactoryEvent} when an event cannot be
 * decoded against the frozen schema. Carries a stable {@link FactoryEventErrorCode}
 * and an optional correlation id so indexers can log actionable failures.
 */
export class FactoryEventError extends Error {
  readonly code: FactoryEventErrorCode;
  readonly correlationId?: string;

  constructor(code: FactoryEventErrorCode, message?: string, correlationId?: string) {
    super(message ?? code);
    this.name = "FactoryEventError";
    this.code = code;
    this.correlationId = correlationId;
  }
}

/**
 * Build the deterministic correlation id used for idempotent indexing. Stable
 * across replays of the same event and unique across events in a transaction.
 */
export function factoryEventCorrelationId(input: {
  ledger: number;
  txHash: string;
  eventIndex: number;
}): string {
  return `${input.ledger}:${input.txHash}:${input.eventIndex}`;
}

/**
 * Decode a raw Soroban RPC event into a frozen {@link FactoryEventEnvelope}.
 *
 * Fail-closed: throws a typed {@link FactoryEventError} for unknown contract
 * tags, unknown actions, malformed/oversized payloads, or schema mismatches
 * rather than returning a partial or silently-wrong record. Use
 * {@link parseFactoryEvent} when a lenient `null`-on-mismatch filter is wanted.
 *
 * @throws {FactoryEventError} when the event cannot be decoded against the
 * frozen schema.
 */
export function decodeFactoryEvent(event: RawSorobanEvent): FactoryEventEnvelope {
  const topics = event.topic ?? [];

  if (!Array.isArray(topics) || topics.length !== MAX_FACTORY_EVENT_TOPICS) {
    throw new FactoryEventError(
      FactoryEventErrorCode.MalformedTopics,
      `expected ${MAX_FACTORY_EVENT_TOPICS} topics, received ${topics.length}`,
    );
  }

  const [tag, action] = topics;

  if (tag !== FACTORY_CONTRACT_TAG) {
    throw new FactoryEventError(
      FactoryEventErrorCode.UnknownContractTag,
      `unexpected contract tag ${String(tag)}`,
    );
  }

  if (action !== FACTORY_EVENT_TOPICS.deployed && action !== FACTORY_EVENT_TOPICS.meta_set) {
    throw new FactoryEventError(
      FactoryEventErrorCode.UnknownAction,
      `unrecognised factory action ${String(action)}`,
    );
  }

  const data = normaliseData(event.value);
  if (!data) {
    throw new FactoryEventError(
      FactoryEventErrorCode.MalformedPayload,
      "event data payload was missing or not decodable",
    );
  }

  for (const field of data) {
    if (typeof field !== "string" || byteLength(field) > MAX_FACTORY_EVENT_FIELD_BYTES) {
      throw new FactoryEventError(
        FactoryEventErrorCode.OversizedPayload,
        `event field exceeds ${MAX_FACTORY_EVENT_FIELD_BYTES} bytes or is not a string`,
      );
    }
  }

  const ledger = event.ledger ?? 0;
  const txHash = event.txHash ?? "";
  const eventIndex = event.eventIndex ?? 0;
  const correlationId = factoryEventCorrelationId({ ledger, txHash, eventIndex });

  let decoded: FactoryEvent;
  if (action === FACTORY_EVENT_TOPICS.deployed) {
    if (data.length !== 2) {
      throw new FactoryEventError(
        FactoryEventErrorCode.MalformedPayload,
        `deployed expects 2 fields, received ${data.length}`,
        correlationId,
      );
    }
    decoded = { action: "deployed", owner: data[0], accountAddress: data[1] };
  } else {
    if (data.length !== 3) {
      throw new FactoryEventError(
        FactoryEventErrorCode.MalformedPayload,
        `meta_set expects 3 fields, received ${data.length}`,
        correlationId,
      );
    }
    decoded = {
      action: "meta_set",
      owner: data[0],
      accountAddress: data[1],
      version: data[2],
    };
  }

  return {
    schemaVersion: FACTORY_EVENT_SCHEMA_VERSION,
    contractTag: FACTORY_CONTRACT_TAG,
    action,
    correlationId,
    ledger,
    txHash,
    eventIndex,
    event: decoded,
  };
}

/**
 * Parse a raw Soroban RPC event into a typed {@link FactoryEvent}.
 *
 * Returns `null` when the event does not match the factory's contract tag or
 * when the action is unrecognised — allowing callers to filter safely with
 * a simple `filter(Boolean)`. For fail-closed decoding with typed errors and a
 * correlation id, use {@link decodeFactoryEvent}.
 *
 * The parser is intentionally lenient on the `value` field type because the
 * Stellar SDK may return decoded XDR as an object or as a JSON string depending
 * on the SDK version and the `xdrFormat` query option.
 *
 * @example
 * ```ts
 * import { parseFactoryEvent, FACTORY_CONTRACT_TAG, FACTORY_EVENT_TOPICS } from "./factory-events";
 *
 * const rawEvents = await server.getEvents({ startLedger, filters: [...] });
 * const factoryEvents = rawEvents.records
 *   .map(parseFactoryEvent)
 *   .filter((e): e is FactoryEvent => e !== null);
 * ```
 */
export function parseFactoryEvent(event: RawSorobanEvent): FactoryEvent | null {
  const [tag, action] = event.topic ?? [];

  // Guard: must carry the factory contract tag.
  if (tag !== FACTORY_CONTRACT_TAG) return null;

  // Normalise the data field — handle both SDK v11 array-style and raw string.
  const data = normaliseData(event.value);
  if (!data) return null;

  if (action === FACTORY_EVENT_TOPICS.deployed) {
    if (data.length !== 2) return null;
    return { action: "deployed", owner: data[0], accountAddress: data[1] };
  }

  if (action === FACTORY_EVENT_TOPICS.meta_set) {
    if (data.length !== 3) return null;
    return {
      action: "meta_set",
      owner: data[0],
      accountAddress: data[1],
      version: data[2],
    };
  }

  return null;
}

// ── Internal helpers ───────────────────────────────────────────────────────────

/**
 * Normalise the raw `value` field into a flat string array. Accepts either an
 * already-decoded array of scalars or a JSON-encoded string. Returns `null`
 * when the payload cannot be coerced into a string array.
 */
function normaliseData(value: string | unknown): string[] | null {
  let parsed: unknown = value;

  if (typeof value === "string") {
    try {
      parsed = JSON.parse(value);
    } catch {
      // A bare string is treated as a single-field payload.
      return [value];
    }
  }

  if (!Array.isArray(parsed)) return null;

  return parsed.map((field) => (typeof field === "string" ? field : String(field)));
}

/**
 * UTF-8 byte length of a string without depending on Node's Buffer, so this
 * module stays usable in browser and edge runtimes.
 */
function byteLength(value: string): number {
  let bytes = 0;
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i);
    if (code < 0x80) bytes += 1;
    else if (code < 0x800) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff) {
      bytes += 4;
      i++;
    } else bytes += 3;
  }
  return bytes;
}
