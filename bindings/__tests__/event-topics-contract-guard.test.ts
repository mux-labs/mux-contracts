/**
 * Guard: every event topic constant exposed by the parser modules must match
 * the exact topic string emitted by the corresponding Rust contract.
 *
 * The parser modules (factory-events.ts, registry-events.ts,
 * recovery-events.ts, policy-events.ts, batcher-events.ts) each export a
 * `*_EVENT_TOPICS` object whose values are used to decode on-chain events.
 * If a contract adds/renames an emitted topic (`symbol_short!(...)` in the
 * contract's `lib.rs`) without updating the matching TS constant, the
 * parser silently drops that event (falls through to its `default: return
 * null` branch) instead of surfacing a decode error.
 *
 * This test hard-codes the topic strings actually emitted via
 * `symbol_short!("...")` / `emit(&env, symbol_short!("..."), ...)` in each
 * contract's `contracts/<name>/src/lib.rs`, and asserts the TS constants are
 * a byte-for-byte match (no missing, no extra, no renamed topics).
 */
import { FACTORY_EVENT_TOPICS } from "../src/factory-events";
import { REGISTRY_EVENT_TOPICS } from "../src/registry-events";
import { RECOVERY_EVENT_TOPICS } from "../src/recovery-events";
import { POLICY_EVENT_TOPICS } from "../src/policy-events";
import { BATCHER_EVENT_TOPICS } from "../src/batcher-events";

function expectTopicsMatch(
  label: string,
  topics: Record<string, string>,
  expectedContractTopics: string[],
) {
  const tsTopics = Object.values(topics).sort();
  const contractTopics = [...expectedContractTopics].sort();

  it(`${label}: TS topic constants match contract-emitted topics exactly`, () => {
    expect(tsTopics).toEqual(contractTopics);
  });
}

describe("Event topic constants vs. contract-emitted topics", () => {
  // contracts/mux-account-factory/src/lib.rs: deploy_account / deploy_account_with_metadata
  expectTopicsMatch("FACTORY_EVENT_TOPICS", FACTORY_EVENT_TOPICS, [
    "deployed",
    "meta_set",
  ]);

  // contracts/mux-registry/src/lib.rs: initialize / register / register_with_metadata
  expectTopicsMatch("REGISTRY_EVENT_TOPICS", REGISTRY_EVENT_TOPICS, [
    "init",
    "reg",
    "regmeta",
  ]);

  // contracts/mux-recovery/src/lib.rs: initialize, initiate_recovery,
  // approve_recovery, execute_recovery, approve_recovery_admin,
  // cancel_recovery, add_guardian, remove_guardian, set_registry
  expectTopicsMatch("RECOVERY_EVENT_TOPICS", RECOVERY_EVENT_TOPICS, [
    "init",
    "rec_init",
    "rec_appr",
    "rec_exec",
    "rec_adm",
    "rec_cncl",
    "grd_add",
    "grd_rm",
    "reg_link",
  ]);

  // contracts/mux-policy/src/lib.rs: initialize / set_daily_limit /
  // record_spend / reset_daily_counter
  expectTopicsMatch("POLICY_EVENT_TOPICS", POLICY_EVENT_TOPICS, [
    "init",
    "lmt_set",
    "spent",
    "ctr_rst",
  ]);

  // contracts/mux-batcher/src/lib.rs: initialize / execute_batch / simulate_batch
  expectTopicsMatch("BATCHER_EVENT_TOPICS", BATCHER_EVENT_TOPICS, [
    "init",
    "bat_start",
    "executed",
    "bat_ok",
    "bat_abort",
    "sim_done",
    "fee_chg",
  ]);

  it("RECOVERY_EVENT_TOPICS previously omitted the rec_appr topic emitted by approve_recovery (#982 regression guard)", () => {
    // approve_recovery (contracts/mux-recovery/src/lib.rs) emits
    // symbol_short!("rec_appr"); the TS constant must track it so
    // parseRecoveryEvent does not silently drop the event.
    expect(Object.values(RECOVERY_EVENT_TOPICS)).toContain("rec_appr");
  });
});
