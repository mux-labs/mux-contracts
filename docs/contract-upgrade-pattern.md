# Contract Upgrade Pattern

This document describes the safe contract upgrade pattern used across Mux Protocol's Soroban contracts.

## Overview

Soroban contracts are immutable once deployed — their WASM bytecode cannot be changed. Upgrades
work by uploading new WASM to the ledger and then calling `upgrade()` on the deployed contract
instance, which atomically replaces the running code with the new WASM hash.

## Upgrade Flow

1. **Build new contract WASM**
   ```bash
   cargo build --target wasm32-unknown-unknown --release
   ```

2. **Upload new WASM to the network** (returns `new_wasm_hash`)
   ```bash
   stellar contract upload \
     --wasm target/wasm32-unknown-unknown/release/<contract>.wasm \
     --source $DEPLOYER_ACCOUNT \
     --network $NETWORK
   ```

3. **Call `upgrade()` on the live contract instance**
   ```bash
   stellar contract invoke \
     --id $CONTRACT_ID \
     --source $ADMIN_ACCOUNT \
     --network $NETWORK \
     -- upgrade \
     --new_wasm_hash $NEW_WASM_HASH
   ```

4. **Verify the upgrade**
   The contract instance now runs the new WASM at the same address.
   Run post-upgrade smoke tests.

## Soroban Contract-Side Implementation

Every upgradeable Mux contract must implement the `upgrade` entry point:

```rust
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

#[contract]
pub struct MuxContract;

#[contractimpl]
impl MuxContract {
    /// Upgrade the contract WASM.
    /// Only the admin stored in contract storage may call this.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        // 1. Authorise — only admin may upgrade
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not initialised");
        admin.require_auth();

        // 2. Atomically update the running WASM
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
```

### Key invariants

| Invariant | Why it matters |
|-----------|---------------|
| `require_auth()` on admin before `update_current_contract_wasm()` | Prevents unauthorised upgrades |
| Admin set during `initialize()`, never overwritten without auth | Ensures admin key rotation is itself protected |
| No storage migration in `upgrade()` itself | Storage layout must be backward-compatible, or migrated in a separate `migrate()` call |

## Storage Compatibility Rules

When changing storage layout between versions:

1. **Add fields only** — never remove or rename existing `DataKey` variants.
   Existing ledger entries remain valid after the upgrade.
2. **Use `Option<T>` for new fields** — allows old entries to deserialise
   without the new field.
3. **If removal is required** — implement a `migrate(env: Env)` function that
   rewrites entries before the new code path reads them.

```rust
pub fn migrate(env: Env) {
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin).expect("not initialised");
    admin.require_auth();

    // Example: rename OldKey → NewKey
    if let Some(val) = env.storage().persistent().get::<_, OldType>(&DataKey::OldKey) {
        env.storage().persistent().set(&DataKey::NewKey, &val);
        env.storage().persistent().remove(&DataKey::OldKey);
    }
}
```

## Upgrade Checklist

Before every production upgrade:

- [ ] New WASM hash verified with `scripts/verify-wasm-hash.sh` (see #113)
- [ ] All existing tests pass against the new WASM
- [ ] Storage layout changes are backward-compatible or a `migrate()` function is ready
- [ ] Testnet deploy completed and smoke-tested (see #110)
- [ ] Admin key is available and hardware-secured
- [ ] Upgrade transaction simulated (`--fee-bump` if needed)
- [ ] Rollback plan documented (prior WASM hash retained)

## Rollback

Soroban does not natively support rolling back an upgrade. Mitigation:

1. **Keep the previous WASM hash** — it is always reuploaded if needed (WASM is
   content-addressed; the hash is permanent).
2. **Call `upgrade()` again** with the prior hash to revert.
3. **If storage was migrated** — run a reverse `migrate()` that was prepared before
   the upgrade.

## Testing Upgrades in CI

Add an integration test that:

1. Deploys contract v1 to a local Soroban sandbox.
2. Registers state (stores entries, calls functions).
3. Uploads contract v2 WASM and calls `upgrade()`.
4. Asserts all v1 state is readable from v2.
5. Asserts new v2 behaviour is correct.

```rust
#[cfg(test)]
mod upgrade_tests {
    use soroban_sdk::{testutils::Address as _, Address, Env};
    use crate::MuxContractClient;

    #[test]
    fn upgrade_preserves_state() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy v1
        let admin = Address::generate(&env);
        let contract_id = env.register_contract(None, super::MuxContract);
        let client = MuxContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Record state before upgrade
        let state_before = client.get_state();

        // Upload v2 WASM (in tests, this is the same binary — replace with v2 in CI)
        let new_wasm_hash = env.deployer().upload_contract_wasm(super::WASM);
        client.upgrade(&new_wasm_hash);

        // Assert state preserved
        assert_eq!(client.get_state(), state_before);
    }
}
```

## References

- [Soroban Contract Upgrade Docs](https://developers.stellar.org/docs/build/smart-contracts/example-contracts/upgradeable-contract)
- [Stellar CLI: `contract upload`](https://developers.stellar.org/docs/tools/stellar-cli)
- Mux deployment scripts: `scripts/deploy-testnet.sh` (see #110)
- Mux WASM hash verification: `scripts/verify-wasm-hash.sh` (see #113)

## v0.1.0 → Unreleased: Registry Upgrade Migration Notes

### mux-registry

No breaking storage changes in this cycle. The `DataKey` enum gained two new
variants (`Metadata(Symbol)` for rich metadata and `Names` for the contract-name
index); both are **additive** — existing ledger entries remain valid.

**If you are upgrading a live `mux-registry` instance:**

1. Upload the new WASM and call `upgrade(new_wasm_hash)` as the admin.
2. No `migrate()` call is required — the `Names` vec is bootstrapped lazily if
   absent, and `Metadata` entries are optional (reads fall back to
   `ContractNotFound` rather than panicking).
3. Verify with `get_version` and `list_contracts` that pre-upgrade registrations
   are still readable.

### mux-wallet-registry

No storage layout changes in v0.1.0. The `DataKey::Wallet(Symbol)` key is
unchanged. No migration step is needed when upgrading to any patch release in
this series.

**General rule for both registries:** upgrade → smoke-test → keep prior WASM
hash for rollback. See the [Rollback](#rollback) section above.

### mux-account

No breaking storage changes in this cycle. The `DataKey` enum gained one new
variant (`Metadata` for optional `RegistryMeta`); it is **additive** — existing
ledger entries remain valid.

**If you are upgrading a live `mux-account` instance:**

1. Upload the new WASM and call `upgrade(new_wasm_hash)` as the owner (once
   upgrade support is enabled on the contract).
2. No `migrate()` call is required — `Metadata` is optional and reads return
   `None` when unset.
3. Verify with `owner`, `delegates`, and `get_metadata` that pre-upgrade state
   is still readable.

See [docs/account-upgrade-migration.md](./account-upgrade-migration.md) for the
full storage layout and breaking-change checklist.
