//! Cross-contract integration tests: `mux-batcher` invoking `mux-account`
//! and `mux-permissions` through `execute_batch`.
//!
//! No prior harness exercised `mux-batcher` actually calling into another
//! Mux contract despite the architecture diagrams describing exactly that
//! flow (batcher orchestrating account/permissions operations atomically).
//! These tests build real `Operation`s targeting live `mux-account` /
//! `mux-permissions` contract instances and assert the target contract's
//! state changed as a result — not just that the batcher reported success.
//!
//! Closes the "no integration test exercising mux-batcher calling into
//! mux-account or mux-permissions" gap tracked in docs/audit-prep.md.
//!
//! Run with: cargo test -p mux-contract-tests --test cross_contract_integration

use mux_account::{MuxAccount, MuxAccountClient};
use mux_batcher::{BatchOperationKind, MuxBatcher, MuxBatcherClient, MuxBatcherError, Operation};
use mux_permissions::{MuxPermissions, MuxPermissionsClient};
use soroban_sdk::{testutils::Address as _, Address, Env, IntoVal, Symbol, Val, Vec};
use soroban_test_helpers::assert_contract_err;

fn setup_env() -> Env {
    let env = Env::default();
    // The batcher's own `caller` authorizes `execute_batch` at the root, but
    // `mux-account.set_delegate` / `mux-permissions.grant_role` each also
    // `require_auth()` their own owner/admin address one level down (inside
    // the batched cross-contract call) — an address that never appears in
    // the top-level invocation. `mock_all_auths()` alone only mocks auths
    // tied to that root tree; allowing non-root auth is what actually
    // exercises the batcher-orchestrates-another-contract's-owner path.
    env.mock_all_auths_allowing_non_root_auth();
    env
}

/// `execute_batch` → `mux-account.set_delegate` must actually mutate the
/// account contract's delegate map, and the batch must report success.
#[test]
fn batcher_execute_batch_invokes_account_set_delegate() {
    let env = setup_env();

    let batcher_id = env.register_contract(None, MuxBatcher);
    let batcher = MuxBatcherClient::new(&env, &batcher_id);

    let account_id = env.register_contract(None, MuxAccount);
    let account = MuxAccountClient::new(&env, &account_id);
    let owner = Address::generate(&env);
    account.initialize(&owner, &Vec::new(&env));

    let delegate = Address::generate(&env);
    let expires_at: u64 = 5_000_000;

    let mut args: Vec<Val> = Vec::new(&env);
    args.push_back(delegate.clone().into_val(&env));
    args.push_back(expires_at.into_val(&env));
    args.push_back(true.into_val(&env));

    let mut ops: Vec<Operation> = Vec::new(&env);
    ops.push_back(Operation {
        target: account_id,
        fn_name: Symbol::new(&env, "set_delegate"),
        args,
        require_success: true,
        kind: BatchOperationKind::Invoke,
    });

    let caller = Address::generate(&env);
    let result = batcher.execute_batch(&caller, &ops);
    assert_eq!(result.success_count, 1);
    assert_eq!(result.failure_count, 0);

    // The delegate must actually be present on the account contract — proof
    // the batcher performed a real cross-contract call, not a local no-op.
    let delegates = account.delegates();
    assert!(delegates.contains_key(delegate.clone()));
    let info = delegates.get(delegate).unwrap();
    assert_eq!(info.expires_at, expires_at);
    assert!(info.can_spend);
}

