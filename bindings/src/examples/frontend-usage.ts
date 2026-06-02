/**
 * Frontend import example for @mux-protocol/contracts
 *
 * This file demonstrates how to integrate the Mux Protocol SDK in a
 * TypeScript/JavaScript frontend application (browser, React, Next.js, etc.)
 *
 * Installation:
 *   npm install @mux-protocol/contracts @stellar/stellar-sdk
 *
 * Environment variables (set in .env or your hosting platform):
 *   SOROBAN_NETWORK=testnet          # localnet | testnet | mainnet
 *   TESTNET_MUX_ACCOUNT_ID=C...      # contract address on your target network
 *   TESTNET_MUX_BATCHER_ID=C...
 *   TESTNET_MUX_PERMISSIONS_ID=C...
 */

import { Keypair } from "@stellar/stellar-sdk";
import {
  getNetworkConfig,
  getValidatedAddresses,
  contractErrorToHttp,
  DEFAULT_ADDRESSES,
  type NetworkConfig,
  type MuxAccountError,
  type MuxBatcherError,
} from ".."; // replace with "@mux-protocol/contracts" in your project

// ---------------------------------------------------------------------------
// 1. Bootstrap: load network config and contract addresses
// ---------------------------------------------------------------------------

/**
 * Returns the active NetworkConfig (RPC URL, passphrase, contract IDs).
 * Reads SOROBAN_NETWORK from the environment; falls back to "localnet".
 */
export function bootstrapNetwork(): NetworkConfig {
  // Validate that all three contract addresses are populated before proceeding.
  getValidatedAddresses(
    process.env.SOROBAN_NETWORK ?? "localnet",
    DEFAULT_ADDRESSES
  );
  return getNetworkConfig();
}

// ---------------------------------------------------------------------------
// 2. Reading on-chain state (no signing required)
// ---------------------------------------------------------------------------

/**
 * Fetches the owner address of a deployed mux-account contract.
 *
 * The generated clients accept { contractId, networkPassphrase, rpcUrl } and
 * expose every contract function as a typed async method.  Read-only calls
 * (queries) do not need a real signer; pass a throwaway Keypair.
 */
export async function fetchAccountOwner(
  contractId: string
): Promise<string> {
  const { MuxAccountClient } = await import("../generated/mux-account");
  const config = getNetworkConfig();

  const client = new MuxAccountClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });

  // owner() is a read-only view function; pass a random keypair for simulation.
  const signer = Keypair.random();
  const owner = await client.owner(signer);
  return owner.toString();
}

// ---------------------------------------------------------------------------
// 3. Sending a transaction (mutation with real keypair)
// ---------------------------------------------------------------------------

/**
 * Grants delegate access to `delegateAddress` on the caller's mux-account.
 *
 * @param contractId      - Deployed mux-account contract ID
 * @param ownerKeypair    - Owner's keypair (signs the transaction)
 * @param delegateAddress - Address to grant delegation to
 * @param expiryLedger    - Ledger sequence number at which the delegate expires
 * @param canSpend        - Whether the delegate may call debit_spend
 */
export async function grantDelegate(
  contractId: string,
  ownerKeypair: Keypair,
  delegateAddress: string,
  expiryLedger: number,
  canSpend: boolean
): Promise<void> {
  const { MuxAccountClient } = await import("../generated/mux-account");
  const { Address } = await import("@stellar/stellar-sdk");
  const config = getNetworkConfig();

  const client = new MuxAccountClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });

  try {
    await client.setDelegate(
      ownerKeypair,
      Address.fromString(delegateAddress),
      expiryLedger,
      canSpend
    );
  } catch (err) {
    // Map contract error strings to HTTP-style responses for your API layer.
    const httpError = contractErrorToHttp(err as MuxAccountError);
    throw new Error(`Contract error ${httpError.statusCode}: ${httpError.message}`);
  }
}

// ---------------------------------------------------------------------------
// 4. Checking permissions via mux-permissions
// ---------------------------------------------------------------------------

/**
 * Returns true if `accountAddress` holds the given `permission` through any
 * of its roles in the mux-permissions registry.
 */
export async function checkPermission(
  contractId: string,
  accountAddress: string,
  permission: string
): Promise<boolean> {
  const { MuxPermissionsClient } = await import("../generated/mux-permissions");
  const { Address } = await import("@stellar/stellar-sdk");
  const config = getNetworkConfig();

  const client = new MuxPermissionsClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });

  const signer = Keypair.random();
  // `permission` is a plain string — the generated client converts it to ScvSymbol internally.
  return client.hasPermission(
    signer,
    Address.fromString(accountAddress),
    permission
  );
}

// ---------------------------------------------------------------------------
// 5. Atomic batch execution via mux-batcher
// ---------------------------------------------------------------------------

/**
 * Executes multiple contract calls atomically via mux-batcher.
 *
 * @param batcherContractId - Deployed mux-batcher contract ID
 * @param callerKeypair     - Signer who authorises the batch
 * @param operations        - Array of { target, fnName, args, requireSuccess }
 * @returns BatchResult with successCount and failureCount
 */
export async function executeBatch(
  batcherContractId: string,
  callerKeypair: Keypair,
  operations: Array<{
    target: string;
    fnName: string;
    args: import("@stellar/stellar-sdk").xdr.ScVal[];
    requireSuccess: boolean;
    kind: import("..").BatchOperationKind;
  }>
) {
  const { MuxBatcherClient } = await import("../generated/mux-batcher");
  const { Address } = await import("@stellar/stellar-sdk");
  const config = getNetworkConfig();

  const client = new MuxBatcherClient({
    contractId: batcherContractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });

  const ops = operations.map((op) => ({
    target: Address.fromString(op.target),
    fnName: op.fnName,
    args: op.args,
    requireSuccess: op.requireSuccess,
    kind: op.kind,
  }));

  try {
    // `caller` is the on-chain Address that authorises the batch.
    const callerAddress = Address.fromString(callerKeypair.publicKey());
    return await client.executeBatch(callerKeypair, callerAddress, ops);
  } catch (err) {
    const httpError = contractErrorToHttp(err as MuxBatcherError);
    throw new Error(`Batch error ${httpError.statusCode}: ${httpError.message}`);
  }
}

// ---------------------------------------------------------------------------
// 6. React hook pattern (illustrative — no React dependency in this package)
// ---------------------------------------------------------------------------

/**
 * Example of how you would wrap the SDK in a React hook.
 *
 * ```tsx
 * // In your component:
 * const { owner, loading, error } = useMuxAccountOwner(contractId);
 * ```
 *
 * Since this file must not import React, the hook body is shown as a comment:
 *
 * ```ts
 * import { useState, useEffect } from "react";
 * import { fetchAccountOwner } from "@mux-protocol/contracts/examples/frontend-usage";
 *
 * export function useMuxAccountOwner(contractId: string) {
 *   const [owner, setOwner] = useState<string | null>(null);
 *   const [loading, setLoading] = useState(true);
 *   const [error, setError] = useState<Error | null>(null);
 *
 *   useEffect(() => {
 *     fetchAccountOwner(contractId)
 *       .then(setOwner)
 *       .catch(setError)
 *       .finally(() => setLoading(false));
 *   }, [contractId]);
 *
 *   return { owner, loading, error };
 * }
 * ```
 */
export const REACT_HOOK_EXAMPLE = "see JSDoc above";
