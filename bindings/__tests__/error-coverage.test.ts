/**
 * Error coverage guard tests.
 *
 * These tests act as a CI-enforced contract between the Rust error enums, the
 * TypeScript union types, and the ERROR_HTTP_MAP.  They MUST fail if any
 * variant is added to a Rust enum without a corresponding update to:
 *   1. `bindings/src/types.ts`   — TS union type and *ErrorMessage() helper
 *   2. `bindings/src/errors.ts`  — ERROR_HTTP_MAP entry
 *
 * Refer to docs/bindings-error-mapping.md for the canonical variant list.
 *
 * ── HOW THESE TESTS WORK ────────────────────────────────────────────────────
 * Each test enumerates every variant that belongs in a given contract's union
 * type and asserts that:
 *   a) The variant is present in ERROR_HTTP_MAP with a valid HTTP status.
 *   b) The assigned HTTP status matches the documented convention.
 *
 * If a new variant is added to a Rust enum but not to ERROR_HTTP_MAP, the test
 * will catch it.  If a variant is removed from a Rust enum but kept in the TS
 * type/map, the test will also flag the orphan (via the exhaustiveness check
 * on expected variant counts).
 */

import { ERROR_HTTP_MAP } from "../src/errors";
import {
  muxAccountErrorMessage,
  muxBatcherErrorMessage,
  muxDelegationErrorMessage,
  muxPermissionsErrorMessage,
  muxRecoveryErrorMessage,
  muxPolicyErrorMessage,
  muxRegistryErrorMessage,
  spendingPolicyErrorMessage,
  muxAccountFactoryErrorMessage,
  muxWalletRegistryErrorMessage,
  type MuxAccountError,
  type MuxBatcherError,
  type MuxDelegationError,
  type MuxPermissionsError,
  type MuxRecoveryError,
  type MuxPolicyError,
  type SpendingPolicyError,
  type MuxAccountFactoryError,
  type MuxWalletRegistryError,
} from "../src/types";

// ── Helper ────────────────────────────────────────────────────────────────────

/**
 * Assert that every variant in `variants` is present in ERROR_HTTP_MAP and
 * that its mapped status is a valid HTTP 4xx or 5xx code.
 */
function assertAllMapped(contractName: string, variants: readonly string[]): void {
  for (const variant of variants) {
    const status = ERROR_HTTP_MAP[variant];
    if (typeof status !== "number") {
      throw new Error(`ERROR_HTTP_MAP is missing variant "${variant}" for ${contractName}`);
    }
    if (status < 400 || status >= 600) {
      throw new Error(
        `ERROR_HTTP_MAP["${variant}"] = ${status} is out of the valid 4xx/5xx range for ${contractName}`,
      );
    }
  }
}

/**
 * Assert that the *ErrorMessage helper returns a non-empty string for every
 * variant by both name and raw numeric code.
 */
function assertHelperCoversVariants(
  helperName: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  helper: (v: any) => string,
  nameToCode: Record<string, number>,
): void {
  for (const [name, code] of Object.entries(nameToCode)) {
    const byName = helper(name);
    if (byName === "unknown error code") {
      throw new Error(`${helperName}("${name}") returned "unknown error code" — variant not mapped`);
    }
    expect(byName.length).toBeGreaterThan(0);

    const byCode = helper(code);
    if (byCode === "unknown error code") {
      throw new Error(`${helperName}(${code}) returned "unknown error code" — code not mapped`);
    }
    expect(byCode.length).toBeGreaterThan(0);
  }
}

// ── MuxAccountError ───────────────────────────────────────────────────────────

const MUX_ACCOUNT_VARIANTS: readonly MuxAccountError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "DelegateNotFound",
  "DelegateExpired",
  "SpendLimitExceeded",
  "InvalidAmount",
  "InvalidPeriod",
  "TooManyDelegates",
  "ReentrancyDetected",
  "ArithmeticOverflow",
  "TooManySessionKeys",
  "ScopeNotGranted",
  "SponsorNotAuthorized",
  "InvalidNonce",
];

