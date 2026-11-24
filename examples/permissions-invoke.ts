/**
 * Permissions Contract Invoke Example
 *
 * Demonstrates invoking the mux-permissions contract via Soroban RPC:
 * - Initialize the permissions contract with an admin
 * - Create a role with a set of permissions
 * - Grant and revoke roles for accounts
 * - Query permissions and role membership
 *
 * Prerequisites:
 * - Node.js and npm installed
 * - Environment variables set (see below)
 * - Sufficient balance on the signer account
 *
 * Configuration (env vars or .env file):
 *   RPC_URL              - Soroban RPC endpoint (e.g., https://soroban-testnet.stellar.org)
 *   SERVER_SECRET_KEY    - Stellar secret key for signing (starts with S)
 *   SOROBAN_NETWORK      - Network (localnet|testnet|mainnet, default: localnet)
 *   PERMISSIONS_CONTRACT - mux-permissions contract address (CADDRESS...)
 *
 * Usage:
 *   npx ts-node examples/permissions-invoke.ts
 */

import {
  Keypair,
  Networks,
  TransactionBuilder,
  Operation,
  rpc,
  xdr,
  nativeToScVal,
  Address,
} from "@stellar/stellar-sdk";

// ── Configuration ────────────────────────────────────────────────────────────

const RPC_URL = process.env.RPC_URL || "http://localhost:8000";
const SERVER_SECRET_KEY = process.env.SERVER_SECRET_KEY;
const SOROBAN_NETWORK = process.env.SOROBAN_NETWORK || "localnet";
const PERMISSIONS_CONTRACT = process.env.PERMISSIONS_CONTRACT;

if (!SERVER_SECRET_KEY) {
  console.error("Error: SERVER_SECRET_KEY environment variable is not set");
  process.exit(1);
}
if (!PERMISSIONS_CONTRACT) {
  console.error("Error: PERMISSIONS_CONTRACT environment variable is not set");
  process.exit(1);
}

const networkConfig: Record<string, string> = {
  localnet: "Standalone Network ; February 2025",
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

const networkPassphrase = networkConfig[SOROBAN_NETWORK];
if (!networkPassphrase) {
  console.error(
    `Unknown network: ${SOROBAN_NETWORK}. Use: localnet, testnet, or mainnet`
  );
  process.exit(1);
}

const serverKeypair = Keypair.fromSecret(SERVER_SECRET_KEY);
const serverPublicKey = serverKeypair.publicKey();
const sorobanRpc = new rpc.SorobanRpc.Server(RPC_URL, { allowHttp: true });

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Outcome of one build/simulate/submit/poll cycle.
 *
 * Failure is reported to the caller (never swallowed) so `main()` can abort
 * the remaining steps and exit non-zero — the example is fail-closed like the
 * other examples in `examples/` (issue #966).
 */
type StepResult =
  | { ok: true; ledger: number }
  | { ok: false; reason: string };

async function buildAndSubmit(
  functionName: string,
  args: xdr.ScVal[]
): Promise<StepResult> {
  const account = await sorobanRpc.getAccount(serverPublicKey);

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
          contractId: PERMISSIONS_CONTRACT!,
          functionName,
          args,
        },
        auth: [],
      })
    )
    .build();

  const simResult = await sorobanRpc.simulateTransaction(tx);
  if (simResult.error) {
    console.error(`Simulation failed for ${functionName}:`, simResult.error);
    return { ok: false, reason: `simulation failed: ${simResult.error}` };
  }
  console.log(`[${functionName}] simulation OK`);

  tx.sign(serverKeypair);
  const submitResult = await sorobanRpc.sendTransaction(tx);
  console.log(`[${functionName}] submitted — hash: ${submitResult.hash}`);

  let attempts = 0;
  while (attempts < 30) {
    attempts++;
    const resp = await sorobanRpc.getTransaction(submitResult.hash);
    if (resp.status === "SUCCESS") {
      console.log(`[${functionName}] confirmed on ledger ${resp.ledger}`);
      return { ok: true, ledger: resp.ledger };
    }
    if (resp.status === "FAILED") {
      console.error(`[${functionName}] failed:`, resp.resultXdr);
      return { ok: false, reason: `transaction FAILED: ${resp.resultXdr}` };
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
  console.error(`[${functionName}] timed out waiting for confirmation`);
  return { ok: false, reason: "timed out waiting for confirmation" };
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  // NOTE: `initialize` is expected to fail on an already-initialized contract
  // (AlreadyInitialized). That failure is now surfaced as a non-zero exit
  // instead of being swallowed, so a re-run against a live contract stops
  // after step 1 rather than attempting the remaining steps (issue #966).
  console.log("=== mux-permissions invoke example ===");
  console.log(`  RPC:      ${RPC_URL}`);
  console.log(`  Network:  ${SOROBAN_NETWORK}`);
  console.log(`  Contract: ${PERMISSIONS_CONTRACT}`);
  console.log(`  Admin:    ${serverPublicKey}\n`);

  // 1. Initialize the permissions contract
  console.log("--- Step 1: Initialize ---");
  const initResult = await buildAndSubmit("initialize", [
    new Address(serverPublicKey).toScVal(),
  ]);
  if (initResult.ok === false) {
    console.error(`Aborting: initialize failed (${initResult.reason}); remaining steps skipped`);
    process.exit(1);
  }

  // 2. Create an "operator" role with "transfer" and "mint" permissions
  console.log("\n--- Step 2: Create role ---");
  const createRoleResult = await buildAndSubmit("create_role", [
    nativeToScVal("operator", { type: "symbol" }),
    nativeToScVal(["transfer", "mint"], { type: "symbol" }),
  ]);
  if (createRoleResult.ok === false) {
    console.error(`Aborting: create_role failed (${createRoleResult.reason}); remaining steps skipped`);
    process.exit(1);
  }

  // 3. Grant the role to a target account
  console.log("\n--- Step 3: Grant role ---");
  const targetPublicKey = process.env.TARGET_ACCOUNT || serverPublicKey;
  const grantResult = await buildAndSubmit("grant_role", [
    new Address(targetPublicKey).toScVal(),
    nativeToScVal("operator", { type: "symbol" }),
  ]);
  if (grantResult.ok === false) {
    console.error(`Aborting: grant_role failed (${grantResult.reason}); remaining steps skipped`);
    process.exit(1);
  }

  // 4. Check permission (read-only, simulation only)
  console.log("\n--- Step 4: Check permission (has_permission) ---");
  const account = await sorobanRpc.getAccount(serverPublicKey);
  const checkTx = new TransactionBuilder(account, {
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
          contractId: PERMISSIONS_CONTRACT!,
          functionName: "has_permission",
          args: [
            new Address(targetPublicKey).toScVal(),
            nativeToScVal("transfer", { type: "symbol" }),
          ],
        },
        auth: [],
      })
    )
    .build();

  const simCheck = await sorobanRpc.simulateTransaction(checkTx);
  if (simCheck.error) {
    console.error("has_permission simulation failed:", simCheck.error);
    console.error("Aborting: has_permission check failed; remaining steps skipped");
    process.exit(1);
  } else {
    console.log("has_permission simulation OK (check returnValue for result)");
  }

  // 5. Revoke the role
  console.log("\n--- Step 5: Revoke role ---");
  const revokeResult = await buildAndSubmit("revoke_role", [
    new Address(targetPublicKey).toScVal(),
    nativeToScVal("operator", { type: "symbol" }),
  ]);
  if (revokeResult.ok === false) {
    console.error(`Aborting: revoke_role failed (${revokeResult.reason})`);
    process.exit(1);
  }

  console.log("\n=== Done ===");
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
