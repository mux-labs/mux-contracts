#!/usr/bin/env bash
# check-contract-sizes.sh — Issue #103
#
# Verifies that each compiled WASM stays under its size budget.
# Exits non-zero if any contract exceeds its limit (fails CI).
#
# Usage:
#   bash scripts/check-contract-sizes.sh [--wasm-dir <path>]
#
# Limits (bytes) — Soroban max upload size is 128 KiB (131 072 bytes).
# Individual budgets are set conservatively below that ceiling.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_DIR="${REPO_ROOT}/target/wasm32-unknown-unknown/release"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --wasm-dir) WASM_DIR="${2:?'--wasm-dir requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

# Contract size budgets in bytes (< Soroban 128 KiB upload limit)
declare -A LIMITS=(
  ["mux_account.wasm"]=65536
  ["mux_account_factory.wasm"]=65536
  ["mux_batcher.wasm"]=65536
  ["mux_permissions.wasm"]=65536
  ["mux_registry.wasm"]=65536
)

FAILED=0

echo "Contract size report (limit: per-contract budget)"
echo "---------------------------------------------------"
printf "%-40s %10s %10s %6s\n" "Contract" "Size (B)" "Limit (B)" "Status"
echo "---------------------------------------------------"

for contract in "${!LIMITS[@]}"; do
  wasm="${WASM_DIR}/${contract}"
  limit="${LIMITS[$contract]}"

  if [[ ! -f "${wasm}" ]]; then
    printf "%-40s %10s %10s %6s\n" "${contract}" "MISSING" "${limit}" "SKIP"
    continue
  fi

  size=$(wc -c < "${wasm}")
  if (( size > limit )); then
    status="FAIL"
    FAILED=1
  else
    status="OK"
  fi
  printf "%-40s %10d %10d %6s\n" "${contract}" "${size}" "${limit}" "${status}"
done

echo "---------------------------------------------------"

if (( FAILED )); then
  echo "ERROR: One or more contracts exceed their size budget." >&2
  exit 1
fi

echo "All contracts within size budget."