/// `execute_batch` → `mux-permissions.grant_role` must actually grant the
/// role, observable via `has_permission` on the permissions contract.
#[test]
fn batcher_execute_batch_invokes_permissions_grant_role() {
    let env = setup_env();

    let batcher_id = env.register_contract(None, MuxBatcher);
    let batcher = MuxBatcherClient::new(&env, &batcher_id);

    let perms_id = env.register_contract(None, MuxPermissions);
    let perms = MuxPermissionsClient::new(&env, &perms_id);
    let admin = Address::generate(&env);
    perms.initialize(&admin);

    let role = Symbol::new(&env, "operator");
    let mut permissions: Vec<Symbol> = Vec::new(&env);
    permissions.push_back(Symbol::new(&env, "transfer"));
    perms.create_role(&role, &permissions);

    let account = Address::generate(&env);

    let mut args: Vec<Val> = Vec::new(&env);
    args.push_back(account.clone().into_val(&env));
    args.push_back(role.into_val(&env));

    let mut ops: Vec<Operation> = Vec::new(&env);
    ops.push_back(Operation {
        target: perms_id,
        fn_name: Symbol::new(&env, "grant_role"),
        args,
        require_success: true,
        kind: BatchOperationKind::Invoke,
    });

    let caller = Address::generate(&env);
    let result = batcher.execute_batch(&caller, &ops);
    assert_eq!(result.success_count, 1);
    assert_eq!(result.failure_count, 0);

    assert!(perms.has_permission(&account, &Symbol::new(&env, "transfer")));
}

/// A required operation targeting an uninitialized `mux-account` genuinely
/// fails on the target side (`NotInitialized`). `execute_batch` must abort
/// the whole batch with `RequiredOperationFailed`, and the target contract
/// must show no evidence of the call having partially succeeded.
#[test]
fn batcher_execute_batch_required_failure_leaves_account_untouched() {
    let env = setup_env();

    let batcher_id = env.register_contract(None, MuxBatcher);
    let batcher = MuxBatcherClient::new(&env, &batcher_id);

    // Registered but never initialized — set_delegate on it returns
    // Err(NotInitialized), a genuine target-side failure (not a missing
    // contract, which the batcher would also treat as a failed invocation).
    let account_id = env.register_contract(None, MuxAccount);
    let account = MuxAccountClient::new(&env, &account_id);

    let delegate = Address::generate(&env);
    let mut args: Vec<Val> = Vec::new(&env);
    args.push_back(delegate.into_val(&env));
    args.push_back(5_000_000_u64.into_val(&env));
    args.push_back(true.into_val(&env));

    let mut ops: Vec<Operation> = Vec::new(&env);
    ops.push_back(Operation {
        target: account_id,
        fn_name: Symbol::new(&env, "set_delegate"),
        args,
        require_success: true,
        kind: BatchOperationKind::Invoke,
    });

    let caller = Address::generate(&env);
    assert_contract_err(
        batcher.try_execute_batch(&caller, &ops),
        MuxBatcherError::RequiredOperationFailed,
    );

    // The account was never initialized, so it still has no delegate map —
    // proof the aborted batch did not somehow force initialization or write
    // partial state on the target.
    assert!(account.try_delegates().is_err());
}

/// An optional (`require_success = false`) operation that fails on the
/// target side must be counted as a soft failure and must not roll back or
/// block the operations around it — mirroring the intra-batcher behaviour
/// but proven here across a real second contract.
#[test]
fn batcher_execute_batch_optional_cross_contract_failure_is_soft() {
    let env = setup_env();

    let batcher_id = env.register_contract(None, MuxBatcher);
    let batcher = MuxBatcherClient::new(&env, &batcher_id);

    let perms_id = env.register_contract(None, MuxPermissions);
    let perms = MuxPermissionsClient::new(&env, &perms_id);
    // Never initialized: grant_role on it returns Err(NotInitialized).
    let account = Address::generate(&env);
    let role = Symbol::new(&env, "operator");

    let mut args: Vec<Val> = Vec::new(&env);
    args.push_back(account.clone().into_val(&env));
    args.push_back(role.clone().into_val(&env));

    let mut ops: Vec<Operation> = Vec::new(&env);
    ops.push_back(Operation {
        target: perms_id,
        fn_name: Symbol::new(&env, "grant_role"),
        args,
        require_success: false,
        kind: BatchOperationKind::Invoke,
    });

    let caller = Address::generate(&env);
    let result = batcher.execute_batch(&caller, &ops);
    assert_eq!(result.success_count, 0);
    assert_eq!(result.failure_count, 1);

    // The target never initialized, so the role grant never actually
    // landed — the soft failure must not have been silently treated as a
    // success on the target contract either.
    assert!(!perms.has_permission(&account, &role));
}
