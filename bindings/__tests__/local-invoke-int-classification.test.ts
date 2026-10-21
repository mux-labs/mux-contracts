import { nativeToScVal, scValToNative } from "@stellar/stellar-sdk";
import { parseRawArgument } from "../src/local-invoke";

/**
 * Regression suite for #987: `parseRawArgument` must classify bare integer
 * strings against the real i64 range (not Number.MAX_SAFE_INTEGER), and must
 * not let JSON.parse round large JSON numbers through an IEEE-754 double
 * before they reach the BigInt-based parsing path.
 */
describe("parseRawArgument — integer classification (#987)", () => {
  it("classifies values within the real i64 range as i64, even above Number.MAX_SAFE_INTEGER", () => {
    // 9007199254740993 is MAX_SAFE_INTEGER + 2, well within i64 range, but
    // beyond the old (incorrect) Number.MAX_SAFE_INTEGER cutoff.
    const scVal = parseRawArgument("9007199254740993");
    expect(scVal.toXDR("base64")).toBe(
      nativeToScVal(9007199254740993n, { type: "i64" }).toXDR("base64")
    );
  });

  it("classifies the exact i64 boundary values as i64", () => {
    const max = parseRawArgument("9223372036854775807");
    expect(max.toXDR("base64")).toBe(
      nativeToScVal(9223372036854775807n, { type: "i64" }).toXDR("base64")
    );

    const min = parseRawArgument("-9223372036854775808");
    expect(min.toXDR("base64")).toBe(
      nativeToScVal(-9223372036854775808n, { type: "i64" }).toXDR("base64")
    );
  });

  it("only promotes to i128 once the value exceeds the i64 range", () => {
    const scVal = parseRawArgument("9223372036854775808");
    expect(scVal.toXDR("base64")).toBe(
      nativeToScVal(9223372036854775808n, { type: "i128" }).toXDR("base64")
    );

    const negScVal = parseRawArgument("-9223372036854775809");
    expect(negScVal.toXDR("base64")).toBe(
      nativeToScVal(-9223372036854775809n, { type: "i128" }).toXDR("base64")
    );
  });

  it("rejects values beyond the i128 range with a descriptive error", () => {
    const tooBig = (2n ** 127n).toString();
    expect(() => parseRawArgument(tooBig)).toThrow(/out of i128 range/);
  });

  it("parses a large bare JSON number without IEEE-754 precision loss", () => {
    // Beyond Number.MAX_SAFE_INTEGER; JSON.parse's default number handling
    // would silently round this before it ever reaches BigInt parsing.
    const scVal = parseRawArgument("9007199254740993");
    expect(BigInt(scValToNative(scVal) as bigint)).toBe(9007199254740993n);
  });

  it("parses a large number nested in a JSON array without precision loss", () => {
    const scVal = parseRawArgument("[9223372036854775807]");
    const native = scValToNative(scVal) as unknown[];
    expect(BigInt(native[0] as bigint)).toBe(9223372036854775807n);
  });

  it("leaves small JSON numbers and floats working as before", () => {
    const scVal = parseRawArgument("[1, 2, 3]");
    const native = scValToNative(scVal) as unknown[];
    expect(native.map((v) => BigInt(v as bigint))).toEqual([1n, 2n, 3n]);
  });

  it("does not mangle large numeric literals that are already inside quoted strings", () => {
    const scVal = parseRawArgument('{"type":"string","value":"id-9223372036854775807-x"}');
    expect(scValToNative(scVal)).toBe("id-9223372036854775807-x");
  });
});
