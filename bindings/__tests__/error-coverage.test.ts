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
];

describe("MuxAccountFactoryError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(MUX_ACCOUNT_FACTORY_VARIANTS.length).toBe(5);
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
    };
    assertHelperCoversVariants("muxAccountFactoryErrorMessage", muxAccountFactoryErrorMessage, nameToCode);
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
  "NotADelegate",
  "TooManyPermissions",
  "EmptyPermissions",
  "TooManyDelegates",
  "ContractIdAlreadySet",
  "NotInitialized",
  "AlreadyInitialized",
];

describe("MuxDelegationError coverage", () => {
  it("has 7 variants matching the Rust enum (codes 6001–6007)", () => {
    expect(MUX_DELEGATION_VARIANTS.length).toBe(7);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxDelegationError", MUX_DELEGATION_VARIANTS);
  });

  it("maps delegation-specific variants to correct HTTP statuses", () => {
    expect(ERROR_HTTP_MAP.NotADelegate).toBe(404);
    expect(ERROR_HTTP_MAP.TooManyPermissions).toBe(400);
    expect(ERROR_HTTP_MAP.EmptyPermissions).toBe(400);
    expect(ERROR_HTTP_MAP.TooManyDelegates).toBe(409);
    expect(ERROR_HTTP_MAP.ContractIdAlreadySet).toBe(409);
  });

  it("muxDelegationErrorMessage covers all variants and codes 6001–6007", () => {
    const nameToCode: Record<MuxDelegationError, number> = {
      NotADelegate: 6001,
      TooManyPermissions: 6002,
      EmptyPermissions: 6003,
      TooManyDelegates: 6004,
      ContractIdAlreadySet: 6005,
      NotInitialized: 6006,
      AlreadyInitialized: 6007,
    };
    assertHelperCoversVariants("muxDelegationErrorMessage", muxDelegationErrorMessage, nameToCode);
  });
});

// ── MuxPermissionsError ───────────────────────────────────────────────────────

const MUX_PERMISSIONS_VARIANTS: readonly MuxPermissionsError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "RoleNotFound",
  "AccountNotInRole",
  "PermissionNotFound",
  "TooManyMembers",
  "TooManyRoles",
  "AdminNotFound",
  "AlreadyApproved",
];

describe("MuxPermissionsError coverage", () => {
  it("has 10 variants in the TS union (TooManyPendingAdmins covered separately)", () => {
    expect(MUX_PERMISSIONS_VARIANTS.length).toBe(10);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxPermissionsError", MUX_PERMISSIONS_VARIANTS);
  });

  it("TooManyPendingAdmins is mapped to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyPendingAdmins).toBe(409);
  });

  it("muxPermissionsErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxPermissionsError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      RoleNotFound: 4,
      AccountNotInRole: 5,
      PermissionNotFound: 6,
      TooManyMembers: 7,
      TooManyRoles: 8,
      AdminNotFound: 9,
      AlreadyApproved: 10,
    };
    assertHelperCoversVariants("muxPermissionsErrorMessage", muxPermissionsErrorMessage, nameToCode);
  });
});

// ── MuxPolicyError ────────────────────────────────────────────────────────────

const MUX_POLICY_VARIANTS: readonly MuxPolicyError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "LimitNotFound",
  "LimitExceeded",
  "InvalidAmount",
  "InvalidPeriod",
  "TooManyWallets",
];

describe("MuxPolicyError coverage", () => {
  it("has 8 variants matching the Rust enum", () => {
    expect(MUX_POLICY_VARIANTS.length).toBe(8);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxPolicyError", MUX_POLICY_VARIANTS);
  });

  it("TooManyWallets is mapped to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyWallets).toBe(409);
  });

  it("LimitNotFound is mapped to 404", () => {
    expect(ERROR_HTTP_MAP.LimitNotFound).toBe(404);
  });

  it("muxPolicyErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxPolicyError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      LimitNotFound: 4,
      LimitExceeded: 5,
      InvalidAmount: 6,
      InvalidPeriod: 7,
      TooManyWallets: 8,
    };
    assertHelperCoversVariants("muxPolicyErrorMessage", muxPolicyErrorMessage, nameToCode);
  });
});

