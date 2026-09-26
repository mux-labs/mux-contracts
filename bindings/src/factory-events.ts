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
}

// ── Parser ─────────────────────────────────────────────────────────────────────

/**
 * Parse a raw Soroban RPC event into a typed {@link FactoryEvent}.
 *
 * Returns `null` when the event does not match the factory's contract tag or
 * when the action is unrecognised — allowing callers to filter safely with
 * a simple `filter(Boolean)`.
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

  switch (action as FactoryEventAction) {
    case FACTORY_EVENT_TOPICS.deployed: {
      const [owner, accountAddress] = extractAddressPair(data);
      if (!owner || !accountAddress) return null;
      return { action: "deployed", owner, accountAddress };
    }

    case FACTORY_EVENT_TOPICS.meta_set: {
      const [owner, accountAddress, version] = extractAddressAddressString(data);
      if (!owner || !accountAddress || !version) return null;
      return { action: "meta_set", owner, accountAddress, version };
    }

    default:
      return null;
  }
}

// ── Internal helpers ───────────────────────────────────────────────────────────

/**
 * Normalise the raw `value` field from a Soroban RPC event into a plain
 * array of primitives. Handles both object form (SDK v11+) and string form.
 *
 * @internal
 */
function normaliseData(raw: unknown): unknown[] | null {
  if (Array.isArray(raw)) return raw as unknown[];

  // Object with a `.vec` property (XDR-decoded struct from stellar-sdk).
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (Array.isArray(obj["vec"])) return obj["vec"] as unknown[];
    // Some SDK versions wrap in { _value: [...] }
    if (Array.isArray(obj["_value"])) return obj["_value"] as unknown[];
  }

  // JSON-encoded string fallback.
  if (typeof raw === "string") {
    try {
      const parsed: unknown = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed as unknown[];
    } catch {
      // Not JSON — fall through.
    }
  }

  return null;
}

/**
 * Extract the first two elements as address strings from a normalised data
 * array. Returns an empty tuple on type mismatches.
 *
 * @internal
 */
function extractAddressPair(data: unknown[]): [string, string] | [] {
  const [a, b] = data;
  const owner = resolveAddress(a);
  const accountAddress = resolveAddress(b);
  if (!owner || !accountAddress) return [];
  return [owner, accountAddress];
}

/**
 * Extract two address strings and a version string from a normalised data
 * array. Returns an empty tuple on type mismatches.
 *
 * @internal
 */
function extractAddressAddressString(
  data: unknown[],
): [string, string, string] | [] {
  const [a, b, c] = data;
  const owner = resolveAddress(a);
  const accountAddress = resolveAddress(b);
  const version = resolveString(c);
  if (!owner || !accountAddress || !version) return [];
  return [owner, accountAddress, version];
}

/**
 * Coerce a raw data element to an address string.  Handles both plain strings
 * (Strkey addresses) and objects with an `address` property.
 *
 * @internal
 */
function resolveAddress(raw: unknown): string | null {
  if (typeof raw === "string" && raw.startsWith("C")) return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["address"] === "string") return obj["address"];
  }
  return null;
}

/**
 * Coerce a raw data element to a plain string.  Handles both plain strings and
 * objects with a `string` property.
 *
 * @internal
 */
function resolveString(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["string"] === "string") return obj["string"];
  }
  return null;
}
