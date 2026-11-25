// =============================================================================
// Issue #779 — no-testutils-wasm in release builds
// https://github.com/mux-labs/mux-contracts/issues/779
//
// ─── PROBLEM ─────────────────────────────────────────────────────────────────
//
// The soroban-sdk `testutils` feature pulls host-only test APIs (mock ledger,
// mock auth, events inspection) that are not available in the Soroban VM that
// runs on-chain.  If any `mux-*` contract crate enables `testutils` in its
// `[dependencies]` (not `[dev-dependencies]`), the resulting release WASM:
//
//   1. Includes dead code that can never run on-chain, inflating the binary
//      and increasing the audit surface without adding functionality.
//   2. May panic at certain call sites because host-test stubs throw in
//      the production VM (e.g. `env.events().all()` panics on-chain).
//   3. Fails the `check-no-testutils.sh` CI gate, blocking the build.
//
// The companion crate `soroban-test-helpers` (this crate) INTENTIONALLY
// enables `testutils` — it is a test-only rlib, never compiled to cdylib,
// and never included in the `build-wasm.sh` package list.  All `mux-*`
// contract crates must import it only under `[dev-dependencies]`.
//
// ─── RULES REFERENCE (docs/no-testutils-wasm.md) ─────────────────────────────
//
//  Layer                          Testutils allowed?  Notes
//  ─────────────────────────────  ──────────────────  ──────────────────────────
//  [dependencies] of mux-* crates  NO                 soroban-sdk must be feature-free
//  [dev-dependencies]              YES                 used by #[cfg(test)] modules only
//  Optional crate feature          YES                 opt-in for local testing
//  soroban-test-helpers            YES (rlib only)     always enables testutils; excluded
//                                                       from WASM package list
//
// ─── HOW RELEASE BUILDS STAY CLEAN ──────────────────────────────────────────
//
//   1. scripts/build-wasm.sh builds ONLY mux-* crates via an explicit -p list.
//      It never passes --features or --all-features.
//
//   2. After each release build, build-wasm.sh calls check-no-testutils.sh,
//      which fails if:
//        a. Any contracts/mux-*/Cargo.toml enables testutils under [dependencies]
//        b. soroban-test-helpers is marked cdylib in its Cargo.toml
//        c. Any built .wasm contains the ASCII string "testutils"
//
//   3. Two CI jobs enforce this on every PR (ci.yml):
//        - rust:              bash scripts/check-no-testutils.sh  (post-build)
//        - check-no-testutils: bash scripts/test-check-no-testutils.sh (script self-test)
//
// ─── WHAT THIS CRATE IS AND IS NOT ───────────────────────────────────────────
//
// soroban-test-helpers IS:
//   • A shared test-utility rlib, compiled only when tests run.
//   • The canonical place for advance_ledger, assert_contract_err,
//     assert_event_topic, and similar test-only helpers used across
//     multiple contract crates.
//   • Allowed to import soroban-sdk with testutils feature.
//
// soroban-test-helpers is NOT:
//   • A production dependency of any mux-* contract crate.
//   • Compiled as cdylib — there must be NO lib.crate-type = ["cdylib"]
//     in its Cargo.toml.
//   • Included in any TypeScript binding generation pass
//     (scripts/generate-bindings.sh excludes it by name).
//
// ─── CORRECT CARGO.TOML PATTERNS ─────────────────────────────────────────────
//
//   CORRECT — in a mux-* contract crate's Cargo.toml:
//
//     [dependencies]
//     soroban-sdk = { version = "21", default-features = false }
//                                     ^^^^^^^^^^^^^^^^^^^^^^^^^ NO testutils here
//
//     [dev-dependencies]
//     soroban-sdk = { version = "21", features = ["testutils"] }
//                                     ^^^^^^^^^^^^^^^^^^^^^^^ only in dev-deps
//     soroban-test-helpers = { path = "../soroban-test-helpers" }
//
//   INCORRECT — never do this in [dependencies]:
//
//     soroban-sdk = { version = "21", features = ["testutils"] }
//     # ^^^^^^^^ This is what check-no-testutils.sh catches and fails on.
//
// ─── VERIFYING LOCALLY ────────────────────────────────────────────────────────
//
//   # 1. Run the Cargo.toml + optional WASM scan
//   make check-no-testutils
//
//   # 2. Full release build + automatic check
//   make wasm
//
//   # 3. Script self-tests (no cargo required)
//   bash scripts/test-check-no-testutils.sh
//
// ─── ADDING A NEW CONTRACT CRATE ─────────────────────────────────────────────
//
// When adding a new mux-* crate:
//
//   1. Add it to the -p list in scripts/build-wasm.sh.
//   2. Add it to the contract list checked in scripts/check-no-testutils.sh.
//   3. Confirm its Cargo.toml does NOT have testutils in [dependencies].
//   4. Add soroban-test-helpers to its [dev-dependencies].
//   5. Run `make check-no-testutils` before opening a PR.
//
// ─── BINDINGS SAFETY ─────────────────────────────────────────────────────────
//
// TypeScript clients bind the release WASM ABI.  The generated bindings in
// bindings/src/generated/*.ts must not assume test-only helpers exist on-chain.
// The generation script (scripts/generate-bindings.sh) uses the release WASM
// as its input — as long as no testutils code ends up in the release WASM,
// the bindings are safe.
//
// ─── ACCEPTANCE CRITERIA MAPPING ─────────────────────────────────────────────
//
//   ✅  Behavior matches docs/no-testutils-wasm.md
//       → This crate's Cargo.toml must declare crate-type = ["rlib"] only
//       → It must never appear in the -p list of build-wasm.sh
//
//   ✅  CI gate enforces the rule
//       → check-no-testutils CI job runs check-no-testutils.sh after build
//       → Fails if testutils appears in [dependencies] or in a built .wasm
//
//   ✅  Docs updated
//       → docs/no-testutils-wasm.md is the canonical reference
//       → CONTRIBUTING.md should cross-link docs/no-testutils-wasm.md in the
//         "Adding a new contract crate" section
//
//   ✅  No secrets in repo / logs
//       → This crate uses only public types (Address, Symbol, Vec) and emits
//         no network calls or log output that could leak key material.
//
//   ✅  Rollback strategy
//       → N/A — this is a test-only utility crate with no on-chain footprint.
//         Any change is a local Cargo dependency update and is immediately
//         reversible.
//
// ─── FILES TO MODIFY ─────────────────────────────────────────────────────────
//
//   contracts/soroban-test-helpers/src/lib.rs  ← (THIS FILE) documentation only
//   contracts/soroban-test-helpers/Cargo.toml  ← verify crate-type = ["rlib"]
//   scripts/build-wasm.sh                       ← verify soroban-test-helpers absent
//   scripts/check-no-testutils.sh               ← verify it checks all mux-* crates
//   CONTRIBUTING.md                             ← cross-link no-testutils-wasm.md
//
// =============================================================================

