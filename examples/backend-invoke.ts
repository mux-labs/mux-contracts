/**
 * Backend Contract Invoke Example
 *
 * Demonstrates how a backend service invokes a Mux contract function via Soroban RPC.
 * This example covers:
 * - Building a transaction to invoke a contract function
 * - Signing the transaction with a server keypair
 * - Submitting the transaction to Soroban RPC
 * - Waiting for and handling the response
 *
 * Prerequisites:
 * - Node.js and npm installed
 * - Environment variables set (see Configuration section below)
 * - Sufficient balance on the signer account
 *
 * Configuration:
 * Set these environment variables or create a .env file:
 *   RPC_URL              - Soroban RPC endpoint (e.g., https://soroban-testnet.stellar.org)
 *   SERVER_SECRET_KEY    - Stellar secret key for signing transactions (starts with S)
 *   SOROBAN_NETWORK      - Network to use (localnet|testnet|mainnet, default: localnet)
 *   CONTRACT_ADDRESS     - Mux contract address to invoke (e.g., CADDRESS...)
 *
 * Example .env file:
 *   RPC_URL=https://soroban-testnet.stellar.org
 *   SERVER_SECRET_KEY=SABC123...
 *   SOROBAN_NETWORK=testnet
 *   CONTRACT_ADDRESS=CXXX...
 *
 * Usage:
 *   npx ts-node examples/backend-invoke.ts
 *   or compile to JS first:
 *   tsc examples/backend-invoke.ts && node examples/backend-invoke.js
 */

import {
  Keypair,
  Networks,
  TransactionBuilder,
  Operation,
  Asset,
  rpc,
} from "@stellar/stellar-sdk";

// ─────────────────────────────────────────────────────────────────────────────
// Configuration: Load from environment variables
// ─────────────────────────────────────────────────────────────────────────────

const RPC_URL = process.env.RPC_URL || "http://localhost:8000";
const SERVER_SECRET_KEY = process.env.SERVER_SECRET_KEY;
const SOROBAN_NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const CONTRACT_ADDRESS = process.env.CONTRACT_ADDRESS;

// Validate required environment variables
if (!SERVER_SECRET_KEY) {
  console.error("❌ Error: SERVER_SECRET_KEY environment variable is not set");
  console.error("Set it to your Stellar secret key (starts with S)");
  process.exit(1);
}

if (!CONTRACT_ADDRESS) {
  console.error("❌ Error: CONTRACT_ADDRESS environment variable is not set");
  console.error("Set it to the Mux contract address you want to invoke (CADDRESS...)");
  process.exit(1);
}

console.log(`✓ Configuration loaded:`);
console.log(`  RPC URL: ${RPC_URL}`);
console.log(`  Network: ${SOROBAN_NETWORK}`);
console.log(`  Contract: ${CONTRACT_ADDRESS}`);

// ─────────────────────────────────────────────────────────────────────────────
// Network Configuration
// ─────────────────────────────────────────────────────────────────────────────

