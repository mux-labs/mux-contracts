import {
  contractErrorToHttp,
  ERROR_HTTP_MAP,
  isContractError,
} from "../src/errors";

describe("Error Mapping", () => {
  describe("ERROR_HTTP_MAP", () => {
    it("maps Unauthorized to 401", () => {
      expect(ERROR_HTTP_MAP.Unauthorized).toBe(401);
    });

    it("maps not-found errors to 404", () => {
      expect(ERROR_HTTP_MAP.DelegateNotFound).toBe(404);
      expect(ERROR_HTTP_MAP.RoleNotFound).toBe(404);
      expect(ERROR_HTTP_MAP.PermissionNotFound).toBe(404);
    });

    it("maps validation errors to 400", () => {
      expect(ERROR_HTTP_MAP.InvalidAmount).toBe(400);
      expect(ERROR_HTTP_MAP.InvalidPeriod).toBe(400);
      expect(ERROR_HTTP_MAP.SpendLimitExceeded).toBe(400);
      expect(ERROR_HTTP_MAP.EmptyBatch).toBe(400);
    });

    it("maps AlreadyInitialized to 409", () => {
      expect(ERROR_HTTP_MAP.AlreadyInitialized).toBe(409);
    });

    it("maps security guard errors to 409", () => {
      expect(ERROR_HTTP_MAP.ReentrancyDetected).toBe(409);
    });

    it("maps TooManyDelegates to 409", () => {
      expect(ERROR_HTTP_MAP.TooManyDelegates).toBe(409);
    });

    it("maps MetadataNotFound to 404", () => {
      expect(ERROR_HTTP_MAP.MetadataNotFound).toBe(404);
    });

    it("maps arithmetic overflow to 500", () => {
      expect(ERROR_HTTP_MAP.ArithmeticOverflow).toBe(500);
    });

    it("maps initialization errors to 500", () => {
      expect(ERROR_HTTP_MAP.NotInitialized).toBe(500);
    });

    it("maps WalletNotFound to 404", () => {
      expect(ERROR_HTTP_MAP.WalletNotFound).toBe(404);
    });
  });

  describe("contractErrorToHttp", () => {
    it("converts known errors to appropriate HTTP status codes", () => {
      const unauthorizedError = contractErrorToHttp("Unauthorized");
      expect(unauthorizedError.statusCode).toBe(401);
      expect(unauthorizedError.errorType).toBe("Unauthorized");

      const notFoundError = contractErrorToHttp("DelegateNotFound");
      expect(notFoundError.statusCode).toBe(404);

      const badRequestError = contractErrorToHttp("InvalidAmount");
      expect(badRequestError.statusCode).toBe(400);
    });

    it("returns 500 for unknown errors", () => {
      const unknownError = contractErrorToHttp("UnknownError");
      expect(unknownError.statusCode).toBe(500);
      expect(unknownError.errorType).toBe("UnknownError");
    });

    it("includes error type in response", () => {
      const error = contractErrorToHttp("Unauthorized");
      expect(error.message).toBe("Unauthorized");
      expect(error.errorType).toBe("Unauthorized");
    });

    it("handles all MuxAccountError variants", () => {
      const accountErrors: string[] = [
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
      ];

      accountErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });
    });

    it("handles all MuxBatcherError variants", () => {
      const batcherErrors: string[] = [
        "EmptyBatch",
        "BatchTooLarge",
        "RequiredOperationFailed",
        "Unauthorized",
        "ReentrancyDetected",
      ];

      batcherErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });
    });

    it("handles all MuxPermissionsError variants", () => {
      const permissionErrors: string[] = [
        "NotInitialized",
        "AlreadyInitialized",
        "Unauthorized",
        "RoleNotFound",
        "AccountNotInRole",
        "PermissionNotFound",
      ];

      permissionErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });
    });

    it("handles all MuxAccountFactoryError variants", () => {
      const factoryErrors: string[] = [
        "Unauthorized",
        "InvalidAccount",
        "TooManyAccounts",
        "MetadataNotFound",
      ];

      factoryErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });
    });

    it("handles all MuxRegistryError variants", () => {
      const registryErrors: string[] = [
        "NotInitialized",
        "AlreadyInitialized",
        "Unauthorized",
        "ContractNotFound",
        "TooManyContracts",
      ];

      registryErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });
    });

    it("handles all MuxWalletRegistryError variants", () => {
      const walletRegistryErrors: string[] = [
        "NotInitialized",
        "AlreadyInitialized",
        "Unauthorized",
        "WalletNotFound",
      ];

      walletRegistryErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });
    });

    it("handles all MuxPolicyError variants", () => {
      const policyErrors: string[] = [
        "NotInitialized",
        "AlreadyInitialized",
        "Unauthorized",
        "LimitNotFound",
        "LimitExceeded",
        "InvalidAmount",
        "InvalidPeriod",
        "TooManyWallets",
        "RegistryNotFound",
      ];

      policyErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });

      // Caller-side policy rejections must be 4xx, not 500.
      expect(ERROR_HTTP_MAP.LimitNotFound).toBeGreaterThanOrEqual(400);
      expect(ERROR_HTTP_MAP.LimitNotFound).toBeLessThan(500);
      expect(ERROR_HTTP_MAP.LimitExceeded).toBeGreaterThanOrEqual(400);
      expect(ERROR_HTTP_MAP.LimitExceeded).toBeLessThan(500);
      expect(ERROR_HTTP_MAP.RegistryNotFound).toBeGreaterThanOrEqual(400);
      expect(ERROR_HTTP_MAP.RegistryNotFound).toBeLessThan(500);
    });

    it("handles all MuxRecoveryError variants", () => {
      const recoveryErrors: string[] = [
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
        "QuorumNotReached",
        "DuplicateApproval",
        "InvalidQuorumThreshold",
        "RegistryNotFound",
      ];

      recoveryErrors.forEach((error) => {
        const response = contractErrorToHttp(error);
        expect(response.statusCode).toBeGreaterThanOrEqual(400);
        expect(response.statusCode).toBeLessThan(600);
      });

      // Timelock and quorum rejections are caller-side, must be 4xx.
      expect(ERROR_HTTP_MAP.TimelockNotExpired).toBeGreaterThanOrEqual(400);
      expect(ERROR_HTTP_MAP.TimelockNotExpired).toBeLessThan(500);
      expect(ERROR_HTTP_MAP.QuorumNotReached).toBeGreaterThanOrEqual(400);
      expect(ERROR_HTTP_MAP.QuorumNotReached).toBeLessThan(500);
    });
  });

  describe("isContractError", () => {
    it("identifies contract errors", () => {
      expect(isContractError("Unauthorized")).toBe(true);
      expect(isContractError("InvalidAmount")).toBe(true);
    });

    it("rejects non-string values", () => {
      expect(isContractError(123)).toBe(false);
      expect(isContractError(null)).toBe(false);
      expect(isContractError(undefined)).toBe(false);
      expect(isContractError({})).toBe(false);
    });

    it("treats any string as potential contract error", () => {
      // Conservative approach: any string could be a contract error
      expect(isContractError("SomeError")).toBe(true);
      expect(isContractError("")).toBe(true);
    });
  });

  describe("HTTP Error Response", () => {
    it("provides consistent response structure", () => {
      const response = contractErrorToHttp("Unauthorized");
      expect(response).toHaveProperty("statusCode");
      expect(response).toHaveProperty("message");
      expect(response).toHaveProperty("errorType");
      expect(typeof response.statusCode).toBe("number");
      expect(typeof response.message).toBe("string");
      expect(typeof response.errorType).toBe("string");
    });
  });
});
