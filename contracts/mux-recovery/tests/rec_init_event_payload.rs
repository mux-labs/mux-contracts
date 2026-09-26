//! Integration test pinning the `rec_init` audit-event payload.
//!
//! `docs/recovery-trust-model.md` §4.5 documents the payload as a five-tuple
//! `(guardian, new_owner, initiated_at, executable_at, expires_at)` carrying
//! the full timelock window, so off-chain watchers can surface deadlines
//! without a follow-up storage read. `docs/audit-events.md` and the
//! TypeScript bindings (`bindings/src/recovery-events.ts`) agree.
//!
//! This test pins that ABI: it fails if `initiate_recovery` ever regresses
//! to emitting only `(guardian, new_owner)` — the payload shape the trust
//! model previously (incorrectly) claimed.

use mux_recovery::{MuxRecovery, MuxRecoveryClient, RECOVERY_EXPIRY, RECOVERY_TIMELOCK};
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{vec, Address, Env, FromVal, Symbol};

#[test]
fn test_rec_init_event_payload_is_five_tuple() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let new_owner = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);
    let initiated_at = env.ledger().sequence();
    client.initiate_recovery(&guardian, &new_owner);

    let events = env.events().all();
    // init + rec_init
    assert_eq!(events.len(), 2);

    let (_, topics, data) = events.get(1).unwrap();
    // topics[0] = contract tag, topics[1] = action
    assert_eq!(
        Symbol::from_val(&env, &topics.get(1).unwrap()),
        Symbol::new(&env, "rec_init")
    );

    // The payload must be the full five-tuple, not just (guardian, new_owner).
    let (ev_guardian, ev_new_owner, ev_initiated, ev_executable, ev_expires): (
        Address,
        Address,
        u32,
        u32,
        u32,
    ) = FromVal::from_val(&env, &data);

    assert_eq!(ev_guardian, guardian);
    assert_eq!(ev_new_owner, new_owner);
    assert_eq!(ev_initiated, initiated_at);
    assert_eq!(ev_executable, initiated_at + RECOVERY_TIMELOCK);
    assert_eq!(ev_expires, initiated_at + RECOVERY_EXPIRY);
}
