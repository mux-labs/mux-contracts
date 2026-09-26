# mux-account Upgrade Migration Notes

## Decision: mux-account is permanently immutable

**There is no `upgrade()` entry point in `mux-account` and none will be added.**
This is a deliberate, permanent policy decision — not a temporary gap.

- [`docs/mainnet-immutable-flag-guidance.md`](mainnet-immutable-flag-guidance.md)
  classifies `mux-account` as **immutable by design**: "core AA logic; immutability
  is a user trust guarantee."
- [`docs/upgrade-auth-requirements.md`](upgrade-auth-requirements.md) records the
  per-contract upgrade status table; `mux-account` is listed as
  **No (immutable by design)**.

Do not plan around calling `upgrade()` on a live `mux-account` instance — it does
not exist and will not be introduced. (For contracts that **do** support in-place
upgrades — e.g. `mux-policy`, `mux-permissions` — see
[docs/contract-upgrade-pattern.md](./contract-upgrade-pattern.md).)

## `mux-account` is immutable — there is no in-place upgrade path

`mux-account` has no `upgrade()` or `migrate()` entry point in
[`contracts/mux-account/src/lib.rs`](../contracts/mux-account/src/lib.rs).
This is intentional, permanent policy, not a temporary gap:
[`docs/mainnet-immutable-flag-guidance.md`](mainnet-immutable-flag-guidance.md)
classifies `mux-account` as **immutable by design** — "core AA logic;
immutability is a user trust guarantee" — and recommends it stay that way
even after upgrade tooling exists for other Mux contracts. Do not plan around
calling `upgrade()` on a live `mux-account` instance; it does not exist and
is not expected to.

(For contracts that **do** support in-place upgrades — e.g. `mux-policy` —
see [docs/contract-upgrade-pattern.md](./contract-upgrade-pattern.md) for the
general `upgrade()`/`migrate()` procedure. None of that applies here.)

## What migration means in practice: deploy a new instance and cut over

Since there is no on-chain upgrade path, moving an account to new logic means
deploying a **new** `mux-account` contract instance and cutting the owner
over to it at the application layer. The old and new instances coexist
independently on-chain — there is no automatic transfer of one into the
other, and no on-chain "deprecate" flag for the old one.

### Steps

1. **Deploy the new `mux-account` WASM instance** (`stellar contract deploy`).

2. **Initialize it**: `initialize(owner, guardians)` with the same `owner`
   address as the instance being replaced. `guardians` here is the
   informational `Vec<Address>` stored under `DataKey::GuardianSet` — note
   that `mux-account` has no `add_guardian`/`remove_guardian` method of its
   own, so this list is fixed at `initialize` time on the new instance too.

3. **Register the new instance** via
   `mux-account-factory::deploy_account_with_metadata(owner, new_account_address, version, description, author)`.
   This is how off-chain tooling (indexers, dApp backends, TypeScript
   clients) discovers "the current account for owner X" — via
   `get_accounts(owner)` / `get_account_metadata(owner, account_address)`.
   There is no factory call to retire the old entry; if you need to signal
   that the prior instance is superseded, encode that in the old entry's
   `description` (e.g. via a fresh `deploy_account_with_metadata` call
   is not possible after the fact — plan the metadata convention before the
   first deploy, or track supersession off-chain).

4. **Guardian-driven recovery (`mux-recovery`) is a separate contract and is
   not automatically linked to a specific `mux-account` instance.**
   `mux-recovery::execute_recovery` only updates `mux-recovery`'s own
   internal `Owner` key — it does not call into `mux-account` to change
   `mux-account`'s stored owner. There is no on-chain link between the two
   contracts beyond the optional `set_registry` pointer used for discovery.
   Practical implications for a cutover:
   - If the guardian set is unchanged, the existing `mux-recovery` instance
     can keep serving the new `mux-account` instance — nothing about it
     references the old `mux-account` address, so nothing needs to change
     on-chain.
   - If guardians are changing as part of this migration, deploy a fresh
     `mux-recovery` instance, `initialize(owner, guardians)` it, and update
     `set_registry` / off-chain config accordingly.
   - Either way, "recovery executed" and "`mux-account` owner changed" are
     two independent facts today. Any application that relies on recovery to
     determine which `mux-account` address is authoritative must reconcile
     that off-chain (e.g. via `mux-account-factory` metadata), not assume the
     contracts do it for you.

