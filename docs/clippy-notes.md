# Clippy Notes for Mux Contracts

This document explains how clippy is configured for the workspace, which pedantic lints apply to Soroban contracts, and which lints are explicitly allowed with justification.

---

## Workspace lint configuration

Lints are declared centrally in the root `Cargo.toml` `[workspace.lints]` table so every crate in the workspace inherits the same baseline:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all      = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used  = "warn"
expect_used  = "warn"
```

The `priority = -1` on `all` and `pedantic` lets individual lint overrides (added at higher priority) take effect without fighting the group-level setting.

---

## Running clippy locally

### Standard check (used in CI)

```bash
cargo clippy --workspace --all-features --all-targets -- -D warnings
```

This is what `make clippy` runs.  `-D warnings` promotes every lint warning to a hard error so CI fails on any new warning.  `--all-features` ensures feature-gated code paths are linted; `--all-targets` includes tests, examples, and benchmarks so warnings in those targets do not silently accumulate.

### Pedantic-only pass (for exploration)

```bash
cargo clippy --workspace --all-targets -- -W clippy::pedantic
```

### Single crate

```bash
cargo clippy -p mux-account -- -D warnings
```

### With a cargo alias (see `.cargo/config.toml`)

```bash
cargo lint            # equivalent to cargo clippy --workspace --all-targets -- -D warnings
cargo lint-pedantic   # adds -W clippy::pedantic on top of -D warnings
```

---

## Pedantic lints relevant to Soroban contracts

| Lint | Why it matters for Soroban |
|------|---------------------------|
| `clippy::must_use_candidate` | Flags functions whose return values callers might silently discard — important for error-return functions in contracts |
| `clippy::missing_errors_doc` | Ensures public functions document error cases in doc-comments |
| `clippy::missing_panics_doc` | Documents any `unwrap`/`expect` paths (should be zero in production contract code) |
| `clippy::module_name_repetitions` | Encourages concise type names that do not repeat the module name |
| `clippy::items_after_statements` | Improves readability by keeping item definitions at the top of blocks |
| `clippy::wildcard_imports` | Prevents accidental name shadowing in contract entry points |
| `clippy::too_many_lines` | Encourages decomposing long contract functions into helpers |

---

## Allowed lints with justification

The following lints are suppressed at specific sites in the codebase.  Each suppression must include an inline justification comment.

### `clippy::must_use_candidate`

```rust
// Soroban `#[contract]` impl blocks expose methods via the SDK macro — the
// macro-generated dispatch layer handles the return value; annotating every
// method as `#[must_use]` would be misleading to callers using the XDR ABI.
#[allow(clippy::must_use_candidate)]
```

### `clippy::module_name_repetitions`

```rust
// Re-exported types deliberately repeat the module name so that downstream
// consumers using `use mux_account::*` get unambiguous type names.
#[allow(clippy::module_name_repetitions)]
```

### `clippy::wildcard_imports`

```rust
// `use soroban_sdk::*` is the idiomatic Soroban pattern recommended by the
// SDK documentation; restricting it would make contract code significantly
// more verbose with no safety benefit.
#[allow(clippy::wildcard_imports)]
```

---

## Adding a new lint suppression

1. Prefer fixing the lint over suppressing it.
2. If suppression is necessary, add `#[allow(clippy::<lint_name>)]` at the **narrowest** scope (expression > statement > function > module > crate).
3. Always include a comment explaining why the suppression is justified.
4. Update this file with the new entry in the table above.

---

## Related resources

- [Clippy lints index](https://rust-lang.github.io/rust-clippy/master/)
- [Workspace `Cargo.toml`](../Cargo.toml) — canonical lint configuration
- [`.cargo/config.toml`](../.cargo/config.toml) — `cargo lint` / `cargo lint-pedantic` aliases
