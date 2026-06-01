.PHONY: all build test clean fmt lint clippy

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
