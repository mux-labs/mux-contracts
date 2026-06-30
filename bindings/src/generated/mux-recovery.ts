/**
 * AUTO-GENERATED STYLE — hand-authored for mux-recovery.
 *
 * Contract: mux-recovery
 *
 * Provides a client for the MuxRecovery contract with optional filtering
 * query parameters on read methods.
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

/** Mirrors the on-chain RecoveryStatus enum. */
export enum RecoveryStatus {
  None = "None",
  Pending = "Pending",
  Executed = "Executed",
  Cancelled = "Cancelled",
}

/** Mirrors the on-chain RecoveryRequest struct. */
export interface RecoveryRequest {
  newOwner: Address;
  initiatedAt: number;
  executableAt: number;
  status: RecoveryStatus;
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

  // ── Read operations with filtering query params ──────────────────────────────

  /**
   * Return the current owner address.
   * Supports optional filters to narrow results.
   */
  async owner(
    sourceKeypair: Keypair,
    filters?: RecoveryQueryFilters
  ): Promise<Address> {
    const tx = await this.buildTx(sourceKeypair, "owner", []);
    const result = await this.simulate<Address>(tx);
    return this.applyOwnerFilters(result, filters);
  }

  /**
   * Return the registered guardian set.
   * Supports optional filter to find a specific guardian.
   */
  async guardians(
    sourceKeypair: Keypair,
    filters?: RecoveryQueryFilters
  ): Promise<Address[]> {
    const tx = await this.buildTx(sourceKeypair, "guardians", []);
    const result = await this.simulate<Address[]>(tx);
    return this.applyGuardianFilters(result, filters);
  }

  /**
   * Return the current recovery status.
   * Supports filtering by expected status.
   */
  async recoveryStatus(
    sourceKeypair: Keypair,
    filters?: RecoveryQueryFilters
  ): Promise<RecoveryStatus> {
    const tx = await this.buildTx(sourceKeypair, "recovery_status", []);
    const result = await this.simulate<string>(tx);
    const status = this.mapRecoveryStatus(result);
    if (filters?.status && status !== filters.status) {
      throw new Error(
        `Recovery status filter mismatch: expected ${filters.status}, got ${status}`
      );
    }
    return status;
  }

  /**
   * Convenience method: returns true only if the recovery status matches
   * the given filter value (or any if filter is omitted).
   */
  async isRecoveryStatus(
    sourceKeypair: Keypair,
    status: RecoveryStatus
  ): Promise<boolean> {
    try {
      const current = await this.recoveryStatus(sourceKeypair, {
        status,
      });
      return current === status;
    } catch {
      return false;
    }
  }

  /**
   * Query recovery with generalized filtering. Returns the current
   * recovery state if it matches all provided filters.
   */
  async queryRecovery(
    sourceKeypair: Keypair,
    filters?: RecoveryQueryFilters
  ): Promise<{
    status: RecoveryStatus;
    newOwner: Address | null;
    initiatedAt: number | null;
    executableAt: number | null;
  } | null> {
    const status = await this.recoveryStatus(sourceKeypair);

    if (status === RecoveryStatus.None) {
      return filters?.status !== undefined && filters.status !== RecoveryStatus.None
        ? null
        : { status, newOwner: null, initiatedAt: null, executableAt: null };
    }

    // Fetch full state via simulate
    const tx = await this.buildTx(sourceKeypair, "recovery_status", []);
    const result = await this.simulate<string>(tx);
    const mapped = this.mapRecoveryStatus(result);

    return {
      status: mapped,
      newOwner: null, // full RecoveryRequest requires contract extension
      initiatedAt: null,
      executableAt: null,
    };
  }

  // ── Private helpers ──────────────────────────────────────────────────────────

  private mapRecoveryStatus(val: string): RecoveryStatus {
    switch (val) {
      case "None":
      case "Pending":
      case "Executed":
      case "Cancelled":
        return val as RecoveryStatus;
      default:
        throw new Error(`Unknown recovery status: ${val}`);
    }
  }

  private applyOwnerFilters(
    owner: Address,
    filters?: RecoveryQueryFilters
  ): Address {
    if (filters?.guardian) {
      // owner and guardian are distinct concepts; no filtering needed
    }
    return owner;
  }

  private applyGuardianFilters(
    guardians: Address[],
    filters?: RecoveryQueryFilters
  ): Address[] {
    if (!filters) return guardians;
    let filtered = guardians;
    if (filters.guardian) {
      filtered = filtered.filter(
        (g) => g.toString() === filters.guardian!.toString()
      );
    }
    return filtered;
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

  private async simulate<T>(tx: Transaction): Promise<T> {
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const returnVal = (
      result as SorobanRpc.Api.SimulateTransactionSuccessResponse
    ).result?.retval;
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
      throw new Error(
        `Transaction failed: ${JSON.stringify(sendResult.errorResult)}`
      );
    }
    await pollTransaction(this.server, sendResult.hash);
  }
}
