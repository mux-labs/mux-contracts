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
 * Stable error codes for address configuration failures.
 * Consumers (CI, ops tooling) can branch on these instead of parsing messages.
 */
export const AddressErrorCode = {
  NETWORK_NOT_FOUND: "ADDR_NETWORK_NOT_FOUND",
  MISSING_ADDRESSES: "ADDR_MISSING_ADDRESSES",
  MAINNET_REVIEW_REQUIRED: "ADDR_MAINNET_REVIEW_REQUIRED",
  MALFORMED_ADDRESS: "ADDR_MALFORMED_CONTRACT_ID",
} as const;

/**
 * Strkey contract-ID format: a leading "C" followed by 55 base32 characters
 * (RFC 4648 alphabet without padding), 56 characters total. Matches the
 * `ADDRESS_RE` convention already used for decoding event addresses in
 * `event-decode.ts` / `batcher-events.ts`, narrowed to contract ("C") ids
 * since only contract addresses are configured here.
 */
export const CONTRACT_ID_RE = /^C[A-Z2-7]{55}$/;

export type AddressErrorCode =
  (typeof AddressErrorCode)[keyof typeof AddressErrorCode];

/**
 * Error thrown for address configuration failures. Carries a stable `code`
 * so callers and CI can fail closed on specific conditions.
 */
export class AddressConfigError extends Error {
  readonly code: AddressErrorCode;

  constructor(code: AddressErrorCode, message: string) {
    super(message);
    this.name = "AddressConfigError";
    this.code = code;
  }
}

/**
 * Mainnet address PR review rule.
 *
 * Mainnet entries in config/addresses.json are money-path configuration and
 * MUST be reviewed/approved before merge. A mainnet address change is only
 * accepted when an explicit review marker is present, so an unreviewed edit
 * fails closed instead of silently shipping to mainnet.
 *
 * The marker is supplied out-of-band (CI secret / PR label) via the
 * `MAINNET_ADDRESS_REVIEW_APPROVED` environment variable. It must be the
 * literal string "true" to approve a mainnet change.
 */
export const MAINNET_REVIEW_ENV = "MAINNET_ADDRESS_REVIEW_APPROVED";

/**
 * Returns true when the mainnet address review marker is present and valid.
 */
export function isMainnetReviewApproved(
  env: Record<string, string | undefined> = process.env
): boolean {
  return env[MAINNET_REVIEW_ENV] === "true";
}

/**
 * Enforce the mainnet address PR review rule for a network.
 *
 * Fails closed: any mainnet address resolution requires the review marker.
 * Testnet/localnet flows are unaffected.
 */
export function assertMainnetReviewApproved(
  network: string,
  env: Record<string, string | undefined> = process.env
): void {
  if (network !== "mainnet") {
    return;
  }
  if (!isMainnetReviewApproved(env)) {
    throw new AddressConfigError(
      AddressErrorCode.MAINNET_REVIEW_REQUIRED,
      `Mainnet address change requires review approval. ` +
        `Set ${MAINNET_REVIEW_ENV}=true only after the mainnet address PR has been reviewed and approved.`
    );
  }
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
    throw new AddressConfigError(
      AddressErrorCode.NETWORK_NOT_FOUND,
      `Network "${network}" not found in addresses config. Available networks: ${Object.keys(config).join(", ")}`
    );
  }

  // Mainnet addresses are money-path config: fail closed unless reviewed.
  assertMainnetReviewApproved(network);

  // Load from environment variables with network-specific prefixes
  const envPrefix = network.toUpperCase();
  const addresses: MuxContractIds = {
    muxAccount:
      process.env[`${envPrefix}_MUX_ACCOUNT_ID`] || networkConfig.muxAccount,
    muxAccountFactory:
      process.env[`${envPrefix}_MUX_ACCOUNT_FACTORY_ID`] || networkConfig.muxAccountFactory || "",
    muxBatcher:
      process.env[`${envPrefix}_MUX_BATCHER_ID`] || networkConfig.muxBatcher,
    muxDelegation:
      process.env[`${envPrefix}_MUX_DELEGATION_ID`] || networkConfig.muxDelegation,
    muxPermissions:
      process.env[`${envPrefix}_MUX_PERMISSIONS_ID`] ||
      networkConfig.muxPermissions,
    muxPolicy:
      process.env[`${envPrefix}_MUX_POLICY_ID`] || networkConfig.muxPolicy || "",
    muxRecovery:
      process.env[`${envPrefix}_MUX_RECOVERY_ID`] || networkConfig.muxRecovery || "",
    muxRegistry:
      process.env[`${envPrefix}_MUX_REGISTRY_ID`] || networkConfig.muxRegistry || "",
    muxSpendingPolicy:
      process.env[`${envPrefix}_MUX_SPENDING_POLICY_ID`] || networkConfig.muxSpendingPolicy || "",
    muxWalletRegistry:
      process.env[`${envPrefix}_MUX_WALLET_REGISTRY_ID`] ||
      networkConfig.muxWalletRegistry,
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
    throw new AddressConfigError(
      AddressErrorCode.MISSING_ADDRESSES,
      `Missing contract addresses for network "${network}": ${missing.join(", ")}. ` +
      `Set environment variables (e.g., ${network.toUpperCase()}_MUX_ACCOUNT_ID) or update config/addresses.json.`
    );
  }

  // Required addresses are present; also confirm each one is a well-formed
  // contract-ID strkey (56 chars, leading "C"). A typo'd or truncated
  // address would otherwise pass validation and only fail later, opaquely,
  // at the RPC layer.
  const malformed: string[] = [];
  const requiredKeys: Array<keyof MuxContractIds> = [
    "muxAccount",
    "muxBatcher",
    "muxDelegation",
    "muxPermissions",
    "muxWalletRegistry",
  ];
  for (const key of requiredKeys) {
    const value = addresses[key];
    if (value && !CONTRACT_ID_RE.test(value)) {
      malformed.push(key);
    }
  }

  if (malformed.length > 0) {
    throw new AddressConfigError(
      AddressErrorCode.MALFORMED_ADDRESS,
      `Malformed contract address for network "${network}": ${malformed.join(", ")}. ` +
      `Contract IDs must be 56-character strkey values starting with "C" (e.g., ${network.toUpperCase()}_MUX_ACCOUNT_ID).`
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
