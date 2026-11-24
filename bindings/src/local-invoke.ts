import {
  Contract,
  Keypair,
  nativeToScVal,
  SorobanRpc,
  Transaction,
  TransactionBuilder,
  xdr,
} from "@stellar/stellar-sdk";
import { NETWORK_CONFIGS } from "./network";
import { loadContractAddresses } from "./addresses";
import { DEFAULT_ADDRESSES } from "./addresses-config";
import { pollTransaction } from "./horizon";

export interface LocalInvokeOptions {
  rpcUrl: string;
  networkPassphrase: string;
  contractId: string;
  functionName: string;
  signerSecret: string;
  args?: string[];
  allowHttp?: boolean;
  simulateOnly?: boolean;
}

export type NamedContract =
  | "mux-account"
  | "mux-account-factory"
  | "mux-batcher"
  | "mux-delegation"
  | "mux-policy"
  | "mux-permissions"
  | "mux-recovery"
  | "mux-registry"
  | "mux-spending-policy"
  | "mux-wallet-registry";

export function resolveContractId(
  contractName: NamedContract,
  network: string
): string {
  const supported: Record<NamedContract, keyof import("./types").MuxContractIds> = {
    "mux-account": "muxAccount",
    "mux-account-factory": "muxAccountFactory",
    "mux-batcher": "muxBatcher",
    "mux-delegation": "muxDelegation",
    "mux-policy": "muxPolicy",
    "mux-permissions": "muxPermissions",
    "mux-recovery": "muxRecovery",
    "mux-registry": "muxRegistry",
    "mux-spending-policy": "muxSpendingPolicy",
    "mux-wallet-registry": "muxWalletRegistry",
  };

  const contractKey = supported[contractName];
  const addresses = loadContractAddresses(network, DEFAULT_ADDRESSES);
  const contractId = (addresses as unknown as Record<string, string | undefined>)[contractKey];
  if (!contractId) {
    throw new Error(
      `Contract address for ${contractName} is not configured on network ${network}. ` +
        `Set ${network.toUpperCase()}_${contractKey.toUpperCase()}_ID or update config/addresses.json.`
    );
  }

  return contractId;
}

function parseJsonValue(value: unknown): xdr.ScVal {
  if (value === null) {
    throw new Error("null is not a supported contract argument type");
  }

  if (Array.isArray(value)) {
    return xdr.ScVal.scvVec(value.map(parseJsonValue));
  }

  if (typeof value === "object") {
    const typed = value as Record<string, unknown>;
    if ("type" in typed && "value" in typed) {
      return parseExplicitScVal(typed.type as string, typed.value);
    }

    throw new Error(
      `Unsupported JSON argument shape: ${JSON.stringify(value)}. ` +
        `Use a primitive or an object with { type, value }.`
    );
  }

  if (typeof value === "boolean") {
    return nativeToScVal(value, { type: "bool" });
  }

  if (typeof value === "number") {
    return parseBigIntScVal("i64", value);
  }

  if (typeof value === "string") {
    return parseRawArgument(value);
  }

  throw new Error(`Unsupported argument type: ${typeof value}`);
}

/** Inclusive bounds for signed/unsigned 64-bit contract arguments. */
const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;
const U64_MAX = 2n ** 64n - 1n;
/** Inclusive bounds for the signed 128-bit contract argument type. */
const I128_MIN = -(2n ** 127n);
const I128_MAX = 2n ** 127n - 1n;

/**
 * Quote unquoted JSON integer literals with more than 15 digits so that
 * `JSON.parse` never has a chance to round them through an IEEE-754 double.
 * Strings, floats/exponents, and numbers already inside string literals are
 * left untouched. This lets {@link parseRawArgument}'s own BigInt-based
 * decimal parsing (which has full 64/128-bit precision) handle the value
 * once JSON.parse hands it back as a string.
 */
