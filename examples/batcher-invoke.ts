/**
 * Batcher Invoke Example
 *
 * Demonstrates how to use MuxBatcherClient to submit a multi-operation
 * batch to the mux-batcher Soroban contract via Soroban RPC.
 *
 * Covered in this example:
 *   1. Estimate fees for a planned batch
 *   2. Simulate the batch (dry-run, no on-chain effect)
 *   3. Execute the batch and inspect the result
 *
 * Prerequisites:
 *   npm install @mux-protocol/contracts @stellar/stellar-sdk
 *
 * Required environment variables:
 *   RPC_URL             Soroban RPC endpoint
 *                       (e.g. https://soroban-testnet.stellar.org)
 *   SECRET_KEY          Stellar secret key of the signing account (S...)
 *   BATCHER_CONTRACT_ID Deployed mux-batcher contract ID (C...)
 *   TARGET_CONTRACT_ID  Any on-chain contract to include in the batch (C...)
 *   SOROBAN_NETWORK     localnet | testnet | mainnet  (default: testnet)
 *
 * Usage:
 *   npx ts-node examples/batcher-invoke.ts
 */

import { Address, Keypair, Networks, nativeToScVal } from "@stellar/stellar-sdk";
import { MuxBatcherClient } from "../bindings/src/generated/mux-batcher";
import type { Operation } from "../bindings/src/types";

// ── Configuration ────────────────────────────────────────────────────────────

const RPC_URL = process.env.RPC_URL ?? "https://soroban-testnet.stellar.org";
const SECRET_KEY = process.env.SECRET_KEY;
const BATCHER_CONTRACT_ID = process.env.BATCHER_CONTRACT_ID;
const TARGET_CONTRACT_ID = process.env.TARGET_CONTRACT_ID;
const NETWORK = process.env.SOROBAN_NETWORK ?? "testnet";

const PASSPHRASES: Record<string, string> = {
  localnet: "Standalone Network ; February 2017",
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

if (!SECRET_KEY) {
  console.error("SECRET_KEY is required");
  process.exit(1);
}
if (!BATCHER_CONTRACT_ID) {
  console.error("BATCHER_CONTRACT_ID is required");
  process.exit(1);
}
if (!TARGET_CONTRACT_ID) {
  console.error("TARGET_CONTRACT_ID is required");
  process.exit(1);
}

const networkPassphrase = PASSPHRASES[NETWORK];
if (!networkPassphrase) {
  console.error(`Unknown network: ${NETWORK}. Use localnet, testnet, or mainnet`);
  process.exit(1);
}

// ── Setup ────────────────────────────────────────────────────────────────────

const signer = Keypair.fromSecret(SECRET_KEY);
const callerAddress = Address.fromString(signer.publicKey());

console.log(`Network:          ${NETWORK}`);
console.log(`RPC URL:          ${RPC_URL}`);
console.log(`Batcher contract: ${BATCHER_CONTRACT_ID}`);
console.log(`Target contract:  ${TARGET_CONTRACT_ID}`);
console.log(`Caller:           ${signer.publicKey()}`);

const client = new MuxBatcherClient({
  contractId: BATCHER_CONTRACT_ID,
  networkPassphrase,
  rpcUrl: RPC_URL,
});

// ── Build operations ─────────────────────────────────────────────────────────

const operations: Operation[] = [
  {
    target: Address.fromString(TARGET_CONTRACT_ID),
    fnName: "owner",
    args: [],
    requireSuccess: false,
    kind: "Invoke",
  },
  {
    target: Address.fromString(TARGET_CONTRACT_ID),
    fnName: "max_batch_size",
    args: [],
    requireSuccess: false,
    kind: "Invoke",
  },
];

// ── Step 1: Estimate fees ────────────────────────────────────────────────────

async function estimateFees(): Promise<void> {
  console.log(`\nStep 1 — Estimating fees for ${operations.length} operation(s)...`);
  const stroops = await client.estimateFees(signer, operations.length);
  console.log(`  Estimated fee: ${stroops} stroops`);
}

// ── Step 2: Simulate batch ───────────────────────────────────────────────────

async function simulateBatch(): Promise<void> {
  console.log(`\nStep 2 — Simulating batch (no on-chain effect)...`);
  const result = await client.simulateBatch(signer, callerAddress, operations);
  console.log(`  Simulation result:`);
  console.log(`    Success count: ${result.successCount}`);
  console.log(`    Failure count: ${result.failureCount}`);
}

// ── Step 3: Execute batch ────────────────────────────────────────────────────

async function executeBatch(): Promise<void> {
  console.log(`\nStep 3 — Executing batch on-chain...`);
  const result = await client.executeBatch(signer, callerAddress, operations);
  console.log(`  Batch result:`);
  console.log(`    Success count: ${result.successCount}`);
  console.log(`    Failure count: ${result.failureCount}`);

  if (result.failureCount > 0) {
    console.log(`  Warning: ${result.failureCount} operation(s) failed.`);
  } else {
    console.log(`  All operations succeeded.`);
  }
}

// ── Entry point ──────────────────────────────────────────────────────────────

(async () => {
  try {
    await estimateFees();
    await simulateBatch();
    await executeBatch();
    console.log("\nDone.");
  } catch (err) {
    console.error("\nError:", err instanceof Error ? err.message : err);
    process.exit(1);
  }
})();
