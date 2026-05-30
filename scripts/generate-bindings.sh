#!/usr/bin/env bash
# generate-bindings.sh
#
# Compiles Mux Protocol Soroban contracts and generates TypeScript bindings
# using the Stellar CLI (`stellar contract bindings typescript`).
#
# Usage:
#   bash scripts/generate-bindings.sh [--network testnet|mainnet] [--skip-build]
#
# Flags:
#   --network <name>   Stellar network name passed to stellar CLI (default: testnet)
#   --skip-build       Skip `cargo build`; use pre-built WASMs from target/ (e.g. in CI)
#
# Prerequisites:
#   - stellar CLI installed (https://developers.stellar.org/docs/tools/stellar-cli)
#   - Rust + cargo installed (unless --skip-build is set)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINDINGS_DIR="${REPO_ROOT}/bindings/src/generated"
WASM_DIR="${REPO_ROOT}/target/wasm32-unknown-unknown/release"
NETWORK_VALUE="testnet"
SKIP_BUILD=false

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --network)
      NETWORK_VALUE="${2:?'--network requires a value'}"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=true
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

CONTRACTS=(
  "mux-account"
  "mux-batcher"
  "mux-permissions"
)

if [[ "${SKIP_BUILD}" == "false" ]]; then
  echo "==> Building Soroban contracts..."
  cd "${REPO_ROOT}"
  cargo build \
    --target wasm32-unknown-unknown \
    --release \
    --workspace
else
  echo "==> Skipping build (--skip-build set); using pre-built WASMs from ${WASM_DIR}"
fi

echo "==> Generating TypeScript bindings into ${BINDINGS_DIR}..."
mkdir -p "${BINDINGS_DIR}"

for contract in "${CONTRACTS[@]}"; do
  wasm_name="${contract//-/_}.wasm"
  wasm_path="${WASM_DIR}/${wasm_name}"
  out_dir="${BINDINGS_DIR}/${contract}"

  if [[ ! -f "${wasm_path}" ]]; then
    echo "  [WARN] WASM not found for ${contract} at ${wasm_path}, skipping."
    continue
  fi

  echo "  Generating bindings for ${contract}..."
  # --contract-id is omitted intentionally: when using --wasm, the CLI infers
  # a placeholder ID from the WASM hash. Passing an empty string causes an error.
  stellar contract bindings typescript \
    --wasm "${wasm_path}" \
    --network "${NETWORK_VALUE}" \
    --output-dir "${out_dir}" \
    --overwrite

  echo "  Done -> ${out_dir}"
done

echo "==> Bindings generation complete."
