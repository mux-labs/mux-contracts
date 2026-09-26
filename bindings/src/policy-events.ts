/**
 * policy-events.ts — TypeScript helpers for consuming mux-policy
 * Soroban events.
 *
 * Every state-mutating policy entrypoint emits structured contract events
 * with a two-element topic vector:
 *
 *   topics[0]  "mux_pol"   — contract-family tag (Symbol)
 *   topics[1]  action name  — one of POLICY_EVENT_TOPICS (Symbol)
 *
 * Data payloads per action:
 *
 *   init      →  admin: Address
 *   lmt_set   →  (wallet: Address, limit: i128, day_ledgers: u32)
 *   spent     →  (wallet: Address, amount: i128)
 *   ctr_rst   →  wallet: Address
 *
 * See docs/audit-events.md and docs/event-topic-conventions.md for the
 * canonical event catalog.
 *
 * @module policy-events
 */

// ── Topic constants ────────────────────────────────────────────────────────────

/**
 * The contract-family tag emitted as `topics[0]` in every mux-policy
 * event. Use this when constructing Soroban RPC `getEvents` filters.
 */
export const POLICY_CONTRACT_TAG = "mux_pol" as const;

/**
 * Action names emitted as `topics[1]` in mux-policy events.
 * Values are stable ABI — renaming requires a breaking-change doc update.
 */
export const POLICY_EVENT_TOPICS = {
  /** Emitted by `initialize`. */
  init: "init",
  /** Emitted by `set_daily_limit`. */
  lmt_set: "lmt_set",
  /** Emitted by `record_spend`. */
  spent: "spent",
  /** Emitted by `reset_daily_counter`. */
  ctr_rst: "ctr_rst",
} as const;

export type PolicyEventAction = (typeof POLICY_EVENT_TOPICS)[keyof typeof POLICY_EVENT_TOPICS];

// ── Parsed event types ─────────────────────────────────────────────────────────

export interface PolicyInitEvent {
  action: "init";
  admin: string;
}

export interface PolicyLmtSetEvent {
  action: "lmt_set";
  wallet: string;
  limit: string;
  dayLedgers: string;
}

export interface PolicySpentEvent {
  action: "spent";
  wallet: string;
  amount: string;
}

export interface PolicyCtrRstEvent {
  action: "ctr_rst";
  wallet: string;
}

export type PolicyEvent =
  | PolicyInitEvent
  | PolicyLmtSetEvent
  | PolicySpentEvent
  | PolicyCtrRstEvent;

// ── Raw Soroban event shape (minimal — avoids importing the full SDK) ──────────

// Import shared RawSorobanEvent from factory-events to avoid duplication
import type { RawSorobanEvent } from "./factory-events";

// ── Parser ─────────────────────────────────────────────────────────────────────

/**
 * Parse a raw Soroban RPC event into a typed {@link PolicyEvent}.
 *
 * Returns `null` when the event does not match the policy's contract tag or
 * when the action is unrecognised.
 *
 * @example
 * ```ts
 * import { parsePolicyEvent, POLICY_CONTRACT_TAG } from "./policy-events";
 *
 * const rawEvents = await server.getEvents({ startLedger, filters: [...] });
 * const policyEvents = rawEvents.records
 *   .map(parsePolicyEvent)
 *   .filter((e): e is PolicyEvent => e !== null);
 * ```
 */
export function parsePolicyEvent(event: RawSorobanEvent): PolicyEvent | null {
  const [tag, action] = event.topic ?? [];

  if (tag !== POLICY_CONTRACT_TAG) return null;

  const data = normaliseData(event.value);
  if (!data) return null;

  switch (action as PolicyEventAction) {
    case POLICY_EVENT_TOPICS.init: {
      const admin = resolveAddress(data[0]);
      if (!admin) return null;
      return { action: "init", admin };
    }

    case POLICY_EVENT_TOPICS.lmt_set: {
      const wallet = resolveAddress(data[0]);
      const limit = resolveI128(data[1]);
      const dayLedgers = resolveU32(data[2]);
      if (!wallet || !limit || !dayLedgers) return null;
      return { action: "lmt_set", wallet, limit, dayLedgers };
    }

    case POLICY_EVENT_TOPICS.spent: {
      const wallet = resolveAddress(data[0]);
      const amount = resolveI128(data[1]);
      if (!wallet || !amount) return null;
      return { action: "spent", wallet, amount };
    }

    case POLICY_EVENT_TOPICS.ctr_rst: {
      const wallet = resolveAddress(data[0]);
      if (!wallet) return null;
      return { action: "ctr_rst", wallet };
    }

    default:
      return null;
  }
}

// ── Internal helpers ───────────────────────────────────────────────────────────

function normaliseData(raw: unknown): unknown[] | null {
  if (Array.isArray(raw)) return raw as unknown[];

  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (Array.isArray(obj["vec"])) return obj["vec"] as unknown[];
    if (Array.isArray(obj["_value"])) return obj["_value"] as unknown[];
  }

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

function resolveAddress(raw: unknown): string | null {
  if (typeof raw === "string" && raw.startsWith("C")) return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["address"] === "string") return obj["address"];
  }
  return null;
}

function resolveI128(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (typeof raw === "number") return String(raw);
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["i128"] === "string") return obj["i128"];
    if (typeof obj["i128"] === "number") return String(obj["i128"]);
  }
  return null;
}

function resolveU32(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (typeof raw === "number") return String(raw);
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["u32"] === "string") return obj["u32"];
    if (typeof obj["u32"] === "number") return String(obj["u32"]);
  }
  return null;
}
