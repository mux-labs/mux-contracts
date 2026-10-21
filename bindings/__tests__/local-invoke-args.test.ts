import { nativeToScVal, scValToNative } from "@stellar/stellar-sdk";
import {
  buildLocalInvokeArgs,
  parseExplicitScVal,
  parseRawArgument,
} from "../src/local-invoke";

/**
 * Regression suite for #935: explicit i64/u64 arguments must preserve the
 * full 64-bit range (BigInt conversion, no Number() precision loss), reject
 * out-of-range values with a descriptive error, and u64 must be expressible
 * via the bare-string `u64:<decimal>` form.
 */
describe("parseExplicitScVal — 64-bit integer fidelity (#935)", () => {
  it("preserves the i64 maximum exactly", () => {
    const scVal = parseExplicitScVal("i64", "9223372036854775807");
    expect(scVal.toXDR("base64")).toBe(
      nativeToScVal(9223372036854775807n, { type: "i64" }).toXDR("base64")
    );
    expect(BigInt(scValToNative(scVal) as bigint)).toBe(9223372036854775807n);
  });

  it("preserves the i64 minimum exactly", () => {
    const scVal = parseExplicitScVal("i64", "-9223372036854775808");
    expect(scVal.toXDR("base64")).toBe(
      nativeToScVal(-9223372036854775808n, { type: "i64" }).toXDR("base64")
    );
    expect(BigInt(scValToNative(scVal) as bigint)).toBe(-9223372036854775808n);
  });

  it("preserves the u64 maximum exactly", () => {
    const scVal = parseExplicitScVal("u64", "18446744073709551615");
    expect(scVal.toXDR("base64")).toBe(
      nativeToScVal(18446744073709551615n, { type: "u64" }).toXDR("base64")
    );
    expect(BigInt(scValToNative(scVal) as bigint)).toBe(18446744073709551615n);
  });

  it("preserves values just above the JS safe-integer range", () => {
    const scVal = parseExplicitScVal("i64", "9007199254740993");
    expect(BigInt(scValToNative(scVal) as bigint)).toBe(9007199254740993n);

    const uScVal = parseExplicitScVal("u64", "9007199254740993");
    expect(BigInt(scValToNative(uScVal) as bigint)).toBe(9007199254740993n);
  });

  it("accepts safe small values for i64 and u64", () => {
    expect(BigInt(scValToNative(parseExplicitScVal("i64", "42")) as bigint)).toBe(42n);
    expect(BigInt(scValToNative(parseExplicitScVal("u64", "0")) as bigint)).toBe(0n);
  });

  it("rejects i64 values above the signed 64-bit maximum", () => {
    expect(() => parseExplicitScVal("i64", "9223372036854775808")).toThrow(
      /i64 value out of range: 9223372036854775808/
    );
  });

  it("rejects i64 values below the signed 64-bit minimum", () => {
    expect(() => parseExplicitScVal("i64", "-9223372036854775809")).toThrow(
      /i64 value out of range: -9223372036854775809/
    );
  });

  it("rejects negative u64 values", () => {
    expect(() => parseExplicitScVal("u64", "-1")).toThrow(
      /u64 value out of range: -1/
    );
  });

  it("rejects u64 values above the unsigned 64-bit maximum", () => {
    expect(() => parseExplicitScVal("u64", "18446744073709551616")).toThrow(
      /u64 value out of range: 18446744073709551616/
    );
  });

  it("rejects non-integer i64/u64 payloads with a descriptive error", () => {
    expect(() => parseExplicitScVal("i64", "not-a-number")).toThrow(
      /Invalid i64 argument value: not-a-number/
    );
    expect(() => parseExplicitScVal("u64", "1.5")).toThrow(
      /Invalid u64 argument value: 1.5/
    );
  });
});

describe("parseRawArgument — explicit u64 form (#935)", () => {
  it("expresses u64 via the u64: prefix without precision loss", () => {
    const scVal = parseRawArgument("u64:18446744073709551615");
    expect(scVal.toXDR("base64")).toBe(
      nativeToScVal(18446744073709551615n, { type: "u64" }).toXDR("base64")
    );
    expect(BigInt(scValToNative(scVal) as bigint)).toBe(18446744073709551615n);
  });

  it("expresses u64:0 correctly", () => {
    const scVal = parseRawArgument("u64:0");
    expect(BigInt(scValToNative(scVal) as bigint)).toBe(0n);
  });

  it("rejects out-of-range u64: prefix values", () => {
    expect(() => parseRawArgument("u64:18446744073709551616")).toThrow(
      /u64 value out of range: 18446744073709551616/
    );
    expect(() => parseRawArgument("u64:-1")).toThrow(
      /Invalid u64 argument value: -1/
    );
    expect(() => parseRawArgument("u64:abc")).toThrow(
      /Invalid u64 argument value: abc/
    );
  });

  it("keeps bare-numeric strings on the i64/i128 path", () => {
    const small = parseRawArgument("42");
    expect(small.toXDR("base64")).toBe(
      nativeToScVal(42, { type: "i64" }).toXDR("base64")
    );

    // 9223372036854775807 is exactly i64::MAX, so it must classify as i64
    // (see #987 regression suite below) — only values beyond the real i64
    // range should ever be promoted to i128.
    const atI64Max = parseRawArgument("9223372036854775807");
    expect(atI64Max.toXDR("base64")).toBe(
      nativeToScVal(9223372036854775807n, { type: "i64" }).toXDR("base64")
    );

    const large = parseRawArgument("9223372036854775808");
    expect(large.toXDR("base64")).toBe(
      nativeToScVal(9223372036854775808n, { type: "i128" }).toXDR("base64")
    );
  });

  it("round-trips explicit JSON i64/u64 objects end to end", () => {
    const i64Val = parseRawArgument('{"type":"i64","value":"9223372036854775807"}');
    expect(BigInt(scValToNative(i64Val) as bigint)).toBe(9223372036854775807n);

    const u64Val = parseRawArgument('{"type":"u64","value":"18446744073709551615"}');
    expect(BigInt(scValToNative(u64Val) as bigint)).toBe(18446744073709551615n);

    expect(() => parseRawArgument('{"type":"u64","value":"18446744073709551616"}')).toThrow(
      /u64 value out of range: 18446744073709551616/
    );
  });

  it("builds args lists containing u64: prefixed values", () => {
    const args = buildLocalInvokeArgs(["u64:18446744073709551615", "true"]);
    expect(args).toHaveLength(2);
    expect(BigInt(scValToNative(args[0]) as bigint)).toBe(18446744073709551615n);
  });
});
