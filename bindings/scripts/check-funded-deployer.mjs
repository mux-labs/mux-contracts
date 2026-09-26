import { Horizon, Keypair } from "@stellar/stellar-sdk";

const args = new Set(process.argv.slice(2));
const dryRun = args.has("--dry-run");
const minimumBalance = Number(process.env.MIN_DEPLOYER_BALANCE_XLM ?? "2");
const horizonUrl = process.env.HORIZON_URL ?? "https://horizon-testnet.stellar.org";
const secret = process.env.DEPLOYER_PRIVATE_KEY ?? "";

function fail(message) {
  console.error(`funded-deployer-check: ${message}`);
  process.exitCode = 1;
}

if (!Number.isFinite(minimumBalance) || minimumBalance < 0) {
  fail("MIN_DEPLOYER_BALANCE_XLM must be a non-negative number");
} else if (dryRun) {
  console.log(`funded-deployer-check: dry run (Horizon ${horizonUrl})`);
  console.log(`funded-deployer-check: minimum native balance ${minimumBalance} XLM`);
} else if (!secret) {
  fail("DEPLOYER_PRIVATE_KEY is required outside --dry-run mode");
} else {
  try {
    const keypair = Keypair.fromSecret(secret);
    const publicKey = keypair.publicKey();
    const server = new Horizon.Server(horizonUrl);
    const account = await server.loadAccount(publicKey);
    const native = account.balances.find(
      (balance) => balance.asset_type === "native",
    );
    const balance = Number(native?.balance ?? "0");
    if (!Number.isFinite(balance) || balance < minimumBalance) {
      fail(`deployer ${publicKey} has ${balance} XLM; required at least ${minimumBalance} XLM`);
    } else {
      console.log(`funded-deployer-check: ${publicKey} has ${balance} XLM`);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : "unknown Horizon error";
    fail(`unable to derive or verify deployer account: ${message}`);
  }
}
