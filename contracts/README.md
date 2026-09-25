# Mux Protocol Contracts

This directory contains the Soroban smart contracts for the Mux Protocol. Each
crate compiles to a WASM module deployable on Stellar.

## Contract Index

| Crate | Description |
|---|---|
| [`mux-account`](mux-account/) | Smart wallet with owner, delegates, session keys, and spend limits |
| [`mux-account-factory`](mux-account-factory/) | Factory for deploying and registering account instances with metadata |
| [`mux-batcher`](mux-batcher/) | Atomic multi-operation batching with optional per-op failure handling |
| [`mux-delegation`](mux-delegation/) | Grant / revoke specific permissions or voting power to delegates |
| [`mux-permissions`](mux-permissions/) | Role-based access control (RBAC) — roles, permissions, grant/revoke |
| [`mux-policy`](mux-policy/) | Per-wallet daily spend-limit policy with auto-reset |
| [`mux-recovery`](mux-recovery/) | Account recovery with timelock and admin approval |
| [`mux-registry`](mux-registry/) | Contract version and metadata registry |
| [`mux-spending-policy`](mux-spending-policy/) | Per-account/per-asset spend-limit policy and validation |
| [`mux-wallet-registry`](mux-wallet-registry/) | Named wallet registry for address lookup |

### Shared Crate

| Crate | Description |
|---|---|
| [`soroban-test-helpers`](soroban-test-helpers/) | Shared mock environment and test utilities (not compiled to WASM) |

> **Note — two registry crates:** `mux-registry` and `mux-wallet-registry`
> both ship WASM and share the "registry" naming, but serve different purposes:
> `mux-registry` tracks deployed contract versions (protocol infra); `mux-wallet-registry`
> maps symbolic names to wallet addresses (application layer). See
> [`docs/registry-contracts-comparison.md`](../docs/registry-contracts-comparison.md)
> for a full side-by-side comparison including error codes, auth models, and
> storage layouts.

## Quick Reference

### Build

```bash
cargo build --target wasm32-unknown-unknown --release --workspace
```

### Test

```bash
cargo test --workspace --all-features
```

### Per-contract test

```bash
cargo test --package <crate>
```

## Common Patterns

All contracts follow these conventions:

- **`#![no_std]`** — no Rust standard library; WASM-target compatible.
- **Single error enum** — each contract defines one `#[contracterror]` enum with
  `#[repr(u32)]` codes starting at 1. Common variants (`NotInitialized = 1`,
  `AlreadyInitialized = 2`, `Unauthorized = 3`) appear in most contracts.
- **Storage griefing guards** — every collection-backed storage (Vec, Map) has an
  explicit `MAX_*` cap with a dedicated error variant.
- **TTL management** — persistent entries call `extend_ttl` on every write;
  instance storage is extended after state-mutating functions.
- **Event emission** — all state changes emit audit events under a contract-
  specific topic prefix.

## Error Codes

See [docs/error_codes.md](../docs/error_codes.md) for the full error code
reference across all contracts.

## TypeScript Bindings

Auto-generated clients for every contract live in [`../bindings/src/generated/`](../bindings/src/generated/).
After changing any contract interface, regenerate with:

```bash
bash scripts/generate-bindings.sh
```

## Documentation

- [`mux-account` public interface](../docs/mux-account-interface.md)
- Consistent error handling with custom error types
- Soroban SDK best practices
- Storage optimization with TTL management
- Comprehensive event emission for auditability
- Modular design for easy integration

## `no_std` and `alloc` Constraints

All Soroban contract crates in this workspace are `#![no_std]`. The workspace
`Cargo.toml` sets `unsafe_code = "forbid"` at the workspace level, so no
crate may use `unsafe` blocks.

### Why `no_std`?

Soroban smart contracts compile to WASM (`wasm32-unknown-unknown`) and run
inside the Soroban VM, which does not provide a system allocator or OS
services. Using `no_std` ensures:

1. **Correct compilation target** — the WASM target has no `std` library.
2. **No hidden syscalls** — prevents accidental use of file I/O, networking,
   or other OS primitives unavailable on-chain.
3. **Smaller binary size** — `no_std` binaries are typically smaller, reducing
   deployment costs.

### `extern crate alloc`

Only `mux-registry` currently uses `extern crate alloc`. This is allowed
because the Soroban VM provides a heap allocator, and `alloc` types
(`Vec`, `String`, `BTreeMap`, etc.) are safe to use in a `no_std` context
when an allocator is available.

Other contracts avoid `alloc` and rely exclusively on Soroban SDK types
(`soroban_sdk::Vec`, `soroban_sdk::String`, etc.) which are backed by the
Soroban host and do not require the Rust `alloc` crate.

### Constraints for contributors

- **Never add `extern crate std`** to any contract crate.
- **Never add `unsafe` code** — the workspace-level `forbid` enforces this.
- **Prefer `soroban_sdk` collection types** over `alloc` types for
  consistency and gas predictability.
- If `alloc` is needed, document why in the crate-level doc comment and
  ensure the crate still compiles to `wasm32-unknown-unknown`.
