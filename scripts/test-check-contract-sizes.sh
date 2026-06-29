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

# Forward check: every contract directory must have a LIMITS entry.
for dir in "${CONTRACTS_DIR}"/mux-*/; do
  name="$(basename "$dir")"
  wasm_name="${name//-/_}.wasm"

  if ! grep -q "\"${wasm_name}\"" "${SIZE_SCRIPT}"; then
    echo "MISSING: ${wasm_name} not found in check-contract-sizes.sh"
    FAILED=1
  fi
done

# Reverse check: every LIMITS entry must have a matching contract directory.
# This catches stale entries that linger after a contract is removed.
while IFS= read -r wasm_name; do
  [[ -z "${wasm_name}" ]] && continue
  wasm_base="${wasm_name%.wasm}"
  dir_name="${wasm_base//_/-}"
  if [[ ! -d "${CONTRACTS_DIR}/${dir_name}" ]]; then
    echo "STALE: ${wasm_name} has a LIMITS entry but no contract directory at contracts/${dir_name}"
    FAILED=1
  fi
done < <(grep -oP '\[\"\K[^\"]+\.wasm' "${SIZE_SCRIPT}")

if (( FAILED )); then
  echo "ERROR: Size check coverage is incomplete." >&2
  exit 1
fi

echo "All contract directories have matching size check entries, and vice versa."
