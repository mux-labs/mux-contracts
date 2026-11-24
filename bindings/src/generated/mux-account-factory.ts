/**
 * AUTO-GENERATED â€” do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-account-factory
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
import { pollTransaction } from "../horizon";
import {
  MAX_ACCOUNTS_PAGE_SIZE,
  FactoryBoundsError,
  FactoryBoundsErrorCode,
} from "../factory-events";

export interface MuxAccountFactoryClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

/** Maximum allowed length for account metadata version string. */
export const FACTORY_MAX_VERSION_LENGTH = 32;

/** Maximum allowed length for account metadata description string. */
export const FACTORY_MAX_DESCRIPTION_LENGTH = 256;

/** Maximum allowed length for account metadata author string. */
export const FACTORY_MAX_AUTHOR_LENGTH = 64;

/**
 * Hard maximum page size for `getAccounts`. Defined in {@link MAX_ACCOUNTS_PAGE_SIZE}
 * (imported from `../factory-events`) and kept in sync with the on-chain
 * `MAX_ACCOUNTS_PAGE_SIZE`; requests for a larger limit are rejected with
 * `FactoryBoundsErrorCode.LimitTooLarge` rather than silently clamped, so
 * callers can never receive an unbounded result set.
 */

export type MuxAccountFactoryError =
  | "Unauthorized"
  | "InvalidAccount"
  | "TooManyAccounts"
  | "MetadataNotFound"
  | "MetadataTooLarge"
  | "InvalidBounds";

/**
 * Validates metadata field lengths against contract storage griefing limits.
 * Throws MetadataTooLarge error if any field exceeds its maximum allowed size.
 */
export function validateFactoryMetadata(
  version: string,
  description: string,
  author: string
): void {
  if (version.length > FACTORY_MAX_VERSION_LENGTH) {
    throw new Error(
      `MetadataTooLarge: version length (${version.length}) exceeds maximum of ${FACTORY_MAX_VERSION_LENGTH}`
    );
  }
  if (description.length > FACTORY_MAX_DESCRIPTION_LENGTH) {
    throw new Error(
      `MetadataTooLarge: description length (${description.length}) exceeds maximum of ${FACTORY_MAX_DESCRIPTION_LENGTH}`
    );
  }
  if (author.length > FACTORY_MAX_AUTHOR_LENGTH) {
    throw new Error(
      `MetadataTooLarge: author length (${author.length}) exceeds maximum of ${FACTORY_MAX_AUTHOR_LENGTH}`
    );
  }
}

/**
 * Validates pagination bounds for `getAccounts` before they reach the
 * contract, using the shared `MAX_ACCOUNTS_PAGE_SIZE` and the typed
 * `FactoryBoundsError` from `../factory-events`.
 *
 * Fail-closed: negative offsets, non-positive limits, and limits above
 * `MAX_ACCOUNTS_PAGE_SIZE` all throw `FactoryBoundsError` with a stable code
 * instead of panicking or returning an unbounded result set. No RPC call is
 * made when bounds are invalid.
 */
export function validateGetAccountsBounds(offset: number, limit: number): void {
  if (!Number.isInteger(offset) || offset < 0) {
    throw new FactoryBoundsError(
      FactoryBoundsErrorCode.InvalidOffset,
      `offset must be a non-negative integer, received ${offset}`
    );
  }
  if (!Number.isInteger(limit) || limit < 1) {
    throw new FactoryBoundsError(
      FactoryBoundsErrorCode.InvalidLimit,
      `limit must be a positive integer, received ${limit}`
    );
  }
  if (limit > MAX_ACCOUNTS_PAGE_SIZE) {
    throw new FactoryBoundsError(
      FactoryBoundsErrorCode.LimitTooLarge,
      `limit ${limit} exceeds maximum page size ${MAX_ACCOUNTS_PAGE_SIZE}`
    );
  }
}

/**
 * Re-throws a host simulation error as a `FactoryBoundsError` when the error
 * message indicates the contract rejected the requested bounds, so off-chain
 * callers can branch on a stable code instead of parsing raw strings.
 */
function throwFactoryBoundsOnSimulationError(result: unknown): never {
  const raw = (result as { error?: unknown }).error;
  const message = typeof raw === "string" ? raw : String(raw?.message ?? raw ?? result);
  const lowered = message.toLowerCase();
  if (lowered.includes("offset")) {
    throw new FactoryBoundsError(FactoryBoundsErrorCode.InvalidOffset, message);
  }
  if (lowered.includes("limit") || lowered.includes("page size")) {
    throw new FactoryBoundsError(FactoryBoundsErrorCode.LimitTooLarge, message);
  }
  throw new Error(`Simulation failed: ${message}`);
}

