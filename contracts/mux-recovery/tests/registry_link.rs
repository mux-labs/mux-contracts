//! Integration tests for the `set_registry` / `registry_id` round-trip and
//! the `reg_link` audit event (issue #933).
//!
//! `contracts/mux-recovery/src/lib.rs` documents an optional registry link:
//! "An optional registry contract address can be associated with this
//! recovery contract via `set_registry`. The stored address is readable via
//! `registry_id` (returns `None` if not set)." The header also lists a
//! `reg_link` audit event with payload `registry_id: Address` under the
//! `mux_recv` tag.
//!
//! `set_registry` is fail-closed: it cross-contract-invokes `list_contracts`
//! on the supplied address and rejects with `RecoveryError::RegistryNotFound`
//! when that call fails. These tests pin:
//! 1. `registry_id()` is `None` before any `set_registry` call;
//! 2. `registry_id()` returns the address passed to `set_registry` against a
//!    live registry, and a `reg_link` event is published under `mux_recv`
//!    with the registry address as payload;
//! 3. a `set_registry` call against a non-contract address fails with
//!    `RegistryNotFound` and leaves `registry_id()` unchanged;
//! 4. a non-owner `set_registry` call is rejected before any storage write.

use mux_recovery::{MuxRecovery, MuxRecoveryClient, RecoveryError};
use soroban_sdk::testutils::{Address as _, Events, MockAuth, MockAuthInvoke};
use soroban_sdk::{contract, contractimpl, vec, Address, Env, FromVal, IntoVal, Symbol};

/// Minimal live registry that satisfies `list_contracts` during the
/// cross-contract validation inside `set_registry`.
mod mock_registry {
    use super::*;

    #[contract]
    pub struct MockRegistry;

    #[contractimpl]
    impl MockRegistry {
        pub fn list_contracts(env: Env) -> soroban_sdk::Vec<Symbol> {
            soroban_sdk::Vec::new(&env)
        }
    }
}
use mock_registry::MockRegistry;

/// Count `reg_link` events (topic[1] == `reg_link`) in the full event log.
fn reg_link_count(env: &Env) -> u32 {
    let mut count = 0_u32;
    for (_, topics, _) in env.events().all().iter() {
        if topics.len() >= 2
            && Symbol::from_val(env, &topics.get(1).unwrap()) == Symbol::new(env, "reg_link")
        {
            count += 1;
        }
    }
    count
}

/// `registry_id()` must return `None` before any `set_registry` call.
#[test]
fn test_registry_id_is_none_before_set_registry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);

    assert_eq!(
        client.registry_id(),
        None,
        "registry_id must be None before any set_registry call"
    );
}

/// After `set_registry(addr)` against a live registry, `registry_id()` returns
/// `Some(addr)` and a `reg_link` event with payload `addr` is published under
/// the `mux_recv` tag.
#[test]
fn test_set_registry_round_trip_and_reg_link_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let registry_addr: Address = env.register_contract(None, MockRegistry);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);

    assert_eq!(client.registry_id(), None);

    client.set_registry(&owner, &registry_addr);

    assert_eq!(
        client.registry_id(),
        Some(registry_addr.clone()),
        "registry_id must return the address passed to set_registry"
    );

    // Verify the reg_link event was published with the registry address payload.
    let events = env.events().all();
    let mut found_reg_link = false;
    for (_, topics, data) in events.iter() {
        if topics.len() >= 2
            && Symbol::from_val(&env, &topics.get(1).unwrap()) == Symbol::new(&env, "reg_link")
        {
            found_reg_link = true;
            let payload: Address = FromVal::from_val(&env, &data);
            assert_eq!(
                payload, registry_addr,
                "reg_link event payload must be the registry address"
            );
        }
    }
    assert!(
        found_reg_link,
        "a reg_link event must be emitted by set_registry"
    );
    assert_eq!(
        reg_link_count(&env),
        1,
        "exactly one reg_link event must exist"
    );
}

/// `set_registry` against an address that is not a live registry must fail
/// with `RegistryNotFound` and leave `registry_id()` unchanged (fail-closed).
#[test]
fn test_set_registry_with_non_contract_address_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let not_a_contract = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);

    let err = client
        .try_set_registry(&owner, &not_a_contract)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        RecoveryError::RegistryNotFound,
        "set_registry must reject an address that fails list_contracts validation"
    );

    assert_eq!(
        client.registry_id(),
        None,
        "registry_id must remain None after a rejected set_registry"
    );
    assert_eq!(
        reg_link_count(&env),
        0,
        "no reg_link event may be emitted by a rejected set_registry"
    );
}

/// A non-owner `set_registry` call must fail before any storage write.
#[test]
fn test_non_owner_set_registry_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MuxRecovery);
    let client = MuxRecoveryClient::new(&env, &contract_id);

    let registry_addr: Address = env.register_contract(None, MockRegistry);

    let owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.initialize(&owner, &vec![&env, guardian.clone()], &1_u32);

    // Restrict auth to the stranger — a signed-for, but non-owner address.
    env.mock_auths(&[MockAuth {
        address: &stranger,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "set_registry",
            args: vec![
                &env,
                owner.clone().into_val(&env),
                registry_addr.clone().into_val(&env),
            ]
            .into(),
            sub_invokes: &[],
        },
    }]);

    let result = client.try_set_registry(&owner, &registry_addr);
    assert!(
        result.is_err(),
        "set_registry must reject a non-owner caller"
    );

    // registry_id() must remain None — the rejected call must not persist.
    assert_eq!(
        client.registry_id(),
        None,
        "registry_id must remain None after a rejected set_registry"
    );
}
