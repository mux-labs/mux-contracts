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
   * any transaction. Omitting `expiresAt` grants a non-expiring delegation.
   */
  async grantDelegate(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address,
    permissions: string[],
    expiresAt?: number
  ): Promise<void> {
    if (expiresAt !== undefined) {
      const now = Math.floor(Date.now() / 1000);
      if (!Number.isFinite(expiresAt) || expiresAt <= now) {
        throw new DelegationExpiryError(
          DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_EXPIRY_INVALID
        );
      }
    }

    const args = [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
      xdr.ScVal.scvVec(permissions.map((p) => xdr.ScVal.scvSymbol(p))),
    ];
    if (expiresAt !== undefined) {
      args.push(nativeToScVal(expiresAt, { type: "u64" }));
    }

    const tx = await this.buildTx(sourceKeypair, "grant_delegate", args);
    await this.submit(tx, sourceKeypair);
  }

  async revokeDelegate(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "revoke_delegate", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  // ── Read operations ─────────────────────────────────────────────────────────

  /**
   * Return the permissions granted by `owner` to `delegate`.
   *
   * Accepts optional `DelegationQueryFilters` to narrow results client-side:
   * - `permission`: only return that permission if present in the grant.
   * - `hasAnyPermission`: if `true`, returns the full list only when non-empty;
   *   if `false`, returns the full list only when empty.
   */
  async getDelegatePermissions(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address,
    filters?: DelegationQueryFilters
  ): Promise<string[]> {
    const tx = await this.buildTx(sourceKeypair, "get_delegate_permissions", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
    ]);
    const result = await this.simulateRead<string[]>(tx);
    return this.applyPermissionFilters(result, filters);
  }

  /**
   * Return the expiry timestamp (unix seconds) for the grant from `owner` to
   * `delegate`, or `null` when the grant does not expire.
   *
   * Throws {@link DelegationExpiryError} with `DELEGATION_NOT_FOUND` when no
   * grant exists.
   */
  async getDelegateExpiry(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address
  ): Promise<number | null> {
    const tx = await this.buildTx(sourceKeypair, "get_delegate_expiry", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
    ]);
    const result = await this.simulateRead<number | null>(tx);
    if (result === undefined) {
      throw new DelegationExpiryError(
        DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_NOT_FOUND
      );
    }
    return result;
  }

  /**
   * Assert that the grant from `owner` to `delegate` is currently valid.
   *
   * Fails closed: throws {@link DelegationExpiryError} when the grant is
   * missing, revoked, or expired. Callers MUST treat any thrown error as a
   * denial and MUST NOT proceed with the delegated action.
   *
   * @param correlationId Optional opaque id echoed into the error for ops
   *   tracing. Never include secrets or raw key material.
   */
  async assertDelegateActive(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address,
    correlationId?: string
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "get_delegate_expiry", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
    ]);
    const result = await this.simulateRead<number | null>(tx);
    if (result === undefined) {
      throw new DelegationExpiryError(
        DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_NOT_FOUND,
        correlationId
      );
    }
    if (result !== null && result <= Math.floor(Date.now() / 1000)) {
      throw new DelegationExpiryError(
        DELEGATION_EXPIRY_ERROR_CODES.DELEGATION_EXPIRED,
        correlationId
      );
    }
  }

  async isDelegate(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address,
    permission: string
  ): Promise<boolean> {
    const tx = await this.buildTx(sourceKeypair, "is_delegate", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
      xdr.ScVal.scvSymbol(permission),
    ]);
    return this.simulateRead<boolean>(tx);
  }

  /**
   * Return all delegates registered under `owner`.
   *
   * Accepts optional `DelegationQueryFilters` to narrow results client-side:
   * - `permission`: only include delegates that have been granted this specific
   *   permission (requires an additional `getDelegatePermissions` call per delegate).
   * - `hasAnyPermission`: if `true`, only include delegates with at least one
   *   permission in the current grant set (no-op here since all listed delegates
   *   have at least one permission; included for API symmetry).
   */
  async getDelegates(
    sourceKeypair: Keypair,
    owner: Address,
    filters?: DelegationQueryFilters
  ): Promise<Address[]> {
    const tx = await this.buildTx(sourceKeypair, "get_delegates", [
      nativeToScVal(owner.toString(), { type: "address" }),
    ]);
    const result = await this.simulateRead<Address[]>(tx);
    return this.applyDelegateFilters(sourceKeypair, owner, result, filters);
  }

  /**
   * Convenience read-only check: returns `true` if `owner` has granted
   * `permission` to `delegate`, `false` otherwise (including when no grant
   * exists at all).
   *
   * Calls the `check_delegate` on-chain entrypoint which returns `Ok(())`
   * for a match or `Err(NotADelegate)` when the permission is absent.
   */
  async checkDelegate(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address,
    permission: string
  ): Promise<boolean> {
    try {
      const tx = await this.buildTx(sourceKeypair, "check_delegate", [
        nativeToScVal(owner.toString(), { type: "address" }),
        nativeToScVal(delegate.toString(), { type: "address" }),
        xdr.ScVal.scvSymbol(permission),
      ]);
      await this.simulateRead<void>(tx);
      return true;
    } catch {
      return false;
    }
  }

  // ── Private filter helpers ───────────────────────────────────────────────────

  private applyPermissionFilters(
    permissions: string[],
    filters?: DelegationQueryFilters
  ): string[] {
    if (!filters) return permissions;
    let result = permissions;
    if (filters.permission !== undefined) {
      result = result.filter((p) => p === filters.permission);
    }
    if (filters.hasAnyPermission === true && result.length === 0) {
      return [];
    }
    if (filters.hasAnyPermission === false && result.length > 0) {
      return [];
    }
    return result;
  }

  private async applyDelegateFilters(
    sourceKeypair: Keypair,
    owner: Address,
    delegates: Address[],
    filters?: DelegationQueryFilters
  ): Promise<Address[]> {
    if (!filters) return delegates;
    // If a specific permission filter is given, further narrow the list by
    // checking each delegate's granted permissions client-side.
    if (filters.permission !== undefined) {
      const filtered: Address[] = [];
      for (const d of delegates) {
        const perms = await this.getDelegatePermissions(
          sourceKeypair,
          owner,
          d
        );
        if (perms.includes(filters.permission)) {
          filtered.push(d);
        }
      }
      return filtered;
    }
    // hasAnyPermission is always true for delegates returned by get_delegates
    // (they all have an active grant), so no additional filtering is needed.
    return delegates;
  }

  // ── Private helpers ──────────────────────────────────────────────────────────

  private async buildTx(
    sourceKeypair: Keypair,
    method: string,
    args: xdr.ScVal[]
  ): Promise<Transaction> {
    const account = await this.server.getAccount(sourceKeypair.publicKey());
    return new TransactionBuilder(account, {
      fee: "100",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();
  }

  private async simulateRead<T>(tx: Transaction): Promise<T> {
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("No return value");
    return scValToNative(retval) as T;
  }

  private async submit(tx: Transaction, signer: Keypair): Promise<void> {
    const simResult = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(simResult)) {
      throw new Error(`Simulation failed: ${simResult.error}`);
    }
    const prepared = SorobanRpc.assembleTransaction(
      tx,
      simResult as SorobanRpc.Api.SimulateTransactionSuccessResponse
    ).build();
    prepared.sign(signer);
    const sendResult = await this.server.sendTransaction(prepared);
    if (sendResult.status === "ERROR") {
      throw new Error(`Transaction failed: ${JSON.stringify(sendResult.errorResult)}`);
    }
    await pollTransaction(this.server, sendResult.hash);
  }
}
