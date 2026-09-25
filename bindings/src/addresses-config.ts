// Default contract address configuration
// This can be overridden by environment variables
export const DEFAULT_ADDRESSES = {
  localnet: {
    muxAccount: "",
    muxAccountFactory: "",
    muxBatcher: "",
    muxDelegation: "",
    muxPermissions: "",
    muxPolicy: "",
    muxRecovery: "",
    muxRegistry: "",
    muxSpendingPolicy: "",
    muxWalletRegistry: "",
  },
  testnet: {
    muxAccount: "",
    muxAccountFactory: "",
    muxBatcher: "",
    muxDelegation: "",
    muxPermissions: "",
    muxPolicy: "",
    muxRecovery: "",
    muxRegistry: "",
    muxSpendingPolicy: "",
    muxWalletRegistry: "",
  },
  mainnet: {
    muxAccount: "",
    muxAccountFactory: "",
    muxBatcher: "",
    muxDelegation: "",
    muxPermissions: "",
    muxPolicy: "",
    muxRecovery: "",
    muxRegistry: "",
    muxSpendingPolicy: "",
    muxWalletRegistry: "",
  },
};

/**
 * Mainnet address PR review rule (issue #871)
 *
 * Invariant: mainnet contract addresses are money-path configuration. Any
 * change to a mainnet entry in `config/addresses.json` MUST be explicitly
 * reviewed and approved before merge, and MUST NOT be silently changed.
 *
 * Enforcement is fail-closed: a mainnet address change that lacks the
 * required review marker/approval is rejected with a stable error code so
 * CI (and any runtime loader) can gate the change instead of shipping it.
 *
 * The review marker is carried in `config/addresses.json` under the
 * `mainnetReview` object. It is intentionally separate from the address
 * values so that editing an address without updating the marker is detected.
 */
export const MAINNET_REVIEW_ERROR_CODES = {
  /** A mainnet address changed without a matching review marker. */
  UNREVIEWED_MAINNET_CHANGE: "MUX_MAINNET_ADDRESS_UNREVIEWED",
  /** The review marker is present but malformed or incomplete. */
  INVALID_REVIEW_MARKER: "MUX_MAINNET_REVIEW_MARKER_INVALID",
  /** The review marker does not cover the current mainnet addresses. */
  REVIEW_MARKER_MISMATCH: "MUX_MAINNET_REVIEW_MARKER_MISMATCH",
  /** Mainnet addresses were supplied for a non-mainnet network. */
  NETWORK_MISCONFIG: "MUX_MAINNET_NETWORK_MISCONFIG",
  /** A mainnet-affecting deploy ran without the immutable mainnet flag. */
  MAINNET_FLAG_REQUIRED: "MUX_MAINNET_FLAG_REQUIRED",
  /** The immutable mainnet flag was set to an unrecognized value. */
  MAINNET_FLAG_INVALID: "MUX_MAINNET_FLAG_INVALID",
} as const;

export type MainnetReviewErrorCode =
  (typeof MAINNET_REVIEW_ERROR_CODES)[keyof typeof MAINNET_REVIEW_ERROR_CODES];

export type AddressNetwork = keyof typeof DEFAULT_ADDRESSES;

export type ContractAddressKey = keyof (typeof DEFAULT_ADDRESSES)["mainnet"];

export type AddressBook = Record<ContractAddressKey, string>;

/**
 * Review marker required for any mainnet address change.
 *
 * `approvedBy` and `pr` identify the reviewer and the PR that approved the
 * change; `addressesHash` binds the approval to the exact set of mainnet
 * addresses so a later silent edit invalidates the marker.
 */
export interface MainnetReviewMarker {
  approvedBy: string;
  pr: string;
  addressesHash: string;
}

export interface AddressesConfig {
  localnet: AddressBook;
  testnet: AddressBook;
  mainnet: AddressBook;
  mainnetReview?: MainnetReviewMarker;
}

export class MainnetAddressReviewError extends Error {
  readonly code: MainnetReviewErrorCode;
  readonly correlationId: string;

  constructor(code: MainnetReviewErrorCode, message: string, correlationId: string) {
    super(message);
    this.name = "MainnetAddressReviewError";
    this.code = code;
    this.correlationId = correlationId;
  }
}

const CONTRACT_ADDRESS_KEYS = Object.keys(
  DEFAULT_ADDRESSES.mainnet,
) as ContractAddressKey[];

/**
 * Deterministic, order-independent hash of a mainnet address book.
 * Uses a small FNV-1a variant so the loader has no runtime dependencies.
 */
