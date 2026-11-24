import { nativeToScVal, xdr } from "@stellar/stellar-sdk";
import {
  buildLocalInvokeArgs,
  parseRawArgument,
  resolveContractId,
  type NamedContract,
} from "../src/local-invoke";

describe("Local invoke helpers", () => {
  beforeEach(() => {
    delete process.env.LOCALNET_MUX_ACCOUNT_ID;
  });

  it("parses boolean arguments correctly", () => {
    const parsed = parseRawArgument("true");
    const expected = nativeToScVal(true, { type: "bool" });
    expect(parsed.toXDR("base64")).toBe(expected.toXDR("base64"));
  });

  it("parses integer arguments correctly", () => {
    const parsed = parseRawArgument("42");
    const expected = nativeToScVal(42, { type: "i64" });
    expect(parsed.toXDR("base64")).toBe(expected.toXDR("base64"));
  });

  it("parses address arguments as address types", () => {
    const address = "GATPLJWD4WKPGXT5FVVHO6RXYIBUE6RUHBOBGLAWVE4WDMTBX23EL54Q";
    const parsed = parseRawArgument(address);
    const expected = nativeToScVal(address, { type: "address" });
    expect(parsed.toXDR("base64")).toBe(expected.toXDR("base64"));
  });

  it("parses JSON explicit typed arguments", () => {
    const raw = '{"type":"symbol","value":"TEST"}';
    const parsed = parseRawArgument(raw);
    const expected = nativeToScVal("TEST", { type: "symbol" });
    expect(parsed.toXDR("base64")).toBe(expected.toXDR("base64"));
  });

  it("parses repeated args into ScVals", () => {
    const args = buildLocalInvokeArgs(["true", "12", "hello"]);
    expect(args).toHaveLength(3);
    expect(args[0].toXDR("base64")).toBe(nativeToScVal(true, { type: "bool" }).toXDR("base64"));
    expect(args[1].toXDR("base64")).toBe(nativeToScVal(12, { type: "i64" }).toXDR("base64"));
    expect(args[2].toXDR("base64")).toBe(nativeToScVal("hello", { type: "string" }).toXDR("base64"));
  });

  it("resolves a configured contract ID from environment for localnet", () => {
    const contractId = "C123456789012345678901234567890123456789012345678901234567";
    process.env.LOCALNET_MUX_ACCOUNT_ID = contractId;
    const resolved = resolveContractId("mux-account", "localnet");
    expect(resolved).toBe(contractId);
  });

  it("throws when an unsupported explicit type is provided", () => {
    expect(() => parseRawArgument('{"type":"unknown","value":"x"}')).toThrow(
      /Unsupported explicit contract argument type/i
    );
  });

  const NAMED_CONTRACTS: NamedContract[] = [
    "mux-account",
    "mux-account-factory",
    "mux-batcher",
    "mux-delegation",
    "mux-policy",
    "mux-permissions",
    "mux-recovery",
    "mux-registry",
    "mux-spending-policy",
    "mux-wallet-registry",
  ];

  function envKey(contractName: NamedContract): string {
    return `LOCALNET_${contractName.toUpperCase().replace(/-/g, "_")}_ID`;
  }

  it("NamedContract covers all ten contract crates", () => {
    expect(NAMED_CONTRACTS).toHaveLength(10);
    expect(new Set(NAMED_CONTRACTS).size).toBe(10);
  });

  it("resolves every named contract to its configured localnet address", () => {
    const addresses: Record<string, string> = {};
    for (const [index, name] of NAMED_CONTRACTS.entries()) {
      addresses[name] = `C${String(index).padStart(55, "A")}`;
      process.env[envKey(name)] = addresses[name];
    }
    try {
      for (const name of NAMED_CONTRACTS) {
        expect(resolveContractId(name, "localnet")).toBe(addresses[name]);
      }
    } finally {
      for (const name of NAMED_CONTRACTS) {
        delete process.env[envKey(name)];
      }
    }
  });

  it("throws the descriptive not-configured error when a name's address is absent", () => {
    for (const name of NAMED_CONTRACTS) {
      const previous = process.env[envKey(name)];
      delete process.env[envKey(name)];
      try {
        expect(() => resolveContractId(name, "localnet")).toThrow(
          new RegExp(
            `Contract address for ${name} is not configured on network localnet`,
            "i"
          )
        );
      } finally {
        if (previous !== undefined) {
          process.env[envKey(name)] = previous;
        }
      }
    }
  });
});