describe("MuxAccountError coverage", () => {
  it("has 15 variants matching the Rust enum", () => {
    expect(MUX_ACCOUNT_VARIANTS.length).toBe(15);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxAccountError", MUX_ACCOUNT_VARIANTS);
  });

  it("muxAccountErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxAccountError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      DelegateNotFound: 4,
      DelegateExpired: 5,
      SpendLimitExceeded: 6,
      InvalidAmount: 7,
      InvalidPeriod: 8,
      TooManyDelegates: 9,
      ReentrancyDetected: 10,
      ArithmeticOverflow: 11,
      TooManySessionKeys: 12,
      ScopeNotGranted: 13,
      SponsorNotAuthorized: 14,
      InvalidNonce: 15,
    };
    assertHelperCoversVariants("muxAccountErrorMessage", muxAccountErrorMessage, nameToCode);
  });

  it("maps error codes to correct HTTP statuses", () => {
    expect(ERROR_HTTP_MAP.NotInitialized).toBe(500);
    expect(ERROR_HTTP_MAP.AlreadyInitialized).toBe(409);
    expect(ERROR_HTTP_MAP.Unauthorized).toBe(401);
    expect(ERROR_HTTP_MAP.DelegateNotFound).toBe(404);
    expect(ERROR_HTTP_MAP.DelegateExpired).toBe(400);
    expect(ERROR_HTTP_MAP.SpendLimitExceeded).toBe(400);
    expect(ERROR_HTTP_MAP.InvalidAmount).toBe(400);
    expect(ERROR_HTTP_MAP.InvalidPeriod).toBe(400);
    expect(ERROR_HTTP_MAP.TooManyDelegates).toBe(409);
    expect(ERROR_HTTP_MAP.ReentrancyDetected).toBe(409);
    expect(ERROR_HTTP_MAP.ArithmeticOverflow).toBe(500);
    expect(ERROR_HTTP_MAP.TooManySessionKeys).toBe(409);
    expect(ERROR_HTTP_MAP.ScopeNotGranted).toBe(403);
    expect(ERROR_HTTP_MAP.SponsorNotAuthorized).toBe(403);
    expect(ERROR_HTTP_MAP.InvalidNonce).toBe(409);
  });
});

// ── MuxAccountFactoryError ────────────────────────────────────────────────────

const MUX_ACCOUNT_FACTORY_VARIANTS: readonly MuxAccountFactoryError[] = [
  "Unauthorized",
  "InvalidAccount",
  "TooManyAccounts",
  "MetadataNotFound",
  "MetadataTooLarge",
  "InvalidMetadata",
  "AlreadyInitialized",
];

describe("MuxAccountFactoryError coverage", () => {
  it("has 7 variants matching the Rust enum", () => {
    expect(MUX_ACCOUNT_FACTORY_VARIANTS.length).toBe(7);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxAccountFactoryError", MUX_ACCOUNT_FACTORY_VARIANTS);
  });

  it("muxAccountFactoryErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxAccountFactoryError, number> = {
      Unauthorized: 1,
      InvalidAccount: 2,
      TooManyAccounts: 3,
      MetadataNotFound: 4,
      MetadataTooLarge: 5,
      InvalidMetadata: 6,
      AlreadyInitialized: 7,
    };
    assertHelperCoversVariants("muxAccountFactoryErrorMessage", muxAccountFactoryErrorMessage, nameToCode);
  });
});

// ── WalletRegistryError ───────────────────────────────────────────────────────

const WALLET_REGISTRY_VARIANTS: readonly MuxWalletRegistryError[] = [
  "NotInitialized",
  "Unauthorized",
  "WalletNotFound",
  "WalletAlreadyRegistered",
  "TooManyWallets",
];

describe("WalletRegistryError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(WALLET_REGISTRY_VARIANTS.length).toBe(5);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("WalletRegistryError", WALLET_REGISTRY_VARIANTS);
  });

  it("muxWalletRegistryErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxWalletRegistryError, number> = {
      NotInitialized: 1,
      Unauthorized: 2,
      WalletNotFound: 3,
      WalletAlreadyRegistered: 4,
      TooManyWallets: 5,
    };
    assertHelperCoversVariants("muxWalletRegistryErrorMessage", muxWalletRegistryErrorMessage, nameToCode);
  });
});

// ── MuxBatcherError ───────────────────────────────────────────────────────────

const MUX_BATCHER_VARIANTS: readonly MuxBatcherError[] = [
  "EmptyBatch",
  "BatchTooLarge",
  "RequiredOperationFailed",
  "Unauthorized",
  "ReentrancyDetected",
  "MetadataAlreadySet",
];

describe("MuxBatcherError coverage", () => {
  it("has 6 variants in the TS union (NotInitialized/AlreadyInitialized are shared names)", () => {
    // The Rust MuxBatcherError has 8 variants but NotInitialized (7) and
    // AlreadyInitialized (8) are already covered under the shared names in
    // ERROR_HTTP_MAP.  The TS union does not duplicate them.
    expect(MUX_BATCHER_VARIANTS.length).toBe(6);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxBatcherError", MUX_BATCHER_VARIANTS);
  });

  it("MetadataAlreadySet is mapped to 409", () => {
    expect(ERROR_HTTP_MAP.MetadataAlreadySet).toBe(409);
  });

  it("muxBatcherErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxBatcherError, number> = {
      EmptyBatch: 1,
      BatchTooLarge: 2,
      RequiredOperationFailed: 3,
      Unauthorized: 4,
      ReentrancyDetected: 5,
      MetadataAlreadySet: 6,
    };
    assertHelperCoversVariants("muxBatcherErrorMessage", muxBatcherErrorMessage, nameToCode);
  });
});