export class MuxAccountFactoryClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxAccountFactoryClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  async deployAccount(
    sourceKeypair: Keypair,
    owner: Address,
    accountAddress: Address,
    simulateOnly = false
  ): Promise<Address> {
    const tx = await this.buildTx(sourceKeypair, "deploy_account", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(accountAddress.toString(), { type: "address" }),
    ]);
    if (simulateOnly) {
      return this.simulate<Address>(tx);
    }
    return this.submitAndRead<Address>(tx, sourceKeypair);
  }

  async deployAccountWithMetadata(
    sourceKeypair: Keypair,
    owner: Address,
    accountAddress: Address,
    version: string,
    description: string,
    author: string
  ): Promise<Address> {
    validateFactoryMetadata(version, description, author);
    const tx = await this.buildTx(sourceKeypair, "deploy_account_with_metadata", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(accountAddress.toString(), { type: "address" }),
      nativeToScVal(version, { type: "string" }),
      nativeToScVal(description, { type: "string" }),
      nativeToScVal(author, { type: "string" }),
    ]);
    return this.submitAndRead<Address>(tx, sourceKeypair);
  }

  /**
   * Simulate a deploy_account call without submitting any on-chain transaction
   * (dry-run). Validates inputs and returns the account address that would be
   * registered, or throws if validation fails.
   */
  async simulateDeploy(
    sourceKeypair: Keypair,
    owner: Address,
    accountAddress: Address
  ): Promise<Address> {
    const tx = await this.buildTx(sourceKeypair, "simulate_deploy", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(accountAddress.toString(), { type: "address" }),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("Simulation returned no value");
    return retval.value() as unknown as Address;
  }

  /**
   * Simulate a deploy_account_with_metadata call without submitting any
   * on-chain transaction (dry-run). Validates inputs and returns the account
   * address that would be registered, or throws if validation fails.
   */
  async simulateDeployWithMetadata(
    sourceKeypair: Keypair,
    owner: Address,
    accountAddress: Address,
    version: string,
    description: string,
    author: string
  ): Promise<Address> {
    validateFactoryMetadata(version, description, author);
    const tx = await this.buildTx(sourceKeypair, "simulate_deploy_with_metadata", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(accountAddress.toString(), { type: "address" }),
      nativeToScVal(version, { type: "string" }),
      nativeToScVal(description, { type: "string" }),
      nativeToScVal(author, { type: "string" }),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("Simulation returned no value");
    return retval.value() as unknown as Address;
  }

  /**
   * Return a bounded page of accounts owned by `owner`.
   *
   * `offset` must be a non-negative integer and `limit` must be a positive
   * integer no greater than `MAX_ACCOUNTS_PAGE_SIZE` (imported from
   * `../factory-events`). Invalid bounds throw `FactoryBoundsError` with a
   * stable code before any RPC call is made (fail-closed). When the host
   * rejects the bounds at simulation time, the raw simulation error is
   * rethrown as a `FactoryBoundsError` with the matching code.
   */
  async getAccounts(
    sourceKeypair: Keypair,
    owner: Address,
    offset = 0,
    limit = MAX_ACCOUNTS_PAGE_SIZE
  ): Promise<Address[]> {
    validateGetAccountsBounds(offset, limit);
    const tx = await this.buildTx(sourceKeypair, "get_accounts", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(offset, { type: "u32" }),
      nativeToScVal(limit, { type: "u32" }),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throwFactoryBoundsOnSimulationError(result);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) return [];
    return retval.value() as unknown as Address[];
  }

  async getAccountMetadata(
    sourceKeypair: Keypair,
    owner: Address,
    accountAddress: Address
  ): Promise<{ version: string; description: string; author: string }> {
    const tx = await this.buildTx(sourceKeypair, "get_account_metadata", [
      nativeToScVal(owner.toString(), { type: "address" }),
      nativeToScVal(accountAddress.toString(), { type: "address" }),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) throw new Error("Simulation returned no value");
    return retval.value() as unknown as { version: string; description: string; author: string };
  }

  async accountCount(sourceKeypair: Keypair): Promise<bigint> {
    const tx = await this.buildTx(sourceKeypair, "account_count", []);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) return 0n;
    return retval.value() as unknown as bigint;
  }

  /**
   * Return the maximum number of accounts allowed per owner (currently 64).
   *
   * Query this before calling deployAccount to avoid a TooManyAccounts error
   * at execution time.
   */
  async maxAccountsPerOwner(sourceKeypair: Keypair): Promise<number> {
    const tx = await this.buildTx(sourceKeypair, "max_accounts_per_owner", []);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) return 64; // contract constant fallback
    return retval.value() as unknown as number;
  }

  // â”€â”€ Private helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
    if (!retval) return {} as T;
    return retval.value() as unknown as T;
  }

  private async simulate<T>(tx: Transaction): Promise<T> {
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) return {} as T;
    return retval.value() as unknown as T;
  }
}
