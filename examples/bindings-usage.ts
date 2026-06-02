/**
 * TypeScript Bindings Usage Example
 *
 * Demonstrates importing and calling check_spend (MuxSpendingPolicyClient)
 * and register_wallet (MuxWalletRegistryClient) from the bindings package.
 *
 * Prerequisites:
 *   npm install @mux-protocol/contracts
 *   npx ts-node examples/bindings-usage.ts
 *
 * Required env vars:
 *   RPC_URL            — Soroban RPC endpoint
 *   SECRET_KEY         — Stellar secret key (starts with S)
 *   SPENDING_CONTRACT  — MuxSpendingPolicy contract address
 *   WALLET_CONTRACT    — MuxWalletRegistry contract address
 *   SOROBAN_NETWORK    — localnet | testnet | mainnet  (default: testnet)
 */

import { Address, Keypair, Networks } from "@stellar/stellar-sdk";
import {
  MuxSpendingPolicyClient,
  MuxWalletRegistryClient,
} from "@mux-protocol/contracts";

// ── Config ──────────────────────────────────────────────────────────────────

const RPC_URL = process.env.RPC_URL ?? "https://soroban-testnet.stellar.org";
const SECRET_KEY = process.env.SECRET_KEY!;
const SPENDING_CONTRACT = process.env.SPENDING_CONTRACT!;
const WALLET_CONTRACT = process.env.WALLET_CONTRACT!;
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

// ── check_spend example ──────────────────────────────────────────────────────

async function demoCheckSpend() {
  const client = new MuxSpendingPolicyClient({
    contractId: SPENDING_CONTRACT,
    networkPassphrase,
    rpcUrl: RPC_URL,
  });

  const account = Address.fromString(signer.publicKey());
  // Simulate a spend of 500 units of a native-asset proxy address.
  const asset = Address.fromString(SPENDING_CONTRACT); // placeholder
  const amount = 500n;

  console.log("check_spend: verifying spend is within policy limit…");
  try {
    await client.checkSpend(signer, account, asset, amount);
    console.log(`  OK — ${amount} units is within the policy limit.`);
  } catch (err) {
    console.log(`  Blocked — ${(err as Error).message}`);
  }
}

// ── register_wallet example ──────────────────────────────────────────────────

async function demoRegisterWallet() {
  const client = new MuxWalletRegistryClient({
    contractId: WALLET_CONTRACT,
    networkPassphrase,
    rpcUrl: RPC_URL,
  });

  const walletAddress = Address.fromString(signer.publicKey());
  const name = "treasury";

  console.log(`register_wallet: registering "${name}"…`);
  await client.registerWallet(signer, name, walletAddress);
  console.log("  Registered.");

  const fetched = await client.getWallet(signer, name);
  console.log(`  Lookup OK — address: ${fetched.toString()}`);
}

// ── Entry point ──────────────────────────────────────────────────────────────

(async () => {
  await demoCheckSpend();
  await demoRegisterWallet();
})();
