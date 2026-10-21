import {
  loadContractAddresses,
  validateAddresses,
  getValidatedAddresses,
} from "../src/addresses";
import { DEFAULT_ADDRESSES } from "../src/addresses-config";
import { validateAddresses as validateGeneratorAddresses } from "../../scripts/generate-addresses";

describe("Contract Address Configuration", () => {
  // Mainnet address resolution is fail-closed: it requires the mainnet PR
  // review marker (`MAINNET_ADDRESS_REVIEW_APPROVED=true`). Approve it here
  // so tests that resolve mainnet addresses exercise the loader/validation
  // logic instead of being rejected by the review gate.
  beforeAll(() => {
    process.env.MAINNET_ADDRESS_REVIEW_APPROVED = "true";
  });

  afterAll(() => {
    delete process.env.MAINNET_ADDRESS_REVIEW_APPROVED;
  });

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
    // Well-formed strkey contract IDs (56 chars, leading "C") for fixtures
    // that must pass the format check.
    const wellFormed = {
      muxAccount: "C" + "A".repeat(55),
      muxBatcher: "C" + "B".repeat(55),
      muxDelegation: "C" + "D".repeat(55),
      muxPermissions: "C" + "E".repeat(55),
      muxWalletRegistry: "C" + "F".repeat(55),
    };

    it("passes when all addresses are present and well-formed", () => {
      expect(() => {
        validateAddresses("testnet", wellFormed);
      }).not.toThrow();
    });

    it("throws when muxAccount is missing", () => {
      const addresses = {
        ...wellFormed,
        muxAccount: "",
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
        ...wellFormed,
        muxAccount: "",
        muxBatcher: "",
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
        muxWalletRegistry: "",
      };

      expect(() => {
        validateAddresses("localnet", addresses);
      }).toThrow("LOCALNET_MUX");
    });

    it("throws when an address is present but not a well-formed contract ID (#993)", () => {
      const addresses = {
        ...wellFormed,
        muxAccount: "CAcc1", // too short, not a valid strkey contract ID
      };

      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("Malformed contract address");
      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("muxAccount");
    });

    it("throws when an address does not start with the contract prefix 'C' (#993)", () => {
      const addresses = {
        ...wellFormed,
        // Valid-length strkey but a "G" (account, not contract) prefix.
        muxBatcher: "G" + "B".repeat(55),
      };

      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("Malformed contract address");
      expect(() => {
        validateAddresses("testnet", addresses);
      }).toThrow("muxBatcher");
    });

    it("flags every malformed address when more than one is invalid (#993)", () => {
      const addresses = {
        ...wellFormed,
        muxAccount: "not-a-contract-id",
        muxPermissions: "short",
      };

      const fn = () => validateAddresses("testnet", addresses);
      expect(fn).toThrow("muxAccount");
      expect(fn).toThrow("muxPermissions");
    });
  });

  describe("getValidatedAddresses", () => {
    it("loads and validates addresses in one call", () => {
      const addresses = getValidatedAddresses("testnet", {
        localnet: {
          muxAccount: "C" + "1".repeat(55),
          muxBatcher: "C" + "2".repeat(55),
          muxDelegation: "C" + "3".repeat(55),
          muxPermissions: "C" + "4".repeat(55),
          muxWalletRegistry: "C" + "5".repeat(55),
        },
        testnet: {
          muxAccount: "C" + "6".repeat(55),
          muxBatcher: "C" + "7".repeat(55),
          muxDelegation: "C" + "A".repeat(55),
          muxPermissions: "C" + "B".repeat(55),
          muxWalletRegistry: "C" + "D".repeat(55),
          muxPolicy: "C" + "D".repeat(55),
        },
        mainnet: {
          muxAccount: "",
          muxBatcher: "",
          muxDelegation: "",
          muxPermissions: "",
          muxWalletRegistry: "",
        },
      });

      expect(addresses.muxAccount).toBe("C" + "6".repeat(55));
      expect(addresses.muxBatcher).toBe("C" + "7".repeat(55));
      expect(addresses.muxDelegation).toBe("C" + "A".repeat(55));
      expect(addresses.muxPermissions).toBe("C" + "B".repeat(55));
      expect(addresses.muxPolicy).toBe("C" + "D".repeat(55));
    });

    it("throws if validation fails", () => {
      // testnet is not mainnet, so the mainnet review gate is skipped and the
      // missing-address check runs. DEFAULT_ADDRESSES now carries non-empty
      // placeholders, so assert against an explicitly incomplete testnet book.
      const incomplete = {
        localnet: DEFAULT_ADDRESSES.localnet,
        testnet: {
          muxAccount: "",
          muxBatcher: "",
          muxDelegation: "",
          muxPermissions: "",
          muxWalletRegistry: "",
        },
        mainnet: DEFAULT_ADDRESSES.mainnet,
      };
      expect(() => {
        getValidatedAddresses("testnet", incomplete);
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
        expect(networkAddresses).toHaveProperty("muxWalletRegistry");
      });
    });
  });

  describe("scripts/generate-addresses fail-closed validation", () => {
    const fullBook = {
      muxAccount: "CAcc1",
      muxBatcher: "CB1",
      muxDelegation: "CD1",
      muxPermissions: "CP1",
      muxWalletRegistry: "CW1",
    };

    it("throws when a required network is missing", () => {
      const data: any = {
        localnet: fullBook,
        // testnet intentionally absent
        mainnet: fullBook,
      };
      expect(() => validateGeneratorAddresses(data)).toThrow("testnet");
    });

    it("throws when a network is present but not an object", () => {
      const data: any = {
        localnet: fullBook,
        testnet: null,
        mainnet: fullBook,
      };
      expect(() => validateGeneratorAddresses(data)).toThrow("testnet");
    });

    it("throws when a contract address is empty", () => {
      const data: any = {
        localnet: fullBook,
        testnet: { ...fullBook, muxAccount: "" },
        mainnet: fullBook,
      };
      const fn = () => validateGeneratorAddresses(data);
      expect(fn).toThrow("testnet");
      expect(fn).toThrow("muxAccount");
    });
  });
});
