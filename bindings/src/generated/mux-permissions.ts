/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-permissions
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

export interface MuxPermissionsClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export class MuxPermissionsClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxPermissionsClientOptions) {
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

  async createRole(
    sourceKeypair: Keypair,
    role: string,
    permissions: string[]
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "create_role", [
      xdr.ScVal.scvSymbol(role),
      xdr.ScVal.scvVec(permissions.map((p) => xdr.ScVal.scvSymbol(p))),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async grantRole(
    sourceKeypair: Keypair,
    account: Address,
    role: string
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "grant_role", [
      nativeToScVal(account.toString(), { type: "address" }),
      xdr.ScVal.scvSymbol(role),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async revokeRole(
    sourceKeypair: Keypair,
    account: Address,
    role: string
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "revoke_role", [
      nativeToScVal(account.toString(), { type: "address" }),
      xdr.ScVal.scvSymbol(role),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async hasPermission(
    sourceKeypair: Keypair,
    account: Address,
    permission: string
  ): Promise<boolean> {
    const tx = await this.buildTx(sourceKeypair, "has_permission", [
      nativeToScVal(account.toString(), { type: "address" }),
      xdr.ScVal.scvSymbol(permission),
    ]);
    return this.simulateRead<boolean>(tx);
  }

  async getRoles(sourceKeypair: Keypair, account: Address): Promise<string[]> {
    const tx = await this.buildTx(sourceKeypair, "get_roles", [
      nativeToScVal(account.toString(), { type: "address" }),
    ]);
    return this.simulateRead<string[]>(tx);
  }

  async getRoleMembers(
    sourceKeypair: Keypair,
    role: string
  ): Promise<Address[]> {
    const tx = await this.buildTx(sourceKeypair, "get_role_members", [
      xdr.ScVal.scvSymbol(role),
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
  }
}