const networkConfig: Record<string, string> = {
  localnet: "Standalone Network ; February 2025",
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

const networkPassphrase = networkConfig[SOROBAN_NETWORK];
if (!networkPassphrase) {
  console.error(
    `❌ Unknown network: ${SOROBAN_NETWORK}. Use: localnet, testnet, or mainnet`
  );
  process.exit(1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Initialize Stellar SDK Client and Server Keypair
// ─────────────────────────────────────────────────────────────────────────────

// Parse the server's secret key into a keypair for signing transactions
const serverKeypair = Keypair.fromSecret(SERVER_SECRET_KEY);
const serverPublicKey = serverKeypair.publicKey();

console.log(`\n✓ Server keypair loaded:`);
console.log(`  Public Key: ${serverPublicKey}`);

// Create RPC client
const sorobanRpc = new rpc.SorobanRpc.Server(RPC_URL, { allowHttp: true });

// ─────────────────────────────────────────────────────────────────────────────
// Example: Invoke a Contract Function
// ─────────────────────────────────────────────────────────────────────────────

async function invokeContractFunction() {
  try {
    console.log(`\n🔄 Step 1: Fetching account details from RPC...`);

    // Step 1: Fetch the account's current sequence number
    // This is required to build a valid transaction
    const account = await sorobanRpc.getAccount(serverPublicKey);
    console.log(`✓ Account loaded with sequence: ${account.sequenceNumber()}`);

    // Step 2: Build the transaction
    console.log(`\n🔄 Step 2: Building transaction to invoke contract...`);

    // Create the transaction builder with:
    // - The account (includes sequence number)
    // - Base fee (1000 stroops is typical for Soroban)
    // - Network passphrase
    // - Timeout (300 seconds)
    const transactionBuilder = new TransactionBuilder(account, {
      fee: "1000",
      networkPassphrase: networkPassphrase,
      timebounds: {
        minTime: 0,
        maxTime: Math.floor(Date.now() / 1000) + 300, // 5 minutes from now
      },
    });

    // Add an invoke hosted function operation
    // This is a placeholder example - modify function, args, and auth as needed
    transactionBuilder.addOperation(
      Operation.invokeHostFunction({
        func: {
          type: "Soroban",
          contractId: CONTRACT_ADDRESS,
          functionName: "initialize",
          // Example function arguments - adjust to your contract
          // These would be serialized Soroban values
          args: [],
        },
        // Auth for signature verification (if required by contract)
        auth: [],
      })
    );

    const transaction = transactionBuilder.build();
    console.log(`✓ Transaction built with ${transaction.operations.length} operation(s)`);

    // Step 3: Sign the transaction
    console.log(`\n🔄 Step 3: Signing transaction with server keypair...`);

    // Sign using the server keypair
    transaction.sign(serverKeypair);
    console.log(`✓ Transaction signed`);

    // Step 4: Convert to XDR (the format Soroban RPC expects)
    console.log(`\n🔄 Step 4: Converting transaction to XDR format...`);

    const transactionXdr = transaction.toEnvelope().toXDR("base64");
    console.log(`✓ Transaction XDR prepared (length: ${transactionXdr.length})`);

    // Step 5: Simulate the transaction (optional but recommended)
    // This gives us the fees and resource usage before actual submission
    console.log(`\n🔄 Step 5: Simulating transaction (without executing)...`);

    const simulateResult = await sorobanRpc.simulateTransaction(transaction);

    if (simulateResult.error) {
      console.error(`⚠️  Simulation error: ${simulateResult.error}`);
      return;
    }

    console.log(`✓ Simulation successful`);
    if ("minResourceFee" in simulateResult && simulateResult.minResourceFee) {
      console.log(
        `  Estimated resource fee: ${simulateResult.minResourceFee} stroops`
      );
    }

    // Step 6: Submit the transaction to the network
    console.log(`\n🔄 Step 6: Submitting transaction to Soroban RPC...`);

    const submitResult = await sorobanRpc.sendTransaction(transaction);
    const transactionHash = submitResult.hash;

    console.log(`✓ Transaction submitted`);
    console.log(`  Transaction Hash: ${transactionHash}`);

    // Step 7: Poll for transaction result
    console.log(
      `\n🔄 Step 7: Waiting for transaction result (may take a few seconds)...`
    );

    // Poll until the transaction is included in a ledger
    let finalResult;
    let attempts = 0;
    const maxAttempts = 30;

    while (attempts < maxAttempts) {
      attempts++;
      const response = await sorobanRpc.getTransaction(transactionHash);

      if (response.status === "SUCCESS") {
        console.log(`✓ Transaction confirmed on ledger ${response.ledger}`);
        finalResult = response;
        break;
      } else if (response.status === "FAILED") {
        console.error(`❌ Transaction failed on ledger ${response.ledger}`);
        console.error(`   Reason: ${response.resultXdr}`);
        return;
      } else if (response.status === "NOT_FOUND") {
        // Still pending, wait and retry
        console.log(`  Attempt ${attempts}/${maxAttempts}: Still pending, retrying...`);
        await new Promise((resolve) => setTimeout(resolve, 1000)); // Wait 1 second
      }
    }

    if (!finalResult) {
      console.error(`⏱️  Transaction did not confirm within ${maxAttempts * 1}s`);
      return;
    }

    // Step 8: Parse the result
    console.log(`\n🔄 Step 8: Parsing transaction result...`);

    console.log(`✓ Transaction completed successfully`);
    console.log(`  Status: ${finalResult.status}`);
    console.log(`  Ledger: ${finalResult.ledger}`);
    if ("returnValue" in finalResult) {
      console.log(`  Return Value: ${finalResult.returnValue}`);
    }

    console.log(`\n✅ Backend contract invocation completed successfully!`);
  } catch (error) {
    console.error(`\n❌ Error during contract invocation:`, error);
    process.exit(1);
  }
}

// Run the example
invokeContractFunction();
