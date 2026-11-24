/**
 * AUTO-GENERATED STYLE — hand-authored for mux-recovery.
 *
 * Contract: mux-recovery
 *
 * Provides a client for the MuxRecovery contract with optional filtering
 * query parameters on read methods.
 *
 * ## RecoveryStatus Enum
 *
 * `RecoveryStatus` mirrors the on-chain `RecoveryStatus` Soroban enum and
 * describes the lifecycle state of an account recovery request:
 *
 * ```
 *   None ──► Pending ──► Executed   (guardian executes after timelock)
 *                 └────► Cancelled  (owner cancels)
 * ```
 *
 * Transitions:
 * - `None → Pending`:     `initiate_recovery()` called by a registered guardian.
 * - `Pending → Executed`: `execute_recovery()` called after `RECOVERY_TIMELOCK` ledgers elapse.
 * - `Pending → Cancelled`: `cancel_recovery()` called by the owner at any time.
 * - `Executed` and `Cancelled` are terminal states — no further transitions.
 *   (A new request can be initiated after cancellation or expiry.)
 *
 * ## Guardian Set Rotation Rules
 *
 * The guardian set is the trust root for recovery. Rotation is governed by
 * the invariants in `docs/recovery-trust-model.md`:
 *
 * - **Membership**: only the current owner may rotate the guardian set.
 * - **Threshold**: the new set must contain at least `MIN_GUARDIANS` members
 *   and at most `MAX_GUARDIANS` members.
 * - **Ordering**: guardians are stored in ascending lexicographic order of
 *   their canonical `Address` string so the on-chain set is deterministic.
 * - **No duplicates**: the same guardian address may not appear twice.
 * - **No zero address**: the all-zero address is rejected.
 *
 * Rotation is idempotent: replaying the same rotation (same set, same
 * `rotationId`) is a no-op and returns the existing `rotationId`.
 *
 * ## Audit Events
 *
 * Contract tag: `mux_recv`
 *
 * | Action     | topics[1]   | Trigger             | Status transition   |
 * |------------|-------------|---------------------|---------------------|
 * | `init`     | `init`      | `initialize`        | —                   |
 * | `rec_init` | `rec_init`  | `initiate_recovery` | None → Pending      |
 * | `rec_exec` | `rec_exec`  | `execute_recovery`  | Pending → Executed  |
 * | `rec_cncl` | `rec_cncl`  | `cancel_recovery`   | Pending → Cancelled |
 * | `grd_rot`  | `grd_rot`   | `rotate_guardians`  | —                   |
 *
 * The `rec_init` event carries `(guardian, new_owner, initiated_at,
 * executable_at, expires_at)` so off-chain watchers can surface deadlines
 * without a follow-up RPC call.
 *
 * See `docs/recovery-trust-model.md` for the full security model.
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
import { pollTransaction } from "../horizon";

// ── Types ─────────────────────────────────────────────────────────────────────

/**
 * Lifecycle state of a recovery request. Mirrors the on-chain
 * `RecoveryStatus` Soroban enum in `contracts/mux-recovery/src/lib.rs`.
 *
 * State machine:
 * ```
 *   None ──► Pending ──► Executed   (guardian executes after RECOVERY_TIMELOCK)
 *                 └────► Cancelled  (owner cancels at any time)
 * ```
 *
 * `Executed` and `Cancelled` are **terminal** — no further state transitions
 * occur. A new recovery request can be initiated after `Cancelled` or after
 * an expired (but un-executed) `Pending` request.
 *
 * Use {@link recoveryStatusFromString} to parse the raw string returned by
 * `scValToNative`. Use {@link isTerminalRecoveryStatus} to check finality.
 *
 * Re-exported from the main package index as:
 * ```ts
 * import { RecoveryStatus } from "@mux-protocol/contracts";
 * ```
 */
export enum RecoveryStatus {
  /** No active recovery request. Default state after initialization. */
  None = "None",
  /**
   * A recovery has been initiated but `RECOVERY_TIMELOCK` ledgers have not
   * yet elapsed. The owner may call `cancel_recovery` to abort.
   */
  Pending = "Pending",
  /**
   * The recovery was executed after the timelock: ownership has been
   * transferred to the `new_owner` specified at initiation. Terminal state.
   */
  Executed = "Executed",
  /**
   * The recovery was cancelled by the current owner before execution.
   * Terminal state — a new recovery can be initiated after cancellation.
   */
  Cancelled = "Cancelled",
}

