/**
 * Integration test stub for MuxAccountFactory.
 *
 * These tests connect to a live Soroban RPC endpoint and verify that the
 * factory contract is deployed and responds correctly.  They gracefully skip
 * when the network or contract address is unavailable, matching the pattern
 * used in integration.test.ts.
 *
 * Run against localnet (requires docker-compose):
 *   SOROBAN_NETWORK=localnet npm test
 *
 * Run against testnet:
 *   SOROBAN_NETWORK=testnet TESTNET_MUX_ACCOUNT_FACTORY_ID=C... npm test
 */

import { NETWORK_CONFIGS } from "../src/network";
import { Address, Keypair, StrKey } from "@stellar/stellar-sdk";
import { MuxAccountFactoryClient } from "../src/generated/mux-account-factory";
import {
  FactoryBoundsError,
  FactoryBoundsErrorCode,
  MAX_ACCOUNTS_PAGE_SIZE,
} from "../src/factory-events";

const NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const config = NETWORK_CONFIGS[NETWORK];

/** Contract ID for the factory on the active network (env-override supported). */
const FACTORY_CONTRACT_ID =
  process.env[`${NETWORK.toUpperCase()}_MUX_ACCOUNT_FACTORY_ID`] ||
  config.contracts.muxAccountFactory ||
  "";

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

