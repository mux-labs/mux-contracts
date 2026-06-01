import { Networks } from "@stellar/stellar-sdk";
import type { MuxContractIds } from "./types";
import { DEFAULT_ADDRESSES } from "./addresses-config";

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