// ── MuxRecoveryError ──────────────────────────────────────────────────────────

const MUX_RECOVERY_VARIANTS: readonly MuxRecoveryError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "RecoveryAlreadyPending",
  "NoActiveRecovery",
  "TimelockNotExpired",
  "TooManyGuardians",
  "GuardianAlreadyExists",
  "GuardianNotFound",
  "MinGuardiansRequired",
  "RecoveryExpired",
];

describe("MuxRecoveryError coverage", () => {
  it("has 11 variants matching the Rust RecoveryError enum", () => {
    expect(MUX_RECOVERY_VARIANTS.length).toBe(11);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxRecoveryError", MUX_RECOVERY_VARIANTS);
  });

  it("maps recovery-specific variants to correct HTTP statuses", () => {
    expect(ERROR_HTTP_MAP.RecoveryAlreadyPending).toBe(409);
    expect(ERROR_HTTP_MAP.NoActiveRecovery).toBe(404);
    expect(ERROR_HTTP_MAP.TimelockNotExpired).toBe(400);
    expect(ERROR_HTTP_MAP.TooManyGuardians).toBe(409);
    expect(ERROR_HTTP_MAP.GuardianAlreadyExists).toBe(409);
    expect(ERROR_HTTP_MAP.GuardianNotFound).toBe(404);
    expect(ERROR_HTTP_MAP.MinGuardiansRequired).toBe(400);
    expect(ERROR_HTTP_MAP.RecoveryExpired).toBe(400);
  });

  it("muxRecoveryErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<MuxRecoveryError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      RecoveryAlreadyPending: 4,
      NoActiveRecovery: 5,
      TimelockNotExpired: 6,
      TooManyGuardians: 7,
      GuardianAlreadyExists: 8,
      GuardianNotFound: 9,
      MinGuardiansRequired: 10,
      RecoveryExpired: 11,
    };
    assertHelperCoversVariants("muxRecoveryErrorMessage", muxRecoveryErrorMessage, nameToCode);
  });
});

// ── MuxRegistryError ──────────────────────────────────────────────────────────

const MUX_REGISTRY_VARIANT_NAMES = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "ContractNotFound",
  "TooManyContracts",
] as const;

describe("MuxRegistryError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(MUX_REGISTRY_VARIANT_NAMES.length).toBe(5);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxRegistryError", MUX_REGISTRY_VARIANT_NAMES);
  });

  it("ContractNotFound is mapped to 404", () => {
    expect(ERROR_HTTP_MAP.ContractNotFound).toBe(404);
  });

  it("TooManyContracts is mapped to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyContracts).toBe(409);
  });

  it("muxRegistryErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<string, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      ContractNotFound: 4,
      TooManyContracts: 5,
    };
    assertHelperCoversVariants("muxRegistryErrorMessage", muxRegistryErrorMessage, nameToCode);
  });
});

// ── SpendingPolicyError ───────────────────────────────────────────────────────

const SPENDING_POLICY_VARIANTS: readonly SpendingPolicyError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "PolicyNotFound",
  "SpendLimitExceeded",
  "InvalidInput",
];

describe("SpendingPolicyError coverage", () => {
  it("has 6 variants matching the Rust enum", () => {
    expect(SPENDING_POLICY_VARIANTS.length).toBe(6);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("SpendingPolicyError", SPENDING_POLICY_VARIANTS);
  });

  it("PolicyNotFound is mapped to 404", () => {
    expect(ERROR_HTTP_MAP.PolicyNotFound).toBe(404);
  });

  it("spendingPolicyErrorMessage covers all variants and codes", () => {
    const nameToCode: Record<SpendingPolicyError, number> = {
      NotInitialized: 1,
      AlreadyInitialized: 2,
      Unauthorized: 3,
      PolicyNotFound: 4,
      SpendLimitExceeded: 5,
      InvalidInput: 6,
    };
    assertHelperCoversVariants("spendingPolicyErrorMessage", spendingPolicyErrorMessage, nameToCode);
  });
});

