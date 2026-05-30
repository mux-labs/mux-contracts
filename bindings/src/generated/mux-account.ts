/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-account
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

export interface MuxAccountClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export class MuxAccountClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxAccountClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  async owner(sourceKeypair: Keypair): Promise<Address> {
    const tx = await this.buildTx(sourceKeypair, "owner", []);
    return this.simulate<Address>(tx);
  }

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

  async setDelegate(
    sourceKeypair: Keypair,
    delegate: Address,
    expiryLedger: number,
    canSpend: boolean
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "set_delegate", [
      nativeToScVal(delegate.toString(), { type: "address" }),
      nativeToScVal(expiryLedger, { type: "u32" }),
      nativeToScVal(canSpend, { type: "bool" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async removeDelegate(
    sourceKeypair: Keypair,
    delegate: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "remove_delegate", [
      nativeToScVal(delegate.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async setSpendLimit(
    sourceKeypair: Keypair,
    asset: Address,
    amount: bigint,
    periodLedgers: number
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "set_spend_limit", [
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(amount, { type: "i128" }),
      nativeToScVal(periodLedgers, { type: "u32" }),
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

  private async simulate<T>(tx: Transaction): Promise<T> {
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const returnVal = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!returnVal) throw new Error("No return value");
    return scValToNative(returnVal) as T;
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
  }
}
