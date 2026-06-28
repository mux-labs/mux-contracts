/**
 * Account Factory Usage Example
 *
 * Demonstrates using MuxAccountFactoryClient to:
 * - Deploy/register new accounts
 * - Deploy accounts with metadata
 * - Retrieve accounts for an owner
 * - Get account metadata
 * - Get total account count
 *
 * Prerequisites:
 *   npm install @mux-protocol/contracts
 *   npx ts-node examples/account-factory-usage.ts
 *
 * Required env vars:
 *   RPC_URL            — Soroban RPC endpoint
 *   SECRET_KEY         — Stellar secret key (starts with S)
 *   FACTORY_CONTRACT   — MuxAccountFactory contract address
 *   SOROBAN_NETWORK    — localnet | testnet | mainnet  (default: testnet)
 */

import { Address, Keypair, Networks } from "@stellar/stellar-sdk";
import { MuxAccountFactoryClient } from "@mux-protocol/contracts";

// ── Config ──────────────────────────────────────────────────────────────────

const RPC_URL = process.env.RPC_URL ?? "https://soroban-testnet.stellar.org";
const SECRET_KEY = process.env.SECRET_KEY!;
const FACTORY_CONTRACT = process.env.FACTORY_CONTRACT!;
const NETWORK = process.env.SOROBAN_NETWORK ?? "testnet";

const PASSPHRASE: Record<string, string> = {
  localnet: "Standalone Network ; February 2025",
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

const networkPassphrase = PASSPHRASE[NETWORK];
if (!networkPassphrase) {
  console.error(`Unknown network: ${NETWORK}`);
  process.exit(1);
}

const signer = Keypair.fromSecret(SECRET_KEY);
const ownerAddress = Address.fromString(signer.publicKey());

// ── Client Initialization ─────────────────────────────────────────────────────

const client = new MuxAccountFactoryClient({
  contractId: FACTORY_CONTRACT,
  networkPassphrase,
  rpcUrl: RPC_URL,
});

// ── deploy_account example ────────────────────────────────────────────────────

async function demoDeployAccount() {
  // Generate a new account address (in practice, this would be a deployed contract)
  const accountAddress = Address.fromString(signer.publicKey()); // placeholder

  console.log("deploy_account: registering new account…");
  try {
    const deployed = await client.deployAccount(signer, ownerAddress, accountAddress);
    console.log(`  OK — account deployed: ${deployed.toString()}`);
  } catch (err) {
    console.log(`  Failed — ${(err as Error).message}`);
  }
}

// ── deploy_account_with_metadata example ─────────────────────────────────────

async function demoDeployAccountWithMetadata() {
  const accountAddress = Address.fromString(signer.publicKey()); // placeholder
  const version = "1.0.0";
  const description = "My smart account";
  const author = "user";

  console.log("deploy_account_with_metadata: registering account with metadata…");
  try {
    const deployed = await client.deployAccountWithMetadata(
      signer,
      ownerAddress,
      accountAddress,
      version,
      description,
      author
    );
    console.log(`  OK — account deployed: ${deployed.toString()}`);
  } catch (err) {
    console.log(`  Failed — ${(err as Error).message}`);
  }
}

// ── get_accounts example ────────────────────────────────────────────────────

async function demoGetAccounts() {
  console.log("get_accounts: retrieving accounts for owner…");
  try {
    const accounts = await client.getAccounts(ownerAddress);
    console.log(`  OK — found ${accounts.length} account(s)`);
    for (let i = 0; i < accounts.length; i++) {
      console.log(`    [${i}] ${accounts[i].toString()}`);
    }
  } catch (err) {
    console.log(`  Failed — ${(err as Error).message}`);
  }
}

// ── get_account_metadata example ─────────────────────────────────────────────

async function demoGetAccountMetadata() {
  const accountAddress = Address.fromString(signer.publicKey()); // placeholder

  console.log("get_account_metadata: retrieving account metadata…");
  try {
    const metadata = await client.getAccountMetadata(ownerAddress, accountAddress);
    console.log(`  OK — metadata retrieved:`);
    console.log(`    Version: ${metadata.version}`);
    console.log(`    Description: ${metadata.description}`);
    console.log(`    Author: ${metadata.author}`);
  } catch (err) {
    console.log(`  Failed — ${(err as Error).message}`);
  }
}

// ── account_count example ────────────────────────────────────────────────────

async function demoAccountCount() {
  console.log("account_count: retrieving total account count…");
  try {
    const count = await client.accountCount();
    console.log(`  OK — total accounts registered: ${count}`);
  } catch (err) {
    console.log(`  Failed — ${(err as Error).message}`);
  }
}

// ── Entry point ──────────────────────────────────────────────────────────────

(async () => {
  console.log(`\n=== Mux Account Factory Usage Example ===`);
  console.log(`Network: ${NETWORK}`);
  console.log(`Factory Contract: ${FACTORY_CONTRACT}`);
  console.log(`Owner: ${ownerAddress.toString()}\n`);

  await demoDeployAccount();
  await demoDeployAccountWithMetadata();
  await demoGetAccounts();
  await demoGetAccountMetadata();
  await demoAccountCount();

  console.log(`\n=== Example completed ===`);
})();
