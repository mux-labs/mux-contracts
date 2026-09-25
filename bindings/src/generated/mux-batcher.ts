/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-batcher
 */

import {
  Address,
  Contract,
  Keypair,
  nativeToScVal,
  SorobanRpc,
  Transaction,
  TransactionBuilder,
  xdr,
} from "@stellar/stellar-sdk";
import type { BatcherMeta, BatchOperationKind, BatchResult, MuxBatcherError, Operation } from "../types";
import { pollTransaction } from "../horizon";

/**
 * Maximum number of operations accepted in a single batch.
 *
 * Mirrors the on-chain `MAX_BATCH_SIZE` constant in the `mux-batcher`
 * contract. `executeBatch` / `submitBatch` reject any batch larger than this
 * with `MuxBatcherError::BatchTooLarge`, and `estimateFees` throws for
 * `opCount` of 0 or greater than this value. See `docs/abi_reference.md`.
 */
export const MAX_BATCH_SIZE = 100;

/**
 * Maximum aggregate operation weight accepted in a single batch.
 *
 * Mirrors the on-chain `MAX_BATCH_WEIGHT` constant in the `mux-batcher`
 * contract. Each operation contributes a weight (see `operationWeight`); the
 * sum across a batch must not exceed this cap or the contract fails closed
 * with `MuxBatcherError::BatchWeightExceeded`. This bounds the total work a
 * single batch can force the contract to perform, independent of the raw
 * operation count. See `docs/batching-limits.md`.
 */
export const MAX_BATCH_WEIGHT = 10_000;

/**
 * Stable error codes surfaced by the `mux-batcher` contract for batching cap
 * violations. These mirror the on-chain `MuxBatcherError` discriminants so
 * clients can branch on them without string matching.
 */
export const MUX_BATCHER_ERROR_CODES = {
  /** Batch contained zero operations. */
  EmptyBatch: 1,
  /** Batch exceeded `MAX_BATCH_SIZE` operations. */
  BatchTooLarge: 2,
  /** Batch exceeded `MAX_BATCH_WEIGHT` aggregate weight. */
  BatchWeightExceeded: 3,
  /** Caller is not authorized for the privileged batch surface. */
  Unauthorized: 4,
  /** Batch was already executed (replay / idempotency guard). */
  BatchAlreadyExecuted: 5,
} as const;

export type MuxBatcherErrorCode =
  (typeof MUX_BATCHER_ERROR_CODES)[keyof typeof MUX_BATCHER_ERROR_CODES];

/**
 * Error thrown client-side when a batch violates an on-chain cap before it is
 * ever submitted. Carries the stable `code` so callers can branch on it.
 */
export class MuxBatcherCapError extends Error {
  readonly code: MuxBatcherErrorCode;

  constructor(code: MuxBatcherErrorCode, message: string) {
    super(message);
    this.name = "MuxBatcherCapError";
    this.code = code;
  }
}

/**
 * Per-operation weight used to compute aggregate batch weight. Mirrors the
 * on-chain weighting so client-side pre-checks match contract enforcement.
 */
export function operationWeight(op: Operation): number {
  // Base cost per operation plus the cost of each argument. Kept in lockstep
  // with the contract's `operation_weight` helper.
  return 1 + op.args.length;
}

/**
 * Compute the aggregate weight of a batch. Used for client-side pre-checks so
 * oversized batches fail fast with a stable error code instead of a wasted
 * round-trip.
 */
export function batchWeight(ops: Operation[]): number {
  return ops.reduce((sum, op) => sum + operationWeight(op), 0);
}

/**
 * Fail-closed pre-check for a batch. Throws `MuxBatcherCapError` with a stable
 * code when the batch is empty, exceeds `MAX_BATCH_SIZE`, or exceeds
 * `MAX_BATCH_WEIGHT`. The contract enforces the same caps; this only avoids
 * submitting a batch that is guaranteed to be rejected.
 */
export function assertBatchWithinCaps(ops: Operation[]): void {
  if (ops.length === 0) {
    throw new MuxBatcherCapError(
      MUX_BATCHER_ERROR_CODES.EmptyBatch,
      "Batch must contain at least one operation"
    );
  }
  if (ops.length > MAX_BATCH_SIZE) {
    throw new MuxBatcherCapError(
      MUX_BATCHER_ERROR_CODES.BatchTooLarge,
      `Batch size ${ops.length} exceeds MAX_BATCH_SIZE (${MAX_BATCH_SIZE})`
    );
  }
  const weight = batchWeight(ops);
  if (weight > MAX_BATCH_WEIGHT) {
    throw new MuxBatcherCapError(
      MUX_BATCHER_ERROR_CODES.BatchWeightExceeded,
      `Batch weight ${weight} exceeds MAX_BATCH_WEIGHT (${MAX_BATCH_WEIGHT})`
    );
  }
}

