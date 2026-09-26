# WASM Build Script Usage

`scripts/build-wasm.sh` compiles every Mux Protocol Soroban contract to
`wasm32-unknown-unknown` without the `testutils` feature, producing
production-safe WASM artifacts.

## Quick start

```bash
# Release build (default) — output lands in target/wasm/
bash scripts/build-wasm.sh

# Dev build (faster, larger output)
bash scripts/build-wasm.sh --dev

# Custom output directory
bash scripts/build-wasm.sh --out-dir /tmp/my-wasms
```

## Flags

| Flag | Default | Description |
|---|---|---|
| `--release` | ✓ | Optimised release profile |
| `--dev` | | Dev profile — faster but not suitable for deploy |
| `--out-dir <path>` | `target/wasm` | Directory where final `.wasm` files are copied |

## What the script does

1. Runs `cargo build --target wasm32-unknown-unknown` for each contract crate
   listed in `CONTRACT_PACKAGES` (never includes `soroban-test-helpers`).
2. Copies every `.wasm` from `target/wasm32-unknown-unknown/<profile>/` to
   `--out-dir`, printing the file size.
3. On release builds, runs `scripts/check-no-testutils.sh` to verify the
   `testutils` feature is absent from the compiled artifacts.

## Why testutils must be excluded

The `soroban-sdk` `testutils` feature adds mock host functions that are
**not available on-chain**. Including them in a deployed WASM causes a
contract initialization failure at runtime. The build script explicitly
omits `--features` / `--all-features` to prevent this.

## Verifying artifact integrity

After a release build, verify WASM hashes before deploy:

```bash
bash scripts/verify-wasm-hash.sh
```

Check contract sizes against per-contract budgets:

```bash
bash scripts/check-contract-sizes.sh
# or via make:
make check-sizes
make size-check   # alias
```

## Makefile integration

```bash
make build        # equivalent to bash scripts/build-wasm.sh --release
make check-sizes  # build + size budget check
```

## CI usage

The GitHub Actions workflow runs `bash scripts/build-wasm.sh` followed by
`bash scripts/check-contract-sizes.sh` on every push to `main` and on pull
requests. A size-budget failure blocks the merge.

## Adding a new contract

1. Add the crate name to `CONTRACT_PACKAGES` in `scripts/build-wasm.sh`.
2. Add a size budget entry in `scripts/check-contract-sizes.sh`.
3. Run `bash scripts/build-wasm.sh` locally to confirm the new contract
   compiles without `testutils`.
