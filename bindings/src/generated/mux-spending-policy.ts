/**
 * AUTO-GENERATED — do not edit by hand.
 * Run `npm run generate` to regenerate from the compiled contract WASM.
 *
 * Contract: mux-spending-policy
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
import type { SpendingPolicyLimit } from "../types";
import { pollTransaction } from "../horizon";

export interface MuxSpendingPolicyClientOptions {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

/** Minimum spend limit value (> 0). Setting limit <= 0 fails with InvalidInput (code 6). */
export const SPEND_LIMIT_MIN = 1n;

/** Maximum allowable i128 value for spend limits (2^127 - 1). */
export const SPEND_LIMIT_MAX = 170141183460469231731687303715884105727n;

/** Minimum period length in ledgers (> 0). period_ledgers == 0 fails with InvalidPeriod (code 7). */
export const PERIOD_LEDGERS_MIN = 1;

/** Maximum period length in ledgers (u32::MAX). */
export const PERIOD_LEDGERS_MAX = 4294967295;

/**
 * Validates spend limit policy boundaries according to spending-policy-semantics.md:
 * - limit must be > 0 and <= i128::MAX
 * - period_ledgers must be > 0 and <= u32::MAX
 */
export function validateSpendPolicyBoundaries(limit: bigint, periodLedgers?: number): void {
  if (limit <= 0n) {
    throw new Error(`InvalidInput: limit must be strictly positive (> 0), got ${limit}`);
  }
  if (limit > SPEND_LIMIT_MAX) {
    throw new Error(`InvalidInput: limit exceeds maximum i128 value (${SPEND_LIMIT_MAX})`);
  }
  if (periodLedgers !== undefined) {
    if (periodLedgers <= 0) {
      throw new Error(`InvalidPeriod: period_ledgers must be strictly positive (> 0), got ${periodLedgers}`);
    }
    if (periodLedgers > PERIOD_LEDGERS_MAX) {
      throw new Error(`InvalidPeriod: period_ledgers exceeds maximum u32 value (${PERIOD_LEDGERS_MAX})`);
    }
  }
}

/**
 * Validates check_spend amount boundaries:
 * - amount must be >= 0 (non-negative)
 * - amount must be <= i128::MAX
 */
export function validateSpendAmountBoundaries(amount: bigint): void {
  if (amount < 0n) {
    throw new Error(`InvalidInput: spend amount cannot be negative, got ${amount}`);
  }
  if (amount > SPEND_LIMIT_MAX) {
    throw new Error(`InvalidInput: spend amount exceeds maximum i128 value (${SPEND_LIMIT_MAX})`);
  }
}

/**
 * Spend asset allowlist (issue #813).
 *
 * Invariants (see docs/spending-policy-semantics.md):
 * - Deny-by-default: an asset is spendable only if it has been explicitly
 *   allowlisted by the admin/owner. Absence of an entry means "not allowed".
 * - The contract is the source of truth: `check_asset_allowed` is
 *   simulate-only and fails closed, so clients cannot bypass the allowlist by
 *   skipping the client-side check.
 * - Allowlisting is a privileged surface: only the admin/owner (or an
 *   authorized delegate) may add or remove entries.
 * - Mutations are idempotent per `requestId`; replayed requests are rejected.
 */
export interface SpendAssetAllowlistEntry {
  asset: string;
  /** Ledger at which the asset was allowlisted; 0 when unset. */
  allowlistedAtLedger: number;
  /** Whether the asset is currently spendable. */
  allowed: boolean;
}

/** Stable error codes returned by the contract for allowlist failures. */
export const SpendAssetAllowlistErrorCode = {
  Unauthorized: 1,
  AssetNotAllowed: 2,
  InvalidAsset: 3,
  ReplayedRequest: 4,
  DependencyUnavailable: 5,
} as const;

export type SpendAssetAllowlistErrorCode =
  (typeof SpendAssetAllowlistErrorCode)[keyof typeof SpendAssetAllowlistErrorCode];

/**
 * Relayer fee sponsorship limits (issue #847).
 *
 * A relayer may only sponsor fees up to `perTxLimit` per transaction and
 * `perWindowLimit` within `windowLedgers`. The contract is the source of
 * truth: `check_sponsorship` is simulate-only and fails closed, so callers
 * cannot bypass policy by skipping the client-side check.
 */
