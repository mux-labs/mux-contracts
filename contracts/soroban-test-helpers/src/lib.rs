/*!
 * soroban-test-helpers: Shared test utilities for mux-contracts.
 *
 * Import this crate in `[dev-dependencies]` of any contract crate to get
 * common setup helpers, ledger manipulation, and event assertion utilities
 * without duplicating boilerplate across test modules.
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
 * use soroban_test_helpers::{advance_ledger, assert_event_topic};
 * ```
 */

#![no_std]

use soroban_sdk::{testutils::Ledger, Env, Symbol, Val, Vec};

// ── Ledger helpers ────────────────────────────────────────────────────────────

/// Advance the test environment's ledger sequence by `delta` ledgers.
///
/// Use this to simulate timelock expiry without running a real network.
///
/// # Example
/// ```rust
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
    events: &Vec<(soroban_sdk::Address, Vec<Val>, Val)>,
    index: u32,
    expected_action: Symbol,
) {
    let (_, topics, _) = events
        .get(index)
        .unwrap_or_else(|| panic!("no event at index {index}"));
    let actual = Symbol::from_val(env, &topics.get(1).unwrap_or_else(|| {
        panic!("event at index {index} has no second topic")
    }));
    assert_eq!(
        actual, expected_action,
        "event[{index}] action mismatch: expected {expected_action:?}, got {actual:?}"
    );
}

/// Return the number of events emitted so far in the test environment.
pub fn event_count(env: &Env) -> u32 {
    env.events().all().len()
}

// ── Address helpers ───────────────────────────────────────────────────────────

/// Generate `n` distinct test addresses.
pub fn generate_addresses(env: &Env, n: u32) -> soroban_sdk::Vec<soroban_sdk::Address> {
    use soroban_sdk::testutils::Address as _;
    let mut v = soroban_sdk::Vec::new(env);
    for _ in 0..n {
        v.push_back(soroban_sdk::Address::generate(env));
    }
    v
}
