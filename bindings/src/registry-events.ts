/**
 * registry-events.ts — TypeScript helpers for consuming mux-registry
 * Soroban events.
 *
 * Every state-mutating registry entrypoint emits structured contract events
 * with a two-element topic vector:
 *
 *   topics[0]  "mux_reg"   — contract-family tag (Symbol)
 *   topics[1]  action name  — one of REGISTRY_EVENT_TOPICS (Symbol)
 *
 * Data payloads per action:
 *
 *   init       →  admin: Address
 *   reg        →  (name: Symbol, version: String)
 *   regmeta    →  (name: Symbol, version: String)
 *
 * See docs/audit-events.md and docs/event-topic-conventions.md for the
 * canonical event catalog.
 *
 * @module registry-events
 */

// ── Topic constants ────────────────────────────────────────────────────────────

/**
 * The contract-family tag emitted as `topics[0]` in every mux-registry
 * event. Use this when constructing Soroban RPC `getEvents` filters.
 */
export const REGISTRY_CONTRACT_TAG = "mux_reg" as const;

/**
 * Action names emitted as `topics[1]` in mux-registry events.
 * Values are stable ABI — renaming requires a breaking-change doc update.
 */
export const REGISTRY_EVENT_TOPICS = {
  /** Emitted by `initialize`. */
  init: "init",
  /** Emitted by `register`. */
  reg: "reg",
  /** Emitted by `register_with_metadata`. */
  regmeta: "regmeta",
} as const;

export type RegistryEventAction = (typeof REGISTRY_EVENT_TOPICS)[keyof typeof REGISTRY_EVENT_TOPICS];

// ── Parsed event types ─────────────────────────────────────────────────────────

export interface RegistryInitEvent {
  action: "init";
  admin: string;
}

export interface RegistryRegEvent {
  action: "reg";
  name: string;
  version: string;
}

export interface RegistryRegMetaEvent {
  action: "regmeta";
  name: string;
  version: string;
}

export type RegistryEvent =
  | RegistryInitEvent
  | RegistryRegEvent
  | RegistryRegMetaEvent;

// ── Raw Soroban event shape (minimal — avoids importing the full SDK) ──────────

// Import shared RawSorobanEvent from factory-events to avoid duplication
import type { RawSorobanEvent } from "./factory-events";

// ── Parser ─────────────────────────────────────────────────────────────────────

/**
 * Parse a raw Soroban RPC event into a typed {@link RegistryEvent}.
 *
 * Returns `null` when the event does not match the registry's contract tag or
 * when the action is unrecognised.
 *
 * @example
 * ```ts
 * import { parseRegistryEvent, REGISTRY_CONTRACT_TAG } from "./registry-events";
 *
 * const rawEvents = await server.getEvents({ startLedger, filters: [...] });
 * const registryEvents = rawEvents.records
 *   .map(parseRegistryEvent)
 *   .filter((e): e is RegistryEvent => e !== null);
 * ```
 */
export function parseRegistryEvent(event: RawSorobanEvent): RegistryEvent | null {
  const [tag, action] = event.topic ?? [];

  if (tag !== REGISTRY_CONTRACT_TAG) return null;

  const data = normaliseData(event.value);
  if (!data) return null;

  switch (action as RegistryEventAction) {
    case REGISTRY_EVENT_TOPICS.init: {
      const admin = resolveAddress(data[0]);
      if (!admin) return null;
      return { action: "init", admin };
    }

    case REGISTRY_EVENT_TOPICS.reg: {
      const name = resolveSymbol(data[0]);
      const version = resolveString(data[1]);
      if (!name || !version) return null;
      return { action: "reg", name, version };
    }

    case REGISTRY_EVENT_TOPICS.regmeta: {
      const name = resolveSymbol(data[0]);
      const version = resolveString(data[1]);
      if (!name || !version) return null;
      return { action: "regmeta", name, version };
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

function resolveSymbol(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["sym"] === "string") return obj["sym"];
  }
  return null;
}

function resolveString(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["string"] === "string") return obj["string"];
  }
  return null;
}
