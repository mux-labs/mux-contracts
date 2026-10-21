//! Library target for shared integration-test helpers (issue #919).
//!
//! The previous `#[cfg(test)]` gate on `mock_env` meant the helpers were
//! only visible to unit tests inside this crate's own test target, so
//! integration test binaries under `tests/` had to re-implement
//! `Env::default() / mock_all_auths / register_contract` by hand.
//!
//! Import from any integration test with:
//! `use mux_contract_tests::mock_env::{make_env, make_address, ...};`

pub mod mock_env;
