/**
 * Batcher namespace — re-exports the MuxBatcherClient together with all
 * batcher-specific types so consumers can import from a single module.
 *
 * Usage (named import):
 *   import { MuxBatcherClient, type Operation, type BatchResult } from "@mux-protocol/contracts";
 *
 * Usage (namespace import):
 *   import { batcher } from "@mux-protocol/contracts";
 *   const client = new batcher.MuxBatcherClient({ ... });
 */

export { MuxBatcherClient } from "./generated/mux-batcher";
export type { MuxBatcherClientOptions } from "./generated/mux-batcher";
export type {
  BatchOperationKind,
  BatchResult,
  MuxBatcherError,
  Operation,
} from "./types";