/**
 * Parse a raw string value from a Soroban RPC response into a typed
 * {@link RecoveryStatus} variant.
 *
 * `scValToNative` converts the on-chain `RecoveryStatus` enum to a plain
 * string (e.g. `"Pending"`). Use this helper to convert it safely.
 *
 * Throws if the value is not a recognised variant.
 *
 * @example
 * ```ts
 * import { recoveryStatusFromString, RecoveryStatus } from "@mux-protocol/contracts";
 *
 * const raw = "Pending"; // from scValToNative(retval)
 * const status = recoveryStatusFromString(raw);
 * if (status === RecoveryStatus.Pending) {
 *   console.log("Recovery is in progress");
 * }
 * ```
 */
export function recoveryStatusFromString(raw: string): RecoveryStatus {
  switch (raw) {
    case "None":      return RecoveryStatus.None;
    case "Pending":   return RecoveryStatus.Pending;
    case "Executed":  return RecoveryStatus.Executed;
    case "Cancelled": return RecoveryStatus.Cancelled;
    default:
      throw new Error(`Unknown RecoveryStatus value: "${raw}"`);
  }
}

/**
 * Return true if a recovery is in a terminal state (Executed or Cancelled)
 * and cannot advance further.
 *
 * @example
 * ```ts
 * if (isTerminalRecoveryStatus(status)) {
 *   console.log("No active recovery — a new one can be initiated.");
 * }
 * ```
 */
export function isTerminalRecoveryStatus(status: RecoveryStatus): boolean {
  return status === RecoveryStatus.Executed || status === RecoveryStatus.Cancelled;
}

/**
 * Return true if a recovery can be cancelled (only when Pending).
 */
export function isCancellableRecoveryStatus(status: RecoveryStatus): boolean {
  return status === RecoveryStatus.Pending;
}

/** Mirrors the on-chain RecoveryRequest struct. */
export interface RecoveryRequest {
  newOwner: Address;
  initiatedAt: number;
  executableAt: number;
  expiresAt: number;
  status: RecoveryStatus;
}

// ── Recovery cancel/advance ───────────────────────────────────────────────────

/**
 * Stable error codes for recovery cancel/advance. Mirrors the on-chain
 * `RecoveryError` codes so clients can branch without string matching.
 *
 * Deny-by-default: any condition not explicitly permitted fails closed.
 */
export enum RecoveryErrorCode {
  /** Caller is not the current owner (cancel) or a registered guardian (advance). */
  Unauthorized = "UNAUTHORIZED",
  /** No recovery request exists (status is `None`). */
  NoActiveRecovery = "NO_ACTIVE_RECOVERY",
  /** The recovery is already in a terminal state (Executed/Cancelled). */
  AlreadyTerminal = "ALREADY_TERMINAL",
  /** `execute_recovery` was called before `executableAt` (timelock not elapsed). */
  TimelockNotElapsed = "TIMELOCK_NOT_ELAPSED",
  /** The recovery request has expired (`expiresAt` passed). */
  RecoveryExpired = "RECOVERY_EXPIRED",
  /** The supplied `requestId` does not match the active request (replay/conflict). */
  RequestIdConflict = "REQUEST_ID_CONFLICT",
  /** A dependency (RPC/Horizon) was unavailable; the write failed closed. */
  DependencyUnavailable = "DEPENDENCY_UNAVAILABLE",
}

/**
 * Typed error thrown by {@link MuxRecoveryClient.cancelRecovery} and
 * {@link MuxRecoveryClient.advanceRecovery}. Carries a stable
 * {@link RecoveryErrorCode} and an optional `correlationId` for log/trace
 * correlation.
 */
export class RecoveryError extends Error {
  readonly code: RecoveryErrorCode;
  readonly correlationId?: string;

  constructor(
    code: RecoveryErrorCode,
    message: string,
    correlationId?: string
  ) {
    super(message);
    this.name = "RecoveryError";
    this.code = code;
    this.correlationId = correlationId;
  }
}

