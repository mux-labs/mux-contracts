# Mainnet Immutable Flag Guidance

**Version:** 1.0.0  
**Date:** 2026-08-25  
**Status:** Final (Audit Ready)  
**Issue:** #683  
**Related:** [Mainnet Deploy Checklist](mainnet-deploy-checklist.md), [Contract Upgrade Pattern](contract-upgrade-pattern.md)

---

## Purpose

This document defines when and how Mux Protocol contracts should be treated as immutable on mainnet, and provides guidance for contracts that intentionally expose upgrade paths versus those that do not.

---

## Immutability model

On Soroban, contracts are **immutable by default** — WASM bytecode cannot be changed after deployment unless the contract explicitly exposes an `upgrade()` entry point. This is a security property, not a limitation.

| Immutability level | Meaning | When to use |
|--------------------|---------|-------------|
| **Fully immutable** | No `upgrade()` function; contract code can never change | Core contracts with stable interfaces, audited and battle-tested |
| **Opt-in upgradeable** | `upgrade()` exists but is gated by an admin that only exists if `initialize()` was called; never calling `initialize()` leaves the deployment exactly as immutable as "fully immutable" | Contracts whose interface is stable today but where the deploying team wants the *option* to react to an audit finding without a full redeploy + state migration (mux-batcher, mux-delegation, mux-permissions) |
| **Conditionally upgradeable** | `upgrade()` exists, gated by a **mandatory** admin set at `initialize()` time | Contracts where feature evolution is expected (e.g., policy rules) |
| **Proxied / replaceable** | Contract delegates to an implementation contract that can be swapped | Complex systems needing hot-swap without state migration |

