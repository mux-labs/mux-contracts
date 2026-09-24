/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-spending-policy
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
import type { SpendingPolicyLimit } from "../types";
import { pollTransaction } from "../horizon";

export interface MuxSpendingPolicyClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

/** Minimum spend limit value (> 0). Setting limit <= 0 fails with InvalidInput (code 6). */
export const SPEND_LIMIT_MIN = 1n;

/** Maximum allowable i128 value for spend limits (2^127 - 1). */
export const SPEND_LIMIT_MAX = 170141183460469231731687303715884105727n;

/** Minimum period length in ledgers (> 0). period_ledgers == 0 fails with InvalidPeriod (code 7). */
export const PERIOD_LEDGERS_MIN = 1;

/** Maximum period length in ledgers (u32::MAX). */
export const PERIOD_LEDGERS_MAX = 4294967295;

/**
 * Validates spend limit policy boundaries according to spending-policy-semantics.md:
 * - limit must be > 0 and <= i128::MAX
 * - period_ledgers must be > 0 and <= u32::MAX
 */
export function validateSpendPolicyBoundaries(limit: bigint, periodLedgers?: number): void {
  if (limit <= 0n) {
    throw new Error(`InvalidInput: limit must be strictly positive (> 0), got ${limit}`);
  }
  if (limit > SPEND_LIMIT_MAX) {
    throw new Error(`InvalidInput: limit exceeds maximum i128 value (${SPEND_LIMIT_MAX})`);
  }
  if (periodLedgers !== undefined) {
    if (periodLedgers <= 0) {
      throw new Error(`InvalidPeriod: period_ledgers must be strictly positive (> 0), got ${periodLedgers}`);
    }
    if (periodLedgers > PERIOD_LEDGERS_MAX) {
      throw new Error(`InvalidPeriod: period_ledgers exceeds maximum u32 value (${PERIOD_LEDGERS_MAX})`);
    }
  }
}

/**
 * Validates check_spend amount boundaries:
 * - amount must be >= 0 (non-negative)
 * - amount must be <= i128::MAX
 */
export function validateSpendAmountBoundaries(amount: bigint): void {
  if (amount < 0n) {
    throw new Error(`InvalidInput: spend amount cannot be negative, got ${amount}`);
  }
  if (amount > SPEND_LIMIT_MAX) {
    throw new Error(`InvalidInput: spend amount exceeds maximum i128 value (${SPEND_LIMIT_MAX})`);
  }
}

export class MuxSpendingPolicyClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxSpendingPolicyClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  async initialize(sourceKeypair: Keypair, admin: Address): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "initialize", [
      nativeToScVal(admin.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async setPolicy(
    sourceKeypair: Keypair,
    account: Address,
    asset: Address,
    limit: bigint,
    periodLedgers?: number
  ): Promise<void> {
    validateSpendPolicyBoundaries(limit, periodLedgers);
    const args: xdr.ScVal[] = [
      nativeToScVal(account.toString(), { type: "address" }),
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(limit, { type: "i128" }),
    ];
    if (periodLedgers !== undefined) {
      args.push(nativeToScVal(periodLedgers, { type: "u32" }));
    }
    const tx = await this.buildTx(sourceKeypair, "set_policy", args);
    await this.submit(tx, sourceKeypair);
  }

  async getPolicy(
    sourceKeypair: Keypair,
    account: Address,
    asset: Address
  ): Promise<SpendingPolicyLimit> {
    const tx = await this.buildTx(sourceKeypair, "get_policy", [
      nativeToScVal(account.toString(), { type: "address" }),
      nativeToScVal(asset.toString(), { type: "address" }),
    ]);
    return this.simulateRead<SpendingPolicyLimit>(tx);
  }

  /** Simulate-only: returns void if within limit, throws if exceeded or policy not found. */
  async checkSpend(
    sourceKeypair: Keypair,
    account: Address,
    asset: Address,
    amount: bigint
  ): Promise<void> {
    validateSpendAmountBoundaries(amount);
    const tx = await this.buildTx(sourceKeypair, "check_spend", [
      nativeToScVal(account.toString(), { type: "address" }),
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(amount, { type: "i128" }),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`check_spend failed: ${result.error}`);
    }
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
    const preparedTx = SorobanRpc.assembleTransaction(
      tx,
      simResult as SorobanRpc.Api.SimulateTransactionSuccessResponse
    ).build();
    preparedTx.sign(signer);
    const sendResult = await this.server.sendTransaction(preparedTx);
    if (sendResult.status === "ERROR") {
      throw new Error(`Transaction failed: ${JSON.stringify(sendResult.errorResult)}`);
    }
    await pollTransaction(this.server, sendResult.hash);
  }
}