export interface MuxBatcherClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export class MuxBatcherClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxBatcherClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  async executeBatch(
    sourceKeypair: Keypair,
    caller: Address,
    ops: Operation[]
  ): Promise<BatchResult> {
    assertBatchWithinCaps(ops);
    const opsVal = xdr.ScVal.scvVec(ops.map(this.operationToScVal));
    const tx = await this.buildTx(sourceKeypair, "execute_batch", [
      nativeToScVal(caller.toString(), { type: "address" }),
      opsVal,
    ]);
    return this.submitAndRead<BatchResult>(tx, sourceKeypair);
  }

  async maxBatchSize(sourceKeypair: Keypair): Promise<number> {
    const tx = await this.buildTx(sourceKeypair, "max_batch_size", []);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("No return value");
    return retval.value() as number;
  }

  /**
   * Return the on-chain aggregate batch weight cap. Pure read — no transaction
   * is submitted. Mirrors `MAX_BATCH_WEIGHT` but reads the live contract value
   * so clients stay in sync if the cap is ever raised via governance.
   */
  async maxBatchWeight(sourceKeypair: Keypair): Promise<number> {
    const tx = await this.buildTx(sourceKeypair, "max_batch_weight", []);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("No return value");
    return retval.value() as number;
  }

  async simulateBatch(
    sourceKeypair: Keypair,
    caller: Address,
    ops: Operation[]
  ): Promise<BatchResult> {
    assertBatchWithinCaps(ops);
    const opsVal = xdr.ScVal.scvVec(ops.map(this.operationToScVal));
    const tx = await this.buildTx(sourceKeypair, "simulate_batch", [
      nativeToScVal(caller.toString(), { type: "address" }),
      opsVal,
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("No return value");
    const native = retval.value() as unknown as { success_count: number; failure_count: number };
    return { successCount: native.success_count, failureCount: native.failure_count };
  }

  /**
   * Return a conservative fee estimate (in stroops) for a batch of `opCount`
   * operations. Throws if `opCount` is 0 or exceeds the contract's
   * `MAX_BATCH_SIZE`. Pure read — no transaction is submitted.
   */
  async estimateFees(sourceKeypair: Keypair, opCount: number): Promise<number> {
    if (opCount === 0) {
      throw new MuxBatcherCapError(
        MUX_BATCHER_ERROR_CODES.EmptyBatch,
        "opCount must be at least 1"
      );
    }
    if (opCount > MAX_BATCH_SIZE) {
      throw new MuxBatcherCapError(
        MUX_BATCHER_ERROR_CODES.BatchTooLarge,
        `opCount ${opCount} exceeds MAX_BATCH_SIZE (${MAX_BATCH_SIZE})`
      );
    }
    const tx = await this.buildTx(sourceKeypair, "estimate_fees", [
      xdr.ScVal.scvU32(opCount),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("No return value");
    return retval.value() as number;
  }

  /**
   * Submit a batch on behalf of the transaction invoker. Convenience wrapper
   * around `executeBatch` that derives the caller from the invoking address,
   * so callers do not need to pass an explicit `caller` argument.
   *
   * Emits the same events as `executeBatch`.
   */
  async submitBatch(
    sourceKeypair: Keypair,
    ops: Operation[]
  ): Promise<BatchResult> {
    assertBatchWithinCaps(ops);
    const opsVal = xdr.ScVal.scvVec(ops.map(this.operationToScVal));
    const tx = await this.buildTx(sourceKeypair, "submit_batch", [opsVal]);
    return this.submitAndRead<BatchResult>(tx, sourceKeypair);
  }

  /**
   * Store registry metadata (description, author) for this batcher instance.
   *
   * Can only be called once; subsequent calls throw with `MetadataAlreadySet`.
   * No authorization is required — metadata is informational only and is
   * expected to be set by the deployer immediately after deployment.
   */
  async setRegistryMetadata(
    sourceKeypair: Keypair,
    description: string,
    author: string
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "set_registry_metadata", [
      nativeToScVal(description, { type: "string" }),
      nativeToScVal(author, { type: "string" }),
    ]);
    await this.submitAndRead<void>(tx, sourceKeypair);
  }

  /**
   * Return the registry metadata for this batcher instance, or `null` if not
   * set. Pure read — no transaction is submitted.
   */
  async getRegistryMetadata(sourceKeypair: Keypair): Promise<BatcherMeta | null> {
    const tx = await this.buildTx(sourceKeypair, "get_registry_metadata", []);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) return null;
    // On-chain Option<BatcherMeta> — void/unit retval means None.
    const raw = retval.value();
    if (raw === undefined || raw === null) return null;
    const native = raw as { description: string; author: string };
    return { description: native.description, author: native.author };
  }

  // ── Private helpers ──────────────────────────────────────────────────────────

  private operationToScVal(op: Operation): xdr.ScVal {
    return xdr.ScVal.scvMap([
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("args"),
        val: xdr.ScVal.scvVec(op.args),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("fn_name"),
        val: xdr.ScVal.scvSymbol(op.fnName),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("kind"),
        val: xdr.ScVal.scvVec([
          xdr.ScVal.scvSymbol(op.kind),
        ]),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("require_success"),
        val: nativeToScVal(op.requireSuccess, { type: "bool" }),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("target"),
        val: nativeToScVal(op.target.toString(), { type: "address" }),
      }),
    ]);
  }

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

  private async submitAndRead<T>(tx: Transaction, signer: Keypair): Promise<T> {
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
      throw new Error(`Transaction failed: ${sendResult.errorResult?.toXDR("base64") ?? "unknown"}`);
    }
    return pollTransaction(this.server, sendResult.hash) as Promise<T>;
  }
}
