import { Networks } from "@stellar/stellar-sdk";
import type { MuxContractIds } from "./types";
import { DEFAULT_ADDRESSES } from "./addresses-config";
import { MuxError, MuxErrorCode } from "./errors";

export interface NetworkConfig {
  rpcUrl: string;
  networkPassphrase: string;
  contracts: MuxContractIds;
}

function getContractAddresses(
  network: string,
  defaults: MuxContractIds
): MuxContractIds {
  const envPrefix = network.toUpperCase();
  return {
    muxAccount:
      process.env[`${envPrefix}_MUX_ACCOUNT_ID`] || defaults.muxAccount,
    muxBatcher:
      process.env[`${envPrefix}_MUX_BATCHER_ID`] || defaults.muxBatcher,
    muxDelegation:
      process.env[`${envPrefix}_MUX_DELEGATION_ID`] || defaults.muxDelegation,
    muxPermissions:
      process.env[`${envPrefix}_MUX_PERMISSIONS_ID`] || defaults.muxPermissions,
    muxWalletRegistry:
      process.env[`${envPrefix}_MUX_WALLET_REGISTRY_ID`] || defaults.muxWalletRegistry,
    muxPolicy:
      process.env[`${envPrefix}_MUX_POLICY_ID`] || defaults.muxPolicy || "",
    muxAccountFactory:
      process.env[`${envPrefix}_MUX_ACCOUNT_FACTORY_ID`] || defaults.muxAccountFactory || "",
    muxRegistry:
      process.env[`${envPrefix}_MUX_REGISTRY_ID`] || defaults.muxRegistry || "",
  };
}

/** Well-known Mux Protocol contract deployments. */
export const NETWORK_CONFIGS: Record<string, NetworkConfig> = {
  localnet: {
    rpcUrl: process.env.LOCALNET_RPC_URL || "http://localhost:8000",
    networkPassphrase:
      process.env.LOCALNET_NETWORK_PASSPHRASE || "Standalone Network ; February 2025",
    contracts: getContractAddresses("localnet", DEFAULT_ADDRESSES.localnet),
  },
  testnet: {
    rpcUrl: "https://soroban-testnet.stellar.org",
    networkPassphrase: Networks.TESTNET,
    contracts: getContractAddresses("testnet", DEFAULT_ADDRESSES.testnet),
  },
  mainnet: {
    rpcUrl: "https://soroban-mainnet.stellar.org",
    networkPassphrase: Networks.PUBLIC,
    contracts: getContractAddresses("mainnet", DEFAULT_ADDRESSES.mainnet),
  },
};

/**
 * Get the active network from environment variable.
 * Defaults to "localnet" for local development.
 */
export function getActiveNetwork(): string {
  return process.env.SOROBAN_NETWORK || "localnet";
}

/**
 * Get the network config for the active network.
 */
export function getNetworkConfig(): NetworkConfig {
  const network = getActiveNetwork();
  const config = NETWORK_CONFIGS[network];
  if (!config) {
    throw new Error(
      `Network "${network}" not found. Available networks: ${Object.keys(NETWORK_CONFIGS).join(", ")}`
    );
  }
  return config;
}

/**
 * Resolve a network name to its config, failing closed when the network is
 * unknown or missing. Unlike {@link getNetworkConfig}, this never falls back
 * to a default network: an absent/unknown name is rejected so callers cannot
 * silently invoke against the wrong chain.
 */
export function resolveNetworkConfig(network: string | undefined | null): NetworkConfig {
  if (!network) {
    throw new MuxError(
      MuxErrorCode.NETWORK_NOT_CONFIGURED,
      "No target network provided; refusing to resolve a default network"
    );
  }
  const config = NETWORK_CONFIGS[network];
  if (!config) {
    throw new MuxError(
      MuxErrorCode.NETWORK_UNKNOWN,
      `Network "${network}" not found. Available networks: ${Object.keys(
        NETWORK_CONFIGS
      ).join(", ")}`
    );
  }
  return config;
}

/**
 * Guard a cross-network invocation: reject any call whose target network does
 * not match the resolved/configured network. Deny-by-default — a missing or
 * unknown target network blocks rather than falling through to a default.
 *
 * @param targetNetwork Network the invocation intends to hit.
 * @param configuredNetwork Network the client is configured for. Defaults to
 *   the active network from the environment.
 * @returns The resolved config for the matched network.
 */
export function assertSameNetwork(
  targetNetwork: string | undefined | null,
  configuredNetwork: string | undefined | null = getActiveNetwork()
): NetworkConfig {
  const target = resolveNetworkConfig(targetNetwork);
  const configured = resolveNetworkConfig(configuredNetwork);

  if (target.networkPassphrase !== configured.networkPassphrase) {
    throw new MuxError(
      MuxErrorCode.CROSS_NETWORK_BLOCKED,
      `Cross-network invoke blocked: target "${targetNetwork}" does not match configured "${configuredNetwork}"`
    );
  }

  return target;
}