**Opt-in vs. conditionally upgradeable — the difference that matters:** both
have a live `upgrade()` function in the deployed WASM. The distinction is
whether the admin gating it is guaranteed to exist. `mux-policy`'s
`initialize()` is mandatory — every deployment has an admin from the first
transaction, so `mux-policy` is upgradeable from day one. `mux-batcher`,
`mux-delegation`, and `mux-permissions`'s admin comes from a *separate,
optional* `initialize()` call — a deployment that never calls it has no
`DataKey::Admin`, so `upgrade()` unconditionally returns `NotInitialized`
and the deployment is immutable in exactly the sense Option A below
describes. The immutability guarantee is a **deploy-time operational
choice** (call `initialize()` or don't), not a property you can read off the
WASM alone — see "Verification" below for how to confirm which choice a
given deployment made.

---

## Current contract immutability status

| Contract | Upgradeable | Rationale |
|----------|-------------|-----------|
| `mux-account` | **No** | Core AA logic; immutability is a user trust guarantee |
| `mux-account-factory` | **No** | Factory registration logic is stable; no evolution expected |
| `mux-batcher` | **Opt-in** (immutable unless `initialize()` is called) | Atomic batching semantics are fixed by protocol design — the deploying team is not expected to call `initialize()` on mainnet, but the escape hatch exists. See [batcher-upgrade.md](batcher-upgrade.md). |
| `mux-delegation` | **Opt-in** (immutable unless `initialize()` is called) | Delegate permission model is audited and stable — same opt-in rationale as `mux-batcher`. See [delegation-upgrade.md](delegation-upgrade.md). |
| `mux-permissions` | **Opt-in for `upgrade()` specifically** — `initialize()` is mandatory for RBAC to function at all, but calling `upgrade()` is a separate, later choice by whoever holds the admin key | RBAC registry with a stable interface today; the admin already required for role management can also authorise a WASM replace if ever needed. See [permissions-upgrade-migration.md](permissions-upgrade-migration.md). |
| `mux-policy` | **Yes** | Policy rules may evolve; admin-gated `upgrade()` exposed, admin is mandatory from `initialize()` |
| `mux-recovery` | **No** | Recovery timelock is time-critical; upgrade path requires careful design |
| `mux-registry` | **No** | Version registry with stable interface |
| `mux-spending-policy` | **No** | Spend limit logic is stable |
| `mux-wallet-registry` | **No** | Wallet registration is stable |

**Mainnet deployment guidance for the three opt-in contracts:** to deploy
`mux-batcher` or `mux-delegation` as *fully* immutable (matching the
"Atomic batching semantics are fixed" / "audited and stable" rationale
above), simply never call `initialize()` — do not do this by accident, since
there is no way to un-set an admin once one is set (no
`renounce_upgrade_authority()` exists for these three; see Option C below if
that guarantee is needed). `mux-permissions` always has an admin (mandatory
for RBAC), so its immutability decision reduces to: does the team intend to
ever call `upgrade()`? Document that decision explicitly in the deploy
runbook and record it in the "Immutability registry" below regardless of
which way it goes.

---

## Making a contract immutable on mainnet

### Option A: Do not expose `upgrade()` (strongest guarantee)

Simply omit the `upgrade()` function from the contract. Without it, there is no on-chain mechanism to replace the WASM. The contract is immutable from the moment it is deployed. This is how `mux-account`, `mux-account-factory`, `mux-recovery`, `mux-registry`, `mux-spending-policy`, and `mux-wallet-registry` remain immutable — they have no `upgrade()` in the ABI at all.

```rust
// No upgrade() function means no way to change the code.
// This is the strongest immutability guarantee.
```

### Option A′: Never call the optional `initialize()` (mux-batcher, mux-delegation)

`mux-batcher` and `mux-delegation` ship with `upgrade()` in the ABI, but it
is gated by an admin that is only ever set by a separate, optional
`initialize(admin)` call. If mainnet deployment never calls `initialize()`,
`upgrade()` unconditionally returns `NotInitialized` — there is no admin to
authorise it, so the deployment is immutable in practice even though
`upgrade()` is present in the WASM. This is weaker than Option A only in
that the escape hatch exists in the deployed code; it is equivalent in that
no one can actually invoke it without first getting an `initialize()`
transaction signed and submitted, which is itself an auditable on-chain
event. Prefer this over Option B when the team wants to preserve the option
to upgrade later without a redeploy.

### Option B: Remove `upgrade()` before mainnet deploy

If a contract currently has `upgrade()` for testing but should be immutable on mainnet:

1. Remove the `upgrade()` function from the `#[contractimpl]` block
2. Optionally remove `DataKey::Admin` if it is only used for upgrade auth
3. Rebuild WASM with `cargo build --target wasm32-unknown-unknown --release`
4. Deploy the new WASM to mainnet (this is the only deploy — no prior upgrade path exists)

### Option C: Admin-revocable upgrade authority

For contracts where upgrade authority should exist but be revocable:

1. Store admin in instance storage during `initialize()`
2. Add a `renounce_upgrade_authority()` function that:
   - Calls `require_admin()`
   - Removes `DataKey::Admin` from storage
   - Once removed, `upgrade()` can never succeed again

```rust
pub fn renounce_upgrade_authority(env: Env) -> Result<(), ContractError> {
    Self::require_admin(&env)?;
    env.storage().instance().remove(&DataKey::Admin);
    // Note: upgrade() will now always fail because require_admin() reads from
    // DataKey::Admin which no longer exists.
    Ok(())
}
```

---

## Resolving the immutability vs upgrade() conflict

### The core tension

The presence of an `upgrade()` function in a contract's WASM appears to conflict with immutability claims. However, **having `upgrade()` in the code does not make a contract upgradeable in practice** — it depends on whether the admin authorization path can be satisfied.

### Three patterns for Mux contracts

#### Pattern 1: No upgrade() function (strongest immutability)

Contracts: `mux-account`, `mux-account-factory`, `mux-recovery`, `mux-registry`, `mux-spending-policy`, `mux-wallet-registry`

**Guarantee:** WASM cannot be changed because no entry point exists to invoke `update_current_contract_wasm()`.

**Verification:**
```bash
# Confirm upgrade() is not in the contract ABI
stellar contract inspect --wasm target/wasm32-unknown-unknown/release/mux_account.wasm \
  | grep -q "upgrade" && echo "ERROR: upgrade found" || echo "OK: no upgrade"
```

#### Pattern 2: upgrade() with optional admin (opt-in upgradability)

Contracts: `mux-batcher`, `mux-delegation`

**Mechanism:**
- `initialize(admin)` is optional — the contract works without it
- If `initialize()` is never called, `DataKey::Admin` is never written
- `upgrade()` calls `require_admin()` which reads `DataKey::Admin`
- Without admin in storage, `upgrade()` always returns `NotInitialized`

**Immutability guarantee:** Deployment is immutable if `initialize()` is never called.

**Mainnet deployment guidance:**
```bash
# Deploy the contract
stellar contract deploy --wasm mux_batcher.wasm --network mainnet

# DO NOT call initialize() if you want immutability
# The contract is now functionally immutable despite having upgrade() in WASM
```

**Verification:**
```bash
# Confirm initialize() was never called by checking for Admin key
# If this call fails with NotInitialized or KeyNotFound, deployment is immutable
stellar contract invoke \
  --id $CONTRACT_ID \
  --network mainnet \
  -- \
  admin

# Or check event history for 'init' events
stellar contract events --id $CONTRACT_ID --network mainnet \
  | grep -q "init" && echo "WARNING: initialized" || echo "OK: never initialized"
```

#### Pattern 3: upgrade() with mandatory admin (deliberate upgradeability)

Contracts: `mux-policy`, `mux-permissions`

**Mechanism:**
- `initialize(admin)` is mandatory for the contract to function
- Admin is always present in storage after initialization
- `upgrade()` can always be called by the admin

**Immutability approach:** Use Option C (renounce authority) if immutability is desired post-deploy.

**For mux-policy specifically:**
The policy contract is designed for evolution — daily spend limits may need adjustments based on market conditions or governance decisions. The `upgrade()` function is intentional and should remain callable.

**For mux-permissions specifically:**
Admin is mandatory for RBAC operations, so `DataKey::Admin` always exists. If immutability is desired, deploy with an admin that commits to never calling `upgrade()`, or add a `renounce_upgrade_authority()` function (see Option C).

### Decision tree for mainnet deployment

```
Does the contract have upgrade() in WASM?
│
├─ No → Deploy as-is (Pattern 1: fully immutable)
│
└─ Yes → Is initialize(admin) mandatory for core functionality?
    │
    ├─ No (mux-batcher, mux-delegation)
    │   └─ Want immutability?
    │       ├─ Yes → Never call initialize() (Pattern 2: opt-in)
    │       └─ No → Call initialize(admin) after deploy
    │
    └─ Yes (mux-policy, mux-permissions)
        └─ Want immutability eventually?
            ├─ Yes → Add renounce_upgrade_authority() (Option C)
            └─ No → Accept upgradeable design (Pattern 3)
```

### Audit checklist for immutability claims

For each contract claiming to be immutable on mainnet:

- [ ] **Pattern 1:** Confirm `upgrade()` is not in WASM ABI
  - Method: `stellar contract inspect --wasm <file> | grep upgrade`
  - Expected: No matches

- [ ] **Pattern 2:** Confirm `initialize()` was never called
  - Method: Query admin key on deployed contract
  - Expected: `NotInitialized` or `KeyNotFound` error

- [ ] **Pattern 3:** If claiming eventual immutability, confirm `renounce_upgrade_authority()` exists and was called
  - Method: Check event logs for `upgrade_authority_renounced` event
  - Expected: Event present with timestamp

- [ ] **All patterns:** WASM hash recorded in CONTRACT_IDS.md
- [ ] **All patterns:** Deployment added to immutability registry (below)
- [ ] **All patterns:** Immutability decision documented in mainnet deploy runbook

---

## Why immutability matters on mainnet

| Concern | How immutability helps |
|---------|----------------------|
| **User trust** | Users can verify the contract code matches the audited WASM hash and know it cannot change |
| **Composability** | Other protocols can safely integrate with a known, fixed interface |
| **Audit scope** | Auditors review a fixed codebase; no risk of post-audit behaviour change |
| **Regulatory** | Immutable contracts have clearer legal treatment in some jurisdictions |
| **Attack surface** | No upgrade key to compromise; no admin auth to phish for upgrades |

---

## Trade-offs

| Factor | Immutable | Upgradeable |
|--------|-----------|-------------|
| Bug fixes | Requires redeploy + state migration | In-place WASM swap |
| Feature evolution | Redeploy + migrate users | Add functions in new WASM |
| User confidence | Higher (code is fixed) | Lower (admin could change behaviour) |
| Operational complexity | Higher for protocol changes | Lower for hot-fixes |

---

## Recommended approach by contract type

| Contract type | Recommendation |
|---------------|----------------|
| Core accounting (mux-account) | **Immutable** — user funds depend on fixed logic |
| Factory / registry (mux-account-factory, mux-registry) | **Immutable** — registration logic is stable |
| Policy / spending (mux-policy) | **Conditionally upgradeable** — rules may evolve with governance |
| Recovery (mux-recovery) | **Immutable** — timelock logic is security-critical |
| Permissions (mux-permissions) | **Immutable in practice** — `upgrade()` exists (admin is mandatory for RBAC), but the recommendation is to never call it absent an audit finding |
| Batcher (mux-batcher) | **Immutable by default** — do not call `initialize()` on mainnet unless the upgrade escape hatch is deliberately wanted |
| Delegation (mux-delegation) | **Immutable by default** — do not call `initialize()` on mainnet unless the upgrade escape hatch is deliberately wanted |

---

## Verification

After deploying an immutable contract on mainnet:

1. Record the WASM hash in `CONTRACT_IDS.md`
2. Verify the hash matches the built WASM: `scripts/verify-wasm-hash.sh`
3. Confirm no `upgrade()` function exists in the contract ABI — for
   `mux-batcher` and `mux-delegation`, `upgrade()` exists in the ABI by
   design (see Option A′ above), so instead confirm `initialize()` was never
   called: `get_registry_metadata()` / equivalent read calls succeeding is
   not sufficient evidence; check that an `init` event was never emitted for
   the deployed contract ID via `getEvents`
4. Document the immutability decision in release notes
5. Add the contract to the immutability registry below

---

## Immutability registry

Once a contract is confirmed immutable on mainnet, record it here:

| Contract | Network | WASM hash | Deploy date | Immutable since |
|----------|---------|-----------|-------------|-----------------|
| | | | | |

---

## References

- [Mainnet Deploy Checklist](mainnet-deploy-checklist.md)
- [Contract Upgrade Pattern](contract-upgrade-pattern.md)
- [Soroban Contract Docs](https://developers.stellar.org/docs/build/smart-contracts)