/**
 * Audit event emitted for recovery cancel/advance. Mirrors the on-chain
 * `mux_recv` audit events documented in `docs/audit-events.md`:
 *
 * - `rec_cncl` — `cancel_recovery` (Pending → Cancelled)
 * - `rec_exec` — `execute_recovery` (Pending → Executed)
 *
 * The `correlationId` is propagated from the request so off-chain watchers
 * can join the audit event with the originating call.
 */
export interface RecoveryAuditEvent {
  /** Contract tag, always `mux_recv`. */
  contract: "mux_recv";
  /** `rec_cncl` or `rec_exec`. */
  action: "rec_cncl" | "rec_exec";
  /** The account whose recovery is being cancelled/advanced. */
  account: string;
  /** The caller that triggered the transition (owner or guardian). */
  actor: string;
  /** Status before the transition. */
  from: RecoveryStatus;
  /** Status after the transition. */
  to: RecoveryStatus;
  /** Ledger timestamp of the transition. */
  at: number;
  /** Correlation id propagated from the request, when supplied. */
  correlationId?: string;
}

/**
 * Options accepted by {@link MuxRecoveryClient.cancelRecovery} and
 * {@link MuxRecoveryClient.advanceRecovery}.
 */
export interface RecoveryTransitionOptions {
  /**
   * Idempotency key. Replaying the same `requestId` for the same transition
   * is a no-op and returns the existing audit event rather than re-submitting.
   */
  requestId?: string;
  /** Correlation id propagated into the emitted audit event and error. */
  correlationId?: string;
}

/**
 * Result of a recovery cancel/advance transition.
 */
export interface RecoveryTransitionResult {
  /** The audit event emitted for this transition. */
  event: RecoveryAuditEvent;
  /** True when the transition was a replay of an already-applied request. */
  replayed: boolean;
}

/**
 * Validate that a recovery transition is permitted by policy. Deny-by-default:
 * any state not explicitly permitted throws a {@link RecoveryError}.
 *
 * @param status   Current recovery status.
 * @param action   `cancel` or `advance`.
 * @param now      Current ledger time (seconds).
 * @param request  The active request (required for `advance`).
 */
export function assertRecoveryTransitionAllowed(
  status: RecoveryStatus,
  action: "cancel" | "advance",
  now: number,
  request?: RecoveryRequest,
  correlationId?: string
): void {
  if (status === RecoveryStatus.None) {
    throw new RecoveryError(
      RecoveryErrorCode.NoActiveRecovery,
      "No active recovery request",
      correlationId
    );
  }
  if (isTerminalRecoveryStatus(status)) {
    throw new RecoveryError(
      RecoveryErrorCode.AlreadyTerminal,
      `Recovery is already ${status}`,
      correlationId
    );
  }
  if (action === "advance") {
    if (!request) {
      throw new RecoveryError(
        RecoveryErrorCode.NoActiveRecovery,
        "No active recovery request to advance",
        correlationId
      );
    }
    if (now < request.executableAt) {
      throw new RecoveryError(
        RecoveryErrorCode.TimelockNotElapsed,
        "Recovery timelock has not elapsed",
        correlationId
      );
    }
    if (now > request.expiresAt) {
      throw new RecoveryError(
        RecoveryErrorCode.RecoveryExpired,
        "Recovery request has expired",
        correlationId
      );
    }
  }
}

// ── Guardian set rotation ─────────────────────────────────────────────────────

/**
 * Stable error codes for guardian set rotation. Mirrors the on-chain
 * `GuardianRotationError` codes so clients can branch without string matching.
 *
 * Deny-by-default: any condition not explicitly permitted fails closed.
 */
export enum GuardianRotationErrorCode {
  /** Caller is not the current owner. */
  Unauthorized = "UNAUTHORIZED",
  /** Fewer guardians than `MIN_GUARDIANS`. */
  BelowMinGuardians = "BELOW_MIN_GUARDIANS",
  /** More guardians than `MAX_GUARDIANS`. */
  AboveMaxGuardians = "ABOVE_MAX_GUARDIANS",
  /** The same guardian address appears more than once. */
  DuplicateGuardian = "DUPLICATE_GUARDIAN",
  /** A guardian address is the all-zero address. */
  ZeroGuardian = "ZERO_GUARDIAN",
  /** The supplied `rotationId` was already used with a different set. */
  RotationIdConflict = "ROTATION_ID_CONFLICT",
}

