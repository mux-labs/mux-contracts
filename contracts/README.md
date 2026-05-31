# Mux Protocol Contracts

This directory contains the Soroban smart contracts for the Mux Protocol.

## Quickstart

### Prerequisites

- [Rust Toolchain](https://www.rust-lang.org/tools/install)
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup)

### Build Contracts

To build all contracts in the workspace:

```bash
cargo build --target wasm32-unknown-unknown --release
```

### Run Tests

To execute tests for all contracts:

```bash
cargo test
```

### Lint and Format

Ensure code follows the standard formatting and passes all lints:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```
