#!/usr/bin/env bash
# test-check-contract-sizes.sh
#
# Validates that every contract directory under contracts/ has a matching
# entry in check-contract-sizes.sh so new contracts cannot silently skip
# the WASM size gate in CI.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIZE_SCRIPT="${REPO_ROOT}/scripts/check-contract-sizes.sh"
CONTRACTS_DIR="${REPO_ROOT}/contracts"

FAILED=0

for dir in "${CONTRACTS_DIR}"/mux-*/; do
  name="$(basename "$dir")"
  wasm_name="${name//-/_}.wasm"

  if ! grep -q "\"${wasm_name}\"" "${SIZE_SCRIPT}"; then
    echo "MISSING: ${wasm_name} not found in check-contract-sizes.sh"
    FAILED=1
  fi
done

if (( FAILED )); then
  echo "ERROR: Some contracts are missing from the size check script." >&2
  exit 1
fi

echo "All contract directories have matching size check entries."
