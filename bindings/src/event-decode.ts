/**
 * event-decode.ts — Shared value-decoding helpers for Soroban event parsers.
 *
 * Centralises normaliseData, resolveAddress, resolveSymbol, resolveString and
 * resolveU64 so that every event parser uses the strictest correct behaviour
 * for each, and a fix applies uniformly across all modules.
 *
 * @module event-decode
 */

// Soroban `Address` is an account (G…) or contract (C…) strkey: 56 chars in
// the base-32 alphabet [A-Z2-7], per SEP-23 / RFC-4648.
const ADDRESS_RE = /^[GC][A-Z2-7]{55}$/;

/**
 * Normalise the raw `value` field into a flat array of values. Accepts either
 * an already-decoded array, a `{ vec: [...] }` / `{ _value: [...] }` object, or
 * a JSON-encoded string. When `arity` is 1, a bare scalar is wrapped into a
 * single-element array (used by the batcher parser for single-field payloads).
 */
export function normaliseData(raw: unknown, arity?: number): unknown[] | null {
  if (raw === null || raw === undefined) return null;
  if (Array.isArray(raw)) return raw as unknown[];

  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (Array.isArray(obj["vec"])) return obj["vec"] as unknown[];
    if (Array.isArray(obj["_value"])) return obj["_value"] as unknown[];
  }

  if (typeof raw === "string") {
    try {
      const parsed: unknown = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed as unknown[];
    } catch {
      // Not JSON — fall through.
    }
  }

  return arity === 1 ? [raw] : null;
}

/**
 * Resolve a Soroban Address from a raw event value. Accepts a strkey string
 * (G…/C…) or an `{ address: "…" }` object. Returns null if the value is not a
 * valid 56-character Stellar address strkey.
 */
export function resolveAddress(raw: unknown): string | null {
  if (typeof raw === "string" && ADDRESS_RE.test(raw)) return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["address"] === "string" && ADDRESS_RE.test(obj["address"])) {
      return obj["address"];
    }
  }
  return null;
}

/**
 * Soroban `Symbol` (symbol_short) values are hard-capped at 8 characters by
 * the Stellar protocol. Any decoded value longer than this cannot have been
 * emitted by a conforming contract and must be rejected fail-closed.
 */
export const SYMBOL_SHORT_MAX_LENGTH = 8 as const;

/**
 * Resolve a Soroban Symbol from a raw event value. Accepts a plain string or
 * a `{ symbol: "…" }` / `{ sym: "…" }` object (both SDK shapes are handled).
 * Returns `null` when the resolved value exceeds the {@link SYMBOL_SHORT_MAX_LENGTH}
 * 8-character `symbol_short` limit, rather than silently accepting an
 * oversized name.
 */
export function resolveSymbol(raw: unknown): string | null {
  let value: string | null = null;
  if (typeof raw === "string") {
    value = raw;
  } else if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["symbol"] === "string") value = obj["symbol"];
    else if (typeof obj["sym"] === "string") value = obj["sym"];
  }

  if (value === null) return null;
  if (value.length > SYMBOL_SHORT_MAX_LENGTH) return null;
  return value;
}

/**
 * Resolve a Soroban String from a raw event value. Accepts a plain string or
 * a `{ string: "…" }` object.
 */
export function resolveString(raw: unknown): string | null {
  if (typeof raw === "string") return raw;
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if (typeof obj["string"] === "string") return obj["string"];
  }
  return null;
}

/** Inclusive upper bound of the unsigned 64-bit range. */
const U64_MAX = (1n << 64n) - 1n;

/**
 * Resolve a Soroban u64 from a raw event value. Accepts a decimal string, a
 * safe non-negative integer number, a bigint, or a `{ u64: "…" }` object.
 * Returns the value as a decimal string to preserve full 64-bit precision (JS
 * numbers lose precision above 2^53), or `null` when the value cannot be
 * represented losslessly as a u64.
 */
export function resolveU64(raw: unknown): string | null {
  if (typeof raw === "bigint") {
    return raw >= 0n && raw <= U64_MAX ? raw.toString() : null;
  }
  if (typeof raw === "number") {
    // Non-integer, negative, or unsafe numbers are already lossy for u64;
    // reject them so callers fail loudly instead of silently truncating.
    if (!Number.isSafeInteger(raw) || raw < 0) return null;
    return String(raw);
  }
  if (typeof raw === "string") {
    if (!/^\d+$/.test(raw)) return null;
    const value = BigInt(raw);
    return value <= U64_MAX ? value.toString() : null;
  }
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    if ("u64" in obj) return resolveU64(obj["u64"]);
  }
  return null;
}