function quoteBigIntegers(jsonText: string): string {
  let result = "";
  let inString = false;
  let escape = false;

  for (let i = 0; i < jsonText.length; i += 1) {
    const ch = jsonText[i];

    if (inString) {
      result += ch;
      if (escape) {
        escape = false;
      } else if (ch === "\\") {
        escape = true;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }

    if (ch === '"') {
      inString = true;
      result += ch;
      continue;
    }

    if (ch === "-" || (ch >= "0" && ch <= "9")) {
      let j = i;
      let numStr = "";
      if (jsonText[j] === "-") {
        numStr += "-";
        j += 1;
      }
      while (j < jsonText.length && jsonText[j] >= "0" && jsonText[j] <= "9") {
        numStr += jsonText[j];
        j += 1;
      }

      // Floats/exponents are copied through unchanged; precision loss there
      // is an existing, separate concern from integer misclassification.
      if (jsonText[j] === "." || jsonText[j] === "e" || jsonText[j] === "E") {
        let full = numStr;
        while (j < jsonText.length && /[0-9+\-.eE]/.test(jsonText[j])) {
          full += jsonText[j];
          j += 1;
        }
        result += full;
        i = j - 1;
        continue;
      }

      const digits = numStr.startsWith("-") ? numStr.slice(1) : numStr;
      result += digits.length > 15 ? `"${numStr}"` : numStr;
      i = j - 1;
      continue;
    }

    result += ch;
  }

  return result;
}

/**
 * Convert a raw value to a 64-bit ScVal via BigInt, rejecting values outside
 * the signed (`i64`) or unsigned (`u64`) 64-bit range with a descriptive error.
 * BigInt conversion preserves the full 64-bit range; Number() would silently
 * lose precision above 2^53.
 */
function parseBigIntScVal(type: "i64" | "u64", value: unknown): xdr.ScVal {
  let parsed: bigint;
  try {
    parsed = BigInt(String(value));
  } catch {
    throw new Error(`Invalid ${type} argument value: ${String(value)}`);
  }

  const min = type === "i64" ? I64_MIN : 0n;
  const max = type === "i64" ? I64_MAX : U64_MAX;
  if (parsed < min || parsed > max) {
    throw new Error(
      `${type} value out of range: ${String(value)} (valid range ${min} to ${max})`
    );
  }

  return nativeToScVal(parsed, { type });
}

export function parseExplicitScVal(type: string, value: unknown): xdr.ScVal {
  switch (type) {
    case "address":
      return nativeToScVal(String(value), { type: "address" });
    case "symbol":
      return nativeToScVal(String(value), { type: "symbol" });
    case "string":
      return nativeToScVal(String(value), { type: "string" });
    case "bool":
      return nativeToScVal(Boolean(value), { type: "bool" });
    case "u32":
      return nativeToScVal(Number(value), { type: "u32" });
    case "u64":
      return parseBigIntScVal("u64", value);
    case "i64":
      return parseBigIntScVal("i64", value);
    case "i128":
      return nativeToScVal(BigInt(String(value)), { type: "i128" });
    case "u128":
      return nativeToScVal(BigInt(String(value)), { type: "u128" });
    case "bytes":
      return nativeToScVal(String(value), { type: "bytes" });
    case "vec": {
      if (!Array.isArray(value)) {
        throw new Error(`The value for vec must be an array: ${JSON.stringify(value)}`);
      }
      return xdr.ScVal.scvVec(value.map(parseJsonValue));
    }
    default:
      throw new Error(`Unsupported explicit contract argument type: ${type}`);
  }
}

export function parseRawArgument(raw: string): xdr.ScVal {
  const trimmed = raw.trim();

  if (trimmed === "true" || trimmed === "false") {
    return nativeToScVal(trimmed === "true", { type: "bool" });
  }

  // Explicit unsigned 64-bit form: "u64:<decimal>". The bare-numeric path
  // below only ever emits i64/i128, so u64 arguments require this prefix
  // (or the explicit JSON {"type":"u64",...} object).
  if (trimmed.startsWith("u64:")) {
    const payload = trimmed.slice("u64:".length);
    if (!/^\d+$/.test(payload)) {
      throw new Error(`Invalid u64 argument value: ${payload}`);
    }
    return parseBigIntScVal("u64", payload);
  }

  if (/^-?\d+$/.test(trimmed)) {
    const value = BigInt(trimmed);
    // The full i64 range (~9.2e18) is far larger than Number.MAX_SAFE_INTEGER
    // (~9e15); classifying against the safe-integer bound reclassified
    // perfectly valid i64 values as i128. Classify against the real i64
    // bounds instead, and reject anything that doesn't even fit in i128.
    if (value >= I64_MIN && value <= I64_MAX) {
      return nativeToScVal(value, { type: "i64" });
    }
    if (value >= I128_MIN && value <= I128_MAX) {
      return nativeToScVal(value, { type: "i128" });
    }
    throw new Error(
      `Integer argument out of i128 range: ${trimmed} (valid range ${I128_MIN} to ${I128_MAX})`
    );
  }

  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(quoteBigIntegers(trimmed));
    } catch {
      return nativeToScVal(trimmed, { type: "string" });
    }
    return parseJsonValue(parsed);
  }

  if (/^[GC][A-Z2-7]{55}$/.test(trimmed)) {
    return nativeToScVal(trimmed, { type: "address" });
  }

  return nativeToScVal(trimmed, { type: "string" });
}

