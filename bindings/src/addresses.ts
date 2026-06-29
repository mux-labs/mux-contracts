import type { MuxContractIds } from "./types";

/**
 * Contract address configuration keyed by network name.
 * Addresses can be provided via environment variables or this config file.
 */
export interface AddressesConfig {
  localnet: MuxContractIds;
  testnet: MuxContractIds;
  mainnet: MuxContractIds;
}

/**
 * Load contract addresses for the active network.
 * Reads from environment variables first, then falls back to config file values.
 * Validates that required addresses are present.
 */
export function loadContractAddresses(
  network: string,
  config: AddressesConfig
): MuxContractIds {
  const networkConfig = config[network as keyof AddressesConfig];
  if (!networkConfig) {
    throw new Error(
      `Network "${network}" not found in addresses config. Available networks: ${Object.keys(config).join(", ")}`
    );
  }

  // Load from environment variables with network-specific prefixes
  const envPrefix = network.toUpperCase();
  const addresses: MuxContractIds = {
    muxAccount:
      process.env[`${envPrefix}_MUX_ACCOUNT_ID`] || networkConfig.muxAccount,
    muxBatcher:
      process.env[`${envPrefix}_MUX_BATCHER_ID`] || networkConfig.muxBatcher,
    muxDelegation:
      process.env[`${envPrefix}_MUX_DELEGATION_ID`] || networkConfig.muxDelegation,
    muxPermissions:
      process.env[`${envPrefix}_MUX_PERMISSIONS_ID`] ||
      networkConfig.muxPermissions,
    muxWalletRegistry:
      process.env[`${envPrefix}_MUX_WALLET_REGISTRY_ID`] ||
      networkConfig.muxWalletRegistry,
    muxPolicy:
      process.env[`${envPrefix}_MUX_POLICY_ID`] || networkConfig.muxPolicy || "",
    muxAccountFactory:
      process.env[`${envPrefix}_MUX_ACCOUNT_FACTORY_ID`] || networkConfig.muxAccountFactory || "",
    muxRegistry:
      process.env[`${envPrefix}_MUX_REGISTRY_ID`] || networkConfig.muxRegistry || "",
  };

  return addresses;
}

/**
 * Validate that all required contract addresses are configured for a network.
 * Throws an error with clear message if any addresses are missing.
 */
export function validateAddresses(
  network: string,
  addresses: MuxContractIds
): void {
  const missing: string[] = [];

  if (!addresses.muxAccount) {
    missing.push("muxAccount");
  }
  if (!addresses.muxBatcher) {
    missing.push("muxBatcher");
  }
  if (!addresses.muxDelegation) {
    missing.push("muxDelegation");
  }
  if (!addresses.muxPermissions) {
    missing.push("muxPermissions");
  }
  if (!addresses.muxWalletRegistry) {
    missing.push("muxWalletRegistry");
  }

  if (missing.length > 0) {
    throw new Error(
      `Missing contract addresses for network "${network}": ${missing.join(", ")}. ` +
      `Set environment variables (e.g., ${network.toUpperCase()}_MUX_ACCOUNT_ID) or update config/addresses.json.`
    );
  }
}

/**
 * Load and validate contract addresses for a network.
 * Fails fast with clear error if any required addresses are missing.
 */
export function getValidatedAddresses(
  network: string,
  config: AddressesConfig
): MuxContractIds {
  const addresses = loadContractAddresses(network, config);
  validateAddresses(network, addresses);
  return addresses;
}