export interface RelayerSponsorshipLimit {
  relayer: string;
  perTxLimit: bigint;
  perWindowLimit: bigint;
  windowLedgers: number;
  /** Ledger at which the current window started; 0 when unset. */
  windowStartLedger: number;
  /** Amount already sponsored in the current window. */
  windowSpent: bigint;
}

/** Stable error codes returned by the contract for sponsorship failures. */
export const RelayerSponsorshipErrorCode = {
  Unauthorized: 1,
  RelayerNotRegistered: 2,
  PerTxLimitExceeded: 3,
  WindowLimitExceeded: 4,
  InvalidLimit: 5,
  ReplayedRequest: 6,
} as const;

export type RelayerSponsorshipErrorCode =
  (typeof RelayerSponsorshipErrorCode)[keyof typeof RelayerSponsorshipErrorCode];

export class MuxSpendingPolicyClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  constructor(opts: MuxSpendingPolicyClientOptions) {
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

  async setPolicy(
    sourceKeypair: Keypair,
    account: Address,
    asset: Address,
    limit: bigint,
    periodLedgers?: number
  ): Promise<void> {
    validateSpendPolicyBoundaries(limit, periodLedgers);
    const args: xdr.ScVal[] = [
      nativeToScVal(account.toString(), { type: "address" }),
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(limit, { type: "i128" }),
    ];
    if (periodLedgers !== undefined) {
      args.push(nativeToScVal(periodLedgers, { type: "u32" }));
    }
    const tx = await this.buildTx(sourceKeypair, "set_policy", args);
    await this.submit(tx, sourceKeypair);
  }

  async getPolicy(
    sourceKeypair: Keypair,
    account: Address,
    asset: Address
  ): Promise<SpendingPolicyLimit> {
    const tx = await this.buildTx(sourceKeypair, "get_policy", [
      nativeToScVal(account.toString(), { type: "address" }),
      nativeToScVal(asset.toString(), { type: "address" }),
    ]);
    return this.simulateRead<SpendingPolicyLimit>(tx);
  }

