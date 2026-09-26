/**
 * End-to-end authorization flow for mux-account.
 *
 * The flow is intentionally fail-closed and exercises the same sequence a dApp
 * or relayer uses in production:
 *   1. owner registers an expiring, method-scoped session key;
 *   2. the session key executes an allowed call with the current nonce;
 *   3. an allowlisted relayer submits a sponsored call with both authorizations;
 *   4. the owner revokes the key and the example verifies the next call fails.
 *
 * Required environment:
 *   RPC_URL, OWNER_SECRET_KEY, SESSION_SECRET_KEY, ACCOUNT_CONTRACT,
 *   TARGET_CONTRACT, SOROBAN_NETWORK (localnet|testnet|mainnet).
 * Optional: RELAYER_SECRET_KEY to exercise sponsorship.
 *
 * Run with: npx ts-node examples/authorize-flow.ts
 */
import { Address, Keypair, xdr } from "@stellar/stellar-sdk";
import {
  allowRelayer,
  executeSponsored,
  executeWithSession,
  readNonce,
  registerSessionKey,
  revokeSessionKey,
} from "./session-key-usage";

function required(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is required; refusing to run without explicit configuration`);
  return value;
}

async function main(): Promise<void> {
  const owner = Keypair.fromSecret(required("OWNER_SECRET_KEY"));
  const session = Keypair.fromSecret(required("SESSION_SECRET_KEY"));
  const account = Address.fromString(required("ACCOUNT_CONTRACT"));
  const target = Address.fromString(required("TARGET_CONTRACT"));
  const sessionAddress = Address.fromString(session.publicKey());
  const method = process.env.AUTHORIZE_METHOD ?? "get_metadata";
  const args: xdr.ScVal[] = [];
  const expiresAt = Math.floor(Date.now() / 1000) + 3600;

  console.log(`Authorizing ${session.publicKey()} for ${method} until ${expiresAt}`);
  await registerSessionKey(owner, sessionAddress, expiresAt, [method]);

  const directResult = await executeWithSession(
    session,
    target,
    method,
    args,
    await readNonce(session),
  );
  console.log("Direct session authorization succeeded:", directResult);

  const relayerSecret = process.env.RELAYER_SECRET_KEY;
  if (relayerSecret) {
    const relayer = Keypair.fromSecret(relayerSecret);
    await allowRelayer(owner, Address.fromString(relayer.publicKey()));
    const sponsoredResult = await executeSponsored(
      relayer,
      session,
      target,
      method,
      args,
      await readNonce(relayer),
    );
    console.log("Sponsored authorization succeeded:", sponsoredResult);
  }

  await revokeSessionKey(owner, sessionAddress);
  try {
    await executeWithSession(session, target, method, args, await readNonce(session));
    throw new Error("revoked session unexpectedly executed; fail-closed invariant violated");
  } catch (error) {
    console.log("Revoked session was rejected as expected:", error instanceof Error ? error.message : error);
  }
}

main().catch((error) => {
  console.error("Authorization flow failed:", error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
