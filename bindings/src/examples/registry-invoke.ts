/**
 * Registry invoke examples for @mux-protocol/contracts
 *
 * Demonstrates how to interact with the mux-registry contract:
 * - register a contract version (admin only)
 * - read back a version
 * - list all registered contracts
 *
 * Run via the local-invoke helper:
 *
 *   # Register a contract version (admin signs)
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-registry \
 *     --function register \
 *     --secret-key S... \
 *     --arg '{"type":"symbol","value":"account"}' \
 *     --arg '{"type":"string","value":"1.0.0"}'
 *
 *   # Read a contract version (simulate only)
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-registry \
 *     --function get_version \
 *     --secret-key S... \
 *     --arg '{"type":"symbol","value":"account"}' \
 *     --simulate-only
 *
 *   # List all registered contracts
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-registry \
 *     --function list_contracts \
 *     --secret-key S... \
 *     --simulate-only
 */

import { Keypair } from "@stellar/stellar-sdk";
import { MuxRegistryClient } from "../generated/mux-registry";
import { getNetworkConfig } from "../network";

/**
 * Register a contract version in the mux-registry.
 * The caller must be the registry admin.
 */
export async function registerContractVersion(
  contractId: string,
  adminKeypair: Keypair,
  name: string,
  version: string
): Promise<void> {
  const config = getNetworkConfig();
  const client = new MuxRegistryClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  await client.register(adminKeypair, name, version);
}

/**
 * Read the version of a registered contract. Uses simulation (no fee).
 */
export async function getContractVersion(
  contractId: string,
  name: string
): Promise<string> {
  const config = getNetworkConfig();
  const client = new MuxRegistryClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  return client.getVersion(Keypair.random(), name);
}

/**
 * List all registered contract names. Uses simulation (no fee).
 */
export async function listRegisteredContracts(
  contractId: string
): Promise<string[]> {
  const config = getNetworkConfig();
  const client = new MuxRegistryClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  return client.listContracts(Keypair.random());
}
