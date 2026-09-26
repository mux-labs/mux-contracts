//! Loads the shared JSON test vectors (`tests/fixtures/test_vectors.json`,
//! `tests/fixtures/account_limit_vectors.json`) and actually drives the
//! contracts they describe, asserting the recorded `expect` against real
//! contract behaviour.
//!
//! Before this file, the fixtures were hand-written "shared truth" for
//! Rust/TS tests but nothing loaded or executed them — a Rust test and a
//! fixture could silently drift apart with no test failure to catch it.
//! This file closes that gap on the Rust side; `bindings/__tests__/
//! test-vectors.test.ts` closes it on the TypeScript side.
//!
//! Run with: cargo test -p mux-contract-tests --test fixture_vectors

use mux_account::{MuxAccount, MuxAccountClient, MuxAccountError};
use mux_batcher::{BatchOperationKind, MuxBatcher, MuxBatcherClient, MuxBatcherError, Operation};
use mux_permissions::{MuxPermissions, MuxPermissionsClient, MuxPermissionsError};
use serde_json::Value;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol, Val, Vec};

const TEST_VECTORS_JSON: &str = include_str!("fixtures/test_vectors.json");
const ACCOUNT_LIMIT_VECTORS_JSON: &str = include_str!("fixtures/account_limit_vectors.json");

fn load(json: &str) -> Value {
    serde_json::from_str(json).expect("fixture must be valid JSON")
}

/// Find a vector object by its `id` inside `json[path...]` (an array).
fn vector<'a>(json: &'a Value, path: &[&str], id: &str) -> &'a Value {
    let mut node = json;
    for segment in path {
        node = node
            .get(segment)
            .unwrap_or_else(|| panic!("fixture missing path segment `{segment}`"));
    }
    node.as_array()
        .unwrap_or_else(|| panic!("fixture path {path:?} must be an array"))
        .iter()
        .find(|v| v["id"] == id)
        .unwrap_or_else(|| panic!("fixture vector `{id}` not found under {path:?}"))
}

fn expect_err(v: &Value) -> &str {
    v["expect"]["err"]
        .as_str()
        .unwrap_or_else(|| panic!("vector `{}` has no expect.err", v["id"]))
}

fn expect_ok(v: &Value) -> bool {
    v["expect"]["ok"].as_bool().unwrap_or(false)
}

// ── Sanity: both fixtures parse and cross-reference each other ────────────────

#[test]
fn fixtures_load_and_cross_reference() {
    let vectors = load(TEST_VECTORS_JSON);
    let limits = load(ACCOUNT_LIMIT_VECTORS_JSON);

    assert!(vectors["description"].is_string());
    assert!(limits["description"].is_string());
    assert_eq!(
        vectors["_see_also"]["account_limit_vectors"],
        "tests/fixtures/account_limit_vectors.json"
    );
}

/// The `constants` block in account_limit_vectors.json must match the caps
/// actually enforced on-chain. mux-account's caps have no public getter, so
/// they are cross-checked behaviourally: flooding to the documented cap
/// succeeds and one past it fails closed. mux-batcher exposes its cap
/// directly via `max_batch_size()`.
#[test]
fn documented_constants_match_batcher_cap_getter() {
    let limits = load(ACCOUNT_LIMIT_VECTORS_JSON);

    let env = Env::default();
    let contract_id = env.register_contract(None, MuxBatcher);
    let client = MuxBatcherClient::new(&env, &contract_id);

    assert_eq!(
        limits["constants"]["mux_batcher"]["MAX_BATCH_SIZE"]
            .as_u64()
            .unwrap() as u32,
        client.max_batch_size()
    );
}

// ── mux_account: spend_limit vectors (tests/fixtures/test_vectors.json) ───────

fn setup_account() -> (Env, MuxAccountClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxAccount);
    let client = MuxAccountClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    client.initialize(&owner, &Vec::new(&env));
    (env, client)
}

#[test]
fn vector_acct_spend_ok_debit_within_limit_succeeds() {
    let vectors = load(TEST_VECTORS_JSON);
    let v = vector(&vectors, &["mux_account", "spend_limit"], "acct-spend-ok");
    let (env, client) = setup_account();
    let asset = Address::generate(&env);

    let limit = v["input"]["limit"].as_i64().unwrap() as i128;
    let period = v["input"]["period_ledgers"].as_u64().unwrap() as u32;
    let spend = v["input"]["spend"].as_i64().unwrap() as i128;

    client.set_spend_limit(&asset, &limit, &period);
    let result = client.try_debit_spend(&asset, &spend);
    assert!(expect_ok(v));
    assert!(result.is_ok(), "expected ok per fixture: {result:?}");

    // expect.remaining is not directly queryable — probe it: a further debit
    // of exactly `remaining` must succeed, and one more unit must not.
    let remaining = v["expect"]["remaining"].as_i64().unwrap() as i128;
    assert!(client.try_debit_spend(&asset, &remaining).is_ok());
    assert_eq!(
        client.try_debit_spend(&asset, &1),
        Err(Ok(MuxAccountError::SpendLimitExceeded))
    );
}

