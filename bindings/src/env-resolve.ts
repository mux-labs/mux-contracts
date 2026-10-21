/**
 * Shared env-var resolution for contract addresses (issue #988).
 *
 * `getContractAddresses` (network.ts) and `loadContractAddresses`
 * (addresses.ts) each resolve a `<NETWORK>_MUX_<NAME>_ID` environment
 * variable per contract, falling back to a provided default when unset.
 * Both grew their own copy of that resolution loop, which can drift (e.g. a
 * typo'd env-var suffix fixed in one copy but not the other). This module is
 * the single source of truth for the `key -> env suffix` mapping and the
 * fallback precedence, so both call sites share one implementation.
 */

/** Maps a `MuxContractIds` key to the `<NAME>` segment of its env var. */
const CONTRACT_ENV_SUFFIXES: Record<string, string> = {
  muxAccount: "ACCOUNT",
  muxAccountFactory: "ACCOUNT_FACTORY",
  muxBatcher: "BATCHER",
  muxDelegation: "DELEGATION",
  muxPermissions: "PERMISSIONS",
  muxPolicy: "POLICY",
  muxRecovery: "RECOVERY",
  muxRegistry: "REGISTRY",
  muxSpendingPolicy: "SPENDING_POLICY",
  muxWalletRegistry: "WALLET_REGISTRY",
};

/**
 * Resolve contract addresses for `network` from environment variables,
 * falling back to `defaults` (and then `""`) for any key not overridden.
 *
 * Only resolves the keys present on `defaults`, so a caller that only cares
 * about a subset of `MuxContractIds` (as `network.ts` does) gets back
 * exactly that subset — the shared loop does not widen either call site's
 * existing key set.
 *
 * Precedence per key: `process.env[\`${NETWORK}_MUX_${SUFFIX}_ID\`]` ||
 * `defaults[key]` || `""`.
 */
export function resolveContractAddressesFromEnv<
  T extends Record<string, string | undefined>,
>(network: string, defaults: T): T {
  const envPrefix = network.toUpperCase();
  const resolved = { ...defaults };

  for (const key of Object.keys(defaults) as Array<keyof T>) {
    const suffix = CONTRACT_ENV_SUFFIXES[key as string];
    if (!suffix) {
      continue;
    }
    const envValue = process.env[`${envPrefix}_MUX_${suffix}_ID`];
    resolved[key] = (envValue || defaults[key] || "") as T[keyof T];
  }

  return resolved;
}