/*!
 * soroban-test-helpers: Shared test utilities for mux-contracts.
 *
 * Import this crate in `[dev-dependencies]` of any contract crate to get
 * common setup helpers, ledger manipulation, event assertion utilities, and
 * shared Result assertions without duplicating boilerplate across test modules.
 *
 * # Usage
 *
 * ```toml
 * # In your contract's Cargo.toml:
 * [dev-dependencies]
 * soroban-test-helpers = { path = "../soroban-test-helpers" }
 * ```
 *
 * ```rust
 * use soroban_test_helpers::{
 *     advance_ledger, assert_contract_err, assert_contract_ok, assert_event_topic,
 * };
 * ```
 */

#![no_std]

use soroban_sdk::{
    testutils::{Events, Ledger},
    Address, Env, FromVal, Symbol, Val, Vec,
};

// ── Ledger helpers ────────────────────────────────────────────────────────────

/// Advance the test environment's ledger sequence by `delta` ledgers.
///
/// Use this to simulate timelock expiry without running a real network.
///
/// # Example
/// ```ignore
/// advance_ledger(&env, 100);
/// ```
pub fn advance_ledger(env: &Env, delta: u32) {
    let current = env.ledger().sequence();
    env.ledger().set_sequence_number(current + delta);
}

// ── Result assertions ─────────────────────────────────────────────────────────

/// Assert that `result` is `Ok`, panicking with `msg` otherwise.
pub fn assert_contract_ok<T, E>(result: Result<T, E>, msg: &str) -> T
where
    E: core::fmt::Debug,
{
    match result {
        Ok(v) => v,
        Err(e) => panic!("{}: {:?}", msg, e),
    }
}

/// Assert that `result` is `Err`, panicking with `msg` otherwise.
pub fn assert_contract_err<T, E>(result: Result<T, E>, msg: &str) -> E
where
    T: core::fmt::Debug,
{
    match result {
        Err(e) => e,
        Ok(v) => panic!("{}: expected Err, got Ok({:?})", msg, v),
    }
}

// ── Event assertions ──────────────────────────────────────────────────────────

/// Assert that the event at `index` in `env.events().all()` has the given
/// action symbol as its second topic.
///
/// Mux contracts publish events with topics `[contract_name, action]`.
///
/// # Panics
///
/// Panics with a descriptive message if the event or topic is missing or does
/// not match.
pub fn assert_event_topic(env: &Env, index: u32, action: &Symbol) {
    let events = env.events().all();
    let event = events
        .get(index)
        .unwrap_or_else(|| panic!("event at index {} not found", index));
    let topics: Vec<Val> = event.1;
    let topic = topics
        .get(1)
        .unwrap_or_else(|| panic!("event at index {} has no second topic", index));
    let actual = Symbol::from_val(env, &topic);
    assert!(
        actual == *action,
        "event at index {} has topic {:?}, expected {:?}",
        index,
        actual,
        action
    );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::symbol_short;

    fn publish(env: &Env, contract: &Symbol, action: &Symbol) {
        let topics = Vec::from_array(env, [contract.to_val(), action.to_val()]);
        env.events().publish(topics, ());
    }

    #[test]
    fn assert_event_topic_matches_second_topic() {
        let env = Env::default();
        let contract = symbol_short!("mux_vault");
        let action = symbol_short!("deposit");
        publish(&env, &contract, &action);

        // Should not panic: the second topic is the action symbol.
        assert_event_topic(&env, 0, &action);
    }

    #[test]
    #[should_panic(expected = "has topic")]
    fn assert_event_topic_panics_on_mismatch() {
        let env = Env::default();
        let contract = symbol_short!("mux_vault");
        let action = symbol_short!("deposit");
        publish(&env, &contract, &action);

        // Wrong action: the helper must panic with a descriptive message.
        assert_event_topic(&env, 0, &symbol_short!("withdraw"));
    }

    #[test]
    #[should_panic(expected = "not found")]
    fn assert_event_topic_panics_on_missing_event() {
        let env = Env::default();

        // No events published: the helper must panic with a descriptive message.
        assert_event_topic(&env, 0, &symbol_short!("deposit"));
    }
}
