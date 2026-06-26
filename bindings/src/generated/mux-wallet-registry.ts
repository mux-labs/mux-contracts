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

/**
 * TypeScript client for the `mux-wallet-registry` contract.
 *
 * The registry maps symbolic names to wallet addresses. Only the owner
 * recorded at initialisation may write entries; reads are open to any caller.
 *
 * @example
 * ```ts
 * const client = new MuxWalletRegistryClient({ contractId, networkPassphrase, rpcUrl });
 * await client.initialize(ownerKeypair, new Address(ownerPublicKey));
 * await client.registerWallet(ownerKeypair, "treasury", new Address(walletPublicKey));
 * const addr = await client.getWallet(ownerKeypair, "treasury");
 * ```
 */
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
   * @param name   Symbolic key (max 10 UTF-8 bytes — Soroban `Symbol` limit).
   * @param wallet Wallet address to associate with `name`.
   * @throws if the contract is not initialised or the source is not the owner.
   */
  async registerWallet(
    sourceKeypair: Keypair,
    name: string,
    wallet: Address
  ): Promise<void> {
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
   * @param name Symbolic key to look up.
   * @throws if no wallet is registered under `name` (contract returns
   *         `WalletNotFound`, error code 4).
   */
  async getWallet(sourceKeypair: Keypair, name: string): Promise<Address> {
    const tx = await this.buildTx(sourceKeypair, "get_wallet", [
      xdr.ScVal.scvSymbol(name),
    ]);
    return this.simulateRead<Address>(tx);
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
