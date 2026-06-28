/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-delegation
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

export interface MuxDelegationClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export class MuxDelegationClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxDelegationClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  async grantDelegate(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address,
    permissions: string[]
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "grant_delegate", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
      xdr.ScVal.scvVec(permissions.map((p) => xdr.ScVal.scvSymbol(p))),
    ]);
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

  async getDelegatePermissions(
    sourceKeypair: Keypair,
    owner: Address,
    delegate: Address
  ): Promise<string[]> {
    const tx = await this.buildTx(sourceKeypair, "get_delegate_permissions", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(delegate.toString(), { type: "address" }),
    ]);
    return this.simulateRead<string[]>(tx);
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

  async getDelegates(
    sourceKeypair: Keypair,
    owner: Address
  ): Promise<Address[]> {
    const tx = await this.buildTx(sourceKeypair, "get_delegates", [
      nativeToScVal(owner.toString(), { type: "address" }),
    ]);
    return this.simulateRead<Address[]>(tx);
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
