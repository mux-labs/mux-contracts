/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-wallet-registry
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

/** Options required to construct a {@link MuxWalletRegistryClient}. */
export interface MuxWalletRegistryClientOptions {
  /** On-chain contract ID (Stellar account-style address). */
  contractId: string;
  /** Stellar network passphrase, e.g. `Networks.TESTNET`. */
  networkPassphrase: string;
  /** Soroban RPC endpoint URL. */
  rpcUrl: string;
}

export interface WalletMetadata {
  label: string;
  description: string;
}

/** Allowed charset and pattern for wallet registry names: 1-32 alphanumeric or underscore characters. */
export const WALLET_NAME_REGEX = /^[a-zA-Z0-9_]{1,32}$/;
export const WALLET_NAME_MIN_LEN = 1;
export const WALLET_NAME_MAX_LEN = 32;

/**
 * Checks whether a given string adheres to the wallet registry name charset policy.
 */
export function isValidWalletName(name: string): boolean {
  return typeof name === "string" && WALLET_NAME_REGEX.test(name);
}

/**
 * Validates a wallet registry name against the charset and length policy.
 * Throws an Error with a descriptive message if the name is invalid.
 */
export function validateWalletName(name: string): void {
  if (!name || typeof name !== "string") {
    throw new Error(`Invalid wallet name: name must be a non-empty string`);
  }
  if (name.length < WALLET_NAME_MIN_LEN || name.length > WALLET_NAME_MAX_LEN) {
    throw new Error(
      `Invalid wallet name length (${name.length}): name must be between ${WALLET_NAME_MIN_LEN} and ${WALLET_NAME_MAX_LEN} characters`
    );
  }
  if (!WALLET_NAME_REGEX.test(name)) {
    throw new Error(
      `Invalid wallet name "${name}": must contain only alphanumeric characters and underscores ([a-zA-Z0-9_])`
    );
  }
}

export type MuxWalletRegistryError =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "Unauthorized"
  | "WalletNotFound"
  | "TooManyWallets";

export class MuxWalletRegistryClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxWalletRegistryClientOptions) {
    this.contract = new Contract(opts.contractId);
    this.server = new SorobanRpc.Server(opts.rpcUrl, { allowHttp: false });
    this.networkPassphrase = opts.networkPassphrase;
  }

  /**
   * Initialise the registry and record its owner.
   *
   * Must be called exactly once before any other method. The `owner` keypair
   * must be the source and must authorise the transaction.
   *
   * @throws if the contract is already initialised.
   */
  async initialize(sourceKeypair: Keypair, owner: Address): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "initialize", [
      nativeToScVal(owner.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Register or overwrite the wallet address stored under `name`.
   *
   * `sourceKeypair` must be (or be authorised by) the owner set at
   * initialisation. Calling this with an existing `name` silently replaces
   * the previous entry.
   *
   * @param name   Symbolic key (1-32 characters, [a-zA-Z0-9_]).
   * @param wallet Wallet address to associate with `name`.
   * @throws if the contract is not initialised, the source is not the owner, or the name violates charset policy.
   */
  async registerWallet(
    sourceKeypair: Keypair,
    name: string,
    wallet: Address
  ): Promise<void> {
    validateWalletName(name);
    const tx = await this.buildTx(sourceKeypair, "register_wallet", [
      xdr.ScVal.scvSymbol(name),
      nativeToScVal(wallet.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Return the wallet address registered under `name`.
   *
   * This is a read-only simulation; no on-chain transaction is submitted and
   * no auth is required.
   *
   * @param name Symbolic key to look up (1-32 characters, [a-zA-Z0-9_]).
   * @throws if no wallet is registered under `name` (contract returns
   *         `WalletNotFound`, error code 4) or name violates charset policy.
   */
  async getWallet(sourceKeypair: Keypair, name: string): Promise<Address> {
    validateWalletName(name);
    const tx = await this.buildTx(sourceKeypair, "get_wallet", [
      xdr.ScVal.scvSymbol(name),
    ]);
    return this.simulateRead<Address>(tx);
  }

  /**
   * Register or overwrite a wallet address with descriptive metadata.
   *
   * `sourceKeypair` must be (or be authorised by) the owner set at
   * initialisation. Calling this with an existing `name` silently replaces
   * the previous entry and its metadata.
   *
   * @param name        Symbolic key (max 10 UTF-8 bytes — Soroban `Symbol` limit).
   * @param wallet      Wallet address to associate with `name`.
   * @param label       Short human-readable label for the wallet.
   * @param description Free-form description or notes.
   * @throws if the contract is not initialised, the source is not the owner,
   *         or the wallet cap (128) has been reached (`TooManyWallets`, code 5).
   */
  async registerWalletWithMetadata(
    sourceKeypair: Keypair,
    name: string,
    wallet: Address,
    label: string,
    description: string
  ): Promise<void> {
    validateWalletName(name);
    const tx = await this.buildTx(sourceKeypair, "register_wallet_with_metadata", [
      xdr.ScVal.scvSymbol(name),
      nativeToScVal(wallet.toString(), { type: "address" }),
      xdr.ScVal.scvString(label),
      xdr.ScVal.scvString(description),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Return the metadata for the wallet registered under `name`.
   *
   * This is a read-only simulation; no on-chain transaction is submitted and
   * no auth is required. Only wallets registered via
   * {@link registerWalletWithMetadata} have metadata; wallets registered
   * via {@link registerWallet} alone do not.
   *
   * @param name Symbolic key to look up.
   * @throws if no wallet with metadata is registered under `name`
   *         (`WalletNotFound`, error code 4) or name violates charset policy.
   */
  async getMetadata(sourceKeypair: Keypair, name: string): Promise<WalletMetadata> {
    validateWalletName(name);
    const tx = await this.buildTx(sourceKeypair, "get_metadata", [
      xdr.ScVal.scvSymbol(name),
    ]);
    return this.simulateRead<WalletMetadata>(tx);
  }

  /**
   * List all registered wallet names.
   *
   * This is a read-only simulation; no on-chain transaction is submitted and
   * no auth is required. Returns an empty array when no wallets have been
   * registered or the contract is not yet initialized.
   *
   * @returns Array of symbolic wallet names (Soroban `Symbol` values, returned as strings).
   */
  async listWallets(sourceKeypair: Keypair): Promise<string[]> {
    const tx = await this.buildTx(sourceKeypair, "list_wallets", []);
    return this.simulateRead<string[]>(tx);
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
