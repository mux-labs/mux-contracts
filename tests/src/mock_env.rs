/// mock_env: shared test-environment helpers for Mux Protocol contract tests.
///
/// Provides zero-boilerplate setup functions so individual test modules don't
/// repeat the same `Env::default() / mock_all_auths / register_contract` dance.
/// Import with `use mux_contract_tests::mock_env::*;` in any integration test.
///
/// Issue #101 — Testing & tooling: Add mock env setup utility.
/// Issue #919 — compiled into the crate's library target (no `#[cfg(test)]`
/// gate) so integration test binaries can import the helpers.
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env};

/// A fully-mocked Soroban environment with all auth checks bypassed.
pub fn make_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

/// Generate a fresh random address in `env`.
pub fn make_address(env: &Env) -> Address {
    Address::generate(env)
}

/// Generate `n` distinct addresses.
pub fn make_addresses(env: &Env, n: usize) -> Vec<Address> {
    (0..n).map(|_| Address::generate(env)).collect()
}

/// Advance the ledger sequence by `delta` ledgers (useful for TTL / period tests).
pub fn advance_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|li| li.sequence_number += delta);
}
