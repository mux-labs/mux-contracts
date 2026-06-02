import { nativeToScVal, xdr } from "@stellar/stellar-sdk";
import {
  buildLocalInvokeArgs,
  parseRawArgument,
  resolveContractId,
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
});
