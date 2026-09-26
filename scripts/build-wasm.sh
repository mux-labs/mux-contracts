#!/usr/bin/env bash
# build-wasm.sh — Issue #104
#
# Compiles every Mux Protocol Soroban contract to WASM (wasm32-unknown-unknown).
#
# Release builds intentionally omit the soroban-sdk `testutils` feature:
#   - Packages are listed explicitly (excludes soroban-test-helpers)
#   - No `--features` / `--all-features` flags are passed
#   - Run `scripts/check-no-testutils.sh` after build to verify
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

# Contract crates only — never include soroban-test-helpers (always has testutils).
CONTRACT_PACKAGES=(
  mux-account
  mux-account-factory
  mux-batcher
  mux-delegation
  mux-permissions
  mux-policy
  mux-recovery
  mux-registry
  mux-spending-policy
  mux-wallet-registry
)

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --dev)     PROFILE="dev";     shift ;;
    --out-dir) OUT_DIR="${2:?'--out-dir requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

WASM_SRC="${REPO_ROOT}/target/wasm32-unknown-unknown/${PROFILE}"

PKG_ARGS=()
for pkg in "${CONTRACT_PACKAGES[@]}"; do
  PKG_ARGS+=(-p "$pkg")
done

echo "==> Building Soroban contracts (profile: ${PROFILE}, no testutils)..."
# Do not pass --features / --all-features: testutils must stay off for release WASM.
cargo build \
  --manifest-path "${REPO_ROOT}/Cargo.toml" \
  --target wasm32-unknown-unknown \
  --profile "${PROFILE}" \
  "${PKG_ARGS[@]}"

mkdir -p "${OUT_DIR}"

echo "==> Copying WASMs to ${OUT_DIR}..."
for wasm in "${WASM_SRC}"/*.wasm; do
  [[ -f "${wasm}" ]] || continue
  dest="${OUT_DIR}/$(basename "${wasm}")"
  cp "${wasm}" "${dest}"
  size=$(wc -c < "${dest}")
  echo "  $(basename "${dest}")  ${size} bytes"
done

if [[ "${PROFILE}" == "release" ]]; then
  echo "==> Verifying no testutils in release WASMs..."
  bash "${REPO_ROOT}/scripts/check-no-testutils.sh" --wasm-dir "${WASM_SRC}"
fi

echo "==> Done. WASMs are in ${OUT_DIR}"
