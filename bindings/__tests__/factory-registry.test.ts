/**
 * Unit tests for MuxAccountFactoryClient and MuxRegistryClient binding shapes.
 */

import {
  MuxAccountFactoryClient,
  FACTORY_MAX_VERSION_LENGTH,
  FACTORY_MAX_DESCRIPTION_LENGTH,
  FACTORY_MAX_AUTHOR_LENGTH,
  validateFactoryMetadata,
} from "../src/generated/mux-account-factory";
import { MuxRegistryClient } from "../src/generated/mux-registry";
import { ERROR_HTTP_MAP } from "../src/errors";

describe("MuxAccountFactoryClient shape", () => {
  it("exposes deployAccount as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.deployAccount).toBe("function");
  });

  it("supports simulateOnly as an optional deployAccount argument", () => {
    expect(MuxAccountFactoryClient.prototype.deployAccount.length).toBeGreaterThanOrEqual(4);
  });

  it("exposes deployAccountWithMetadata as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.deployAccountWithMetadata).toBe("function");
  });

  it("exposes getAccounts as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.getAccounts).toBe("function");
  });

  it("exposes accountCount as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.accountCount).toBe("function");
  });

  it("exposes getAccountMetadata as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.getAccountMetadata).toBe("function");
  });

  it("exposes simulateDeploy as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.simulateDeploy).toBe("function");
  });

  it("exposes simulateDeployWithMetadata as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.simulateDeployWithMetadata).toBe("function");
  });

  it("exposes maxAccountsPerOwner as a function", () => {
    expect(typeof MuxAccountFactoryClient.prototype.maxAccountsPerOwner).toBe("function");
  });

  it("simulateDeploy accepts owner and accountAddress arguments", () => {
    // 3 args: sourceKeypair, owner, accountAddress
    expect(MuxAccountFactoryClient.prototype.simulateDeploy.length).toBeGreaterThanOrEqual(3);
  });

  it("simulateDeployWithMetadata accepts owner, accountAddress, version, description, author", () => {
    // 6 args: sourceKeypair, owner, accountAddress, version, description, author
    expect(MuxAccountFactoryClient.prototype.simulateDeployWithMetadata.length).toBeGreaterThanOrEqual(6);
  });

  it("maxAccountsPerOwner accepts a keypair argument", () => {
    expect(MuxAccountFactoryClient.prototype.maxAccountsPerOwner.length).toBeGreaterThanOrEqual(1);
  });
});

describe("MuxRegistryClient shape", () => {
  it("exposes initialize as a function", () => {
    expect(typeof MuxRegistryClient.prototype.initialize).toBe("function");
  });

  it("exposes register as a function", () => {
    expect(typeof MuxRegistryClient.prototype.register).toBe("function");
  });

  it("exposes registerWithMetadata as a function", () => {
    expect(typeof MuxRegistryClient.prototype.registerWithMetadata).toBe("function");
  });

  it("exposes getVersion as a function", () => {
    expect(typeof MuxRegistryClient.prototype.getVersion).toBe("function");
  });

  it("exposes registerWithMetadata as a function", () => {
    expect(typeof MuxRegistryClient.prototype.registerWithMetadata).toBe("function");
  });

  it("exposes getMetadata as a function", () => {
    expect(typeof MuxRegistryClient.prototype.getMetadata).toBe("function");
  });

  it("exposes listContracts as a function", () => {
    expect(typeof MuxRegistryClient.prototype.listContracts).toBe("function");
  });
});

describe("Factory and registry error HTTP mapping", () => {
  it("maps InvalidAccount to 400", () => {
    expect(ERROR_HTTP_MAP.InvalidAccount).toBe(400);
  });

  it("maps TooManyAccounts to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyAccounts).toBe(409);
  });

  it("maps MetadataNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.MetadataNotFound).toBe(404);
  });

  it("maps MetadataTooLarge to 400", () => {
    expect(ERROR_HTTP_MAP.MetadataTooLarge).toBe(400);
  });

  it("maps ContractNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.ContractNotFound).toBe(404);
  });

  it("maps TooManyContracts to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyContracts).toBe(409);
  });
});