/**
 * Typed error thrown by {@link MuxRecoveryClient.rotateGuardians} and
 * {@link validateGuardianSet}. Carries a stable {@link GuardianRotationErrorCode}
 * and an optional `correlationId` for log/trace correlation.
 */
export class GuardianRotationError extends Error {
  readonly code: GuardianRotationErrorCode;
  readonly correlationId?: string;

  constructor(
    code: GuardianRotationErrorCode,
    message: string,
    correlationId?: string
  ) {
    super(message);
    this.name = "GuardianRotationError";
    this.code = code;
    this.correlationId = correlationId;
  }
}

/** Minimum number of guardians permitted in a rotated set. */
export const MIN_GUARDIANS = 1;
/** Maximum number of guardians permitted in a rotated set. */
export const MAX_GUARDIANS = 10;

/** The all-zero Stellar address, rejected as a guardian. */
const ZERO_ADDRESS = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

/**
 * Validate a proposed guardian set against the rotation invariants. Throws a
 * {@link GuardianRotationError} on the first violation (deny-by-default).
 *
 * @param guardians     Proposed guardian addresses.
 * @param correlationId Optional correlation id propagated into the error.
 */
export function validateGuardianSet(
  guardians: Address[],
  correlationId?: string
): void {
  if (guardians.length < MIN_GUARDIANS) {
    throw new GuardianRotationError(
      GuardianRotationErrorCode.BelowMinGuardians,
      `Guardian set must contain at least ${MIN_GUARDIANS} member(s)`,
      correlationId
    );
  }
  if (guardians.length > MAX_GUARDIANS) {
    throw new GuardianRotationError(
      GuardianRotationErrorCode.AboveMaxGuardians,
      `Guardian set must contain at most ${MAX_GUARDIANS} members`,
      correlationId
    );
  }
  const seen = new Set<string>();
  for (const guardian of guardians) {
    const key = guardian.toString();
    if (key === ZERO_ADDRESS) {
      throw new GuardianRotationError(
        GuardianRotationErrorCode.ZeroGuardian,
        "Guardian set contains the zero address",
        correlationId
      );
    }
    if (seen.has(key)) {
      throw new GuardianRotationError(
        GuardianRotationErrorCode.DuplicateGuardian,
        `Duplicate guardian address: ${key}`,
        correlationId
      );
    }
    seen.add(key);
  }
}

/**
 * Canonicalise a guardian set: dedupe is rejected upstream, so this only
 * sorts into ascending lexicographic order of the canonical address string
 * so the on-chain set is deterministic.
 */
export function canonicalizeGuardianSet(guardians: Address[]): Address[] {
  return [...guardians].sort((a, b) =>
    a.toString() < b.toString() ? -1 : a.toString() > b.toString() ? 1 : 0
  );
}

// ── Client ────────────────────────────────────────────────────────────────────

/**
 * Client for the `mux-recovery` contract.
 *
 * All write methods are fail-closed: on dependency outage (RPC/Horizon) they
 * throw a {@link RecoveryError} with {@link RecoveryErrorCode.DependencyUnavailable}
 * rather than silently succeeding. Replayed requests (same `requestId`) are
 * idempotent and return the previously emitted audit event.
 */
export class MuxRecoveryClient {
  private readonly contract: Contract;
  private readonly server: SorobanRpc.Server;
  private readonly source: Keypair;
  private readonly appliedRequests = new Map<string, RecoveryTransitionResult>();

  constructor(
    contractId: string,
    server: SorobanRpc.Server,
    source: Keypair
  ) {
    this.contract = new Contract(contractId);
    this.server = server;
    this.source = source;
  }

  /**
   * Cancel a pending recovery. Only the current owner may cancel; the
   * transition is `Pending → Cancelled` and emits a `rec_cncl` audit event.
   *
   * Idempotent: replaying the same `requestId` returns the prior result.
   */
  async cancelRecovery(
    account: Address,
    options: RecoveryTransitionOptions = {}
  ): Promise<RecoveryTransitionResult> {
    return this.transition("cancel", account, options);
  }