export function buildLocalInvokeArgs(rawArgs?: readonly string[]): xdr.ScVal[] {
  if (!rawArgs || rawArgs.length === 0) {
    return [];
  }

  return rawArgs.map(parseRawArgument);
}

export async function buildLocalInvokeTransaction(
  options: LocalInvokeOptions
): Promise<Transaction> {
  const server = new SorobanRpc.Server(options.rpcUrl, {
    allowHttp: options.allowHttp ?? true,
  });

  const signer = Keypair.fromSecret(options.signerSecret);
  const account = await server.getAccount(signer.publicKey());
  const contract = new Contract(options.contractId);
  const args = buildLocalInvokeArgs(options.args);

  return new TransactionBuilder(account, {
    fee: "100",
    networkPassphrase: options.networkPassphrase,
  })
    .addOperation(contract.call(options.functionName, ...args))
    .setTimeout(30)
    .build();
}

export async function localInvoke(
  options: LocalInvokeOptions
): Promise<
  | SorobanRpc.Api.GetSuccessfulTransactionResponse
  | SorobanRpc.Api.SimulateTransactionSuccessResponse
> {
  const server = new SorobanRpc.Server(options.rpcUrl, {
    allowHttp: options.allowHttp ?? true,
  });
  const signer = Keypair.fromSecret(options.signerSecret);
  const transaction = await buildLocalInvokeTransaction(options);

  const simulateResult = await server.simulateTransaction(transaction);
  if (SorobanRpc.Api.isSimulationError(simulateResult)) {
    throw new Error(`Simulation failed: ${simulateResult.error}`);
  }

  if (options.simulateOnly) {
    return simulateResult as SorobanRpc.Api.SimulateTransactionSuccessResponse;
  }

  const assembled = SorobanRpc.assembleTransaction(
    transaction,
    simulateResult as SorobanRpc.Api.SimulateTransactionSuccessResponse
  ).build();

  assembled.sign(signer);
  const sendResult = await server.sendTransaction(assembled);

  if (sendResult.status === "ERROR") {
    throw new Error(`Transaction submission failed: ${JSON.stringify(sendResult.errorResult)}`);
  }

  return await pollTransaction(server, sendResult.hash);
}

interface LocalInvokeCliOptions {
  network: string;
  rpcUrl?: string;
  contractId?: string;
  contractName?: NamedContract;
  functionName?: string;
  signerSecret?: string;
  args: string[];
  simulateOnly: boolean;
}

