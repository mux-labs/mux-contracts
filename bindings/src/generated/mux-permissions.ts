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
import type { MuxPermissionsError } from "../types";
import { pollTransaction } from "../horizon";

export interface MuxPermissionsClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

/**
 * Role inheritance model (see docs/permissions-role-model.md).
 *
 * A role may declare a parent role; a member of a child role inherits every
 * permission granted to its ancestors. Inheritance is strictly upward: a child
 * can never grant permissions beyond those declared on its parent chain, so
 * privilege escalation outside the declared hierarchy is denied by default.
 */
export interface RoleDefinition {
  role: string;
  permissions: string[];
  /** Parent role whose permissions are inherited. `null` for root roles. */
  parent: string | null;
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
    permissions: string[],
    parent: string | null = null
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "create_role", [
      xdr.ScVal.scvSymbol(role),
      xdr.ScVal.scvVec(permissions.map((p) => xdr.ScVal.scvSymbol(p))),
      parent === null
        ? xdr.ScVal.scvVoid()
        : xdr.ScVal.scvSymbol(parent),
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

  /**
   * Returns the declared role definition, including its parent link, so callers
   * and tests can assert the inheritance hierarchy without re-deriving it.
   */
  async getRoleDefinition(
    sourceKeypair: Keypair,
    role: string
  ): Promise<RoleDefinition> {
    const tx = await this.buildTx(sourceKeypair, "get_role_definition", [
      xdr.ScVal.scvSymbol(role),
    ]);
    return this.simulateRead<RoleDefinition>(tx);
  }

  /**
   * Resolves the effective permission set for a role by walking the declared
   * parent chain. Mirrors the on-chain inheritance semantics so tests can
   * assert child roles inherit parent permissions and never escalate beyond
   * the declared hierarchy.
   */
  async getEffectivePermissions(
    sourceKeypair: Keypair,
    role: string
  ): Promise<string[]> {
    const tx = await this.buildTx(sourceKeypair, "get_effective_permissions", [
      xdr.ScVal.scvSymbol(role),
    ]);
    return this.simulateRead<string[]>(tx);
  }

  // ── Multisig Admin ──────────────────────────────────────────────────────────

  async setAdminThreshold(
    sourceKeypair: Keypair,
    threshold: number
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "set_admin_threshold", [
      nativeToScVal(threshold, { type: "u32" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async proposeAdmin(
    sourceKeypair: Keypair,
    newAdmin: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "propose_admin", [
      nativeToScVal(newAdmin.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async approveAdmin(
    sourceKeypair: Keypair,
    approver: Address,
    newAdmin: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "approve_admin", [
      nativeToScVal(approver.toString(), { type: "address" }),
      nativeToScVal(newAdmin.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  async getPendingAdmins(sourceKeypair: Keypair): Promise<Address[]> {
    const tx = await this.buildTx(sourceKeypair, "get_pending_admins", []);
    return this.simulateRead<Address[]>(tx);
  }

  // ── TTL Management ─────────────────────────────────────────────────────────

  async bumpTtl(sourceKeypair: Keypair): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "bump_ttl", []);
    await this.submit(tx, sourceKeypair);
  }

  async ttlConfig(
    sourceKeypair: Keypair
  ): Promise<{ threshold: number; extendTo: number }> {
    const tx = await this.buildTx(sourceKeypair, "ttl_config", []);
    const [threshold, extendTo] = await this.simulateRead<[number, number]>(tx);
    return { threshold, extendTo };
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
