/**
 * Guard coverage for the bindings-layer error code catalogs in errors.ts:
 * CROSS_NETWORK_ERROR_CODES and BATCHING_CAP_ERROR_CODES.
 *
 * These are stable, typed error codes emitted by the bindings layer itself
 * (not by the contracts) for the cross-network invoke guard
 * (bindings/src/network.ts) and the on-chain batching DoS caps
 * (bindings/src/batcher.ts), as documented in the comments above each
 * constant in errors.ts. This suite locks in:
 *  - exact membership (no accidental additions/removals)
 *  - uniqueness of values (no two codes collapsing to the same string)
 *  - identity mapping (each code's key equals its value, per current design)
 *  - presence in ERROR_HTTP_MAP with the documented status codes
 */
import {
  CROSS_NETWORK_ERROR_CODES,
  BATCHING_CAP_ERROR_CODES,
  ERROR_HTTP_MAP,
  type CrossNetworkErrorCode,
  type BatchingCapErrorCode,
} from "../src/errors";

describe("CROSS_NETWORK_ERROR_CODES", () => {
  it("contains exactly the documented cross-network guard codes", () => {
    expect(Object.keys(CROSS_NETWORK_ERROR_CODES).sort()).toEqual(
      [
        "CrossNetworkInvokeBlocked",
        "NetworkConfigMissing",
        "NetworkConfigInvalid",
      ].sort(),
    );
  });

  it("maps every key to itself (stable string identity)", () => {
    for (const [key, value] of Object.entries(CROSS_NETWORK_ERROR_CODES)) {
      expect(value).toBe(key);
    }
  });

  it("has unique values", () => {
    const values = Object.values(CROSS_NETWORK_ERROR_CODES);
    expect(new Set(values).size).toBe(values.length);
  });

  it("every code resolves to its documented HTTP status", () => {
    expect(ERROR_HTTP_MAP[CROSS_NETWORK_ERROR_CODES.CrossNetworkInvokeBlocked]).toBe(403);
    expect(ERROR_HTTP_MAP[CROSS_NETWORK_ERROR_CODES.NetworkConfigMissing]).toBe(403);
    expect(ERROR_HTTP_MAP[CROSS_NETWORK_ERROR_CODES.NetworkConfigInvalid]).toBe(400);
  });

  it("every CrossNetworkErrorCode value is one of CROSS_NETWORK_ERROR_CODES' values", () => {
    const allowed = new Set<string>(Object.values(CROSS_NETWORK_ERROR_CODES));
    const sample: CrossNetworkErrorCode = CROSS_NETWORK_ERROR_CODES.NetworkConfigInvalid;
    expect(allowed.has(sample)).toBe(true);
  });
});

describe("BATCHING_CAP_ERROR_CODES", () => {
  it("contains exactly the documented batching cap codes", () => {
    expect(Object.keys(BATCHING_CAP_ERROR_CODES).sort()).toEqual(
      [
        "BatchTooLarge",
        "BatchAggregateTooLarge",
        "BatchEmpty",
        "BatchCapConfigMissing",
        "BatchCapConfigInvalid",
      ].sort(),
    );
  });

  it("maps every key to itself (stable string identity)", () => {
    for (const [key, value] of Object.entries(BATCHING_CAP_ERROR_CODES)) {
      expect(value).toBe(key);
    }
  });

  it("has unique values", () => {
    const values = Object.values(BATCHING_CAP_ERROR_CODES);
    expect(new Set(values).size).toBe(values.length);
  });

  it("every code resolves to its documented HTTP status", () => {
    expect(ERROR_HTTP_MAP[BATCHING_CAP_ERROR_CODES.BatchTooLarge]).toBe(400);
    expect(ERROR_HTTP_MAP[BATCHING_CAP_ERROR_CODES.BatchAggregateTooLarge]).toBe(400);
    expect(ERROR_HTTP_MAP[BATCHING_CAP_ERROR_CODES.BatchEmpty]).toBe(400);
    expect(ERROR_HTTP_MAP[BATCHING_CAP_ERROR_CODES.BatchCapConfigMissing]).toBe(403);
    expect(ERROR_HTTP_MAP[BATCHING_CAP_ERROR_CODES.BatchCapConfigInvalid]).toBe(400);
  });

  it("every BatchingCapErrorCode value is one of BATCHING_CAP_ERROR_CODES' values", () => {
    const allowed = new Set<string>(Object.values(BATCHING_CAP_ERROR_CODES));
    const sample: BatchingCapErrorCode = BATCHING_CAP_ERROR_CODES.BatchCapConfigMissing;
    expect(allowed.has(sample)).toBe(true);
  });
});

describe("CROSS_NETWORK_ERROR_CODES and BATCHING_CAP_ERROR_CODES do not collide", () => {
  it("share no overlapping error code string", () => {
    const crossNetwork = new Set(Object.values(CROSS_NETWORK_ERROR_CODES));
    const batching = new Set(Object.values(BATCHING_CAP_ERROR_CODES));
    for (const value of crossNetwork) {
      expect(batching.has(value)).toBe(false);
    }
  });
});
