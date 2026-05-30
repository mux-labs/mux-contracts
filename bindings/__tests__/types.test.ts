import { NETWORK_CONFIGS } from "../src/network";

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
});