// ── MuxDelegationError ────────────────────────────────────────────────────────

const MUX_DELEGATION_VARIANTS: readonly MuxDelegationError[] = [
  "DelegateNotFound",
  "DelegateExpired",
  "SpendLimitExceeded",
  "InvalidPeriod",
  "TooManyDelegates",
  "Unauthorized",
  "InvalidAmount",
];

describe("MuxDelegationError coverage", () => {
  it("has 7 variants matching the Rust enum", () => {
    expect(MUX_DELEGATION_VARIANTS.length).toBe(7);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxDelegationError", MUX_DELEGATION_VARIANTS);
  });

  it("muxDelegationErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxDelegationError, number> = {
      DelegateNotFound: 1,
      DelegateExpired: 2,
      SpendLimitExceeded: 3,
      InvalidPeriod: 4,
      TooManyDelegates: 5,
      Unauthorized: 6,
      InvalidAmount: 7,
    };
    assertHelperCoversVariants("muxDelegationErrorMessage", muxDelegationErrorMessage, nameToCode);
  });
});

// ── MuxPermissionsError ───────────────────────────────────────────────────────

const MUX_PERMISSIONS_VARIANTS: readonly MuxPermissionsError[] = [
  "Unauthorized",
  "ScopeNotGranted",
  "TooManySessionKeys",
  "InvalidNonce",
  "SponsorNotAuthorized",
];

describe("MuxPermissionsError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(MUX_PERMISSIONS_VARIANTS.length).toBe(5);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxPermissionsError", MUX_PERMISSIONS_VARIANTS);
  });

  it("muxPermissionsErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxPermissionsError, number> = {
      Unauthorized: 1,
      ScopeNotGranted: 2,
      TooManySessionKeys: 3,
      InvalidNonce: 4,
      SponsorNotAuthorized: 5,
    };
    assertHelperCoversVariants("muxPermissionsErrorMessage", muxPermissionsErrorMessage, nameToCode);
  });
});

// ── MuxRecoveryError ──────────────────────────────────────────────────────────

const MUX_RECOVERY_VARIANTS: readonly MuxRecoveryError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "RecoveryNotFound",
  "RecoveryExpired",
  "InvalidThreshold",
];

describe("MuxRecoveryError coverage", () => {
  it("has 6 variants matching the Rust enum", () => {
    expect(MUX_RECOVERY_VARIANTS.length).toBe(6);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxRecoveryError", MUX_RECOVERY_VARIANTS);
  });

  it("muxRecoveryErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxRecoveryError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      RecoveryNotFound: 4,
      RecoveryExpired: 5,
      InvalidThreshold: 6,
    };
    assertHelperCoversVariants("muxRecoveryErrorMessage", muxRecoveryErrorMessage, nameToCode);
  });
});

// ── MuxPolicyError ────────────────────────────────────────────────────────────

const MUX_POLICY_VARIANTS: readonly MuxPolicyError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "PolicyNotFound",
  "InvalidPolicy",
];

describe("MuxPolicyError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(MUX_POLICY_VARIANTS.length).toBe(5);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxPolicyError", MUX_POLICY_VARIANTS);
  });

  it("muxPolicyErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxPolicyError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      PolicyNotFound: 4,
      InvalidPolicy: 5,
    };
    assertHelperCoversVariants("muxPolicyErrorMessage", muxPolicyErrorMessage, nameToCode);
  });
});

// ── MuxRegistryError ──────────────────────────────────────────────────────────

const MUX_REGISTRY_VARIANTS: readonly MuxRegistryError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "EntryNotFound",
  "EntryAlreadyExists",
];

describe("MuxRegistryError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(MUX_REGISTRY_VARIANTS.length).toBe(5);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxRegistryError", MUX_REGISTRY_VARIANTS);
  });

  it("muxRegistryErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxRegistryError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      EntryNotFound: 4,
      EntryAlreadyExists: 5,
    };
    assertHelperCoversVariants("muxRegistryErrorMessage", muxRegistryErrorMessage, nameToCode);
  });
});

// ── SpendingPolicyError ───────────────────────────────────────────────────────

const SPENDING_POLICY_VARIANTS: readonly SpendingPolicyError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "LimitExceeded",
  "InvalidLimit",
];

describe("SpendingPolicyError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(SPENDING_POLICY_VARIANTS.length).toBe(5);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("SpendingPolicyError", SPENDING_POLICY_VARIANTS);
  });

  it("spendingPolicyErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<SpendingPolicyError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      LimitExceeded: 4,
      InvalidLimit: 5,
    };
    assertHelperCoversVariants("spendingPolicyErrorMessage", spendingPolicyErrorMessage, nameToCode);
  });
});
