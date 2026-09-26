# Event topic conventions

Mux Soroban contracts emit structured contract events for auditability and
TypeScript / indexer bindings. This document is the canonical convention for
topic layout, naming, and filtering. Per-contract action catalogs live in
[audit-events.md](audit-events.md).

## Topic layout (required)

Every state-mutating success path publishes:

```text
topics[0]  contract_tag   Symbol   e.g. mux_acct, mux_fac, mux_dlg
topics[1]  action         Symbol   e.g. init, dlg_set, deployed
data       payload        Val      action-specific tuple / address / ()
```

Rust shape used across crates:

```rust
env.events().publish((symbol_short!("mux_xxx"), action), data);
```

Rules:

1. **Exactly two topics** — never add a third topic for addresses or amounts; put those in `data`.
2. **`symbol_short!` only** — both topics must fit the 9-character Soroban short-symbol limit.
3. **Success only** — failed `Result::Err` paths and auth host failures must not emit events.
4. **Stable ABI** — renaming a contract tag or action is a breaking change for indexers and TS clients; bump bindings and document in `docs/BREAKING_CHANGES.md`.

## Contract tags

| Crate | Tag (`topics[0]`) |
|---|---|
| `mux-account` | `mux_acct` |
| `mux-account-factory` | `mux_fac` |
| `mux-permissions` | `mux_perm` |
| `mux-delegation` | `mux_dlg` |
| `mux-batcher` | `mux_bat` |
| `mux-policy` | `mux_pol` |
| `mux-spending-policy` | `mux_spend` |
| `mux-registry` | `mux_reg` |
| `mux-wallet-registry` | `mux_wreg` |
| `mux-recovery` | `mux_recv` |

Tags are snake-ish abbreviations under 9 chars. Do not reuse a tag across crates.

## Repository-wide short-tag registry

Contract tags (`topics[0]`) identify the emitting crate. The action tag in `topics[1]` is normally interpreted together with that contract tag as the stable key `contract_tag/action`. A small set of short action tags is also consumed by shared Mux indexer pipelines and therefore has a **repository-wide uniqueness reservation**. These tags must not be assigned to a different action in another contract:

| Reserved action tag | Contract | Event | Indexer meaning |
|---|---|---|---|
| `ses_exe` | `mux-account` | `execute_with_session`, `execute_with_session_sponsored` | A session-authorized call was dispatched and completed |
| `spn_set` | `mux-account` | `set_sponsor` | A relayer was added to or removed from the sponsorship allowlist |
| `bat_ok` | `mux-batcher` | `execute_batch` | A batch completed with zero failures |
| `rec_init` | `mux-recovery` | `initiate_recovery` | A recovery timelock was started |

Before introducing a new globally indexed short tag, search this document and [Audit Log Events](audit-events.md), then reserve the tag here in the same change. Reusing generic action names such as `init` is allowed only when consumers filter on the full `(topics[0], topics[1])` pair; the reserved tags above are the exception and must remain unique across contracts. Indexers should reject or quarantine an event whose reserved tag appears under a different contract tag or with a different payload schema.

The uniqueness rule is an ABI compatibility requirement: changing one of these tags or its payload shape can silently misclassify events in downstream indexers. Any intentional rename requires updating the bindings, audit-event catalog, and migration notes before deployment.

## Action naming

| Pattern | Example | Use |
|---|---|---|
| Verb-ish past / short noun | `init`, `deployed`, `executed` | One-shot lifecycle |
| Domain prefix + verb | `dlg_set`, `dlg_rm`, `role_grt` | Domain-scoped mutations |
| Status / outcome | `bat_ok`, `bat_abort` | Secondary outcome beside a primary event |

Guidelines:

- Prefer ≤ 8 characters so `symbol_short!` stays readable.
- Keep action names unique **within** a contract tag; cross-contract reuse of names like `init` / `meta_set` is fine.
- Read-only entrypoints (`get_*`, `simulate_*`, `is_*`, `has_*`) emit **no** events.
  Exception: `mux-permissions::has_permission` emits `perm_ok` on a granted
  check (a positive audit trail is intentional there) but emits nothing on a
  denial — see [audit-events.md](audit-events.md#mux-permissions-events).

## Data payload

- Prefer plain tuples of Soroban types (`Address`, `Symbol`, `u32`, `i128`, `String`, `Bytes`).
- Put the primary actor first when present: `(owner, …)`, `(caller, …)`, `(admin, …)`.
- Do not encode auth proofs or full call args — keep payloads minimal for indexers.
- Empty success may use `()` (unit).

## TypeScript / RPC filtering

Soroban RPC `getEvents` filters map 1:1 to topics:

```ts
const events = await server.getEvents({
  startLedger: fromLedger,
  filters: [{
    type: "contract",
    contractIds: [CONTRACT_ID],
    // topics[0] = contract tag, topics[1] = action
    topics: [["mux_acct"], ["dlg_set"]],
  }],
});
```

Bindings notes:

- Generated clients do not wrap events; consumers should filter by tag + action as above.
- When adding a new action, update [audit-events.md](audit-events.md) and any binding integration tests that assert topic symbols.
- Shared test helper `soroban_test_helpers::assert_event_topic` asserts `topics[1]` only — also assert `topics[0]` when introducing a new contract tag.

## Security / audit expectations

- Events are append-only host logs; they are not a substitute for storage state.
- Auth failures (`require_auth`) must leave storage unchanged **and** emit zero events (covered by factory unauthorized deploy tests).
- Do not put secrets, recovery secrets, or full session payloads in event data beyond what is already public on-chain.
