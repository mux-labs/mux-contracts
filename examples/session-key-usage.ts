/**
 * Session Key Usage Example
 *
 * Frontend / relayer integration example for the mux-account session-key flow:
 * - Owner registers a scoped, expiring session key
 * - The dApp reads the account nonce and executes a call with that session key
 *   (owner signature not needed)
 * - A relayer executes the same call on the user's behalf and pays the fee
 * - Owner revokes the session key
 *
 * Prerequisites:
 *   npm install @stellar/stellar-sdk
 *   npx ts-node examples/session-key-usage.ts
 *
 * Required env vars:
 *   RPC_URL            - Soroban RPC endpoint
 *   OWNER_SECRET_KEY   - Stellar secret key of the account owner (starts with S)
 *   SESSION_SECRET_KEY - Stellar secret key generated for this session
 *   RELAYER_SECRET_KEY - Stellar secret key of the sponsoring relayer (optional)
 *   ACCOUNT_CONTRACT   - Deployed mux-account contract address
 *   TARGET_CONTRACT    - Contract the session key is allowed to call
 *   SOROBAN_NETWORK    - localnet | testnet | mainnet  (default: testnet)
 */

import {
  Address,
  Contract,
  Keypair,
  Networks,
  nativeToScVal,
  rpc,
  scValToNative,
  Transaction,
  TransactionBuilder,
  xdr,
} from "@stellar/stellar-sdk";

// === Config

const RPC_URL = process.env.RPC_URL ?? "https://soroban-testnet.stellar.org";
const NETWORK = process.env.SOROBAN_NETWORK ?? "testnet";
const ACCOUNT_CONTRACT = process.env.ACCOUNT_CONTRACT!;
const TARGET_CONTRACT = process.env.TARGET_CONTRACT!;

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

const server = new rpc.Server(RPC_URL, { allowHttp: NETWORK === "localnet" });
const account = new Contract(ACCOUNT_CONTRACT);

// === Helpers

/**
 * Build, sign, and submit a call to the mux-account contract.
 *
 * `signers` is the full set of keys whose `require_auth()` the entrypoint
 * asserts. The FIRST signer is the transaction source and therefore the party
 * that pays the network fee — this is what "gas abstraction" means here: for a
 * sponsored call the relayer is the source, while the session key only
 * authorizes the invocation.
 */
async function invoke(
  method: string,
  args: xdr.ScVal[],
  signers: Keypair[]
): Promise<unknown> {
  const source = signers[0];
  const sourceAccount = await server.getAccount(source.publicKey());

  const built = new TransactionBuilder(sourceAccount, {
    fee: "1000000",
    networkPassphrase,
  })
    .addOperation(account.call(method, ...args))
    .setTimeout(60)
    .build();

  const simulated = await server.simulateTransaction(built);
  if (rpc.Api.isSimulationError(simulated)) {
    throw new Error(`${method} simulation failed: ${simulated.error}`);
  }

  const prepared = rpc.assembleTransaction(built, simulated).build() as Transaction;
  for (const signer of signers) {
    prepared.sign(signer);
  }

  const sent = await server.sendTransaction(prepared);
  if (sent.status === "ERROR") {
    throw new Error(`${method} submission failed: ${JSON.stringify(sent.errorResult)}`);
  }

  let result = await server.getTransaction(sent.hash);
  while (result.status === rpc.Api.GetTransactionStatus.NOT_FOUND) {
    await new Promise((resolve) => setTimeout(resolve, 1000));
    result = await server.getTransaction(sent.hash);
  }
  if (result.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`${method} failed on-chain: ${result.status}`);
  }
  return result.returnValue ? scValToNative(result.returnValue) : undefined;
}

/**
 * Read the account's current transaction nonce. Every execution entrypoint
 * requires exactly this value and advances it by one, so a relayer with queued
 * calls must submit them in nonce order.
 */
async function readNonce(source: Keypair): Promise<bigint> {
  const sourceAccount = await server.getAccount(source.publicKey());
  const tx = new TransactionBuilder(sourceAccount, {
    fee: "100",
    networkPassphrase,
  })
    .addOperation(account.call("nonce"))
    .setTimeout(30)
    .build();

  const simulated = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(simulated)) {
    throw new Error(`nonce simulation failed: ${simulated.error}`);
  }
  const retval = (simulated as rpc.Api.SimulateTransactionSuccessResponse).result?.retval;
  if (!retval) throw new Error("nonce returned no value");
  return BigInt(scValToNative(retval) as string | number | bigint);
}

