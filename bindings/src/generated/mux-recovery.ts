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
 * Validate a proposed guardian set against the rotation invariants from
 * `docs/recovery-trust-model.md`:
 *
 * - membership count within `[MIN_GUARDIANS, MAX_GUARDIANS]`
 * - no duplicate addresses
 * - no zero address
 *
 * Returns the canonical, ascending-ordered, de-duplicated set on success.
 * Throws {@link GuardianRotationError} (fail-closed) on any violation.
 *
 * @param guardians Proposed guardian addresses.
 * @param correlationId Optional correlation id propagated into the error.
 */
export function validateGuardianSet(
  guardians: Address[],
  correlationId?: string
): Address[] {
  if (guardians.length < MIN_GUARDIANS) {
    throw new GuardianRotationError(
      GuardianRotationErrorCode.BelowMinGuardians,
      `Guardian set must contain at least ${MIN_GUARDIANS} guardian(s)`,
      correlationId
    );
  }
  if (guardians.length > MAX_GUARDIANS) {
    throw new GuardianRotationError(
      GuardianRotationErrorCode.AboveMaxGuardians,
      `Guardian set must contain at most ${MAX_GUARDIANS} guardian(s)`,
      correlationId
    );
  }

  const seen = new Set<string>();
  for (const g of guardians) {
    const key = g.toString();
    if (key === ZERO_ADDRESS) {
      throw new GuardianRotationError(
        GuardianRotationErrorCode.ZeroGuardian,
        "Guardian set must not contain the zero address",
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

  // Canonical ordering: ascending lexicographic by canonical address string.
  return [...guardians].sort((a, b) =>
    a.toString() < b.toString() ? -1 : a.toString() > b.toString() ? 1 : 0
  );
}

/** Optional filter parameters for recovery queries. */
export interface RecoveryQueryFilters {
  status?: RecoveryStatus;
  guardian?: Address;
  initiatedAfter?: number;
  initiatedBefore?: number;
}

// ── Client ────────────────────────────────────────────────────────────────────

export interface MuxRecoveryClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export class MuxRecoveryClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxRecoveryClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  // ── Write operations ────────────────────────────────────────────────────────

  async initialize(
    sourceKeypair: Keypair,
    owner: Address,
    guardians: Address[]
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "initialize", [
      nativeToScVal(owner.toString(), { type: "address" }),
      xdr.ScVal.scvVec(
        guardians.map((g) => nativeToScVal(g.toString(), { type: "address" }))
      ),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Rotate the guardian set. Only the current owner may call this.
   *
   * Enforces the rotation invariants (threshold, membership, ordering, no
   * duplicates) via {@link validateGuardianSet} before submitting, and passes
   * a caller-supplied `rotationId` for on-chain idempotency / replay
   * protection. Replaying the same `rotationId` with the same set is a no-op
   * on-chain; replaying it with a different set fails with
   * {@link GuardianRotationErrorCode.RotationIdConflict}.
   *
   * @param sourceKeypair Owner keypair authorizing the rotation.
   * @param owner Current owner address (must match on-chain owner).
   * @param guardians Proposed guardian set (validated + canonicalized).
   * @param rotationId Idempotency key for this rotation.
   * @param correlationId Optional correlation id for logs/traces.
   */
  async rotateGuardians(
    sourceKeypair: Keypair,
    owner: Address,
    guardians: Address[],
    rotationId: string,
    correlationId?: string
  ): Promise<void> {
    if (!rotationId) {
      throw new GuardianRotationError(
        GuardianRotationErrorCode.RotationIdConflict,
        "rotationId is required for idempotent guardian rotation",
        correlationId
      );
    }
    const canonical = validateGuardianSet(guardians, correlationId);
    const tx = await this.buildTx(sourceKeypair, "rotate_guardians", [
      nativeToScVal(owner.toString(), { type: "address" }),
      xdr.ScVal.scvVec(
        canonical.map((g) => nativeToScVal(g.toString(), { type: "address" }))
      ),
      nativeToScVal(rotationId, { type: "string" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async initiateRecovery(
    sourceKeypair: Keypair,
    guardian: Address,
    newOwner: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "initiate_recovery", [
      nativeToScVal(guardian.toString(), { type: "address" }),
      nativeToScVal(newOwner.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async cancelRecovery(sourceKeypair: Keypair): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "cancel_recovery", []);
    await this.submit(tx, sourceKeypair);
  }

  async executeRecovery(
    sourceKeypair: Keypair,
    guardian: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "execute_recovery", [
      nativeToScVal(guardian.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Link a registry contract address to this recovery contract.
   * Only the current owner may call this method.
   */
  async setRegistry(
    sourceKeypair: Keypair,
    owner: Address,
    registryId: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "set_registry", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(registryId.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Return the linked registry contract address, or null if
   * no registry has been linked yet.
   */
  async getRegistry(): Promise<Address | null> {
    const result = await this.server.simulateTransaction(
      await this.buildTx(
        // read-only simulation uses a throwaway source; caller supplies none
        Keypair.random(),
        "get_registry",
        []
      )
    );
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`get_registry simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse)
      .result?.retval;
    if (!retval) return null;
    const native = scValToNative(retval);
    return native ? new Address(native) : null;
  }

  // ── Internals ───────────────────────────────────────────────────────────────

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

  private async submit(tx: Transaction, sourceKeypair: Keypair): Promise<void> {
    tx.sign(sourceKeypair);
    const sent = await this.server.sendTransaction(tx);
    await pollTransaction(this.server, sent.hash);
  }
}
