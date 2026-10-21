import { xdr } from "@stellar/stellar-sdk";
import {
  NETWORK_CONFIGS,
  assertSameNetwork,
  getActiveNetwork,
  getNetworkConfig,
  resolveNetworkConfig,
} from "../src/network";
import { MuxError, MuxErrorCode } from "../src/errors";
import type { MuxContractIds } from "../src/types";
import { DEFAULT_ADDRESSES } from "../src/addresses-config";

// Type-level assertion: MuxContractIds and DEFAULT_ADDRESSES values
// must share exactly the same set of keys so the two public types
// cannot drift.
type Equals<A, B> =
  (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2)
    ? true
    : false;
type AddressKeys = keyof (typeof DEFAULT_ADDRESSES)["localnet"];
type ContractIdsKeys = keyof MuxContractIds;
type _AssertKeysMatch = Equals<AddressKeys, ContractIdsKeys>;
// If this line fails to compile, MuxContractIds and DEFAULT_ADDRESSES
// have diverged — add or remove the mismatched key in both places.
const _keysMatch: _AssertKeysMatch = true;
import { MuxSpendingPolicyClient, MuxWalletRegistryClient } from "../src/index";
import type { BatchOperationKind, Operation } from "../src/types";
import { muxAccountErrorMessage } from "../src/types";

describe("NETWORK_CONFIGS", () => {
  it("defines localnet config", () => {
    expect(NETWORK_CONFIGS.localnet).toBeDefined();
    expect(NETWORK_CONFIGS.localnet.rpcUrl).toBeTruthy();
  });

  it("defines testnet config", () => {
    expect(NETWORK_CONFIGS.testnet).toBeDefined();
    expect(NETWORK_CONFIGS.testnet.rpcUrl).toContain("testnet");
  });

  it("defines mainnet config", () => {
    expect(NETWORK_CONFIGS.mainnet).toBeDefined();
    expect(NETWORK_CONFIGS.mainnet.rpcUrl).toContain("mainnet");
  });

  it("testnet and mainnet have different passphrases", () => {
    expect(NETWORK_CONFIGS.testnet.networkPassphrase).not.toEqual(
      NETWORK_CONFIGS.mainnet.networkPassphrase
    );
  });

  it("contract IDs are strings", () => {
    const { contracts } = NETWORK_CONFIGS.testnet;
    expect(typeof contracts.muxAccount).toBe("string");
    expect(typeof contracts.muxBatcher).toBe("string");
    expect(typeof contracts.muxPermissions).toBe("string");
  });

  it("all networks have RPC URLs", () => {
    Object.values(NETWORK_CONFIGS).forEach((config) => {
      expect(config.rpcUrl).toBeTruthy();
    });
  });

  it("all networks have network passphrases", () => {
    Object.values(NETWORK_CONFIGS).forEach((config) => {
      expect(config.networkPassphrase).toBeTruthy();
    });
  });

  it("MuxContractIds keys match DEFAULT_ADDRESSES keys", () => {
    const defaultKeys = Object.keys(DEFAULT_ADDRESSES.localnet).sort();
    const contractIdKeys: Array<keyof MuxContractIds> = [
      "muxAccount",
      "muxBatcher",
      "muxPermissions",
    ];
    const sortedKeys = [...contractIdKeys].sort();
    expect(defaultKeys).toEqual(sortedKeys);
  });
});

