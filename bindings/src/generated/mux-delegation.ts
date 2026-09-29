/**
 * AUTO-GENERATED STYLE — hand-authored for mux-delegation.
 *
 * Contract: mux-delegation
 *
 * Provides a client for the MuxDelegation contract with optional filtering
 * query parameters on read methods and a convenience `checkDelegate` method.
 *
 * ## Audit Events
 *
 * Every successful state mutation emits a Soroban contract event under the
 * `mux_dlg` contract tag. Topic layout:
 *
 * ```
 * topics[0]  "mux_dlg"   — contract tag (Symbol)
 * topics[1]  <action>    — "dlg_grant" | "dlg_rev"
 * data       (owner: Address, delegate: Address)
 * ```
 *
 * Subscribe via Soroban RPC `getEvents`:
 *
 * ```ts
 * import { DELEGATION_CONTRACT_TAG, DELEGATION_GRANT_ACTION, DELEGATION_REVOKE_ACTION } from "./mux-delegation";
 *
 * const rawEvents = await server.getEvents({
 *   startLedger,
 *   filters: [{
 *     type: "contract",
 *     contractIds: [DELEGATION_CONTRACT_ID],
 *     topics: [[DELEGATION_CONTRACT_TAG]],
 *   }],
 * });
 *
 * for (const ev of rawEvents.records) {
 *   const action = ev.topic[1]; // "dlg_grant" | "dlg_rev"
 *   const [owner, delegate] = ev.value.obj?.map ?? [];
 * }
 * ```
 *
 * See `docs/audit-events.md` for the full event schema reference.
 */

import {
  Address,
  Contract,
  Keypair,
  nativeToScVal,
  scValToNative,
  SorobanRpc,
  Transaction,
  TransactionBuilder,
  xdr,
} from "@stellar/stellar-sdk";
import type { MuxDelegationError } from "../types";
import { pollTransaction } from "../horizon";

// ── Event constants ───────────────────────────────────────────────────────────

/**
 * Soroban contract tag for all `mux-delegation` events (`topics[0]`).
 *
 * Use this value in `getEvents` filter topics to subscribe to all delegation
 * events from a specific contract instance.
 *
 * @example
 * ```ts
 * filters: [{ type: "contract", contractIds: [id], topics: [[DELEGATION_CONTRACT_TAG]] }]
 * ```
 */
export const DELEGATION_CONTRACT_TAG = "mux_dlg" as const;

/**
 * Action symbol emitted by `grant_delegate` on success (`topics[1]`).
 *
 * Data payload: `(owner: Address, delegate: Address)`
 */
export const DELEGATION_GRANT_ACTION = "dlg_grant" as const;

/**
 * Action symbol emitted by `revoke_delegate` on success (`topics[1]`).
 *
 * Data payload: `(owner: Address, delegate: Address)`
 */
export const DELEGATION_REVOKE_ACTION = "dlg_rev" as const;

// ── Expiry error codes ────────────────────────────────────────────────────────

/**
 * Stable error codes for delegation expiry failures.
 *
 * These are surfaced by the contract and mapped to typed errors by the client
 * so callers can branch on a stable code rather than parsing free-form text.
 *
 * - `DELEGATION_EXPIRED` — the grant's `expiresAt` is in the past.
 * - `DELEGATION_REVOKED` — the grant was explicitly revoked.
 * - `DELEGATION_NOT_FOUND` — no grant exists for (owner, delegate).
 * - `DELEGATION_EXPIRY_INVALID` — `expiresAt` is not strictly in the future.
 */
export const DELEGATION_EXPIRY_ERROR_CODES = {
  DELEGATION_EXPIRED: "DELEGATION_EXPIRED",
  DELEGATION_REVOKED: "DELEGATION_REVOKED",
  DELEGATION_NOT_FOUND: "DELEGATION_NOT_FOUND",
  DELEGATION_EXPIRY_INVALID: "DELEGATION_EXPIRY_INVALID",
} as const;

export type DelegationExpiryErrorCode =
  (typeof DELEGATION_EXPIRY_ERROR_CODES)[keyof typeof DELEGATION_EXPIRY_ERROR_CODES];

/**
 * Typed error thrown by expiry-aware delegation operations.
 *
 * Carries a stable {@link DelegationExpiryErrorCode} plus an optional
 * `correlationId` so ops can trace a failure across logs without exposing
 * secrets or raw key material.
 */
