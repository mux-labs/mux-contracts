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
 * Maximum number of wallets the registry will hold. Mirrors `MAX_WALLETS` in
 * `contracts/mux-wallet-registry/src/lib.rs`; a divergence test asserts the two
 * stay in sync.
 */
export const MAX_WALLETS = 128;

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

export type MuxWalletRegistryErrorCode =
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
export const MUX_WALLET_REGISTRY_ERROR_CODES: Record<MuxWalletRegistryErrorCode, number> = {
  NotInitialized: 1,
  AlreadyInitialized: 2,
  Unauthorized: 3,
  WalletNotFound: 4,
  TooManyWallets: 5,
};

/**
 * Documented HTTP statuses for each registry error code, used when surfacing
 * contract failures through an HTTP boundary.
 */
export const MUX_WALLET_REGISTRY_ERROR_HTTP_STATUS: Record<MuxWalletRegistryErrorCode, number> = {
  NotInitialized: 500,
  AlreadyInitialized: 409,
  Unauthorized: 403,
  WalletNotFound: 404,
  TooManyWallets: 409,
};

/**
 * Typed error thrown by {@link MuxWalletRegistryClient} for contract-level
 * failures. Carries the stable {@link MuxWalletRegistryError} code plus an
 * optional `correlationId` so ops can trace a failed register/lookup across
 * logs without exposing key material.
 */
export class MuxWalletRegistryError extends Error {
  readonly code: MuxWalletRegistryErrorCode;
  readonly numericCode: number;
  readonly httpStatus: number;
  readonly correlationId?: string;

  constructor(code: MuxWalletRegistryErrorCode, correlationId?: string) {
    super(`mux-wallet-registry: ${code} (code ${MUX_WALLET_REGISTRY_ERROR_CODES[code]})`);
    this.name = "MuxWalletRegistryError";
    this.code = code;
    this.numericCode = MUX_WALLET_REGISTRY_ERROR_CODES[code];
    this.httpStatus = MUX_WALLET_REGISTRY_ERROR_HTTP_STATUS[code];
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

/**
 * Asserts that a read result for a single wallet lookup is a non-empty entry.
 * A missing entry is a hard `WalletNotFound` failure, never a default address.
 */
export function assertWalletFound(
  wallet: Address | null | undefined,
  correlationId?: string
): Address {
  if (wallet === null || wallet === undefined || wallet.toString() === "") {
    throw new MuxWalletRegistryError("WalletNotFound", correlationId);
  }
  return wallet;
}

/**
 * Asserts that a returned name list does not exceed the contract's
 * `MAX_WALLETS` cap. A list of exactly `MAX_WALLETS` is accepted.
 */
export function assertWalletListWithinCap(
  names: readonly string[],
  correlationId?: string
): readonly string[] {
  if (names.length > MAX_WALLETS) {
    throw new MuxWalletRegistryError("TooManyWallets", correlationId);
  }
  return names;
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
    const result = await this.simulateRead<Address | null>(tx);
    return assertWalletFound(result);
  }

  /**
   * Register or overwrite a wallet address with descriptive metadata.
   *
   * `sourceKeypair` must be (or be authorised by) the owner set at
   * initialisation. Ca

/* … truncated 5126 chars — edit only what you need near the top … */
