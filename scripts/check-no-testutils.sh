#!/usr/bin/env bash
# check-no-testutils.sh
#
# Hardens release WASM builds against accidental inclusion of the
# soroban-sdk `testutils` feature.
#
# Checks:
#   1. No mux-* contract Cargo.toml enables `testutils` under [dependencies]
#      (dev-dependencies and the optional `testutils` feature are allowed).
#   2. If compiled WASMs exist, none contain the ASCII string "testutils"
#      (a cheap static scan; #[cfg(test)] is already stripped by the compiler).
#
# Usage:
#   bash scripts/check-no-testutils.sh [--wasm-dir <path>]
#
# Exit 0 on success, 1 on failure.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_DIR="${REPO_ROOT}/target/wasm32-unknown-unknown/release"
CONTRACTS_DIR="${REPO_ROOT}/contracts"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --wasm-dir) WASM_DIR="${2:?'--wasm-dir requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

FAILED=0

echo "==> Checking Cargo.toml: no testutils in [dependencies]"
for toml in "${CONTRACTS_DIR}"/mux-*/Cargo.toml; do
  name="$(basename "$(dirname "$toml")")"
  # Extract the [dependencies] section only (stop at the next [header]).
  deps_section=$(awk '
    /^\[dependencies\]/ { in_deps=1; next }
    /^\[/ { in_deps=0 }
    in_deps { print }
  ' "$toml")

  if echo "$deps_section" | grep -Eq 'features\s*=\s*\[[^]]*testutils'; then
    echo "  FAIL: ${name} enables testutils in [dependencies]"
    FAILED=1
  else
    echo "  OK:   ${name}"
  fi
done

# soroban-test-helpers is test-only (rlib) and may enable testutils — confirm
# it is never a cdylib so it cannot ship as release WASM.
helpers_toml="${CONTRACTS_DIR}/soroban-test-helpers/Cargo.toml"
if [[ -f "$helpers_toml" ]]; then
  if grep -Eq 'crate-type\s*=\s*\[[^]]*cdylib' "$helpers_toml"; then
    echo "  FAIL: soroban-test-helpers must not be cdylib (would ship as WASM)"
    FAILED=1
  else
    echo "  OK:   soroban-test-helpers is rlib-only (excluded from WASM)"
  fi
fi

echo ""
echo "==> Scanning release WASMs for 'testutils' string (if present)"
shopt -s nullglob
wasms=("${WASM_DIR}"/*.wasm)
if (( ${#wasms[@]} == 0 )); then
  echo "  SKIP: no WASMs in ${WASM_DIR} (run scripts/build-wasm.sh first)"
else
  for wasm in "${wasms[@]}"; do
    base="$(basename "$wasm")"
    if strings "$wasm" 2>/dev/null | grep -Fq "testutils"; then
      echo "  FAIL: ${base} contains 'testutils'"
      FAILED=1
    else
      echo "  OK:   ${base}"
    fi
  done
fi

echo ""
if (( FAILED )); then
  echo "ERROR: testutils leakage detected. Release WASM must not enable the"
  echo "       soroban-sdk testutils feature. Keep it in [dev-dependencies]"
  echo "       and/or the optional crate feature only."
  exit 1
fi

echo "All checks passed: no testutils in release WASM path."
