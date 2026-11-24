import fs from "node:fs";
import { Address, Keypair, Networks, nativeToScVal, xdr } from "@stellar/stellar-sdk";
import { MuxBatcherClient } from "./generated/mux-batcher";
import type { Operation } from "./types";

type JsonArg = { type: string; value: unknown };
type InputOperation = {
  target: string;
  fnName: string;
  args?: JsonArg[];
  requireSuccess?: boolean;
  kind?: Operation["kind"];
};

type BatchInput = { caller?: string; operations: InputOperation[] };

const PASSPHRASES: Record<string, string> = {
  localnet: "Standalone Network ; February 2017",
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

function usage(): never {
  console.error(`Usage: npm run simulate-batch -- --input batch.json\n\nEnvironment:\n  SECRET_KEY          signer used only to build and simulate the transaction\n  BATCHER_CONTRACT_ID deployed mux-batcher contract ID\n  RPC_URL             Soroban RPC URL (default: testnet)\n  SOROBAN_NETWORK     localnet|testnet|mainnet (default: testnet)`);
  process.exit(2);
}

function option(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function toScVal(arg: JsonArg): xdr.ScVal {
  if (!arg || typeof arg.type !== "string") throw new Error("Each argument needs a type and value");
  switch (arg.type) {
    case "address":
      return nativeToScVal(Address.fromString(String(arg.value)).toString(), { type: "address" });
    case "string":
      return nativeToScVal(String(arg.value), { type: "string" });
    case "bool":
      return nativeToScVal(Boolean(arg.value), { type: "bool" });
    case "u32":
      return xdr.ScVal.scvU32(Number(arg.value));
    case "u64":
      return nativeToScVal(BigInt(String(arg.value)), { type: "u64" });
    case "i128":
      return nativeToScVal(BigInt(String(arg.value)), { type: "i128" });
    default:
      throw new Error(`Unsupported argument type: ${arg.type}`);
  }
}

function readInput(path: string): BatchInput {
  const input = JSON.parse(fs.readFileSync(path, "utf8")) as BatchInput;
  if (!input || !Array.isArray(input.operations) || input.operations.length === 0) {
    throw new Error("Input must contain a non-empty operations array");
  }
  return input;
}

async function main(): Promise<void> {
  const inputPath = option("--input");
  const secret = process.env.SECRET_KEY;
  const contractId = process.env.BATCHER_CONTRACT_ID;
  const network = process.env.SOROBAN_NETWORK ?? "testnet";
  const rpcUrl = process.env.RPC_URL ?? (network === "localnet" ? "http://localhost:8000" : "https://soroban-testnet.stellar.org");
  if (!inputPath || !secret || !contractId || !PASSPHRASES[network]) usage();

  const signer = Keypair.fromSecret(secret);
  const input = readInput(inputPath);
  const caller = Address.fromString(input.caller ?? signer.publicKey());
  const operations: Operation[] = input.operations.map((operation) => ({
    target: Address.fromString(operation.target),
    fnName: operation.fnName,
    args: (operation.args ?? []).map(toScVal),
    requireSuccess: operation.requireSuccess ?? true,
    kind: operation.kind ?? "Invoke",
  }));

  const client = new MuxBatcherClient({ contractId, networkPassphrase: PASSPHRASES[network], rpcUrl });
  const result = await client.simulateBatch(signer, caller, operations);
  console.log(JSON.stringify({ simulated: true, submitted: false, successCount: result.successCount, failureCount: result.failureCount }, null, 2));
}

main().catch((error: unknown) => {
  console.error(`Simulation failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});
