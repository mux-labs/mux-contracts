#!/usr/bin/env bash
# build-wasm.sh — Issue #104
#
# Compiles every Mux Protocol Soroban contract to WASM (wasm32-unknown-unknown).
#
# Usage:
#   bash scripts/build-wasm.sh [--release|--dev] [--out-dir <path>]
#
# Flags:
#   --release   Build with release profile (default)
#   --dev       Build with dev profile
#   --out-dir   Copy final WASMs to this directory (default: target/wasm)
#
# Output files are named <contract_name>.wasm and placed in --out-dir.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="release"
OUT_DIR="${REPO_ROOT}/target/wasm"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --dev)     PROFILE="dev";     shift ;;
    --out-dir) OUT_DIR="${2:?'--out-dir requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

WASM_SRC="${REPO_ROOT}/target/wasm32-unknown-unknown/${PROFILE}"

echo "==> Building Soroban contracts (profile: ${PROFILE})..."
cargo build \
  --manifest-path "${REPO_ROOT}/Cargo.toml" \
  --target wasm32-unknown-unknown \
  --profile "${PROFILE}" \
  --workspace

mkdir -p "${OUT_DIR}"

echo "==> Copying WASMs to ${OUT_DIR}..."
for wasm in "${WASM_SRC}"/*.wasm; do
  [[ -f "${wasm}" ]] || continue
  dest="${OUT_DIR}/$(basename "${wasm}")"
  cp "${wasm}" "${dest}"
  size=$(wc -c < "${dest}")
  echo "  $(basename "${dest}")  ${size} bytes"
done

echo "==> Done. WASMs are in ${OUT_DIR}"
