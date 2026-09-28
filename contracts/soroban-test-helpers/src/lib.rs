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
/// advance_ledger(&env, RECOVERY_TIMELOCK + 1);
/// ```
pub fn advance_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number.saturating_add(delta);
    });
}

/// Set the ledger sequence to an absolute value.
pub fn set_ledger_sequence(env: &Env, sequence: u32) {
    env.ledger().with_mut(|l| {
        l.sequence_number = sequence;
    });
}

/// Set the ledger timestamp (Unix seconds).
pub fn set_ledger_timestamp(env: &Env, timestamp: u64) {
    env.ledger().with_mut(|l| {
        l.timestamp = timestamp;
    });
}

// ── Event helpers ─────────────────────────────────────────────────────────────

/// Assert that the event at `index` in `env.events().all()` has the given
/// action symbol as its second topic.
///
/// Mux contracts publish events with topics `[contract_name, action]`.
///
/// # Panics
/// Panics with a descriptive message if the event or topic is missing or
/// does not match.
pub fn assert_event_topic(
    env: &Env,
    events: &Vec<(Address, Vec<Val>, Val)>,
    index: u32,
    expected_action: Symbol,
) {
    let (_, topics, _) = events
        .get(index)
        .unwrap_or_else(|| panic!("no event at index {index}"));
    let actual = Symbol::from_val(
        env,
        &topics
            .get(1)
            .unwrap_or_else(|| panic!("event at index {index} has no second topic")),
    );
    assert_eq!(
        actual, expected_action,
        "event[{index}] action mismatch: expected {expected_action:?}, got {actual:?}"
    );
}

/// Return the number of events emitted so far in the test environment.
pub fn event_count(env: &Env) -> u32 {
    env.events().all().len()
}

/// Assert that exactly `expected` events have been emitted.
///
/// # Panics
/// Panics when the emitted event count does not equal `expected`.
pub fn assert_event_count(env: &Env, expected: u32) {
    let actual = event_count(env);
    assert_eq!(
        actual, expected,
        "event count mismatch: expected {expected}, got {actual}"
    );
}

// ── Address helpers ───────────────────────────────────────────────────────────

/// Generate `n` distinct test addresses.
pub fn generate_addresses(env: &Env, n: u32) -> Vec<Address> {
    use soroban_sdk::testutils::Address as _;
    let mut v = Vec::new(env);
    for _ in 0..n {
        v.push_back(Address::generate(env));
    }
    v
}

// ── Shared Result assertions ──────────────────────────────────────────────────
//
// Soroban contract clients expose `try_*` helpers whose return type is:
//
//   Result<Result<T, ConversionError>, Result<E, InvokeError>>
//
// where:
//   Ok(Ok(t))  — contract returned successfully
//   Ok(Err(c)) — host-side conversion / decode failure
//   Err(Ok(e)) — contract returned a typed contract error `E`
//   Err(Err(i)) — host invoke failure (auth, panic, budget, …)
//
// These helpers centralise that match so individual tests stay readable and
// produce consistent panic messages across crates.

/// Assert that a `try_*` client call succeeded and return the inner value.
///
/// # Panics
/// Panics if the call returned a contract error, conversion error, or invoke error.
pub fn assert_contract_ok<T, C, E, I>(result: Result<Result<T, C>, Result<E, I>>) -> T
where
    T: core::fmt::Debug,
    C: core::fmt::Debug,
    E: core::fmt::Debug,
    I: core::fmt::Debug,
{
    match result {
        Ok(Ok(value)) => value,
        Ok(Err(conv)) => panic!("expected Ok, got conversion error: {conv:?}"),
        Err(Ok(err)) => panic!("expected Ok, got contract error: {err:?}"),
        Err(Err(inv)) => panic!("expected Ok, got invoke error: {inv:?}"),
    }
}

/// Assert that a `try_*` client call returned the expected contract error.
///
/// Matches the common pattern:
/// `assert_eq!(client.try_foo(...), Err(Ok(MyError::Variant)))`.
///
/// # Panics
/// Panics if the call succeeded, returned a different contract error, or
/// failed at the conversion / invoke layer.
pub fn assert_contract_err<T, C, E, I>(
    result: Result<Result<T, C>, Result<E, I>>,
    expected: E,
) where
    T: core::fmt::Debug,
    C: core::fmt::Debug,
    E: core::fmt::Debug + PartialEq,
    I: core::fmt::Debug,
{
    match result {
        Err(Ok(err)) => {
            assert_eq!(
                err, expected,
                "contract error mismatch: expected {expected:?}, got {err:?}"
            );
        }
        Ok(Ok(value)) => {
            panic!("expected contract error {expected:?}, got Ok({value:?})")
        }
        Ok(Err(conv)) => {
            panic!("expected contract error {expected:?}, got conversion error: {conv:?}")
        }
        Err(Err(inv)) => {
            panic!("expected contract error {expected:?}, got invoke error: {inv:?}")
        }
    }
}

/// Assert that `actual` does not exceed `max` (storage / batch bound check).
///
/// Use after cap-rejection tests to prove collections did not grow past the
/// configured griefing bound.
///
/// # Panics
/// Panics when `actual > max`.
pub fn assert_len_at_most(actual: u32, max: u32, label: &str) {
    assert!(
        actual <= max,
        "{label}: length {actual} exceeds bound {max}"
    );
}

/// Assert that `actual` equals `expected` exactly.
///
/// # Panics
/// Panics when the lengths differ.
pub fn assert_len_eq(actual: u32, expected: u32, label: &str) {
    assert_eq!(
        actual, expected,
        "{label}: length mismatch: expected {expected}, got {actual}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_contract_ok_unwraps_success() {
        let result: Result<Result<u32, &str>, Result<&str, &str>> = Ok(Ok(42));
        assert_eq!(assert_contract_ok(result), 42);
    }

    #[test]
    #[should_panic(expected = "expected Ok, got contract error")]
    fn assert_contract_ok_panics_on_contract_err() {
        let result: Result<Result<u32, &str>, Result<&str, &str>> = Err(Ok("boom"));
        let _ = assert_contract_ok(result);
    }

    #[test]
    fn assert_contract_err_matches_expected() {
        let result: Result<Result<u32, &str>, Result<&str, &str>> = Err(Ok("cap"));
        assert_contract_err(result, "cap");
    }

    #[test]
    #[should_panic(expected = "expected contract error")]
    fn assert_contract_err_panics_on_ok() {
        let result: Result<Result<u32, &str>, Result<&str, &str>> = Ok(Ok(1));
        assert_contract_err(result, "cap");
    }

    #[test]
    fn assert_len_helpers() {
        assert_len_at_most(64, 64, "accounts");
        assert_len_eq(50, 50, "batch");
    }

    #[test]
    #[should_panic(expected = "exceeds bound")]
    fn assert_len_at_most_panics_when_over() {
        assert_len_at_most(65, 64, "accounts");
    }
}
