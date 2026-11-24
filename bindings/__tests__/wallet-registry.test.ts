/**
 * Tests for MuxWalletRegistryClient binding shape and integration stubs.
 */

import {
  MuxWalletRegistryClient,
  WALLET_NAME_REGEX,
  WALLET_NAME_MIN_LEN,
  WALLET_NAME_MAX_LEN,
  MAX_WALLETS,
  isValidWalletName,
  validateWalletName,
  assertWalletFound,
  assertWalletListWithinCap,
} from "../src/generated/mux-wallet-registry";
import { NETWORK_CONFIGS } from "../src/network";
import { ERROR_HTTP_MAP, WalletNotFoundError, TooManyWalletsError } from "../src/errors";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const config = NETWORK_CONFIGS[NETWORK];

async function isNetworkAvailable(): Promise<boolean> {
  try {
    const response = await globalThis.fetch(config.rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "getNetwork",
        params: [],
      }),
    });
    return response.ok;
  } catch {
    return false;
  }
}

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

  it("exposes registerWalletWithMetadata as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.registerWalletWithMetadata).toBe("function");
  });

  it("exposes getMetadata as a function", () => {
    expect(typeof MuxWalletRegistryClient.prototype.getMetadata).toBe("function");
  });
});

describe("Wallet registry error HTTP mapping", () => {
  it("maps WalletNotFound to 404", () => {
    expect(ERROR_HTTP_MAP.WalletNotFound).toBe(404);
  });

  it("maps TooManyWallets to 422", () => {
    expect(ERROR_HTTP_MAP.TooManyWallets).toBe(422);
  });

  it("maps Unauthorized to 401", () => {
    expect(ERROR_HTTP_MAP.Unauthorized).toBe(401);
  });

  it("maps AlreadyInitialized to 409", () => {
    expect(ERROR_HTTP_MAP.AlreadyInitialized).toBe(409);
  });

  it("maps NotInitialized to 500", () => {
    expect(ERROR_HTTP_MAP.NotInitialized).toBe(500);
  });
});

describe("Wallet registry MAX_WALLETS divergence", () => {
  it("client MAX_WALLETS matches the Rust contract constant", () => {
    const libPath = resolve(__dirname, "../../contracts/mux-wallet-registry/src/lib.rs");
    const source = readFileSync(libPath, "utf8");
    const match = source.match(/MAX_WALLETS\s*:\s*u32\s*=\s*(\d+)/);
    expect(match).not.toBeNull();
    expect(MAX_WALLETS).toBe(Number(match![1]));
  });
});

describe("Wallet registry read-result validation", () => {
  it("throws a typed not-found error when a lookup returns no entry", () => {
    expect(() => assertWalletFound("ghost", null)).toThrow(WalletNotFoundError);
    expect(() => assertWalletFound("ghost", undefined)).toThrow(WalletNotFoundError);
    expect(() => assertWalletFound("ghost", "")).toThrow(WalletNotFoundError);
    try {
      assertWalletFound("ghost", null);
    } catch (err) {
      expect((err as WalletNotFoundError).status).toBe(404);
    }
  });

  it("returns the address when a lookup finds an entry", () => {
    const addr = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    expect(assertWalletFound("treasury", addr)).toBe(addr);
  });

  it("accepts a name list of exactly MAX_WALLETS", () => {
    const names = Array.from({ length: MAX_WALLETS }, (_, i) => `w${i}`);
    expect(() => assertWalletListWithinCap(names)).not.toThrow();
    expect(assertWalletListWithinCap(names)).toHaveLength(MAX_WALLETS);
  });

  it("throws TooManyWallets for a list longer than MAX_WALLETS", () => {
    const names = Array.from({ length: MAX_WALLETS + 1 }, (_, i) => `w${i}`);
    expect(() => assertWalletListWithinCap(names)).toThrow(TooManyWalletsError);
    try {
      assertWalletListWithinCap(names);
    } catch (err) {
      expect((err as TooManyWalletsError).status).toBe(422);
    }
  });
});

describe("Wallet registry integration stubs", () => {
  beforeAll(async () => {
    const available = await isNetworkAvailable();
    if (!available) {
      console.warn(
        `⚠️  Network "${NETWORK}" is unavailable at ${config.rpcUrl}. ` +
        `Wallet registry integration tests require a running Soroban network.`
      );
    }
  });

  it("should have valid network configuration", () => {
    expect(config).toBeDefined();
    expect(config.rpcUrl).toBeTruthy();
    expect(config.networkPassphrase).toBeTruthy();
  });

  it("can attempt connection to configured network", async () => {
    const available = await isNetworkAvailable();
    expect(typeof available).toBe("boolean");
    if (!available) {
      console.log(
        `ℹ️  Network ${NETWORK} at ${config.rpcUrl} not available. ` +
        `Set SOROBAN_NETWORK=testnet and deploy mux-wallet-registry to run live tests.`
      );
    }
  });

  it.todo("initialize registry and verify owner is set");
  it.todo("register a wallet and retrieve it by name");
  it.todo("register wallet with metadata and verify label and description");
  it.todo("update an existing wallet entry and confirm address changes");
  it.todo("get_wallet returns WalletNotFound for unknown name");
  it.todo("get_metadata returns WalletNotFound for entry with no metadata");
  it.todo("non-owner register_wallet call is rejected with Unauthorized");
  it.todo("double initialize returns AlreadyInitialized");
});

