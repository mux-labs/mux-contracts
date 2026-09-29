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

/**
 * Stable numeric error codes returned by the `mux-wallet-registry` contract.
 *
 * These mirror the contract's `#[contracterror]` discriminants and are the
 * canonical mapping used to translate on-chain failures into typed client
 * errors. Do not renumber: clients and runbooks depend on these values.
 */
export const MUX_WALLET_REGISTRY_ERROR_CODES: Record<MuxWalletRegistryError, number> = {
  NotInitialized: 1,
  AlreadyInitialized: 2,
  Unauthorized: 3,
  WalletNotFound: 4,
  TooManyWallets: 5,
};

/**
 * Typed error thrown by {@link MuxWalletRegistryClient} for contract-level
 * failures. Carries the stable {@link MuxWalletRegistryError} code plus an
 * optional `correlationId` so ops can trace a failed register/lookup across
 * logs without exposing key material.
 */
export class MuxWalletRegistryError extends Error {
  readonly code: MuxWalletRegistryError;
  readonly numericCode: number;
  readonly correlationId?: string;

  constructor(code: MuxWalletRegistryError, correlationId?: string) {
    super(`mux-wallet-registry: ${code} (code ${MUX_WALLET_REGISTRY_ERROR_CODES[code]})`);
    this.name = "MuxWalletRegistryError";
    this.code = code;
    this.numericCode = MUX_WALLET_REGISTRY_ERROR_CODES[code];
    this.correlationId = correlationId;
  }
}

/**
 * Authorization roles recognised by the registry. `Owner` is the address set
 * at {@link MuxWalletRegistryClient.initialize}; `Delegate` and `Guardian`
 * are optional secondary authorities that may be granted register rights.
 * Lookups are unauthenticated (read-only simulation).
 */
export type WalletRegistryRole = "Owner" | "Delegate" | "Guardian";

/**
 * Optional authz context for privileged (write) entrypoints. When omitted the
 * client defaults to `Owner` and relies on the contract's own auth checks.
 * Supplying a role lets callers assert intent and fail fast client-side when
 * the source keypair is not the expected authority.
 */
export interface WalletRegistryAuthz {
  role?: WalletRegistryRole;
  /**
   * Optional correlation id propagated into logs/errors for tracing a single
   * register/lookup across services. Never include secrets or key material.
   */
  correlationId?: string;
}

/**
 * Generates a non-secret correlation id suitable for tracing a registry call.
 * Uses a random hex string; contains no key material or PII.
 */
export function newCorrelationId(): string {
  const bytes = new Uint8Array(8);
  if (typeof globalThis.crypto?.getRandomValues === "function") {
    globalThis.crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = Math.floor(Math.random() * 256);
    }
  }
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Redacts a Stellar address for safe logging: keeps the first 4 and last 4
 * characters, masking the middle. Never log full addresses or key material.
 */
export function redactAddress(address: string): string {
  if (typeof address !== "string" || address.length <= 8) {
    return "***";
  }
  return `${address.slice(0, 4)}…${address.slice(-4)}`;
}

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
   * @param authz  Optional authz context (role + correlation id).
   * @throws if the contract is not initialised, the source is not the owner, or the name violates charset policy.
   */
  async registerWallet(
    sourceKeypair: Keypair,
    name: string,
    wallet: Address,
    authz?: WalletRegistryAuthz
  ): Promise<void> {
    validateWalletName(name);
    const tx = await this.buildTx(sourceKeypair, "register_wallet", [
      xdr.ScVal.scvSymbol(name),
      nativeToScVal(wallet.toString(), { type: "address" }),
    ]);
    await this.submit(tx, sourceKeypair, authz);
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
   * @param authz       Optional authz context (role + correlation id).
   * @throws if the contract is not initialised, the source is not the owner,
   *         or the wallet cap (128) has been reached (`TooManyWallets`, code 5).
   */
  async registerWalletWithMetadata(
    sourceKeypair: Keypair,
    name: string,
    wallet: Address,
    label: string,
    description: string,
    authz?: WalletRegistryAuthz
  ): Promise<void> {
    validateWalletName(name);
    const tx = await this.buildTx(sourceKeypair, "register_wallet_with_metadata", [
      xdr.ScVal.scvSymbol(name),
      nativeToScVal(wallet.toString(), { type: "address" }),
      xdr.ScVal.scvString(label),
      xdr.ScVal.scvString(description),
    ]);
    await this.submit(tx, sourceKeypair, authz);
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
    if (!retval) {
      throw new Error("Simulation returned no value");
    }
    return scValToNative(retval) as T;
  }

  private async submit(
    tx: Transaction,
    sourceKeypair: Keypair,
    authz?: WalletRegistryAuthz
  ): Promise<void> {
    const correlationId = authz?.correlationId ?? newCorrelationId();
    const role: WalletRegistryRole = authz?.role ?? "Owner";
    try {
      const prepared = await this.server.prepareTransaction(tx);
      prepared.sign(sourceKeypair);
      const sent = await this.server.sendTransaction(prepared);
      await pollTransaction(this.server, sent.hash);
    } catch (err) {
      const mapped = this.mapContractError(err, correlationId);
      // Ops-safe log: correlation id + role + redacted source only. No secrets.
      // eslint-disable-next-line no-console
      console.error(
        `mux-wallet-registry: register failed role=${role} correlationId=${correlationId} source=${redactAddress(
          sourceKeypair.publicKey()
        )} code=${mapped.code}`
      );
      throw mapped;
    }
  }

  /**
   * Maps a raw Soroban/RPC failure into a typed {@link MuxWalletRegistryError}.
   * Unknown failures fail closed as `Unauthorized` so callers never treat an
   * ambiguous write error as success.
   */
  private mapContractError(err: unknown, correlationId: string): MuxWalletRegistryError {
    const message = err instanceof Error ? err.message : String(err);
    for (const [code, numeric] of Object.entries(MUX_WALLET_REGISTRY_ERROR_CODES)) {
      if (message.includes(code) || message.includes(`code ${numeric}`)) {
        return new MuxWalletRegistryError(code as MuxWalletRegistryError, correlationId);
      }
    }
    return new MuxWalletRegistryError("Unauthorized", correlationId);
  }
}