  /** Simulate-only: returns void if within limit, throws if exceeded or policy not found. */
  async checkSpend(
    sourceKeypair: Keypair,
    account: Address,
    asset: Address,
    amount: bigint
  ): Promise<void> {
    validateSpendAmountBoundaries(amount);
    const tx = await this.buildTx(sourceKeypair, "check_spend", [
      nativeToScVal(account.toString(), { type: "address" }),
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(amount, { type: "i128" }),
    ]);
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`check_spend failed: ${result.error}`);
    }
  }

  /**
   * Admin/owner-only: allowlist `asset` for spending. Deny-by-default — the
   * contract enforces that the caller is the admin or an authorized delegate;
   * clients cannot bypass this by calling directly. `requestId` provides
   * idempotency so replayed/concurrent requests are rejected.
   */
  async allowlistAsset(
    sourceKeypair: Keypair,
    asset: Address,
    requestId: string
  ): Promise<void> {
    if (!requestId) {
      throw new Error(
        `InvalidInput: requestId is required (code ${SpendAssetAllowlistErrorCode.InvalidAsset})`
      );
    }
    const tx = await this.buildTx(sourceKeypair, "allowlist_asset", [
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(requestId, { type: "string" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /**
   * Admin/owner-only: remove `asset` from the allowlist. Idempotent per
   * `requestId`; replayed requests are rejected by the contract.
   */
  async removeAllowlistedAsset(
    sourceKeypair: Keypair,
    asset: Address,
    requestId: string
  ): Promise<void> {
    if (!requestId) {
      throw new Error(
        `InvalidInput: requestId is required (code ${SpendAssetAllowlistErrorCode.InvalidAsset})`
      );
    }
    const tx = await this.buildTx(sourceKeypair, "remove_allowlisted_asset", [
      nativeToScVal(asset.toString(), { type: "address" }),
      nativeToScVal(requestId, { type: "string" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /** Read the allowlist entry for `asset`; `allowed` is false when unset. */
  async getAssetAllowlistEntry(
    sourceKeypair: Keypair,
    asset: Address
  ): Promise<SpendAssetAllowlistEntry> {
    const tx = await this.buildTx(sourceKeypair, "get_asset_allowlist_entry", [
      nativeToScVal(asset.toString(), { type: "address" }),
    ]);
    return this.simulateRead<SpendAssetAllowlistEntry>(tx);
  }

  /**
   * Simulate-only: verifies that `asset` is allowlisted for spending.
   * Fails closed on any error (asset not allowed, invalid asset, or RPC
   * outage) so callers never proceed on an unverified asset.
   */
  async checkAssetAllowed(
    sourceKeypair: Keypair,
    asset: Address
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "check_asset_allowed", [
      nativeToScVal(asset.toString(), { type: "address" }),
    ]);
    let result: SorobanRpc.Api.SimulateTransactionResponse;
    try {
      result = await this.server.simulateTransaction(tx);
    } catch (err) {
      // Dependency outage (RPC) — fail closed.
      throw new Error(
        `check_asset_allowed failed closed (code ${SpendAssetAllowlistErrorCode.DependencyUnavailable}): ${(err as Error).message}`
      );
    }
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(
        `check_asset_allowed failed closed (code ${SpendAssetAllowlistErrorCode.AssetNotAllowed}): ${result.error}`
      );
    }
  }

  /**
   * Admin/owner-only: set the relayer fee sponsorship limits for `relayer`.
   * Deny-by-default — the contract enforces that the caller is the admin or an
   * authorized delegate; clients cannot bypass this by calling directly.
   */
  async setRelayerSponsorshipLimit(
    sourceKeypair: Keypair,
    relayer: Address,
    perTxLimit: bigint,
    perWindowLimit: bigint,
    windowLedgers: number
  ): Promise<void> {
    if (perTxLimit < 0n || perWindowLimit < 0n || windowLedgers <= 0) {
      throw new Error(
        `Invalid sponsorship limit (code ${RelayerSponsorshipErrorCode.InvalidLimit})`
      );
    }
    const tx = await this.buildTx(sourceKeypair, "set_relayer_sponsorship_limit", [
      nativeToScVal(relayer.toString(), { type: "address" }),
      nativeToScVal(perTxLimit, { type: "i128" }),
      nativeToScVal(perWindowLimit, { type: "i128" }),
      nativeToScVal(windowLedgers, { type: "u32" }),
    ]);
    await this.submit(tx, sourceKeypair);
  }

  /** Read the current sponsorship limits and window usage for `relayer`. */
  async getRelayerSponsorshipLimit(
    sourceKeypair: Keypair,
    relayer: Address
  ): Promise<RelayerSponsorshipLimit> {
    const tx = await this.buildTx(sourceKeypair, "get_relayer_sponsorship_limit", [
      nativeToScVal(relayer.toString(), { type: "address" }),
    ]);
    return this.simulateRead<RelayerSponsorshipLimit>(tx);
  }

  /**
   * Simulate-only: verifies that `relayer` may sponsor `fee` for `requestId`.
   * Fails closed on any error (limit exceeded, unregistered relayer, replay,
   * or RPC outage) so callers never proceed on an unverified sponsorship.
   */
  async checkRelayerSponsorship(
    sourceKeypair: Keypair,
    relayer: Address,
    fee: bigint,
    requestId: string
  ): Promise<void> {
    const tx = await this.buildTx(sourceKeypair, "check_relayer_sponsorship", [
      nativeToScVal(relayer.toString(), { type: "address" }),
      nativeToScVal(fee, { type: "i128" }),
      nativeToScVal(requestId, { type: "string" }),
    ]);
    let result: SorobanRpc.Api.SimulateTransactionResponse;
    try {
      result = await this.server.simulateTransaction(tx);
    } catch (err) {
      // Dependency outage (RPC)
      throw new Error(
        `check_relayer_sponsorship failed closed (code ${RelayerSponsorshipErrorCode.Unauthorized}): ${(err as Error).message}`
      );
    }
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`check_relayer_sponsorship failed: ${result.error}`);
    }
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

  private async simulateRead<T>(tx: Transaction): Promise<T> {
    const result = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(result)) {
      throw new Error(`simulation failed: ${result.error}`);
    }
    const retval = (result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result?.retval;
    if (!retval) {
      throw new Error("simulation returned no value");
    }
    return scValToNative(retval) as T;
  }

  private async submit(tx: Transaction, sourceKeypair: Keypair): Promise<void> {
    const prepared = await this.server.prepareTransaction(tx);
    prepared.sign(sourceKeypair);
    const response = await this.server.sendTransaction(prepared);
    await pollTransaction(this.server, response.hash);
  }
}
