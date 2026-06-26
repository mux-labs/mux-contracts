/**
 * Wallet Registry Invoke Example
 *
 * Demonstrates how to register and look up a wallet address using the
 * MuxWalletRegistryClient from the @mux-protocol/contracts bindings package.
 *
 * Prerequisites:
 *   npm install @mux-protocol/contracts @stellar/stellar-sdk
 *   npx ts-node examples/wallet-registry-invoke.ts
 *
 * Required env vars:
 *   RPC_URL            — Soroban RPC endpoint
 *   SECRET_KEY         — Stellar secret key for the signer account
 *   WALLET_CONTRACT    — MuxWalletRegistry contract address
 *   SOROBAN_NETWORK    — localnet | testnet | mainnet  (default: testnet)
 */

import { Address, Keypair, Networks } from "@stellar/stellar-sdk";
import { MuxWalletRegistryClient } from "@mux-protocol/contracts";

const RPC_URL = process.env.RPC_URL ?? "https://soroban-testnet.stellar.org";
const SECRET_KEY = process.env.SECRET_KEY;
const WALLET_CONTRACT = process.env.WALLET_CONTRACT;
const NETWORK = process.env.SOROBAN_NETWORK ?? "testnet";

const PASSPHRASE: Record<string, string> = {
  localnet: "Standalone Network ; February 2025",
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

if (!SECRET_KEY) {
  console.error("❌ SECRET_KEY must be set");
  process.exit(1);
}

if (!WALLET_CONTRACT) {
  console.error("❌ WALLET_CONTRACT must be set");
  process.exit(1);
}

const networkPassphrase = PASSPHRASE[NETWORK];
if (!networkPassphrase) {
  console.error(`❌ Unknown network: ${NETWORK}. Use localnet, testnet, or mainnet`);
  process.exit(1);
}

const signer = Keypair.fromSecret(SECRET_KEY);
const walletClient = new MuxWalletRegistryClient({
  contractId: WALLET_CONTRACT,
  networkPassphrase,
  rpcUrl: RPC_URL,
});

async function main() {
  console.log(`Connecting to ${NETWORK} RPC at ${RPC_URL}`);
  console.log(`Wallet registry contract: ${WALLET_CONTRACT}`);

  const name = "treasury";
  const wallet = Address.fromString(signer.publicKey());

  console.log(`\nRegistering wallet name '${name}' → ${wallet.toString()}...`);
  await walletClient.registerWallet(signer, name, wallet);
  console.log("  Registration complete.");

  console.log(`\nLooking up wallet name '${name}'...`);
  const fetched = await walletClient.getWallet(signer, name);
  console.log(`  Fetched wallet address: ${fetched.toString()}`);
}

main().catch((err) => {
  console.error("Error executing wallet registry example:", err);
  process.exit(1);
});
