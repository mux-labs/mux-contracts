/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-policy
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

export interface MuxPolicyClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

/** Minimum settable daily limit (> 0). `limit <= 0` fails with InvalidAmount (code 6). */
export const POLICY_LIMIT_MIN = 1n;

/** Minimum window length in ledgers (> 0). `day_ledgers == 0` fails with InvalidPeriod (code 7). */
export const POLICY_DAY_LEDGERS_MIN = 1;

/** Default window length in ledgers, mirroring the contract's `DEFAULT_DAY_LEDGERS` (~1 day at ~5 s/ledger). */
export const POLICY_DEFAULT_DAY_LEDGERS = 17280;

/** Maximum number of wallets with configured limits (storage-griefing guard, code 8 when exceeded). */
export const POLICY_MAX_WALLETS = 256;

/**
 * Per-wallet daily spend limit record, mirroring the on-chain `DailyLimit`
 * type in `contracts/mux-policy/src/lib.rs`. Field names match the on-chain
 * (snake_case) serialization produced by `scValToNative`.
 */
export interface DailyLimit {
  /** Maximum amount allowed per window, in the asset's smallest unit. */
  limit: bigint;
  /** Amount spent in the current window. */
  spent: bigint;
  /** Ledger sequence at which the current window expires and `spent` resets. */
  reset_ledger: number;
  /** Window length in ledgers (set at limit-creation time). */
  day_ledgers: number;
  /** Optional registry contract address linked for cross-contract validation. */
  registry_id: string | null;
}

/**
 * Validates daily-limit boundaries the same way the contract does, so invalid
 * input fails fast with the variable at fault instead of an opaque simulation
 * error. The contract remains the source of truth.
 */
export function validatePolicyBoundaries(limit: bigint, dayLedgers: number): void {
  if (limit < POLICY_LIMIT_MIN) {
    throw new Error(`InvalidAmount: limit must be strictly positive (> 0), got ${limit}`);
  }
  if (dayLedgers < POLICY_DAY_LEDGERS_MIN) {
    throw new Error(
      `InvalidPeriod: day_ledgers must be strictly positive (> 0), got ${dayLedgers}`
    );
  }
}

export class MuxPolicyClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxPolicyClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  /** Initialize the policy contract with an admin address (one-time; admin authorizes). */
  async initialize(sourceKeypair: Keypair, admin: string | Address): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "initialize", [
      nativeToScVal(admin.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Set or update the daily spend limit for `wallet`. Admin authorizes.
   *
   * `registryId` optionally links the limit to a mux-registry deployment for
   * fail-closed validation at spend time; omit it (or pass null) for no link.
   */
  async setDailyLimit(
    sourceKeypair: Keypair,
    wallet: string | Address,
    limit: bigint,
    dayLedgers: number,
    registryId?: string | Address | null
  ): Promise<void> {
    validatePolicyBoundaries(limit, dayLedgers);
    const registryScVal =
      registryId == null
        ? xdr.ScVal.scvVoid()
        : nativeToScVal(registryId.toString(), { type: "address" });
    const tx = await this.buildTx(sourceKeypair, "set_daily_limit", [
      nativeToScVal(wallet.toString(), { type: "address" }),
      nativeToScVal(limit, { type: "i128" }),
      nativeToScVal(dayLedgers, { type: "u32" }),
      registryScVal,
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Read the current daily limit record for `wallet` (read-only simulation).
   *
   * `spent` is reported as 0 when the window has elapsed, without persisting
   * the reset — only `recordSpend` / `resetDailyCounter` write it.
   */
  async getDailyLimit(sourceKeypair: Keypair, wallet: string | Address): Promise<DailyLimit> {
    const tx = await this.buildTx(sourceKeypair, "get_daily_limit", [
      nativeToScVal(wallet.toString(), { type: "address" }),
    ]);
    return this.simulateRead<DailyLimit>(tx);
  }

  /**
   * Record a spend against `wallet`'s daily limit. The wallet authorizes.
   *
   * Resets the counter if the day window has elapsed, then debits `amount`.
   * Fails with `LimitExceeded` if the debit would exceed the configured limit.
   */
  async recordSpend(
    sourceKeypair: Keypair,
    wallet: string | Address,
    amount: bigint
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "record_spend", [
      nativeToScVal(wallet.toString(), { type: "address" }),
      nativeToScVal(amount, { type: "i128" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /** Explicitly reset a wallet's daily spend counter. Admin authorizes. */
  async resetDailyCounter(sourceKeypair: Keypair, wallet: string | Address): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "reset_daily_counter", [
      nativeToScVal(wallet.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
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
    if (!retval) {
      throw new Error("simulation returned no value");
    }
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
