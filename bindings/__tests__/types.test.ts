import { xdr } from "@stellar/stellar-sdk";
import {
  NETWORK_CONFIGS,
  getActiveNetwork,
  getNetworkConfig,
} from "../src/network";
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
