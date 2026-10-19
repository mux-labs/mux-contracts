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
import {
  MuxWalletRegistryClient,
  isValidWalletName,
  validateWalletName,
  WALLET_NAME_REGEX,
  WALLET_NAME_MAX_LEN,
} from "@mux-protocol/contracts";

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

  // ── Name Charset Policy Demonstration ─────────────────────────────────────
  // Wallet names must follow the Soroban Symbol charset policy:
  // - 1 to 32 characters
  // - Allowed charset: [a-zA-Z0-9_] (alphanumeric and underscore only)
  // - No spaces, dashes, dots, emojis, or special characters
  console.log(`\nValidating wallet name charset policy: ${WALLET_NAME_REGEX} (max ${WALLET_NAME_MAX_LEN} chars)`);

  const validExamples = ["treasury", "hot_wallet_01", "vault_cold", "A1_B2"];
  for (const sample of validExamples) {
    const valid = isValidWalletName(sample);
    if (!valid) {
      throw new Error(`Expected '${sample}' to be a valid wallet name, but it was rejected`);
    }
    console.log(`  ✓ Name '${sample}' is valid: ${valid}`);
  }

  const invalidExamples = ["invalid-name", "has space", "too_many_characters_12345678901234567890", "bad@char!"];
  for (const sample of invalidExamples) {
    if (isValidWalletName(sample)) {
      throw new Error(`Expected '${sample}' to be rejected, but it was accepted`);
    }
    try {
      validateWalletName(sample);
      throw new Error(`Expected '${sample}' to be rejected, but validateWalletName accepted it`);
    } catch (err) {
      if ((err as Error).message.startsWith("Expected")) {
        throw err;
      }
      console.log(`  ✗ Name '${sample}' correctly rejected fail-closed: ${(err as Error).message}`);
    }
  }

  // ── Boundary Cases Derived from WALLET_NAME_MAX_LEN ───────────────────────
  // A name of exactly WALLET_NAME_MAX_LEN characters must pass; one character
  // longer must be rejected.
  const atLimit = "a".repeat(WALLET_NAME_MAX_LEN);
  const overLimit = "a".repeat(WALLET_NAME_MAX_LEN + 1);

  if (!isValidWalletName(atLimit)) {
    throw new Error(`Expected a name of exactly ${WALLET_NAME_MAX_LEN} chars to be valid, but it was rejected`);
  }
  validateWalletName(atLimit);
  console.log(`  ✓ Name of exactly ${WALLET_NAME_MAX_LEN} chars accepted`);

  if (isValidWalletName(overLimit)) {
    throw new Error(`Expected a name of ${WALLET_NAME_MAX_LEN + 1} chars to be rejected, but it was accepted`);
  }
  let overLimitRejected = false;
  try {
    validateWalletName(overLimit);
  } catch (err) {
    overLimitRejected = true;
    console.log(`  ✗ Name of ${WALLET_NAME_MAX_LEN + 1} chars correctly rejected: ${(err as Error).message}`);
  }
  if (!overLimitRejected) {
    throw new Error(`Expected a name of ${WALLET_NAME_MAX_LEN + 1} chars to be rejected, but validateWalletName accepted it`);
  }

  const name = "treasury";
  const wallet = Address.fromString(signer.publicKey());

  console.log(`\nRegistering wallet name '${name}' → ${wallet.toString()}...`);
  await walletClient.registerWallet(signer, name, wallet);
  console.log("  Registration complete.");

  console.log(`\nLooking up wallet name '${name}'...`);
  const fetched = await walletClient.getWallet(signer, name);
  console.log(`  Fetched wallet address: ${fetched.toString()}`);

  if (fetched.toString() !== wallet.toString()) {
    console.error(
      `❌ Round-trip mismatch for '${name}': registered ${wallet.toString()} but fetched ${fetched.toString()}`,
    );
    process.exit(1);
  }
  console.log("  ✓ Fetched address matches registered address.");
}

main().catch((err) => {
  console.error("Error executing wallet registry example:", err);
  process.exit(1);
});