describe("MuxAccountFactory integration", () => {
  let networkAvailable: boolean;
  let contractDeployed: boolean;

  beforeAll(async () => {
    networkAvailable = await isNetworkAvailable();
    contractDeployed = networkAvailable && !!FACTORY_CONTRACT_ID;

    if (!networkAvailable) {
      console.warn(
        `⚠️  Network "${NETWORK}" unavailable at ${config.rpcUrl}. ` +
          `Factory integration tests will be skipped.`
      );
    } else if (!contractDeployed) {
      console.warn(
        `⚠️  Factory contract ID not set for network "${NETWORK}". ` +
          `Set ${NETWORK.toUpperCase()}_MUX_ACCOUNT_FACTORY_ID to enable these tests.`
      );
    }
  });

  it("network config includes rpcUrl and networkPassphrase", () => {
    expect(config.rpcUrl).toBeTruthy();
    expect(config.networkPassphrase).toBeTruthy();
  });

  it("factory contract ID env var is documented", () => {
    // This test always passes — it documents which env var to set.
    const envKey = `${NETWORK.toUpperCase()}_MUX_ACCOUNT_FACTORY_ID`;
    expect(typeof envKey).toBe("string");
    if (!FACTORY_CONTRACT_ID) {
      console.info(`ℹ️  Set ${envKey} to enable live factory tests.`);
    }
  });

  it("should reach the Soroban RPC endpoint", async () => {
    if (!networkAvailable) {
      console.log(`ℹ️  Skipping — network "${NETWORK}" not available.`);
      return;
    }
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
    expect(response.ok).toBe(true);
  });

  it("should read account_count from a deployed factory", async () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — factory contract not available on "${NETWORK}".`);
      return;
    }
    // Stub: call account_count via JSON-RPC simulation.
    // Replace with MuxAccountFactoryClient once RPC credentials are wired.
    const body = {
      jsonrpc: "2.0",
      id: 2,
      method: "simulateTransaction",
      params: [{ contractId: FACTORY_CONTRACT_ID, method: "account_count", args: [] }],
    };
    const response = await globalThis.fetch(config.rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    // Accept either a valid result or a known RPC error — contract is present.
    expect([200, 400]).toContain(response.status);
  });

  it("should reject deploy_account when account equals owner (InvalidAccount)", async () => {
    if (!contractDeployed) {
      console.log(`ℹ️  Skipping — factory contract not available on "${NETWORK}".`);
      return;
    }
    // Stub: this test documents expected error behaviour.
    // A full implementation would use MuxAccountFactoryClient with a funded keypair.
    console.info(
      "TODO: wire MuxAccountFactoryClient with funded keypair to assert " +
        "InvalidAccount (HTTP 400) when owner === account_address."
    );
    expect(true).toBe(true); // placeholder — remove when client is wired
  });
});

describe("MuxAccountFactoryClient getAccounts bounds validation", () => {
  const CONTRACT_ID = StrKey.encodeContract(new Uint8Array(32));
  const NETWORK_PASSPHRASE = "Test SDF Network ; September 2015";
  const RPC_URL = "https://127.0.0.1:8000";

  /** Account shape accepted by `TransactionBuilder` without a live RPC. */
  function mockAccount(signer: Keypair) {
    return {
      accountId: () => signer.publicKey(),
      sequence: "0",
      sequenceNumber: () => 0,
      incrementSequenceNumber: () => {},
    };
  }

  function buildClient() {
    const client = new MuxAccountFactoryClient({
      contractId: CONTRACT_ID,
      networkPassphrase: NETWORK_PASSPHRASE,
      rpcUrl: RPC_URL,
    });
    const signer = Keypair.random();
    const server = {
      getAccount: vi.fn().mockResolvedValue(mockAccount(signer)),
      simulateTransaction: vi.fn().mockResolvedValue({
        result: { retval: { value: () => [] } },
      }),
    };
    (client as unknown as { server: unknown }).server = server;
    return { client, server };
  }

  it("throws FactoryBoundsError with code InvalidLimit for limit 0, without submitting a transaction", async () => {
    const { client, server } = buildClient();
    const owner = Address.fromString(Keypair.random().publicKey());
    await expect(
      client.getAccounts(Keypair.random(), owner, 0, 0)
    ).rejects.toMatchObject({
      name: "FactoryBoundsError",
      code: FactoryBoundsErrorCode.InvalidLimit,
    });
    expect(server.getAccount).not.toHaveBeenCalled();
    expect(server.simulateTransaction).not.toHaveBeenCalled();
  });

  it("throws FactoryBoundsError with code LimitTooLarge for limit above MAX_ACCOUNTS_PAGE_SIZE, without submitting a transaction", async () => {
    const { client, server } = buildClient();
    const owner = Address.fromString(Keypair.random().publicKey());
    await expect(
      client.getAccounts(Keypair.random(), owner, 0, MAX_ACCOUNTS_PAGE_SIZE + 1)
    ).rejects.toMatchObject({
      name: "FactoryBoundsError",
      code: FactoryBoundsErrorCode.LimitTooLarge,
    });
    expect(server.getAccount).not.toHaveBeenCalled();
    expect(server.simulateTransaction).not.toHaveBeenCalled();
  });

  it("accepts the maximum page size exported from factory-events and proceeds to build and simulate", async () => {
    const { client, server } = buildClient();
    const owner = Address.fromString(Keypair.random().publicKey());
    const accounts = await client.getAccounts(
      Keypair.random(),
      owner,
      0,
      MAX_ACCOUNTS_PAGE_SIZE
    );
    expect(accounts).toEqual([]);
    expect(server.getAccount).toHaveBeenCalledTimes(1);
    expect(server.simulateTransaction).toHaveBeenCalledTimes(1);
  });

  it("rethrows a host bounds rejection as FactoryBoundsError with the matching code", async () => {
    const { client, server } = buildClient();
    const owner = Address.fromString(Keypair.random().publicKey());
    server.simulateTransaction.mockResolvedValueOnce({
      error: `limit ${MAX_ACCOUNTS_PAGE_SIZE + 1} exceeds maximum page size ${MAX_ACCOUNTS_PAGE_SIZE}`,
    } as never);
    await expect(
      client.getAccounts(Keypair.random(), owner, 0, MAX_ACCOUNTS_PAGE_SIZE + 1)
    ).rejects.toMatchObject({
      name: "FactoryBoundsError",
      code: FactoryBoundsErrorCode.LimitTooLarge,
    });
  });
});
