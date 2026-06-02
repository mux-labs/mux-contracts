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
    limit: bigint
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "set_policy", [
      nativeToScVal(account.toString(), { type: "address" }),
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(limit, { type: "i128" }),
    ]);
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
