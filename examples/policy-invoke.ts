/**
 * Mux Policy Contract Invocation Example
 *
 * Demonstrates how to interact with the mux-policy contract on Soroban:
 *  - Initialize the contract
 *  - Set a daily spend limit for a wallet
 *  - Query the current daily limit
 *  - Record a spend against the limit
 *
 * Prerequisites:
 *  - Node.js and npm installed
 *  - Environment variables set (see Configuration section below)
 *  - Sufficient balance on the signer account
 *
 * Configuration:
 *   RPC_URL              - Soroban RPC endpoint
 *   SERVER_SECRET_KEY    - Stellar secret key for signing transactions
 *   SOROBAN_NETWORK      - Network to use (localnet|testnet|mainnet, default: localnet)
 *   CONTRACT_ADDRESS     - Deployed mux-policy contract address
 *   WALLET_ADDRESS       - Wallet address to manage (public key starting with G)
 */

import {
  Keypair,
  Networks,
  nativeToScVal,
  TransactionBuilder,
  Operation,
  rpc,
} from "@stellar/stellar-sdk";

// ── Configuration ────────────────────────────────────────────────────────────

const RPC_URL = process.env.RPC_URL || "http://localhost:8000";
const SERVER_SECRET_KEY = process.env.SERVER_SECRET_KEY;
const SOROBAN_NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const CONTRACT_ADDRESS = process.env.CONTRACT_ADDRESS;
const WALLET_ADDRESS = process.env.WALLET_ADDRESS;

if (!SERVER_SECRET_KEY) {
  console.error("Error: SERVER_SECRET_KEY is not set");
  process.exit(1);
}

if (!CONTRACT_ADDRESS) {
  console.error("Error: CONTRACT_ADDRESS is not set");
  process.exit(1);
}

if (!WALLET_ADDRESS) {
  console.error("Error: WALLET_ADDRESS is not set");
  process.exit(1);
}

// ── Network configuration ────────────────────────────────────────────────────

const NETWORK_PASSPHRASE: Record<string, string> = {
  localnet: "Standalone Network ; February 2025",
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

const networkPassphrase = NETWORK_PASSPHRASE[SOROBAN_NETWORK];
if (!networkPassphrase) {
  console.error(`Unknown network: ${SOROBAN_NETWORK}. Use: localnet, testnet, mainnet`);
  process.exit(1);
}

const serverKeypair = Keypair.fromSecret(SERVER_SECRET_KEY);
const sorobanRpc = new rpc.SorobanRpc.Server(RPC_URL, { allowHttp: true });

// ── Helper: invoke a contract function ───────────────────────────────────────

async function invokeContract(
  functionName: string,
  args: xdr.ScVal[],
): Promise<rpc.Api.GetSuccessfulTransactionResponse> {
  const account = await sorobanRpc.getAccount(serverKeypair.publicKey());

  const tx = new TransactionBuilder(account, {
    fee: "1000",
    networkPassphrase,
    timebounds: {
      minTime: 0,
      maxTime: Math.floor(Date.now() / 1000) + 300,
    },
  })
    .addOperation(
      Operation.invokeHostFunction({
        func: {
          type: "Soroban",
          contractId: CONTRACT_ADDRESS,
          functionName,
          args,
        },
        auth: [],
      }),
    )
    .build();

  const simulate = await sorobanRpc.simulateTransaction(tx);
  if (simulate.error) {
    throw new Error(`Simulation error: ${simulate.error}`);
  }

  const prepared = rpc.SorobanRpc.assembleTransaction(
    tx,
    simulate as rpc.Api.SimulateTransactionSuccessResponse,
  ).build();

  prepared.sign(serverKeypair);
  const sent = await sorobanRpc.sendTransaction(prepared);

  let result;
  for (let i = 0; i < 30; i++) {
    const response = await sorobanRpc.getTransaction(sent.hash);
    if (response.status === "SUCCESS") {
      result = response as rpc.Api.GetSuccessfulTransactionResponse;
      break;
    }
    if (response.status === "FAILED") {
      throw new Error(`Transaction failed: ${response.resultXdr}`);
    }
    await new Promise((r) => setTimeout(r, 1000));
  }

  if (!result) {
    throw new Error("Transaction did not confirm within 30s");
  }

  return result;
}

// ── Main ─────────────────────────────────────────────────────────────────────

(async function main() {
  try {
    // 1. Initialize the policy contract (admin-only)
    console.log("1. Initializing policy contract...");
    const initResult = await invokeContract("initialize", [
      nativeToScVal(serverKeypair.publicKey(), { type: "address" }),
    ]);
    console.log("   Contract initialized. Ledger:", initResult.ledger);

    // 2. Set a daily limit of 5000 units over ~1 day (17280 ledgers)
    console.log("\n2. Setting daily spend limit...");
    const DAY_LEDGERS = 17280;
    const limitResult = await invokeContract("set_daily_limit", [
      nativeToScVal(WALLET_ADDRESS, { type: "address" }),
      nativeToScVal(BigInt(5000), { type: "i128" }),
      nativeToScVal(DAY_LEDGERS, { type: "u32" }),
    ]);
    console.log("   Daily limit set to 5000. Ledger:", limitResult.ledger);

    // 3. Query the daily limit for the wallet
    console.log("\n3. Querying daily limit...");
    const queryResult = await invokeContract("get_daily_limit", [
      nativeToScVal(WALLET_ADDRESS, { type: "address" }),
    ]);
    console.log("   Query result:", JSON.stringify(queryResult.returnValue, null, 2));

    // 4. Record a spend of 1200 units (wallet-authenticated)
    console.log("\n4. Recording spend of 1200...");
    const spendResult = await invokeContract("record_spend", [
      nativeToScVal(WALLET_ADDRESS, { type: "address" }),
      nativeToScVal(BigInt(1200), { type: "i128" }),
    ]);
    console.log("   Spend recorded. Ledger:", spendResult.ledger);

    // 5. Verify remaining limit by querying again
    console.log("\n5. Verifying remaining daily limit...");
    const verifyResult = await invokeContract("get_daily_limit", [
      nativeToScVal(WALLET_ADDRESS, { type: "address" }),
    ]);
    const dailyLimit = verifyResult.returnValue as Record<string, any>;
    const remaining = Number(dailyLimit.limit) - Number(dailyLimit.spent);
    console.log(`   Spent: ${dailyLimit.spent}, Remaining: ${remaining}`);

    console.log("\nAll policy operations completed successfully.");
  } catch (err) {
    console.error("Error:", err);
    process.exit(1);
  }
})();
