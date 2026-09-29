/**
 * Registry invoke examples for @mux-protocol/contracts
 *
 * Demonstrates how to interact with the mux-registry contract:
 * - register a contract version (admin only)
 * - read back a version
 * - list all registered contracts
 * - register a named wallet (owner/delegate authz)
 * - look up a named wallet (owner/delegate/guardian authz)
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
 *
 *   # Register a named wallet (owner signs)
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-registry \
 *     --function register_wallet \
 *     --secret-key S... \
 *     --arg '{"type":"string","value":"alice"}' \
 *     --arg '{"type":"address","value":"G..."}'
 *
 *   # Look up a named wallet (owner/delegate/guardian signs)
 *   bash scripts/local-invoke.sh \
 *     --contract-name mux-registry \
 *     --function lookup_wallet \
 *     --secret-key S... \
 *     --arg '{"type":"string","value":"alice"}' \
 *     --simulate-only
 */

import { Keypair } from "@stellar/stellar-sdk";
import { MuxRegistryClient } from "../generated/mux-registry";
import { getNetworkConfig } from "../network";

/**
 * Stable error codes surfaced by the registry entrypoints. Clients should
 * branch on these codes rather than parsing free-form messages.
 */
export enum RegistryErrorCode {
  Unauthorized = "REGISTRY_UNAUTHORIZED",
  InvalidName = "REGISTRY_INVALID_NAME",
  WalletExists = "REGISTRY_WALLET_EXISTS",
  WalletNotFound = "REGISTRY_WALLET_NOT_FOUND",
  DependencyUnavailable = "REGISTRY_DEPENDENCY_UNAVAILABLE",
}

/**
 * Role that authorizes a named-wallet operation. Deny-by-default: callers
 * must present an explicit role, and the contract enforces it on-chain.
 */
export type WalletRole = "owner" | "delegate" | "guardian";

/**
 * Result of a named-wallet lookup. `correlationId` is echoed from the request
 * so ops can trace a call across logs without exposing key material.
 */
export interface NamedWalletRecord {
  name: string;
  address: string;
  role: WalletRole;
  correlationId: string;
}

/**
 * Options shared by named-wallet entrypoints. `correlationId` is required so
 * every privileged call is traceable; `idempotencyKey` makes register safe to
 * retry under concurrent/replayed requests.
 */
export interface NamedWalletOptions {
  correlationId: string;
  idempotencyKey?: string;
}

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

/**
 * Register a named wallet. The signing keypair must hold the `owner` role for
 * the name; the contract rejects any other caller (deny-by-default). The
 * `idempotencyKey` makes concurrent/replayed registrations safe: a repeat with
 * the same key returns the existing record instead of failing.
 */
export async function registerNamedWallet(
  contractId: string,
  ownerKeypair: Keypair,
  name: string,
  address: string,
  options: NamedWalletOptions
): Promise<NamedWalletRecord> {
  const config = getNetworkConfig();
  const client = new MuxRegistryClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  return client.registerWallet(ownerKeypair, name, address, {
    role: "owner",
    correlationId: options.correlationId,
    idempotencyKey: options.idempotencyKey,
  });
}

/**
 * Look up a named wallet. The caller must present an authorized role
 * (owner/delegate/guardian); the contract enforces the policy on-chain so
 * clients cannot bypass it. Uses simulation (no fee).
 */
export async function lookupNamedWallet(
  contractId: string,
  callerKeypair: Keypair,
  name: string,
  role: WalletRole,
  options: NamedWalletOptions
): Promise<NamedWalletRecord> {
  const config = getNetworkConfig();
  const client = new MuxRegistryClient({
    contractId,
    networkPassphrase: config.networkPassphrase,
    rpcUrl: config.rpcUrl,
  });
  return client.lookupWallet(callerKeypair, name, {
    role,
    correlationId: options.correlationId,
  });
}
