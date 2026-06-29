/**
 * Unit tests for MuxAccountFactoryClient and MuxRegistryClient binding shapes.
 */

import { MuxAccountFactoryClient } from "../src/generated/mux-account-factory";
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

  it("maps ContractNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.ContractNotFound).toBe(404);
  });

  it("maps TooManyContracts to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyContracts).toBe(409);
  });
});