describe("Network selection", () => {
  it("getActiveNetwork returns default network", () => {
    const originalEnv = process.env.SOROBAN_NETWORK;
    delete process.env.SOROBAN_NETWORK;

    const network = getActiveNetwork();
    expect(network).toBe("localnet");

    if (originalEnv) {
      process.env.SOROBAN_NETWORK = originalEnv;
    }
  });

  it("getActiveNetwork respects SOROBAN_NETWORK env var", () => {
    const originalEnv = process.env.SOROBAN_NETWORK;
    process.env.SOROBAN_NETWORK = "testnet";

    const network = getActiveNetwork();
    expect(network).toBe("testnet");

    if (originalEnv) {
      process.env.SOROBAN_NETWORK = originalEnv;
    } else {
      delete process.env.SOROBAN_NETWORK;
    }
  });

  it("getNetworkConfig returns config for active network", () => {
    const originalEnv = process.env.SOROBAN_NETWORK;
    process.env.SOROBAN_NETWORK = "mainnet";

    const config = getNetworkConfig();
    expect(config.rpcUrl).toContain("mainnet");

    if (originalEnv) {
      process.env.SOROBAN_NETWORK = originalEnv;
    } else {
      delete process.env.SOROBAN_NETWORK;
    }
  });

  it("getNetworkConfig throws for unknown network", () => {
    const originalEnv = process.env.SOROBAN_NETWORK;
    process.env.SOROBAN_NETWORK = "unknown";

    expect(() => {
      getNetworkConfig();
    }).toThrow("not found");

    if (originalEnv) {
      process.env.SOROBAN_NETWORK = originalEnv;
    } else {
      delete process.env.SOROBAN_NETWORK;
    }
  });

  /**
   * Regression suite for #984: additional coverage for the env-driven
   * network-selection surface in `../src/network` that this file did not
   * previously exercise — an empty-string env var, `resolveNetworkConfig`'s
   * fail-closed (never-defaults) behavior, and `assertSameNetwork`'s
   * cross-network guard.
   */
  it("getActiveNetwork treats an empty-string SOROBAN_NETWORK as falsy and falls back to localnet", () => {
    const originalEnv = process.env.SOROBAN_NETWORK;
    process.env.SOROBAN_NETWORK = "";

    expect(getActiveNetwork()).toBe("localnet");

    if (originalEnv) {
      process.env.SOROBAN_NETWORK = originalEnv;
    } else {
      delete process.env.SOROBAN_NETWORK;
    }
  });

  describe("resolveNetworkConfig", () => {
    it("resolves a known network's config", () => {
      const config = resolveNetworkConfig("testnet");
      expect(config.rpcUrl).toContain("testnet");
    });

    it("throws MuxError(NETWORK_NOT_CONFIGURED) for undefined/null/empty input, never defaulting", () => {
      for (const value of [undefined, null, ""] as const) {
        let caught: unknown;
        try {
          resolveNetworkConfig(value);
        } catch (err) {
          caught = err;
        }
        expect(caught).toBeInstanceOf(MuxError);
        expect((caught as MuxError).code).toBe(MuxErrorCode.NETWORK_NOT_CONFIGURED);
      }
    });

    it("throws MuxError(NETWORK_UNKNOWN) for an unrecognized network name", () => {
      let caught: unknown;
      try {
        resolveNetworkConfig("not-a-real-network");
      } catch (err) {
        caught = err;
      }
      expect(caught).toBeInstanceOf(MuxError);
      expect((caught as MuxError).code).toBe(MuxErrorCode.NETWORK_UNKNOWN);
    });
  });

  describe("assertSameNetwork", () => {
    it("returns the target config when target and configured networks match", () => {
      const config = assertSameNetwork("testnet", "testnet");
      expect(config.rpcUrl).toContain("testnet");
    });

    it("throws MuxError(CROSS_NETWORK_BLOCKED) when target and configured networks differ", () => {
      let caught: unknown;
      try {
        assertSameNetwork("testnet", "mainnet");
      } catch (err) {
        caught = err;
      }
      expect(caught).toBeInstanceOf(MuxError);
      expect((caught as MuxError).code).toBe(MuxErrorCode.CROSS_NETWORK_BLOCKED);
    });

    it("defaults configuredNetwork to the active (env-selected) network", () => {
      const originalEnv = process.env.SOROBAN_NETWORK;
      process.env.SOROBAN_NETWORK = "mainnet";

      expect(() => assertSameNetwork("mainnet")).not.toThrow();
      let caught: unknown;
      try {
        assertSameNetwork("testnet");
      } catch (err) {
        caught = err;
      }
      expect(caught).toBeInstanceOf(MuxError);
      expect((caught as MuxError).code).toBe(MuxErrorCode.CROSS_NETWORK_BLOCKED);

      if (originalEnv) {
        process.env.SOROBAN_NETWORK = originalEnv;
      } else {
        delete process.env.SOROBAN_NETWORK;
      }
    });

    it("fails closed (does not default) when the configured network is unset and no override is given", () => {
      let caught: unknown;
      try {
        assertSameNetwork("testnet", undefined);
      } catch (err) {
        caught = err;
      }
      expect(caught).toBeInstanceOf(MuxError);
      expect((caught as MuxError).code).toBe(MuxErrorCode.NETWORK_NOT_CONFIGURED);
    });
  });
});

describe("muxAccountErrorMessage", () => {
  it("resolves variant names to descriptions", () => {
    expect(muxAccountErrorMessage("DelegateNotFound")).toBe("delegate not found");
    expect(muxAccountErrorMessage("TooManyDelegates")).toBe("too many delegates");
    expect(muxAccountErrorMessage("SpendLimitExceeded")).toBe("spend limit exceeded");
  });

  it("resolves numeric codes to descriptions", () => {
    expect(muxAccountErrorMessage(4)).toBe("delegate not found");
    expect(muxAccountErrorMessage(9)).toBe("too many delegates");
    expect(muxAccountErrorMessage(11)).toBe("arithmetic overflow");
  });

  it("returns unknown for unmapped codes", () => {
    expect(muxAccountErrorMessage(99)).toBe("unknown error code");
  });
});

describe("BatchOperationKind", () => {
  const validKinds: BatchOperationKind[] = ["Invoke", "Transfer", "Approve"];

  it("has exactly three variants", () => {
    expect(validKinds).toHaveLength(3);
  });

  it("all variants are distinct strings", () => {
    const unique = new Set(validKinds);
    expect(unique.size).toBe(3);
  });

  it("Operation accepts each kind variant", () => {
    const { Address } = require("@stellar/stellar-sdk");
    const addr = Address.fromString(
      "GATPLJWD4WKPGXT5FVVHO6RXYIBUE6RUHBOBGLAWVE4WDMTBX23EL54Q"
    );
    validKinds.forEach((kind) => {
      const op: Operation = {
        target: addr,
        fnName: "transfer",
        args: [] as xdr.ScVal[],
        requireSuccess: true,
        kind,
      };
      expect(op.kind).toBe(kind);
    });
  });
});

describe("MuxSpendingPolicyClient export", () => {
  it("is exported from the package index", () => {
    expect(typeof MuxSpendingPolicyClient).toBe("function");
  });

  it("exposes checkSpend method", () => {
    expect(typeof MuxSpendingPolicyClient.prototype.checkSpend).toBe("function");
  });

  it("exposes setPolicy and getPolicy methods", () => {
    expect(typeof MuxSpendingPolicyClient.prototype.setPolicy).toBe("function");
    expect(typeof MuxSpendingPolicyClient.prototype.getPolicy).toBe("function");
  });
});

describe("MuxWalletRegistryClient export", () => {
  it("is exported from the package index", () => {
    expect(typeof MuxWalletRegistryClient).toBe("function");
  });

  it("exposes registerWallet and getWallet methods", () => {
    expect(typeof MuxWalletRegistryClient.prototype.registerWallet).toBe("function");
    expect(typeof MuxWalletRegistryClient.prototype.getWallet).toBe("function");
  });
});