#[test]
fn vector_acct_spend_exceeded_debit_over_limit_rejected() {
    let vectors = load(TEST_VECTORS_JSON);
    let v = vector(
        &vectors,
        &["mux_account", "spend_limit"],
        "acct-spend-exceeded",
    );
    let (env, client) = setup_account();
    let asset = Address::generate(&env);

    let limit = v["input"]["limit"].as_i64().unwrap() as i128;
    let period = v["input"]["period_ledgers"].as_u64().unwrap() as u32;
    let spend = v["input"]["spend"].as_i64().unwrap() as i128;

    client.set_spend_limit(&asset, &limit, &period);
    let result = client.try_debit_spend(&asset, &spend);
    assert_eq!(
        result,
        Err(Ok(match expect_err(v) {
            "SpendLimitExceeded" => MuxAccountError::SpendLimitExceeded,
            other => panic!("unexpected expect.err in fixture: {other}"),
        }))
    );
}

#[test]
fn vector_acct_spend_invalid_amount_and_period_rejected() {
    let vectors = load(TEST_VECTORS_JSON);
    let (env, client) = setup_account();
    let asset = Address::generate(&env);

    let bad_amount = vector(
        &vectors,
        &["mux_account", "spend_limit"],
        "acct-spend-invalid-amount",
    );
    let amount = bad_amount["input"]["amount"].as_i64().unwrap() as i128;
    let period = bad_amount["input"]["period_ledgers"].as_u64().unwrap() as u32;
    assert_eq!(
        client.try_set_spend_limit(&asset, &amount, &period),
        Err(Ok(MuxAccountError::InvalidAmount))
    );
    assert_eq!(expect_err(bad_amount), "InvalidAmount");

    let bad_period = vector(
        &vectors,
        &["mux_account", "spend_limit"],
        "acct-spend-invalid-period",
    );
    let amount = bad_period["input"]["amount"].as_i64().unwrap() as i128;
    let period = bad_period["input"]["period_ledgers"].as_u64().unwrap() as u32;
    assert_eq!(
        client.try_set_spend_limit(&asset, &amount, &period),
        Err(Ok(MuxAccountError::InvalidPeriod))
    );
    assert_eq!(expect_err(bad_period), "InvalidPeriod");
}

// ── mux_account: delegate cap vectors (account_limit_vectors.json) ────────────

#[test]
fn vector_acct_delegate_cap_boundaries() {
    let limits = load(ACCOUNT_LIMIT_VECTORS_JSON);
    let under_cap = vector(
        &limits,
        &["mux_account", "delegate_limits"],
        "acct-dlg-under-cap",
    );
    let at_cap = vector(
        &limits,
        &["mux_account", "delegate_limits"],
        "acct-dlg-at-cap-reject",
    );

    let pre_existing = under_cap["input"]["pre_existing_delegates"]
        .as_u64()
        .unwrap() as u32;
    assert_eq!(
        at_cap["input"]["pre_existing_delegates"].as_u64().unwrap() as u32,
        pre_existing + 1,
        "fixtures must describe consecutive cap boundaries"
    );

    let (env, client) = setup_account();
    for i in 0..(pre_existing + 1) {
        let dlg = Address::generate(&env);
        client.set_delegate(&dlg, &(1_000_000 + i as u64), &true);
    }
    // The (pre_existing + 1)th delegate above lands exactly at cap (matches
    // "acct-dlg-under-cap" reaching delegate_count 64); one more must reject.
    let overflow = Address::generate(&env);
    assert_eq!(
        client.try_set_delegate(&overflow, &2_000_000_u64, &false),
        Err(Ok(MuxAccountError::TooManyDelegates))
    );
    assert_eq!(expect_err(at_cap), "TooManyDelegates");
}

// ── mux_account: session key cap vectors (account_limit_vectors.json) ─────────