function formatHelp(): string {
  return `Usage: node dist/local-invoke.js [options]

Options:
  --network <network>           Use SOROBAN_NETWORK (localnet|testnet|mainnet). Default: localnet
  --rpc-url <url>               Override the Soroban RPC endpoint
  --contract-id <contractId>    Explicit contract ID to invoke
  --contract-name <name>        Named contract (mux-account|mux-account-factory|mux-batcher|mux-policy|mux-permissions|mux-wallet-registry)
  --function <name>             Contract function name to invoke
  --secret-key <secret>         Signer secret key for the transaction
  --arg <value>                 Argument for the contract function (repeatable)
  --simulate-only               Simulate the transaction without submitting
  --help                        Show this help message

Examples:
  node dist/local-invoke.js --contract-name mux-account --function owner --secret-key S... --arg true
  node dist/local-invoke.js --contract-id C... --function initialize --secret-key S... --arg '{"type":"address","value":"G..."}'
`;
}

function parseCliArgs(argv: string[]): LocalInvokeCliOptions {
  const options: LocalInvokeCliOptions = {
    network: process.env.SOROBAN_NETWORK || "localnet",
    args: [],
    simulateOnly: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--network":
        options.network = argv[++index];
        break;
      case "--rpc-url":
        options.rpcUrl = argv[++index];
        break;
      case "--contract-id":
        options.contractId = argv[++index];
        break;
      case "--contract-name":
        options.contractName = argv[++index] as NamedContract;
        break;
      case "--function":
        options.functionName = argv[++index];
        break;
      case "--secret-key":
        options.signerSecret = argv[++index];
        break;
      case "--arg":
        options.args.push(argv[++index]);
        break;
      case "--simulate-only":
        options.simulateOnly = true;
        break;
      case "--help":
      case "-h":
        throw new Error("help");
      default:
        throw new Error(`Unknown option: ${arg}`);
    }
  }

  return options;
}

export async function runLocalInvokeCli(argv: string[]): Promise<number> {
  let options: LocalInvokeCliOptions;

  try {
    options = parseCliArgs(argv);
  } catch (err) {
    if (err instanceof Error && err.message === "help") {
      console.log(formatHelp());
      return 0;
    }
    console.error(`Error parsing command line: ${(err as Error).message}`);
    console.log(formatHelp());
    return 1;
  }

  const networkConfig = NETWORK_CONFIGS[options.network];
  if (!networkConfig) {
    console.error(
      `Unknown network: ${options.network}. Available: ${Object.keys(NETWORK_CONFIGS).join(", ")}`
    );
    return 1;
  }

  const rpcConfig = options.rpcUrl
    ? { rpcUrl: options.rpcUrl, networkPassphrase: networkConfig.networkPassphrase }
    : networkConfig;

  const contractId = options.contractId
    ? options.contractId
    : options.contractName
    ? resolveContractId(options.contractName, options.network)
    : undefined;

  if (!contractId) {
    console.error("Error: either --contract-id or --contract-name must be provided.");
    return 1;
  }

  if (!options.functionName) {
    console.error("Error: --function is required.");
    return 1;
  }

  if (!options.signerSecret) {
    console.error("Error: --secret-key is required.");
    return 1;
  }

  const localInvokeOptions: LocalInvokeOptions = {
    rpcUrl: options.rpcUrl || rpcConfig.rpcUrl,
    networkPassphrase: rpcConfig.networkPassphrase,
    contractId,
    functionName: options.functionName,
    signerSecret: options.signerSecret,
    args: options.args,
    allowHttp: options.rpcUrl ? true : options.network === "localnet",
    simulateOnly: options.simulateOnly,
  };

  try {
    if (options.simulateOnly) {
      console.log("Simulating contract invocation...");
    } else {
      console.log("Submitting contract invocation...");
    }
    const result = await localInvoke(localInvokeOptions);
    console.log("Contract invocation completed.");
    console.log(JSON.stringify(result, null, 2));
    return 0;
  } catch (error) {
    console.error("Contract invocation failed:", error);
    return 1;
  }
}

if (require.main === module) {
  runLocalInvokeCli(process.argv.slice(2)).then((code) => process.exit(code));
}