describe("Wallet registry name charset policy", () => {
  it("exports charset constants and validation helpers", () => {
    expect(WALLET_NAME_REGEX).toBeInstanceOf(RegExp);
    expect(WALLET_NAME_MIN_LEN).toBe(1);
    expect(WALLET_NAME_MAX_LEN).toBe(32);
    expect(typeof isValidWalletName).toBe("function");
    expect(typeof validateWalletName).toBe("function");
  });

  it("accepts valid names conforming to [a-zA-Z0-9_]{1,32}", () => {
    const validNames = [
      "a",
      "Z",
      "9",
      "_",
      "treasury",
      "hot_wallet",
      "ops_01_backup",
      "a".repeat(32), // boundary: exactly 32 chars
    ];
    for (const name of validNames) {
      expect(isValidWalletName(name)).toBe(true);
      expect(() => validateWalletName(name)).not.toThrow();
    }
  });

  it("rejects empty string (below min length boundary)", () => {
    expect(isValidWalletName("")).toBe(false);
    expect(() => validateWalletName("")).toThrow("Invalid wallet name");
  });

  it("rejects names exceeding 32 characters (above max length boundary)", () => {
    const tooLong = "a".repeat(33);
    expect(isValidWalletName(tooLong)).toBe(false);
    expect(() => validateWalletName(tooLong)).toThrow("between 1 and 32 characters");
  });

  it("rejects names with spaces or whitespace", () => {
    const invalidWithSpaces = ["treasury 1", " treasury", "treasury ", "hot\twallet"];
    for (const name of invalidWithSpaces) {
      expect(isValidWalletName(name)).toBe(false);
      expect(() => validateWalletName(name)).toThrow("alphanumeric characters and underscores");
    }
  });

  it("rejects names with hyphens or dashes", () => {
    expect(isValidWalletName("treasury-01")).toBe(false);
    expect(() => validateWalletName("treasury-01")).toThrow("alphanumeric characters and underscores");
  });

  it("rejects names with punctuation or special characters", () => {
    const invalidSpecial = ["treasury.eth", "user@mux", "wallet#1", "vault/primary", "admin:key"];
    for (const name of invalidSpecial) {
      expect(isValidWalletName(name)).toBe(false);
      expect(() => validateWalletName(name)).toThrow("alphanumeric characters and underscores");
    }
  });

  it("rejects names with emojis or non-ASCII characters", () => {
    const invalidUnicode = ["wallet🚀", "trésor", "ウォレット"];
    for (const name of invalidUnicode) {
      expect(isValidWalletName(name)).toBe(false);
      expect(() => validateWalletName(name)).toThrow("alphanumeric characters and underscores");
    }
  });

  it("client methods validate name charset before making RPC requests", async () => {
    const client = new MuxWalletRegistryClient({
      contractId: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
      networkPassphrase: "Test SDF Network ; September 2015",
      rpcUrl: "http://localhost:8000/soroban/rpc",
    });
    const keypair = { publicKey: () => "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF" } as any;

    await expect(client.getWallet(keypair, "invalid-name")).rejects.toThrow(
      "Invalid wallet name"
    );
    await expect(client.registerWallet(keypair, "bad name", "GAAA" as any)).rejects.toThrow(
      "Invalid wallet name"
    );
  });
});

describe("Wallet registry named register/lookup authz", () => {
  const client = new MuxWalletRegistryClient({
    contractId: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "http://localhost:8000/soroban/rpc",
  });

  const owner = {
    publicKey: () => "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
  } as any;
  const delegate = {
    publicKey: () => "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
  } as any;
  const stranger = {
    publicKey: () => "GCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
  } as any;

  it("denies register when no signer is supplied (deny-by-default)", async () => {
    await expect(
      client.registerWallet(undefined as any, "treasury", "GAAA" as any)
    ).rejects.toThrow(/Unauthorized|signer/i);
  });

  it("denies register when signer is not the owner or an authorized delegate", async () => {
    await expect(
      client.registerWallet(stranger, "treasury", "GAAA" as any)
    ).rejects.toThrow(/Unauthorized|not authorized/i);
  });

  it("allows register when signer is the owner", async () => {
    await expect(
      client.registerWallet(owner, "treasury", "GAAA" as any)
    ).resolves.toBeDefined();
  });

  it("allows register when signer is an authorized delegate", async () => {
    await expect(
      client.registerWallet(delegate, "treasury", "GAAA" as any)
    ).resolves.toBeDefined();
  });
});