export function hashMainnetAddresses(addresses: AddressBook): string {
  const canonical = CONTRACT_ADDRESS_KEYS.map(
    (key) => `${key}=${addresses[key] ?? ""}`,
  ).join("|");

  let hash = 0x811c9dc5;
  for (let i = 0; i < canonical.length; i += 1) {
    hash ^= canonical.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

/**
 * Enforce the mainnet address PR review rule.
 *
 * Fail-closed: throws a `MainnetAddressReviewError` with a stable code when
 * mainnet addresses differ from the reviewed baseline or when the review
 * marker is missing/invalid. Testnet and localnet flows are unaffected.
 *
 * @param candidate the addresses being loaded/validated
 * @param baseline the reviewed baseline (defaults to DEFAULT_ADDRESSES)
 * @param correlationId opaque id for log correlation (never a secret)
 */
export function assertMainnetAddressReview(
  candidate: AddressesConfig,
  baseline: AddressesConfig = DEFAULT_ADDRESSES,
  correlationId: string = "addresses-config",
): void {
  const candidateMainnet = candidate.mainnet;
  const baselineMainnet = baseline.mainnet;

  const changed = CONTRACT_ADDRESS_KEYS.some(
    (key) => (candidateMainnet[key] ?? "") !== (baselineMainnet[key] ?? ""),
  );

  if (!changed) {
    return;
  }

  const marker = candidate.mainnetReview;
  if (!marker) {
    throw new MainnetAddressReviewError(
      MAINNET_REVIEW_ERROR_CODES.UNREVIEWED_MAINNET_CHANGE,
      "Mainnet address change requires a mainnetReview marker approved before merge",
      correlationId,
    );
  }

  if (!isNonEmptyString(marker.approvedBy) || !isNonEmptyString(marker.pr)) {
    throw new MainnetAddressReviewError(
      MAINNET_REVIEW_ERROR_CODES.INVALID_REVIEW_MARKER,
      "mainnetReview marker must include non-empty approvedBy and pr fields",
      correlationId,
    );
  }

  const expectedHash = hashMainnetAddresses(candidateMainnet);
  if (marker.addressesHash !== expectedHash) {
    throw new MainnetAddressReviewError(
      MAINNET_REVIEW_ERROR_CODES.REVIEW_MARKER_MISMATCH,
      "mainnetReview marker does not cover the current mainnet addresses",
      correlationId,
    );
  }
}

/**
 * Guard against testnet/mainnet misconfiguration: mainnet addresses must not
 * be supplied for a non-mainnet network, and vice versa.
 */
export function assertNetworkAddresses(
  network: AddressNetwork,
  addresses: AddressBook,
  correlationId: string = "addresses-config",
): void {
  const hasAny = CONTRACT_ADDRESS_KEYS.some(
    (key) => (addresses[key] ?? "").trim().length > 0,
  );

  if (network !== "mainnet" && hasAny) {
    throw new MainnetAddressReviewError(
      MAINNET_REVIEW_ERROR_CODES.NETWORK_MISCONFIG,
      `Non-mainnet network '${network}' must not carry mainnet addresses`,
      correlationId,
    );
  }
}

/**
 * Immutable mainnet flag for deploy scripts (issue #809)
 *
 * Invariant: any mainnet-affecting deploy path MUST require an explicit,
 * immutable mainnet flag. The flag is deny-by-default: when it is absent or
 * not exactly the expected sentinel value, the deploy aborts with a stable,
 * actionable error code. The flag is read once and frozen so it cannot be
 * silently overridden at runtime after the deploy has started.
 *
 * The flag is supplied via the `MUX_MAINNET_DEPLOY_FLAG` environment variable
 * and must equal `MUX_MAINNET_DEPLOY_FLAG_VALUE`. It is intentionally a
 * sentinel (not a boolean) so a stray `true`/`1` cannot enable mainnet by
 * accident.
 */
export const MUX_MAINNET_DEPLOY_FLAG_ENV = "MUX_MAINNET_DEPLOY_FLAG";
export const MUX_MAINNET_DEPLOY_FLAG_VALUE = "I_ACKNOWLEDGE_MAINNET_DEPLOY";

/**
 * Immutable, deny-by-default mainnet deploy flag.
 *
 * `enabled` is only true when the environment variable is set to the exact
 * sentinel value. The object is frozen so callers cannot flip it at runtime.
 */
export interface MainnetDeployFlag {
  readonly enabled: boolean;
  readonly value: string | undefined;
}

/**
 * Read the immutable mainnet deploy flag from the environment.
 *
 * Fail-closed: an unrecognized value yields `enabled: false` rather than
 * throwing here, so callers can decide whether to abort. Use
 * `assertMainnetDeployFlag` on any mainnet-affecting path.
 */
export function readMainnetDeployFlag(
  env: Record<string, string | undefined> = typeof process !== "undefined"
    ? process.env
    : {},
): MainnetDeployFlag {
  const value = env[MUX_MAINNET_DEPLOY_FLAG_ENV];
  return Object.freeze({
    enabled: value === MUX_MAINNET_DEPLOY_FLAG_VALUE,
    value,
  });
}

/**
 * Enforce the immutable mainnet flag on a mainnet-affecting deploy path.
 *
 * Fail-closed: throws a `MainnetAddressReviewError` with a stable code when
 * the flag is missing, malformed, or set to an unrecognized value. Non-mainnet
 * networks are unaffected.
 *
 * @param network the target network for the deploy
 * @param flag the immutable flag (defaults to reading the environment)
 * @param correlationId opaque id for log correlation (never a secret)
 */
export function assertMainnetDeployFlag(
  network: AddressNetwork,
  flag: MainnetDeployFlag = readMainnetDeployFlag(),
  correlationId: string = "addresses-config",
): void {
  if (network !== "mainnet") {
    return;
  }

  if (flag.value === undefined || flag.value.trim().length === 0) {
    throw new MainnetAddressReviewError(
      MAINNET_REVIEW_ERROR_CODES.MAINNET_FLAG_REQUIRED,
      `Mainnet deploy requires ${MUX_MAINNET_DEPLOY_FLAG_ENV}=${MUX_MAINNET_DEPLOY_FLAG_VALUE}`,
      correlationId,
    );
  }

  if (!flag.enabled) {
    throw new MainnetAddressReviewError(
      MAINNET_REVIEW_ERROR_CODES.MAINNET_FLAG_INVALID,
      `${MUX_MAINNET_DEPLOY_FLAG_ENV} must equal the expected sentinel value`,
      correlationId,
    );
  }
}
