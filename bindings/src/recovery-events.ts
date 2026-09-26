/**
 * recovery-events.ts — TypeScript helpers for consuming mux-recovery
 * Soroban events.
 *
 * Every state-mutating recovery entrypoint emits structured contract events
 * with a two-element topic vector:
 *
 *   topics[0]  "mux_recv"  — contract-family tag (Symbol)
 *   topics[1]  action name  — one of RECOVERY_EVENT_TOPICS (Symbol)
 *
 * Data payloads per action:
 *
 *   init        →  admin: Address
 *   rec_init    →  (guardian, new_owner, initiated_at, executable_at, expires_at)
 *   rec_exec    →  (guardian: Address, new_owner: Address)
 *   rec_adm     →  new_owner: Address
 *   rec_cncl    →  ()
 *   grd_add     →  guardian: Address
 *   grd_rm      →  guardian: Address
 *   reg_link    →  registry_id: Address
 *
 * See docs/audit-events.md and docs/event-topic-conventions.md for the
 * canonical event catalog.
 *
 * @module recovery-events
 */

// ── Topic constants ────────────────────────────────────────────────────────────

/**
 * The contract-family tag emitted as `topics[0]` in every mux-recovery
 * event. Use this when constructing Soroban RPC `getEvents` filters.
 */
export const RECOVERY_CONTRACT_TAG = "mux_recv" as const;

/**
 * Action names emitted as `topics[1]` in mux-recovery events.
 * Values are stable ABI — renaming requires a breaking-change doc update.
 */
export const RECOVERY_EVENT_TOPICS = {
  /** Emitted by `initialize`. */
  init: "init",
  /** Emitted by `initiate_recovery`. */
  rec_init: "rec_init",
  /** Emitted by `execute_recovery`. */
  rec_exec: "rec_exec",
  /** Emitted by `approve_recovery_admin`. */
  rec_adm: "rec_adm",
  /** Emitted by `cancel_recovery`. */
  rec_cncl: "rec_cncl",
  /** Emitted by `add_guardian`. */
  grd_add: "grd_add",
  /** Emitted by `remove_guardian`. */
  grd_rm: "grd_rm",
  /** Emitted by `set_registry`. */
  reg_link: "reg_link",
} as const;

export type RecoveryEventAction = (typeof RECOVERY_EVENT_TOPICS)[keyof typeof RECOVERY_EVENT_TOPICS];

// ── Parsed event types ─────────────────────────────────────────────────────────

export interface RecoveryInitEvent {
  action: "init";
  admin: string;
}

export interface RecoveryRecInitEvent {
  action: "rec_init";
  guardian: string;
  newOwner: string;
  initiatedAt: string;
  executableAt: string;
  expiresAt: string;
}

export interface RecoveryRecExecEvent {
  action: "rec_exec";
  guardian: string;
  newOwner: string;
}

export interface RecoveryRecAdmEvent {
  action: "rec_adm";
  newOwner: string;
}

export interface RecoveryRecCnclEvent {
  action: "rec_cncl";
}

export interface RecoveryGrdAddEvent {
  action: "grd_add";
  guardian: string;
}

export interface RecoveryGrdRmEvent {
  action: "grd_rm";
  guardian: string;
}

export interface RecoveryRegLinkEvent {
  action: "reg_link";
  registryId: string;
}

export type RecoveryEvent =
  | RecoveryInitEvent
  | RecoveryRecInitEvent
  | RecoveryRecExecEvent
  | RecoveryRecAdmEvent
  | RecoveryRecCnclEvent
  | RecoveryGrdAddEvent
  | RecoveryGrdRmEvent
  | RecoveryRegLinkEvent;

// ── Raw Soroban event shape (minimal — avoids importing the full SDK) ──────────

// Import shared RawSorobanEvent from factory-events to avoid duplication
import type { RawSorobanEvent } from "./factory-events";

// ── Parser ─────────────────────────────────────────────────────────────────────

/**
 * Parse a raw Soroban RPC event into a typed {@link RecoveryEvent}.
 *
 * Returns `null` when the event does not match the recovery's contract tag or
 * when the action is unrecognised.
 *
 * @example
 * ```ts
 * import { parseRecoveryEvent, RECOVERY_CONTRACT_TAG } from "./recovery-events";
 *
 * const rawEvents = await server.getEvents({ startLedger, filters: [...] });
 * const recoveryEvents = rawEvents.records
 *   .map(parseRecoveryEvent)
 *   .filter((e): e is RecoveryEvent => e !== null);
 * ```
 */
export function parseRecoveryEvent(event: RawSorobanEvent): RecoveryEvent | null {
  const [tag, action] = event.topic ?? [];

  if (tag !== RECOVERY_CONTRACT_TAG) return null;

  const data = normaliseData(event.value);
  if (!data) return null;

  switch (action as RecoveryEventAction) {
    case RECOVERY_EVENT_TOPICS.init: {
      const admin = resolveAddress(data[0]);
      if (!admin) return null;
      return { action: "init", admin };
    }

    case RECOVERY_EVENT_TOPICS.rec_init: {
      const guardian = resolveAddress(data[0]);
      const newOwner = resolveAddress(data[1]);
      const initiatedAt = resolveU64(data[2]);
      const executableAt = resolveU64(data[3]);
      const expiresAt = resolveU64(data[4]);
      if (!guardian || !newOwner || !initiatedAt || !executableAt || !expiresAt) return null;
      return { action: "rec_init", guardian, newOwner, initiatedAt, executableAt, expiresAt };
    }

    case RECOVERY_EVENT_TOPICS.rec_exec: {
      const [guardian, newOwner] = extractAddressPair(data);
      if (!guardian || !newOwner) return null;
      return { action: "rec_exec", guardian, newOwner };
    }

    case RECOVERY_EVENT_TOPICS.rec_adm: {
      const newOwner = resolveAddress(data[0]);
      if (!newOwner) return null;
      return { action: "rec_adm", newOwner };
    }

    case RECOVERY_EVENT_TOPICS.rec_cncl: {
      return { action: "rec_cncl" };
    }

    case RECOVERY_EVENT_TOPICS.grd_add: {
      const guardian = resolveAddress(data[0]);
      if (!guardian) return null;
      return { action: "grd_add", guardian };
    }

    case RECOVERY_EVENT_TOPICS.grd_rm: {
      const guardian = resolveAddress(data[0]);
      if (!guardian) return null;
      return { action: "grd_rm", guardian };
    }

    case RECOVERY_EVENT_TOPICS.reg_link: {
      const registryId = resolveAddress(data[0]);
      if (!registryId) return null;
      return { action: "reg_link", registryId };
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

function extractAddressPair(data: unknown[]): [string, string] | [] {
  const [a, b] = data;
  const addr1 = resolveAddress(a);
  const addr2 = resolveAddress(b);
  if (!addr1 || !addr2) return [];
  return [addr1, addr2];
}

function resolveAddress(raw: unknown): string | null {
  if (typeof raw === "string" && raw.startsWith("C")) return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["address"] === "string") return obj["address"];
  }
  return null;
}

function resolveU64(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (typeof raw === "number") return String(raw);
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["u64"] === "string") return obj["u64"];
    if (typeof obj["u64"] === "number") return String(obj["u64"]);
  }
  return null;
}
