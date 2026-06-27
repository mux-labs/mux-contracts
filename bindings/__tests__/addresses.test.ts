import {
  loadContractAddresses,
  validateAddresses,
  getValidatedAddresses,
} from "../src/addresses";
import { DEFAULT_ADDRESSES } from "../src/addresses-config";

describe("Contract Address Configuration", () => {
  describe("loadContractAddresses", () => {
    it("loads addresses for localnet from config", () => {
      const addresses = loadContractAddresses("localnet", DEFAULT_ADDRESSES);
      expect(addresses).toBeDefined();
      expect(addresses.muxAccount).toBeDefined();
      expect(addresses.muxBatcher).toBeDefined();
      expect(addresses.muxPermissions).toBeDefined();
    });

    it("loads addresses for testnet from config", () => {
      const addresses = loadContractAddresses("testnet", DEFAULT_ADDRESSES);
      expect(addresses).toBeDefined();
      expect(typeof addresses.muxAccount).toBe("string");
    });

    it("loads addresses for mainnet from config", () => {
      const addresses = loadContractAddresses("mainnet", DEFAULT_ADDRESSES);
      expect(addresses).toBeDefined();
      expect(typeof addresses.muxBatcher).toBe("string");
    });

    it("throws error for unknown network", () => {
      expect(() => {
        loadContractAddresses("unknown", DEFAULT_ADDRESSES);
      }).toThrow("not found in addresses config");
    });

    it("overrides config with environment variables", () => {
      const testAddress = "CABC123";
      process.env.LOCALNET_MUX_ACCOUNT_ID = testAddress;

      const addresses = loadContractAddresses("localnet", DEFAULT_ADDRESSES);
      expect(addresses.muxAccount).toBe(testAddress);

      delete process.env.LOCALNET_MUX_ACCOUNT_ID;
    });

    it("respects environment variable prefix based on network", () => {
      const testAddress = "CTEST123";
      process.env.TESTNET_MUX_BATCHER_ID = testAddress;

      const addresses = loadContractAddresses("testnet", DEFAULT_ADDRESSES);
      expect(addresses.muxBatcher).toBe(testAddress);

      delete process.env.TESTNET_MUX_BATCHER_ID;
    });

    it("overrides muxPolicy with environment variable", () => {
      const testAddress = "CPOLICY_ENV";
      process.env.LOCALNET_MUX_POLICY_ID = testAddress;

      const addresses = loadContractAddresses("localnet", DEFAULT_ADDRESSES);
      expect(addresses.muxPolicy).toBe(testAddress);

      delete process.env.LOCALNET_MUX_POLICY_ID;
    });

    it("loads muxPolicy for each network", () => {
      for (const network of ["localnet", "testnet", "mainnet"] as const) {
        const addresses = loadContractAddresses(network, DEFAULT_ADDRESSES);
        expect(addresses).toHaveProperty("muxPolicy");
        expect(typeof addresses.muxPolicy).toBe("string");
      }
    });
  });

  describe("validateAddresses", () => {
    it("passes when all addresses are present", () => {
      const addresses = {
        muxAccount: "CAcc1",
        muxBatcher: "CBatch1",
        muxDelegation: "CDel1",
        muxPermissions: "CPerms1",
        muxPolicy: "CPol1",
      };

      expect(() => {
        validateAddresses("testnet", addresses);
      }).not.toThrow();
    });

    it("throws when muxAccount is missing", () => {
      const addresses = {
        muxAccount: "",
        muxBatcher: "CBatch1",
        muxDelegation: "CDel1",
        muxPermissions: "CPerms1",
        muxPolicy: "CPol1",
      };

      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("Missing contract addresses");
      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("muxAccount");
    });

    it("throws when multiple addresses are missing", () => {
      const addresses = {
        muxAccount: "",
        muxBatcher: "",
        muxDelegation: "CDel1",
        muxPermissions: "CPerms1",
        muxPolicy: "CPol1",
      };

      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("muxAccount");
      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("muxBatcher");
    });

    it("provides helpful error message with environment variable names", () => {
      const addresses = {
        muxAccount: "",
        muxBatcher: "",
        muxDelegation: "",
        muxPermissions: "",
        muxPolicy: "",
      };

      expect(() => {
        validateAddresses("localnet", addresses);
      }).toThrow("LOCALNET_MUX");
    });
  });

  describe("getValidatedAddresses", () => {
    it("loads and validates addresses in one call", () => {
      const addresses = getValidatedAddresses("testnet", {
        localnet: {
          muxAccount: "CLoc1",
          muxBatcher: "CLoc2",
          muxDelegation: "CLoc3",
          muxPermissions: "CLoc4",
          muxPolicy: "CLoc5",
        },
        testnet: {
          muxAccount: "CTest1",
          muxBatcher: "CTest2",
          muxDelegation: "CTest3",
          muxPermissions: "CTest4",
          muxPolicy: "CTest5",
        },
        mainnet: {
          muxAccount: "",
          muxBatcher: "",
          muxDelegation: "",
          muxPermissions: "",
          muxPolicy: "",
        },
      });

      expect(addresses.muxAccount).toBe("CTest1");
      expect(addresses.muxBatcher).toBe("CTest2");
      expect(addresses.muxDelegation).toBe("CTest3");
      expect(addresses.muxPermissions).toBe("CTest4");
      expect(addresses.muxPolicy).toBe("CTest5");
    });

    it("throws if validation fails", () => {
      expect(() => {
        getValidatedAddresses("mainnet", DEFAULT_ADDRESSES);
      }).toThrow("Missing contract addresses");
    });
  });

  describe("DEFAULT_ADDRESSES structure", () => {
    it("has all required networks", () => {
      expect(DEFAULT_ADDRESSES).toHaveProperty("localnet");
      expect(DEFAULT_ADDRESSES).toHaveProperty("testnet");
      expect(DEFAULT_ADDRESSES).toHaveProperty("mainnet");
    });

    it("has contract IDs for each network", () => {
      Object.values(DEFAULT_ADDRESSES).forEach((networkAddresses) => {
        expect(networkAddresses).toHaveProperty("muxAccount");
        expect(networkAddresses).toHaveProperty("muxBatcher");
        expect(networkAddresses).toHaveProperty("muxDelegation");
        expect(networkAddresses).toHaveProperty("muxPermissions");
        expect(networkAddresses).toHaveProperty("muxPolicy");
      });
    });
  });
});