#[test]
fn vector_acct_session_key_cap_boundaries() {
    let limits = load(ACCOUNT_LIMIT_VECTORS_JSON);
    let under_cap = vector(
        &limits,
        &["mux_account", "session_key_limits"],
        "acct-sk-under-cap",
    );
    let at_cap = vector(
        &limits,
        &["mux_account", "session_key_limits"],
        "acct-sk-at-cap-reject",
    );

    let pre_existing = under_cap["input"]["pre_existing_keys"].as_u64().unwrap() as u32;
    assert_eq!(
        at_cap["input"]["pre_existing_keys"].as_u64().unwrap() as u32,
        pre_existing + 1
    );

    let (env, client) = setup_account();
    for i in 0..(pre_existing + 1) {
        let sk = Address::generate(&env);
        client.register_session_key(&sk, &(1_000_000_u64 + i as u64), &Vec::new(&env));
    }
    let overflow = Address::generate(&env);
    assert_eq!(
        client.try_register_session_key(&overflow, &2_000_000_u64, &Vec::new(&env)),
        Err(Ok(MuxAccountError::TooManySessionKeys))
    );
    assert_eq!(expect_err(at_cap), "TooManySessionKeys");
}

// ── mux_batcher: batch size vectors (account_limit_vectors.json) ──────────────

fn make_ops(env: &Env, n: u32) -> Vec<Operation> {
    let mut ops = Vec::new(env);
    for _ in 0..n {
        ops.push_back(Operation {
            target: Address::generate(env),
            fn_name: symbol_short!("noop"),
            args: Vec::<Val>::new(env),
            require_success: false,
            kind: BatchOperationKind::Invoke,
        });
    }
    ops
}

#[test]
fn vector_batcher_size_limits_match_execute_batch() {
    let limits = load(ACCOUNT_LIMIT_VECTORS_JSON);
    let ids = [
        "bat-size-one",
        "bat-size-at-cap",
        "bat-size-one-over",
        "bat-size-empty",
    ];

    let env = Env::default();
    env.mock_all_auths();
    env.budget().reset_unlimited();
    let contract_id = env.register_contract(None, MuxBatcher);
    let client = MuxBatcherClient::new(&env, &contract_id);
    let caller = Address::generate(&env);

    for id in ids {
        let v = vector(&limits, &["mux_batcher", "batch_size_limits"], id);
        let n = v["input"]["ops_count"].as_u64().unwrap() as u32;
        let ops = make_ops(&env, n);
        let result = client.try_execute_batch(&caller, &ops);

        if expect_ok(v) {
            assert!(result.is_ok(), "vector `{id}` expected ok, got {result:?}");
        } else {
            let expected = match expect_err(v) {
                "BatchTooLarge" => MuxBatcherError::BatchTooLarge,
                "EmptyBatch" => MuxBatcherError::EmptyBatch,
                other => panic!("unexpected expect.err in fixture `{id}`: {other}"),
            };
            let err = result
                .err()
                .unwrap_or_else(|| panic!("vector `{id}` expected err, got Ok"))
                .unwrap_or_else(|e| panic!("vector `{id}` host-level error: {e:?}"));
            assert_eq!(err, expected, "vector `{id}`");
            let expected_code = v["expect"]["code"].as_u64().unwrap() as u32;
            assert_eq!(expected as u32, expected_code, "vector `{id}` code drift");
        }
    }
}

// ── mux_permissions: role lifecycle vectors (test_vectors.json) ───────────────

#[test]
fn vector_permissions_role_lifecycle() {
    let vectors = load(TEST_VECTORS_JSON);

    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxPermissions);
    let client = MuxPermissionsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let grant = vector(
        &vectors,
        &["mux_permissions", "role_lifecycle"],
        "perm-grant-ok",
    );
    let account = Address::generate(&env);
    let role = Symbol::new(&env, grant["input"]["role"].as_str().unwrap());
    let perm = Symbol::new(&env, grant["input"]["permissions"][0].as_str().unwrap());
    let mut perms: Vec<Symbol> = Vec::new(&env);
    perms.push_back(perm.clone());
    client.create_role(&role, &perms);

    assert!(client.try_grant_role(&account, &role).is_ok());
    assert_eq!(
        client.has_permission(&account, &perm),
        grant["expect"]["has_permission"].as_bool().unwrap()
    );

    let revoke = vector(
        &vectors,
        &["mux_permissions", "role_lifecycle"],
        "perm-revoke-ok",
    );
    assert!(client.try_revoke_role(&account, &role).is_ok());
    assert_eq!(
        client.has_permission(&account, &perm),
        revoke["expect"]["has_permission"].as_bool().unwrap()
    );

    let missing = vector(
        &vectors,
        &["mux_permissions", "role_lifecycle"],
        "perm-grant-nonexistent-role",
    );
    let ghost_role = Symbol::new(&env, missing["input"]["role"].as_str().unwrap());
    assert_eq!(expect_err(missing), "RoleNotFound");
    assert_eq!(
        client.try_grant_role(&account, &ghost_role),
        Err(Ok(MuxPermissionsError::RoleNotFound))
    );
}
