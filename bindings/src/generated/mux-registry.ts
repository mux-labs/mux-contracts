/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-registry
 */

import {
  Contract,
  Keypair,
  nativeToScVal,
  SorobanRpc,
  Transaction,
  TransactionBuilder,
  xdr,
} from "@stellar/stellar-sdk";
import { pollTransaction } from "../horizon";

export interface MuxRegistryClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export type MuxRegistryError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "ContractNotFound"
  | "TooManyContracts";

export interface ContractMetadata {
  version: string;
  description: string;
  author: string;
}

export class MuxRegistryClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxRegistryClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  async initialize(sourceKeypair: Keypair, admin: string): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "initialize", [
      nativeToScVal(admin, { type: "address" }),
    ]);
    await this.submitAndRead<void>(tx, sourceKeypair);
  }

  async register(
    sourceKeypair: Keypair,
    name: string,
    version: string
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "register", [
      xdr.ScVal.scvSymbol(name),
      nativeToScVal(version, { type: "string" }),
    ]);
    await this.submitAndRead<void>(tx, sourceKeypair);
  }

  async registerWithMetadata(
    sourceKeypair: Keypair,
    name: string,
    version: string,
    description: string,
    author: string
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "register_with_metadata", [
      xdr.ScVal.scvSymbol(name),
      nativeToScVal(version, { type: "string" }),
      nativeToScVal(description, { type: "string" }),
      nativeToScVal(author, { type: "string" }),
    ]);
    await this.submitAndRead<void>(tx, sourceKeypair);
  }

  async getVersion(sourceKeypair: Keypair, name: string): Promise<string> {
    const tx = await this.buildTx(sourceKeypair, "get_version", [
      xdr.ScVal.scvSymbol(name),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("No return value");
    return retval.value() as unknown as string;
  }

  async getMetadata(sourceKeypair: Keypair, name: string): Promise<ContractMetadata> {
    const tx = await this.buildTx(sourceKeypair, "get_metadata", [
      xdr.ScVal.scvSymbol(name),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("No return value");
    return retval.value() as unknown as ContractMetadata;
  }

  async listContracts(sourceKeypair: Keypair): Promise<string[]> {
    const tx = await this.buildTx(sourceKeypair, "list_contracts", []);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) return [];
    return retval.value() as unknown as string[];
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
      throw new Error(`Transaction failed: ${JSON.stringify(sendResult.errorResult)}`);
    }
    const confirmed = await pollTransaction(this.server, sendResult.hash);
    const retval = confirmed.returnValue;
    if (!retval) return undefined as unknown as T;
    return retval.value() as unknown as T;
  }
}