export class DelegationExpiryError extends Error {
  readonly code: DelegationExpiryErrorCode;
  readonly correlationId?: string;

  constructor(code: DelegationExpiryErrorCode, correlationId?: string) {
    super(code);
    this.name = "DelegationExpiryError";
    this.code = code;
    this.correlationId = correlationId;
  }
}

// ── Named-permission error codes ──────────────────────────────────────────────

/**
 * Stable error codes for named-permission grant/revoke failures.
 *
 * These are surfaced by the contract and mapped to typed errors by the client
 * so callers can branch on a stable code rather than parsing free-form text.
 *
 * - `DELEGATION_UNAUTHORIZED` — caller is not owner/authorized delegate/guardian.
 * - `DELEGATION_PERMISSION_UNKNOWN` — permission name is not in the allow-list.
 * - `DELEGATION_PERMISSION_INVALID` — permission name is malformed/empty.
 * - `DELEGATION_BATCH_TOO_LARGE` — batch exceeds the max permissions per call.
 * - `DELEGATION_DELEGATE_REVOKED` — delegate was revoked/expired; deny-by-default.
 * - `DELEGATION_RPC_UNAVAILABLE` — dependency outage; writes fail closed.
 */
export const DELEGATION_PERMISSION_ERROR_CODES = {
  DELEGATION_UNAUTHORIZED: "DELEGATION_UNAUTHORIZED",
  DELEGATION_PERMISSION_UNKNOWN: "DELEGATION_PERMISSION_UNKNOWN",
  DELEGATION_PERMISSION_INVALID: "DELEGATION_PERMISSION_INVALID",
  DELEGATION_BATCH_TOO_LARGE: "DELEGATION_BATCH_TOO_LARGE",
  DELEGATION_DELEGATE_REVOKED: "DELEGATION_DELEGATE_REVOKED",
  DELEGATION_RPC_UNAVAILABLE: "DELEGATION_RPC_UNAVAILABLE",
} as const;

export type DelegationPermissionErrorCode =
  (typeof DELEGATION_PERMISSION_ERROR_CODES)[keyof typeof DELEGATION_PERMISSION_ERROR_CODES];

/**
 * Typed error thrown by named-permission grant/revoke operations.
 *
 * Carries a stable {@link DelegationPermissionErrorCode} plus an optional
 * `correlationId` so ops can trace a failure across logs without exposing
 * secrets or raw key material.
 */
export class DelegationPermissionError extends Error {
  readonly code: DelegationPermissionErrorCode;
  readonly correlationId?: string;

  constructor(code: DelegationPermissionErrorCode, correlationId?: string) {
    super(code);
    this.name = "DelegationPermissionError";
    this.code = code;
    this.correlationId = correlationId;
  }
}

/**
 * Named permissions recognized by the delegation model.
 *
 * See `docs/delegation-permission-model.md` for the authoritative list and
 * semantics. Granting an unknown name fails closed with
 * `DELEGATION_PERMISSION_UNKNOWN`.
 */
export const DELEGATION_NAMED_PERMISSIONS = [
  "spend",
  "recover",
  "admin",
  "withdraw",
  "sign",
] as const;

export type DelegationNamedPermission =
  (typeof DELEGATION_NAMED_PERMISSIONS)[number];

/** Maximum number of permissions accepted in a single grant/revoke batch. */
export const DELEGATION_MAX_PERMISSIONS_PER_CALL = 32;

// ── Event types ───────────────────────────────────────────────────────────────

/**
 * Parsed form of a `dlg_grant` event emitted by `grant_delegate`.
 *
 * The `owner` granted `permissions` to `delegate` at the emitting ledger.
 * Note: the event data carries only `(owner, delegate)` — the permission
 * list must be retrieved via `getDelegatePermissions` if needed.
 */
export interface DelegationGrantEvent {
  action: typeof DELEGATION_GRANT_ACTION;
  owner: string;
  delegate: string;
  /** Ledger sequence at which the event was emitted. */
  ledger: number;
}

/**
 * Parsed form of a `dlg_rev` event emitted by `revoke_delegate`.
 *
 * All permissions previously granted from `owner` to `delegate` have been
 * removed.
 */
export interface DelegationRevokeEvent {
  action: typeof DELEGATION_REVOKE_ACTION;
  owner: string;
  delegate: string;
  /** Ledger sequence at which the event was emitted. */
  ledger: number;
}

/** Union of all delegation event shapes. */
export type DelegationEvent = DelegationGrantEvent | DelegationRevokeEvent;