5. **Re-establish delegates, spend limits, and session keys** on the new
   instance with owner-authorized calls (`set_delegate`, `set_spend_limit`,
   `register_session_key`). None of this state carries over automatically —
   it lives in the old instance's storage and the new instance starts empty.

6. **Point off-chain consumers at the new address**: update indexer
   configuration, dApp backend config, and any hardcoded contract IDs in
   TypeScript bindings usage. Discovery should go through
   `mux-account-factory::get_accounts` rather than a hardcoded address where
   possible.

7. **The old instance keeps running** unless the application layer stops
   using it. It is not paused, disabled, or removed by this process — its
   `unpause`/`is_paused` flag is unrelated to migration status.

## Storage Layout (for off-chain tooling reference)

`mux-account` uses **instance storage** for all state, scoped to a single
immutable instance:

| Key | Type | Notes |
|-----|------|-------|
| `DataKey::Owner` | `Address` | Set once at `initialize`; no setter exists |
| `DataKey::Delegates` | `Map<Address, DelegateInfo>` | Active delegate set |
| `DataKey::SpendLimit(Address)` | `SpendLimit` | Per-asset spend limits |
| `DataKey::GuardianSet` | `Vec<Address>` | Set once at `initialize`; informational only — see the `mux-recovery` note above for the contract that actually drives guardian-based recovery |
| `DataKey::Nonce` | `u64` | Transaction counter |
| `DataKey::SessionKey(Address, Address)` | `SessionKeyRecord` | Session key records |
| `DataKey::SessionKeyIndex(Address)` | `Vec<Address>` | Session key index per owner |
| `DataKey::Paused` | `bool` | Pause flag |
| `DataKey::Executing` | `bool` | Reentrancy guard |
| `DataKey::Metadata` | `RegistryMeta` | Optional registry metadata |

Because there is no upgrade path, this table describes one instance's
storage for its whole lifetime — it is not a "what survives an upgrade"
table. A new instance created via the cutover steps above starts with none
of this state populated except what `initialize` sets.

## If upgrade support is ever added

Should `mux-account` ever gain an `upgrade()`/`migrate()` entry point (a
change to the immutability policy in
[`docs/mainnet-immutable-flag-guidance.md`](mainnet-immutable-flag-guidance.md),
not just a code change), the general Soroban storage-compatibility rules
apply and should be documented here at that time:

- Adding a new `DataKey` variant is non-breaking as long as it gets a
  distinct discriminant and reads use `unwrap_or`/`Option` defaults.
- Removing or renaming a `DataKey` variant is a breaking storage change and
  would need a one-time `migrate()` step plus a major version bump.
- Lowering `MAX_DELEGATES` (currently 64) below the count already stored on
  some live instance would be breaking; raising it is safe.

None of this is actionable today — it is recorded here so the next person
who proposes an `upgrade()` entry point for `mux-account` knows what to
re-litigate first (the immutability policy) and what to design second
(the migration mechanics).

## TTL Considerations

Each `mux-account` instance manages its own storage TTL independently
(`TTL_EXTEND_TO = 518_400` ledgers ≈ 30 days, extended on every write). A
newly deployed instance's TTL starts from its own `initialize` call; it has
no relationship to the TTL of the instance it is replacing. Keepers must be
pointed at the new instance's contract ID as part of the cutover.

## Rollback

There is no `upgrade()` call to roll back. "Rollback" means reverting the
application-layer cutover: point off-chain config and `mux-account-factory`
discovery back at the old instance's address. The old instance was never
stopped, so it remains fully functional for this purpose as long as its
storage TTL has been kept alive.
