/**
 * Mux Account invoke examples for @mux-protocol/contracts
 *
 * Demonstrates how to interact with the mux-account contract:
 * - initialize an account
 * - set and remove delegates
 * - manage spending limits
 * - read owner, delegates, and guardians
 *
 * Run via the local-invoke helper:
 *
 *   # Initialize an account (owner signs)
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-account \
 *     --function initialize \
 *     --secret-key S... \
 *     --arg '{"type":"address","value":"G..."}' \
 *     --arg '{"type":"vec","value":[]}'
 *
 *   # Read the owner (simulate only)
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-account \
 *     --function owner \
 *     --secret-key S... \
 *     --simulate-only
 *
 *   # Set a delegate
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-account \
 *     --function set_delegate \
 *     --secret-key S... \
 *     --arg '{"type":"address","value":"G..."}' \
 *     --arg '{"type":"u32","value":"1000000"}' \
 *     --arg '{"type":"bool","value":"true"}'
 *
 *   # Set a spend limit
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-account \
 *     --function set_spend_limit \
 *     --secret-key S... \
 *     --arg '{"type":"address","value":"G..."}' \
 *     --arg '{"type":"i128","value":"10000000000"}' \
 *     --arg '{"type":"u32","value":"14400"}'
 *
 *   # Check paused status
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-account \
 *     --function is_paused \
 *     --secret-key S... \
 *     --simulate-only
 */

import { Address, Keypair } from "@stellar/stellar-sdk";
import { MuxAccountClient } from "../generated/mux-account";
import { getNetworkConfig } from "../network";

/**
 * Initialize a mux-account contract with owner and guardians.
 * The caller must be the owner.
 */
export async function initializeAccount(
  contractId: string,
  ownerKeypair: Keypair,
  guardians: Address[]
): Promise<void> {
  const config = getNetworkConfig();
  const client = new MuxAccountClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  const ownerAddr = Address.fromString(ownerKeypair.publicKey());
  await client.initialize(ownerKeypair, ownerAddr, guardians);
}

/**
 * Read the owner address of a mux-account contract.
 * Uses simulation (no fee, no signing).
 */
export async function getOwner(
  contractId: string
): Promise<string> {
  const config = getNetworkConfig();
  const client = new MuxAccountClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  const owner = await client.owner(Keypair.random());
  return owner.toString();
}

/**
 * Grant delegate access to a given address.
 */
export async function grantDelegate(
  contractId: string,
  ownerKeypair: Keypair,
  delegate: Address,
  expiryLedger: number,
  canSpend: boolean
): Promise<void> {
  const config = getNetworkConfig();
  const client = new MuxAccountClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  await client.setDelegate(ownerKeypair, delegate, expiryLedger, canSpend);
}

/**
 * Remove a delegate from the account.
 */
export async function revokeDelegate(
  contractId: string,
  ownerKeypair: Keypair,
  delegate: Address
): Promise<void> {
  const config = getNetworkConfig();
  const client = new MuxAccountClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  await client.removeDelegate(ownerKeypair, delegate);
}

/**
 * Set a spending limit for a given asset.
 */
export async function setSpendLimit(
  contractId: string,
  ownerKeypair: Keypair,
  asset: Address,
  amount: bigint,
  periodLedgers: number
): Promise<void> {
  const config = getNetworkConfig();
  const client = new MuxAccountClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  await client.setSpendLimit(ownerKeypair, asset, amount, periodLedgers);
}