// ── Event parser ──────────────────────────────────────────────────────────────

/**
 * Parse a raw Soroban RPC event record into a typed {@link DelegationEvent}.
 *
 * Returns `null` if the event does not match the `mux_dlg` schema.
 *
 * @example
 * ```ts
 * import { parseDelegationEvent } from "@mux-protocol/contracts";
 *
 * const raw = await server.getEvents({ startLedger, filters: [...] });
 * const events: DelegationEvent[] = raw.records
 *   .map(parseDelegationEvent)
 *   .filter((e): e is DelegationEvent => e !== null);
 *
 * const grants = events.filter(e => e.action === "dlg_grant");
 * const revokes = events.filter(e => e.action === "dlg_rev");
 * ```
 */
export function parseDelegationEvent(
  // Accept the raw SorobanRpc event shape generically to avoid a hard SDK
  // dependency at the type level — callers cast from the RPC response.
  raw: {
    topic: string[];
    value: { map?: { key: string; val: unknown }[] } | unknown;
    ledger: number;
  }
): DelegationEvent | null {
  const [tag, action] = raw.topic;
  if (tag !== DELEGATION_CONTRACT_TAG) return null;
  if (action !== DELEGATION_GRANT_ACTION && action !== DELEGATION_REVOKE_ACTION) {
    return null;
  }

  // Data payload is a tuple (owner: Address, delegate: Address).
  // scValToNative converts this to a JS array of strings on the RPC result.
  const data = raw.value as unknown[];
  const owner = typeof data?.[0] === "string" ? data[0] : "unknown";
  const delegate = typeof data?.[1] === "string" ? data[1] : "unknown";

  return {
    action: action as typeof DELEGATION_GRANT_ACTION | typeof DELEGATION_REVOKE_ACTION,
    owner,
    delegate,
    ledger: raw.ledger,
  } as DelegationEvent;
}



/**
 * Optional filter parameters for delegation read queries.
 * Filters are applied client-side after the on-chain call returns.
 */
export interface DelegationQueryFilters {
  /** Filter by permission symbol */
  permission?: string;
  /** Only include delegates with at least one permission */
  hasAnyPermission?: boolean;
}

// ── Client options ────────────────────────────────────────────────────────────

export interface MuxDelegationClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

// ── Named-permission grant/revoke types ───────────────────────────────────────

/**
 * Request to grant one or more named permissions from `owner` to `delegate`.
 *
 * `permissions` must be a non-empty subset of {@link DELEGATION_NAMED_PERMISSIONS}
 * and at most {@link DELEGATION_MAX_PERMISSIONS_PER_CALL} entries. `expiresAt`
 * (unix seconds), when provided, MUST be strictly in the future.
 */
export interface GrantPermissionsRequest {
  owner: string;
  delegate: string;
  permissions: DelegationNamedPermission[];
  /** Optional unix-seconds expiry; must be strictly in the future. */
  expiresAt?: number;
  /** Optional idempotency key to make retries safe. */
  idempotencyKey?: string;
  /** Optional correlation id propagated to logs/errors (never a secret). */
  correlationId?: string;
}

/**
 * Request to revoke one or more named permissions from `owner` to `delegate`.
 *
 * Revoking a permission that is not currently granted is a no-op (idempotent).
 */
export interface RevokePermissionsRequest {
  owner: string;
  delegate: string;
  permissions: DelegationNamedPermission[];
  /** Optional idempotency key to make retries safe. */
  idempotencyKey?: string;
  /** Optional correlation id propagated to logs/errors (never a secret). */
  correlationId?: string;
}

/** Result of a successful grant/revoke call. */
export interface PermissionMutationResult {
  /** Transaction hash of the submitted operation. */
  txHash: string;
  /** Permissions that were effectively granted/revoked. */
  permissions: DelegationNamedPermission[];
  /** Correlation id echoed back for tracing. */
  correlationId?: string;
}

// ── Client ────────────────────────────────────────────────────────────────────

export class MuxDelegationClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxDelegationClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  // ── Write operations ────────────────────────────────────────────────────────

  /**
   * Grant `permissions` from `owner` to `delegate`, optionally expiring at
   * `expiresAt` (unix seconds).
   *
   * When `expiresAt` is provided it MUST be strictly in the future; otherwise
   * the client fails closed with `DELEGATION_EXPIRY_INVALID` before submitting
   * any tr

/* … truncated 9658 chars — edit only what you need near the top … */
