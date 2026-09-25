use mux_recovery::MuxRecoveryClient;
use mux_recovery::RecoveryStatus;
use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::vec;
use soroban_sdk::{Address, Env, IntoVal};

// ── Happy path ────────────────────────────────────────────────────────────────

/// Owner + guardian co-sign: the fast path transfers ownership immediately.
#[test]
fn test_owner_and_guardian_approve_recovery_executes() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);

    // initialize contract with owner and one guardian (quorum 1-of-1)
    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);

    // guardian initiates recovery
    client.initiate_recovery(&guardian, &new_owner);

    // owner + guardian both authorize (mock_all_auths covers both)
    assert!(client.try_approve_recovery_admin(&guardian).is_ok());

    assert_eq!(client.owner(), new_owner);
    assert_eq!(client.recovery_status(), RecoveryStatus::Executed);
}

// ── Guardian co-sign is required ─────────────────────────────────────────────

/// Owner alone (no guardian co-sign) must be rejected — this is the key
/// security property: the timelock bypass requires both the owner AND a
/// guardian, so a compromised owner key alone cannot circumvent the 24 h
/// cancellation window.
#[test]
fn test_approve_recovery_admin_rejects_owner_alone_without_guardian_cosign() {
    let env = Env::default();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);

    // Initialize and initiate using mock_all_auths for setup.
    env.mock_all_auths();
    client.initialize(&owner, &vec![&env, guardian.clone()]);
    client.initiate_recovery(&guardian, &new_owner);

    // Restrict auth to only the owner; guardian does NOT co-sign.
    env.mock_auths(&[MockAuth {
        address: &owner,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "approve_recovery_admin",
            args: vec![&env, guardian.clone().into_val(&env)].into(),
            sub_invokes: &[],
        },
    }]);

    let result = client.try_approve_recovery_admin(&guardian);
    assert!(
        result.is_err(),
        "owner alone must not be able to bypass the timelock — guardian co-sign is required"
    );
    assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    assert_eq!(client.owner(), owner);
}

/// Passing a stranger address as `co_guardian` (even if the stranger signs)
/// must be rejected — the address must be a registered guardian.
#[test]
fn test_approve_recovery_admin_rejects_non_guardian_cosigner() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let stranger = Address::generate(&env);
    let new_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()]);
    client.initiate_recovery(&guardian, &new_owner);

    // Even with mock_all_auths, the guardian-membership check must reject stranger.
    let result = client.try_approve_recovery_admin(&stranger);
    assert!(
        result.is_err(),
        "a non-guardian co-signer must be rejected"
    );
    assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
}

/// A guardian holding a perfectly valid signature over `approve_recovery_admin`
/// must not be able to substitute for the owner: the entrypoint is gated by
/// `require_owner`, which checks the *owner* address specifically.
#[test]
fn test_approve_recovery_admin_rejects_guardian_only_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);
    client.initiate_recovery(&guardian, &new_owner);

    // Only the guardian signs; the owner does NOT authorize.
    env.mock_auths(&[MockAuth {
        address: &guardian,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "approve_recovery_admin",
            args: vec![&env, guardian.clone().into_val(&env)].into(),
            sub_invokes: &[],
        },
    }]);

    let result = client.try_approve_recovery_admin(&guardian);
    assert!(
        result.is_err(),
        "guardian's own valid signature must not satisfy require_owner"
    );
    assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    assert_eq!(client.owner(), owner);
}

/// With no authorization present at all, the timelock bypass must be rejected.
#[test]
fn test_approve_recovery_admin_rejects_when_no_auth_present() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);
    client.initiate_recovery(&guardian, &new_owner);

    env.set_auths(&[]);

    let result = client.try_approve_recovery_admin(&guardian);
    assert!(
        result.is_err(),
        "approve_recovery_admin must reject when no auth is present"
    );
    assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
}

// ── Interaction with the guardian-initiated flow ─────────────────────────────

/// The admin-approve fast path only ever approves an *existing*,
/// guardian-initiated request — the owner cannot fabricate one unilaterally
/// without a guardian having called `initiate_recovery` first.
#[test]
fn test_approve_recovery_admin_without_pending_recovery_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian], &1_u32);

    let result = client.try_approve_recovery_admin(&guardian);
    assert!(
        result.is_err(),
        "approve_recovery_admin must reject with no active recovery request"
    );
    assert_eq!(client.recovery_status(), RecoveryStatus::None);
}

/// Once the owner cancels a guardian-initiated recovery, the admin-approve
/// fast path must no longer apply to that (now-cancelled) request.
#[test]
fn test_approve_recovery_admin_rejects_after_owner_cancels() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);
    client.initiate_recovery(&guardian, &new_owner);
    client.cancel_recovery();

    let result = client.try_approve_recovery_admin(&guardian);
    assert!(
        result.is_err(),
        "approve_recovery_admin must reject a cancelled recovery request"
    );
    assert_eq!(client.recovery_status(), RecoveryStatus::Cancelled);
    assert_eq!(client.owner(), owner);
}

/// After the owner uses the admin-approve fast path, the request is
/// terminal (`Executed`): the guardian must not subsequently be able to
/// `execute_recovery` the same request again.
#[test]
fn test_guardian_cannot_execute_recovery_after_admin_approve() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, mux_recovery::MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);
    client.initiate_recovery(&guardian, &new_owner);

    assert!(client.try_approve_recovery_admin(&guardian).is_ok());
    assert_eq!(client.recovery_status(), RecoveryStatus::Executed);

    let result = client.try_execute_recovery(&guardian);
    assert!(
        result.is_err(),
        "execute_recovery must reject once the request was already admin-approved"
    );
    assert_eq!(client.owner(), new_owner);
    assert_eq!(client.recovery_status(), RecoveryStatus::Executed);
}