describe("MuxRegistryClient shape", () => {
  it("exposes initialize as a function", () => {
    expect(typeof MuxRegistryClient.prototype.initialize).toBe("function");
  });

  it("exposes register as a function", () => {
    expect(typeof MuxRegistryClient.prototype.register).toBe("function");
  });

  it("exposes registerWithMetadata as a function", () => {
    expect(typeof MuxRegistryClient.prototype.registerWithMetadata).toBe("function");
  });

  it("exposes getVersion as a function", () => {
    expect(typeof MuxRegistryClient.prototype.getVersion).toBe("function");
  });

  it("exposes registerWithMetadata as a function", () => {
    expect(typeof MuxRegistryClient.prototype.registerWithMetadata).toBe("function");
  });

  it("exposes getMetadata as a function", () => {
    expect(typeof MuxRegistryClient.prototype.getMetadata).toBe("function");
  });

  it("exposes listContracts as a function", () => {
    expect(typeof MuxRegistryClient.prototype.listContracts).toBe("function");
  });
});

describe("Factory and registry error HTTP mapping", () => {
  it("maps InvalidAccount to 400", () => {
    expect(ERROR_HTTP_MAP.InvalidAccount).toBe(400);
  });

  it("maps TooManyAccounts to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyAccounts).toBe(409);
  });

  it("maps MetadataNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.MetadataNotFound).toBe(404);
  });

  it("maps MetadataTooLarge to 400", () => {
    expect(ERROR_HTTP_MAP.MetadataTooLarge).toBe(400);
  });

  it("maps ContractNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.ContractNotFound).toBe(404);
  });

  it("maps TooManyContracts to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyContracts).toBe(409);
  });
});

describe("Factory metadata size limits (Issue #842)", () => {
  it("exports metadata limit constants matching storage griefing caps", () => {
    expect(FACTORY_MAX_VERSION_LENGTH).toBe(32);
    expect(FACTORY_MAX_DESCRIPTION_LENGTH).toBe(256);
    expect(FACTORY_MAX_AUTHOR_LENGTH).toBe(64);
  });

  it("accepts metadata strings exactly at size limits", () => {
    const version = "v".repeat(32);
    const description = "d".repeat(256);
    const author = "a".repeat(64);
    expect(() => validateFactoryMetadata(version, description, author)).not.toThrow();
  });

  it("rejects version string > 32 chars with MetadataTooLarge", () => {
    const version = "v".repeat(33);
    expect(() => validateFactoryMetadata(version, "desc", "author")).toThrow("MetadataTooLarge");
    expect(() => validateFactoryMetadata(version, "desc", "author")).toThrow("version length (33) exceeds maximum of 32");
  });

  it("rejects description string > 256 chars with MetadataTooLarge", () => {
    const description = "d".repeat(257);
    expect(() => validateFactoryMetadata("1.0.0", description, "author")).toThrow("MetadataTooLarge");
    expect(() => validateFactoryMetadata("1.0.0", description, "author")).toThrow("description length (257) exceeds maximum of 256");
  });

  it("rejects author string > 64 chars with MetadataTooLarge", () => {
    const author = "a".repeat(65);
    expect(() => validateFactoryMetadata("1.0.0", "desc", author)).toThrow("MetadataTooLarge");
    expect(() => validateFactoryMetadata("1.0.0", "desc", author)).toThrow("author length (65) exceeds maximum of 64");
  });

  it("client methods validate metadata before dispatching transactions", async () => {
    const dummyClient = new MuxAccountFactoryClient({
      contractId: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
      networkPassphrase: "Test SDF Network ; September 2015",
      rpcUrl: "http://localhost:8000/soroban/rpc",
    });
    const dummyKeypair = { publicKey: () => "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF" } as any;
    const dummyAddress = { toString: () => "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF" } as any;

    await expect(
      dummyClient.deployAccountWithMetadata(
        dummyKeypair,
        dummyAddress,
        dummyAddress,
        "v".repeat(33),
        "desc",
        "author"
      )
    ).rejects.toThrow("MetadataTooLarge");

    await expect(
      dummyClient.simulateDeployWithMetadata(
        dummyKeypair,
        dummyAddress,
        dummyAddress,
        "1.0.0",
        "d".repeat(257),
        "author"
      )
    ).rejects.toThrow("MetadataTooLarge");
  });
});
