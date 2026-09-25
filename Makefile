.PHONY: all build test test-unit test-all clean fmt fmt-fix lint clippy wasm check-sizes size-check \
        bindings generate-bindings \
        coverage coverage-ci test-coverage \
        deny \
        check-no-testutils \
        verify-wasm-hashes \
        deploy-dry-run deploy-ci

all: fmt lint build test

build:
	cargo build --workspace --all-targets

# Run the complete test suite with all features enabled (matches CONTRIBUTING.md)
test:
	cargo test --workspace --all-features

# Unit tests only (matches CONTRIBUTING.md)
test-unit:
	cargo test --lib

# Alias for full workspace test suite
test-all: test

clean:
	cargo clean

# Check formatting without modifying files (matches CONTRIBUTING.md checklist)
fmt:
	cargo fmt --all -- --check

# Apply formatting fixes across all workspace crates (matches CONTRIBUTING.md code style)
fmt-fix:
	cargo fmt --all

lint: clippy

# Run Clippy across all targets and features with warnings denied (matches CONTRIBUTING.md)
clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

wasm:
	bash scripts/build-wasm.sh --release

check-sizes: wasm
	bash scripts/check-contract-sizes.sh

# Alias so both spellings work: `make check-size` and `make check-sizes`
size-check: check-sizes

# Generate TypeScript bindings from compiled contracts (matches CONTRIBUTING.md)
bindings:
	bash scripts/generate-bindings.sh

generate-bindings: bindings

# Supply-chain license and advisory check. deny.toml controls policy. (#661)
# Requires: cargo install cargo-deny
deny:
	cargo deny check

# Ensure no mux-* Cargo.toml enables soroban-sdk testutils in [dependencies]
# and that built WASMs (if present) contain no testutils bytes. (#663)
check-no-testutils:
	bash scripts/check-no-testutils.sh

# Compute SHA-256 hashes for every built release WASM and print them. (#664)
# Run after `make wasm`. Uses scripts/verify-wasm-hash.sh --compute-only.
verify-wasm-hashes:
	bash scripts/compute-wasm-hashes.sh

# LLVM source-based coverage using cargo-llvm-cov when available; falls back
# to the legacy stub if the tool is not installed. (#662)
# Requires: cargo install cargo-llvm-cov && rustup component add llvm-tools-preview
coverage:
	bash scripts/coverage.sh

# CI coverage target: always produces LCOV output to coverage/lcov.info. (#662)
# Fails if cargo-llvm-cov is not installed (install it in the CI job first).
coverage-ci:
	bash scripts/coverage.sh --lcov

# Validate coverage report stub behavior (matches CONTRIBUTING.md)
test-coverage:
	bash scripts/test-coverage.sh

# Simulate a full deployment without submitting any on-chain transactions.
# No secret keys or live network access required. Useful for local validation. (#449)
deploy-dry-run:
	bash scripts/deploy.sh --dry-run

# Deploy using pre-built WASM artifacts, skipping the cargo build step.
# Intended for CI pipelines that cache build outputs between jobs. (#450)
deploy-ci:
	bash scripts/deploy.sh --skip-build
