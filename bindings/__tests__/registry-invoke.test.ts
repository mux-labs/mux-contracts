/**
 * Smoke-tests for the registry-invoke example module.
 *
 * Verifies that all exported helpers are importable and have the expected
 * shape without connecting to a live network.
 */

import * as registryInvoke from "../src/examples/registry-invoke";

describe("registry-invoke example module", () => {
  it("exports registerContractVersion as a function", () => {
    expect(typeof registryInvoke.registerContractVersion).toBe("function");
  });

  it("exports getContractVersion as a function", () => {
    expect(typeof registryInvoke.getContractVersion).toBe("function");
  });

  it("exports listRegisteredContracts as a function", () => {
    expect(typeof registryInvoke.listRegisteredContracts).toBe("function");
  });
});
