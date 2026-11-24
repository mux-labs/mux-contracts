#!/usr/bin/env bash
# check-contract-sizes.sh — Issue #103 / #237
#
# Verifies that each compiled WASM stays under its size budget.
# Exits non-zero if any contract exceeds its limit (fails CI).
# Prints a WARN line when a contract exceeds 80 % of its budget.
#
# Usage:
#   bash scripts/check-contract-sizes.sh [--wasm-dir <path>]
#
# Limits (bytes) — Soroban max upload size is 128 KiB (131 072 bytes).
# Individual budgets are set conservatively below that ceiling.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_DIR="${REPO_ROOT}/target/wasm32-unknown-unknown/release"

# Budget (bytes) consumed from the same `WASM_SIZE_BUDGET_BYTES` variable
# defined in `.github/workflows/ci.yml`. Default 64 KiB per contract.
BUDGET_BYTES="${WASM_SIZE_BUDGET_BYTES:-65536}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --wasm-dir) WASM_DIR="${2:?'--wasm-dir requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

# Contract size budgets in bytes (< Soroban 128 KiB upload limit).
# WARN threshold fires at 80 % of the per-contract budget.
declare -A LIMITS=(
  ["mux_account.wasm"]=$BUDGET_BYTES
  ["mux_account_factory.wasm"]=$BUDGET_BYTES
  ["mux_batcher.wasm"]=$BUDGET_BYTES
  ["mux_delegation.wasm"]=$BUDGET_BYTES
  ["mux_permissions.wasm"]=$BUDGET_BYTES
  ["mux_policy.wasm"]=$BUDGET_BYTES
  ["mux_recovery.wasm"]=$BUDGET_BYTES
  ["mux_registry.wasm"]=$BUDGET_BYTES
  ["mux_spending_policy.wasm"]=$BUDGET_BYTES
  ["mux_wallet_registry.wasm"]=$BUDGET_BYTES
)

WARN_PCT=80   # warn when usage exceeds this percentage of the budget
FAILED=0
WARNED=0

echo "Contract size report (warn at ${WARN_PCT}% of budget, fail above 100%)"
echo "------------------------------------------------------------------------"
printf "%-38s %10s %10s %7s %6s\n" "Contract" "Size (B)" "Limit (B)" "Usage %" "Status"
echo "------------------------------------------------------------------------"

for contract in "${!LIMITS[@]}"; do
  wasm="${WASM_DIR}/${contract}"
  limit="${LIMITS[$contract]}"

  if [[ ! -f "${wasm}" ]]; then
    printf "%-38s %10s %10s %7s %6s\n" "${contract}" "MISSING" "${limit}" "-" "SKIP"
    continue
  fi

  size=$(wc -c < "${wasm}")
  pct=$(( size * 100 / limit ))

  if (( size > limit )); then
    status="FAIL"
    FAILED=1
  elif (( pct >= WARN_PCT )); then
    status="WARN"
    WARNED=1
  else
    status="OK"
  fi

  printf "%-38s %10d %10d %6d%% %6s\n" "${contract}" "${size}" "${limit}" "${pct}" "${status}"
done

echo "------------------------------------------------------------------------"

if (( WARNED )); then
  echo "WARNING: One or more contracts are above ${WARN_PCT}% of their size budget." >&2
fi

if (( FAILED )); then
  echo "ERROR: One or more contracts exceed their size budget." >&2
  exit 1
fi

echo "All contracts within size budget."