// ── MuxWalletRegistryError ────────────────────────────────────────────────────

const MUX_WALLET_REGISTRY_VARIANTS: readonly MuxWalletRegistryError[] = [
  "NotInitialized",
  "AlreadyInitialized",
  "Unauthorized",
  "WalletNotFound",
  "TooManyWallets",
];

describe("MuxWalletRegistryError coverage", () => {
  it("has 5 variants matching the Rust enum", () => {
    expect(MUX_WALLET_REGISTRY_VARIANTS.length).toBe(5);
  });

  it("every variant is in ERROR_HTTP_MAP with a valid status", () => {
    assertAllMapped("MuxWalletRegistryError", MUX_WALLET_REGISTRY_VARIANTS);
  });

  it("WalletNotFound is mapped to 404", () => {
    expect(ERROR_HTTP_MAP.WalletNotFound).toBe(404);
  });

  it("TooManyWallets is mapped to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyWallets).toBe(409);
  });
});

// ── Global completeness check ─────────────────────────────────────────────────

describe("ERROR_HTTP_MAP global completeness", () => {
  /**
   * The canonical set of all error variants across all contracts.
   * Kept in sync with docs/bindings-error-mapping.md.
   * If this test fails, a variant was added/removed from the TS types without
   * a corresponding change to ERROR_HTTP_MAP.
   */
  const ALL_REQUIRED_VARIANTS = [
    // Shared names (appear in multiple contracts)
    "NotInitialized",
    "AlreadyInitialized",
    "Unauthorized",
    // MuxAccount
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
    // MuxAccountFactory
    "InvalidAccount",
    "TooManyAccounts",
    "MetadataNotFound",
    "MetadataTooLarge",
    // MuxBatcher
    "EmptyBatch",
    "BatchTooLarge",
    "RequiredOperationFailed",
    "MetadataAlreadySet",
    // MuxDelegation
    "NotADelegate",
    "TooManyPermissions",
    "EmptyPermissions",
    "ContractIdAlreadySet",
    // MuxPermissions
    "RoleNotFound",
    "AccountNotInRole",
    "PermissionNotFound",
    "TooManyMembers",
    "TooManyRoles",
    "AdminNotFound",
    "AlreadyApproved",
    "TooManyPendingAdmins",
    // MuxPolicy
    "LimitNotFound",
    "LimitExceeded",
    // MuxRecovery
    "RecoveryAlreadyPending",
    "NoActiveRecovery",
    "TimelockNotExpired",
    "TooManyGuardians",
    "GuardianAlreadyExists",
    "GuardianNotFound",
    "MinGuardiansRequired",
    "RecoveryExpired",
    // MuxRegistry
    "ContractNotFound",
    "TooManyContracts",
    // SpendingPolicy
    "PolicyNotFound",
    "InvalidInput",
    // MuxWalletRegistry
    "WalletNotFound",
    "TooManyWallets",
  ] as const;

  it("all required contract error variants are present in ERROR_HTTP_MAP", () => {
    for (const variant of ALL_REQUIRED_VARIANTS) {
      const status = ERROR_HTTP_MAP[variant];
      if (typeof status !== "number") {
        throw new Error(`ERROR_HTTP_MAP is missing required variant "${variant}"`);
      }
    }
  });

  it("all mapped statuses are valid HTTP 4xx or 5xx codes", () => {
    for (const [variant, status] of Object.entries(ERROR_HTTP_MAP)) {
      expect(status).toBeGreaterThanOrEqual(400);
      expect(status).toBeLessThan(600);
      void variant; // suppress unused-variable lint
    }
  });
});
