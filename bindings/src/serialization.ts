/**
 * SDK data serialization utilities (issue #1350).
 *
 * Provides a thin, format-agnostic layer for tagging data with a format,
 * version, timestamp, and id so callers can round-trip structured values
 * without depending on ad-hoc conventions.
 */

export type SerializationFormat = "json" | "base64" | "hex" | "xdr";

export type SerializationVersion = number;

export interface SerializationRecord<T> {
  format: SerializationFormat;
  version: SerializationVersion;
  data: T;
  timestamp: number;
  id: string;
}

export const SerializationErrorCode = {
  UnsupportedFormat: "UNSUPPORTED_FORMAT",
  VersionMismatch: "VERSION_MISMATCH",
  DataLoss: "DATA_LOSS",
  InvalidInput: "INVALID_INPUT",
} as const;

export type SerializationErrorCode =
  (typeof SerializationErrorCode)[keyof typeof SerializationErrorCode];

export class SerializationError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "SerializationError";
    this.code = code;
  }
}

export const SUPPORTED_FORMATS: readonly SerializationFormat[] = [
  "json",
  "base64",
  "hex",
  "xdr",
] as const;

export const CURRENT_VERSION: SerializationVersion = 1 as const;

// ── Internal history store ───────────────────────────────────────────────────

let _history: SerializationRecord<unknown>[] = [];

// ── Core API ─────────────────────────────────────────────────────────────────

/**
 * Wrap `data` in a {@link SerializationRecord}.
 *
 * For the `"xdr"` format the stored `data` value is a base64-encoded
 * JSON string so it survives transport as plain text. All other formats
 * store the value as-is.
 */
export function serialize<T>(
  data: T,
  format: SerializationFormat,
  version: SerializationVersion = CURRENT_VERSION,
): SerializationRecord<T> {
  const id = Math.random().toString(36).slice(2);
  const timestamp = Date.now();

  if (format === "xdr") {
    const encoded = Buffer.from(JSON.stringify(data)).toString("base64");
    return { format, version, data: encoded as unknown as T, timestamp, id };
  }

  return { format, version, data, timestamp, id };
}

/**
 * Unwrap a {@link SerializationRecord} and return the contained value.
 *
 * Throws {@link SerializationError} with code
 * `SerializationErrorCode.VersionMismatch` when the record's version does not
 * match `expectedVersion`.
 */
export function deserialize<T>(
  record: SerializationRecord<unknown>,
  expectedVersion?: SerializationVersion,
): T {
  if (
    expectedVersion !== undefined &&
    record.version !== expectedVersion
  ) {
    throw new SerializationError(
      SerializationErrorCode.VersionMismatch,
      `Version mismatch: expected ${expectedVersion}, got ${record.version}`,
    );
  }

  if (record.format === "xdr") {
    const json = Buffer.from(record.data as string, "base64").toString("utf8");
    return JSON.parse(json) as T;
  }

  return record.data as T;
}

/**
 * Append `record` to the in-memory serialization history.
 */
export function trackSerialization(record: SerializationRecord<unknown>): void {
  _history.push(record);
}

/**
 * Return a readonly snapshot of the serialization history.
 */
export function getSerializationHistory(): ReadonlyArray<
  SerializationRecord<unknown>
> {
  return [..._history];
}

/**
 * Clear the serialization history.
 */
export function clearSerializationHistory(): void {
  _history = [];
}
