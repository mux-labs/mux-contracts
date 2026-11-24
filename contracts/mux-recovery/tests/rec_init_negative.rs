//! Negative-path tests for `initiate_recovery` (issue #952).
//!
//! `docs/recovery-trust-model.md` §4.5 documents `rec_init` as the audit
//! event that marks a recovery entering `Pending`. The happy path is pinned
//! by `rec_init_event_payload.rs`; this file proves the event stays silent
//! when `initiate_recovery` is rejected, so off-chain watchers can never
//! surface a recovery that never actually entered the queue:
//!
//! 1. a non-guardian caller is rejected and no `rec_init` is emitted;
//! 2. a second `initiate_recovery` while one is already `Pending` fails and
//!    emits no additional `rec_init`.

use mux_recovery::{MuxRecovery, MuxRecoveryClient, RecoveryError, RecoveryStatus};
use soroban_sdk::testutils::{Address as _, Events, MockAuth, MockAuthInvoke};
use soroban_sdk::{vec, Address, Env, FromVal, IntoVal, Symbol};

/// Count `rec_init` events (topic[1] == `rec_init`) in the full event log.
fn rec_init_count(env: &Env) -> u32 {
    let mut count = 0_u32;
    for (_, topics, _) in env.events().all().iter() {
        if topics.len() >= 2
            && Symbol::from_val(env, &topics.get(1).unwrap()) == Symbol::new(env, "rec_init")
        {
            count += 1;
        }
    }
    count
}

/// A caller that is not a registered guardian must be rejected, and the
/// rejected call must not emit `rec_init`.
#[test]
fn test_initiate_recovery_by_non_guardian_rejected_without_rec_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let stranger = Address::generate(&env);
    let new_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);

    // Restrict auth to the stranger — a signed-for, but non-guardian address.
    env.mock_auths(&[MockAuth {
        address: &stranger,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "initiate_recovery",
            args: vec![
                &env,
                guardian.clone().into_val(&env),
                new_owner.clone().into_val(&env),
            ]
            .into(),
            sub_invokes: &[],
        },
    }]);

    let result = client.try_initiate_recovery(&guardian, &new_owner);
    assert!(
        result.is_err(),
        "initiate_recovery must reject a non-guardian caller"
    );

    // The recovery never entered PENDING and no rec_init was published.
    assert_eq!(client.recovery_status(), RecoveryStatus::None);
    assert_eq!(
        rec_init_count(&env),
        0,
        "no rec_init event may be emitted for a rejected initiate_recovery"
    );
}

/// A second `initiate_recovery` while one is already pending must fail with
/// `RecoveryAlreadyPending` and emit no additional `rec_init`.
#[test]
fn test_second_initiate_recovery_while_pending_emits_no_additional_rec_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let other_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);

    // First initiation succeeds and emits exactly one rec_init.
    client.initiate_recovery(&guardian, &new_owner);
    assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    assert_eq!(rec_init_count(&env), 1);

    let events_before = env.events().all().len();

    let err = client
        .try_initiate_recovery(&guardian, &other_owner)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        RecoveryError::RecoveryAlreadyPending,
        "a second initiate_recovery must be rejected while a recovery is pending"
    );

    // Status is unchanged, the original request stands, and no extra event —
    // in particular no second rec_init — was emitted by the rejected call.
    assert_eq!(client.recovery_status(), RecoveryStatus::Pending);
    assert_eq!(
        rec_init_count(&env),
        1,
        "the rejected call must not emit an additional rec_init event"
    );
    assert_eq!(env.events().all().len(), events_before);
}
