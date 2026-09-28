/**
 * @mux-protocol/contracts public entry point.
 *
 * All contract-generated clients, shared types, error helpers, network
 * utilities, event parsers, and address helpers are re-exported from this
 * single entry point so consumers have one stable import surface.
 *
 * Generated clients are produced by scripts/generate-bindings.sh.
 * After changing a contract entrypoint or public type, regenerate and
 * update downstream TypeScript calls in the same release.
 *
 * Package exports audited (#841):
 *  - All generated contract clients (export *)
 *  - All shared types from ./types (explicit export type {})
 *  - All error message helpers from ./types (explicit export {})
 *  - All utility modules: network, horizon, errors, addresses, local-invoke
 *  - addresses-config: named exports to avoid leaking internal symbols
 *  - Event parsers: factory-events, recovery-events, policy-events,
 *    registry-events, batcher-events
 *  - Example namespaces: examples, accountExamples, registryExamples
 *  - batcher namespace re-export (assertBatchWithinCaps, BATCH_CAPS, etc.)
 */

// ── Generated contract clients ───────────────────────────────────────────────
export * from "./generated/mux-account";
export * from "./generated/mux-account-factory";
export * from "./generated/mux-batcher";
export * from "./generated/mux-delegation";
export * from "./generated/mux-permissions";
export * from "./generated/mux-registry";
export * from "./generated/mux-wallet-registry";
export * from "./generated/mux-spending-policy";
export * from "./generated/mux-recovery";

// ── Shared types ─────────────────────────────────────────────────────────────
// All public types defined in ./types are explicitly listed here so that
// adding a new type to types.ts without re-auditing this file is caught by
// TypeScript consumers as a "module has no exported member" error rather than
// silently vanishing from the public API.
export type {
  NetworkPassphrase,
  MuxContractIds,
  SpendLimit,
  SpendingPolicyLimit,
  DelegateInfo,
  Operation,
  BatchOperationKind,
  BatchResult,
  BatcherMeta,
  MuxAccountError,
  MuxAccountFactoryError,
  MuxBatcherError,
  MuxDelegationError,
  MuxPermissionsError,
  MuxPolicyError,
  MuxRecoveryError,
  MuxWalletRegistryError,
  SpendingPolicyError,
} from "./types";

// ── Error message helpers & constants ────────────────────────────────────────
export {
  RECOVERY_TIMELOCK_LEDGERS,
  RECOVERY_EXPIRY_LEDGERS,
  muxAccountErrorMessage,
  muxAccountFactoryErrorMessage,
  muxBatcherErrorMessage,
  muxDelegationErrorMessage,
  muxPermissionsErrorMessage,
  muxPolicyErrorMessage,
  muxRecoveryErrorMessage,
  muxRegistryErrorMessage,
  spendingPolicyErrorMessage,
} from "./types";

// ── Network, horizon, and error utilities ────────────────────────────────────
export * from "./network";
export * from "./horizon";
export * from "./errors";
export * from "./addresses";

// ── Address configuration helpers ────────────────────────────────────────────
// Named exports only — avoids accidentally exposing internal config symbols.
export {
  DEFAULT_ADDRESSES,
  MAINNET_REVIEW_ERROR_CODES,
  assertMainnetAddressReview,
  assertNetworkAddresses,
  hashMainnetAddresses,
  readMainnetDeployFlag,
  assertMainnetDeployFlag,
  MUX_MAINNET_DEPLOY_FLAG_ENV,
  MUX_MAINNET_DEPLOY_FLAG_VALUE,
  MainnetAddressReviewError,
} from "./addresses-config";
export type {
  MainnetReviewErrorCode,
  AddressNetwork,
  ContractAddressKey,
  AddressBook,
  MainnetReviewMarker,
  MainnetDeployFlag,
} from "./addresses-config";

// ── Local invoke helper ───────────────────────────────────────────────────────
export * from "./local-invoke";

// ── Event parsers ─────────────────────────────────────────────────────────────
export {
  FACTORY_CONTRACT_TAG,
  FACTORY_EVENT_TOPICS,
  MAX_ACCOUNTS_PAGE_SIZE,
  FactoryBoundsErrorCode,
  FactoryBoundsError,
  parseFactoryEvent,
} from "./factory-events";
export type {
  FactoryEventAction,
  GetAccountsBounds,
  FactoryDeployedEvent,
  FactoryMetaSetEvent,
  FactoryEvent,
  RawSorobanEvent,
} from "./factory-events";
export * from "./recovery-events";
export * from "./policy-events";
export * from "./registry-events";
export * from "./batcher-events";

// ── Example namespaces ────────────────────────────────────────────────────────
// Exported as namespaces so tree-shakers can drop them in production bundles.
export * as examples from "./examples/frontend-usage";
export * as accountExamples from "./examples/account-invoke";
export * as registryExamples from "./examples/registry-invoke";

// ── Batcher namespace ─────────────────────────────────────────────────────────
// Provides MuxBatcherClient + cap constants + assertBatchWithinCaps under the
// `batcher` namespace for consumers who prefer a grouped import surface.
export * as batcher from "./batcher";
