.PHONY: all build test clean fmt lint clippy wasm check-sizes

all: fmt lint build test

build:
	cargo build --workspace --all-targets

test:
	cargo test --workspace

clean:
	cargo clean

fmt:
	cargo fmt --all -- --check

lint: clippy

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

wasm:
	bash scripts/build-wasm.sh --release

check-sizes: wasm
	bash scripts/check-contract-sizes.sh