  /**
   * Advance (execute) a pending recovery after the timelock. Only a registered
   * guardian may advance; the transition is `Pending → Executed` and emits a
   * `rec_exec` audit event.
   *
   * Idempotent: replaying the same `requestId` returns the prior result.
   */
  async advanceRecovery(
    account: Address,
    options: RecoveryTransitionOptions = {}
  ): Promise<RecoveryTransitionResult> {
    return this.transition("advance", account, options);
  }

  private async transition(
    action: "cancel" | "advance",
    account: Address,
    options: RecoveryTransitionOptions
  ): Promise<RecoveryTransitionResult> {
    const { requestId, correlationId } = options;

    // Idempotency / replay protection: a repeated requestId is a no-op.
    if (requestId) {
      const prior = this.appliedRequests.get(requestId);
      if (prior) {
        return { event: prior.event, replayed: true };
      }
    }

    let status: RecoveryStatus;
    let request: RecoveryRequest | undefined;
    try {
      ({ status, request } = await this.readRecovery(account));
    } catch (err) {
      // Fail closed on dependency outage — never assume a writable state.
      throw new RecoveryError(
        RecoveryErrorCode.DependencyUnavailable,
        `Recovery read failed: ${(err as Error).message}`,
        correlationId
      );
    }

    assertRecoveryTransitionAllowed(
      status,
      action,
      Math.floor(Date.now() / 1000),
      request,
      correlationId
    );

    const method = action === "cancel" ? "cancel_recovery" : "execute_recovery";
    const from = status;
    const to =
      action === "cancel" ? RecoveryStatus.Cancelled : RecoveryStatus.Executed;

    try {
      await this.submit(method, account);
    } catch (err) {
      throw new RecoveryError(
        RecoveryErrorCode.DependencyUnavailable,
        `Recovery ${action} failed: ${(err as Error).message}`,
        correlationId
      );
    }

    const event: RecoveryAuditEvent = {
      contract: "mux_recv",
      action: action === "cancel" ? "rec_cncl" : "rec_exec",
      account: account.toString(),
      actor: this.source.publicKey(),
      from,
      to,
      at: Math.floor(Date.now() / 1000),
      correlationId,
    };

    const result: RecoveryTransitionResult = { event, replayed: false };
    if (requestId) {
      this.appliedRequests.set(requestId, result);
    }
    return result;
  }

  private async readRecovery(
    account: Address
  ): Promise<{ status: RecoveryStatus; request?: RecoveryRequest }> {
    const op = this.contract.call(
      "get_recovery",
      nativeToScVal(account, { type: "address" })
    );
    const tx = new TransactionBuilder(await this.server.getAccount(this.source.publicKey()), {
      fee: "100",
      networkPassphrase: (await this.server.getNetwork()).passphrase,
    })
      .addOperation(op)
      .setTimeout(30)
      .build();
    const sim = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(sim)) {
      throw new Error(sim.error);
    }
    const raw = scValToNative(
      (sim as SorobanRpc.Api.SimulateTransactionSuccessResponse).result!.retval
    ) as { status: string; new_owner: string; initiated_at: number; executable_at: number; expires_at: number } | null;
    if (!raw) {
      return { status: RecoveryStatus.None };
    }
    const status = recoveryStatusFromString(raw.status);
    const request: RecoveryRequest = {
      newOwner: new Address(raw.new_owner),
      initiatedAt: raw.initiated_at,
      executableAt: raw.executable_at,
      expiresAt: raw.expires_at,
      status,
    };
    return { status, request };
  }

  private async submit(method: string, account: Address): Promise<void> {
    const op = this.contract.call(
      method,
      nativeToScVal(account, { type: "address" })
    );
    const account_ = await this.server.getAccount(this.source.publicKey());
    const tx = new TransactionBuilder(account_, {
      fee: "100",
      networkPassphrase: (await this.server.getNetwork()).passphrase,
    })
      .addOperation(op)
      .setTimeout(30)
      .build();
    tx.sign(this.source);
    const sent = await this.server.sendTransaction(tx);
    if (sent.status === "ERROR") {
      throw new Error(`submit failed: ${JSON.stringify(sent.errorResult)}`);
    }
    await pollTransaction(this.server, sent.hash);
  }
}
