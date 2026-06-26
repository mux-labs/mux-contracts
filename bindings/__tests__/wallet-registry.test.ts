/**
 * Tests for the mux-wallet-registry bindings export.
 *
 * Covers: client shape, error type exports, address config wiring,
 * environment-variable override, and HTTP error mapping.
 */

import { MuxWalletRegistryClient } from "../src/generated/mux-wallet-registry";
import type { WalletRegistryError } from "../src/generated/mux-wallet-registry";
import { MuxWalletRegistryClient as MuxWalletRegistryClientFromIndex } from "../src/index";
import { ERROR_HTTP_MAP } from "../src/errors";
import { DEFAULT_ADDRESSES } from "../src/addresses-config";
import { loadContractAddresses, validateAddresses } from "../src/addresses";

// ── Client shape ──────────────────────────────────────────────────────────────

describe("MuxWalletRegistryClient shape", () => {
  it("exposes initialize as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.initialize).toBe("function");
  });

  it("exposes registerWallet as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.registerWallet).toBe("function");
  });

  it("exposes getWallet as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.getWallet).toBe("function");
  });

  it("constructs with required options", () => {
    // Stellar contract IDs are 56-character C-type StrKeys.
    const validContractId = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
    expect(
      () =>
        new MuxWalletRegistryClient({
          contractId: validContractId,
          networkPassphrase: "Test SDF Network ; September 2015",
          rpcUrl: "https://soroban-testnet.stellar.org",
        })
    ).not.toThrow();
  });
});

// ── WalletRegistryError type ──────────────────────────────────────────────────

describe("WalletRegistryError type coverage", () => {
  const allVariants: WalletRegistryError[] = [
    "NotInitialized",
    "AlreadyInitialized",
    "Unauthorized",
    "WalletNotFound",
    "TooManyWallets",
  ];

  it("all error variants are valid WalletRegistryError values", () => {
    allVariants.forEach((v) => {
      expect(typeof v).toBe("string");
    });
  });

  it("maps every variant to an HTTP status code", () => {
    allVariants.forEach((variant) => {
      const code = ERROR_HTTP_MAP[variant];
      expect(code).toBeGreaterThanOrEqual(400);
      expect(code).toBeLessThan(600);
    });
  });
});

// ── HTTP error mapping ────────────────────────────────────────────────────────

describe("Wallet registry error HTTP mapping", () => {
  it("maps WalletNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.WalletNotFound).toBe(404);
  });

  it("maps TooManyWallets to 409", () => {
    expect(ERROR_HTTP_MAP.TooManyWallets).toBe(409);
  });

  it("maps NotInitialized to 500", () => {
    expect(ERROR_HTTP_MAP.NotInitialized).toBe(500);
  });

  it("maps AlreadyInitialized to 409", () => {
    expect(ERROR_HTTP_MAP.AlreadyInitialized).toBe(409);
  });

  it("maps Unauthorized to 401", () => {
    expect(ERROR_HTTP_MAP.Unauthorized).toBe(401);
  });
});

// ── Address config ────────────────────────────────────────────────────────────

describe("muxWalletRegistry address configuration", () => {
  it("DEFAULT_ADDRESSES includes muxWalletRegistry for every network", () => {
    (["localnet", "testnet", "mainnet"] as const).forEach((network) => {
      expect(DEFAULT_ADDRESSES[network]).toHaveProperty("muxWalletRegistry");
    });
  });

  it("loadContractAddresses returns muxWalletRegistry field", () => {
    const addresses = loadContractAddresses("localnet", DEFAULT_ADDRESSES);
    expect(addresses).toHaveProperty("muxWalletRegistry");
    expect(typeof addresses.muxWalletRegistry).toBe("string");
  });

  it("loads muxWalletRegistry from environment variable", () => {
    const testId = "CWALLET_TEST";
    process.env.LOCALNET_MUX_WALLET_REGISTRY_ID = testId;

    const addresses = loadContractAddresses("localnet", DEFAULT_ADDRESSES);
    expect(addresses.muxWalletRegistry).toBe(testId);

    delete process.env.LOCALNET_MUX_WALLET_REGISTRY_ID;
  });

  it("validateAddresses reports muxWalletRegistry when missing", () => {
    const addresses = {
      muxAccount: "CA",
      muxBatcher: "CB",
      muxDelegation: "CD",
      muxPermissions: "CP",
      muxWalletRegistry: "",
    };

    expect(() => validateAddresses("testnet", addresses)).toThrow("muxWalletRegistry");
  });

  it("validateAddresses passes when muxWalletRegistry is present", () => {
    const addresses = {
      muxAccount: "CA",
      muxBatcher: "CB",
      muxDelegation: "CD",
      muxPermissions: "CP",
      muxWalletRegistry: "CW",
    };

    expect(() => validateAddresses("testnet", addresses)).not.toThrow();
  });
});

// ── Index re-export ───────────────────────────────────────────────────────────

describe("index re-export", () => {
  it("MuxWalletRegistryClient re-exported from index is the same class", () => {
    expect(MuxWalletRegistryClientFromIndex).toBe(MuxWalletRegistryClient);
  });
});