/** A `Scope` is a single method name the session key is allowed to invoke. */
function scope(method: string): xdr.ScVal {
  return xdr.ScVal.scvMap([
    new xdr.ScMapEntry({
      key: nativeToScVal("method", { type: "symbol" }),
      val: nativeToScVal(method, { type: "symbol" }),
    }),
  ]);
}

// === Flow

/**
 * Owner grants a session key the right to call `methods` on any target until
 * `expiresAt` (a Unix timestamp in seconds). A key registered with an empty
 * scope list is rejected at execution time, so always grant at least one.
 */
async function registerSessionKey(
  owner: Keypair,
  sessionKey: Address,
  expiresAt: number,
  methods: string[]
): Promise<void> {
  await invoke(
    "register_session_key",
    [
      nativeToScVal(sessionKey.toString(), { type: "address" }),
      nativeToScVal(expiresAt, { type: "u64" }),
      xdr.ScVal.scvVec(methods.map(scope)),
    ],
    [owner]
  );
  console.log(`Registered session key ${sessionKey.toString()} for [${methods.join(", ")}]`);
}

/** The dApp calls the target directly with the session key. */
async function executeWithSession(
  sessionKey: Keypair,
  target: Address,
  method: string,
  args: xdr.ScVal[],
  nonce: bigint
): Promise<unknown> {
  return invoke(
    "execute_with_session",
    [
      nativeToScVal(sessionKey.publicKey(), { type: "address" }),
      nativeToScVal(target.toString(), { type: "address" }),
      nativeToScVal(method, { type: "symbol" }),
      xdr.ScVal.scvVec(args),
      nativeToScVal(nonce, { type: "u64" }),
    ],
    [sessionKey]
  );
}

/** Owner allowlists a relayer so it may submit and pay for session calls. */
async function allowRelayer(owner: Keypair, relayer: Address): Promise<void> {
  await invoke(
    "set_sponsor",
    [
      nativeToScVal(relayer.toString(), { type: "address" }),
      nativeToScVal(true, { type: "bool" }),
    ],
    [owner]
  );
  console.log(`Relayer ${relayer.toString()} is now allowed to sponsor session calls`);
}

/**
 * The relayer submits the call and pays the fee; the session key still
 * authorizes it. Sponsorship never widens what a session key may do.
 */
async function executeSponsored(
  relayer: Keypair,
  sessionKey: Keypair,
  target: Address,
  method: string,
  args: xdr.ScVal[],
  nonce: bigint
): Promise<unknown> {
  return invoke(
    "execute_with_session_sponsored",
    [
      nativeToScVal(sessionKey.publicKey(), { type: "address" }),
      nativeToScVal(relayer.publicKey(), { type: "address" }),
      nativeToScVal(target.toString(), { type: "address" }),
      nativeToScVal(method, { type: "symbol" }),
      xdr.ScVal.scvVec(args),
      nativeToScVal(nonce, { type: "u64" }),
    ],
    [relayer, sessionKey]
  );
}

/** Owner revokes the key; every later call with it fails closed. */
async function revokeSessionKey(owner: Keypair, sessionKey: Address): Promise<void> {
  await invoke(
    "revoke_session_key",
    [nativeToScVal(sessionKey.toString(), { type: "address" })],
    [owner]
  );
  console.log(`Revoked session key ${sessionKey.toString()}`);
}

async function main(): Promise<void> {
  const owner = Keypair.fromSecret(process.env.OWNER_SECRET_KEY!);
  const sessionKey = Keypair.fromSecret(process.env.SESSION_SECRET_KEY!);
  const target = Address.fromString(TARGET_CONTRACT);

  // One hour of delegated authority, scoped to a single method.
  const expiresAt = Math.floor(Date.now() / 1000) + 3600;
  await registerSessionKey(owner, Address.fromString(sessionKey.publicKey()), expiresAt, ["pay"]);

  const direct = await executeWithSession(
    sessionKey,
    target,
    "pay",
    [],
    await readNonce(sessionKey)
  );
  console.log("Direct session call returned:", direct);

  if (process.env.RELAYER_SECRET_KEY) {
    const relayer = Keypair.fromSecret(process.env.RELAYER_SECRET_KEY);
    await allowRelayer(owner, Address.fromString(relayer.publicKey()));
    const sponsored = await executeSponsored(
      relayer,
      sessionKey,
      target,
      "pay",
      [],
      await readNonce(relayer)
    );
    console.log("Sponsored session call returned:", sponsored);
  }

  await revokeSessionKey(owner, Address.fromString(sessionKey.publicKey()));
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error);
    process.exit(1);
  });
}

export {
  readNonce,
  registerSessionKey,
  executeWithSession,
  allowRelayer,
  executeSponsored,
  revokeSessionKey,
};
