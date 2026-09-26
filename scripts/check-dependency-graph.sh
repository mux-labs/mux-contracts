#!/usr/bin/env bash
# Verify docs/dependency_graph.md names every contract crate and only known edges.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOC="${ROOT_DIR}/docs/dependency_graph.md"
CONTRACTS_DIR="${ROOT_DIR}/contracts"
[[ -f "$DOC" ]] || { echo "ERROR: missing $DOC" >&2; exit 1; }

mapfile -t crates < <(find "$CONTRACTS_DIR" -mindepth 1 -maxdepth 1 -type d -name 'mux-*' -printf '%f\n' | sort)
failed=0
for crate in "${crates[@]}"; do
  if grep -q "\`$crate\`" "$DOC"; then
    echo "OK: $crate"
  else
    echo "FAIL: $crate is missing from $(basename "$DOC")"
    failed=1
  fi
done

# Reject stale architecture vocabulary that is not part of this Soroban repo.
for stale in 'EntryPoint' 'Paymaster' 'Mux Token' 'Aggregator' 'ERC-4337'; do
  if grep -q "$stale" "$DOC"; then
    echo "FAIL: stale non-Soroban dependency '$stale' appears in $(basename "$DOC")"
    failed=1
  fi
done

if (( failed )); then
  echo "Dependency graph validation failed." >&2
  exit 1
fi
echo "Dependency graph validation passed for ${#crates[@]} contract crates."
