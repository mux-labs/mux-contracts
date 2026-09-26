#!/usr/bin/env bash
# check-threat-model-coverage.sh
#
# Guards against threat-model drift: every WASM contract crate under
# contracts/ (i.e. every mux-* directory except the non-WASM test-helpers
# crate) must be named in docs/threat-model.md. Catches the case where a new
# contract crate is added to the workspace but the threat model is never
# updated to document its attack surface.
#
# This is the shell-level twin of the Rust test in
# tests/threat_model_coverage.rs (which runs as part of `cargo test
# --workspace`); keeping both means a PR cannot slip past either the cargo
# suite or the CI pre-deploy script.
#
# Usage:
#   bash scripts/check-threat-model-coverage.sh
#
# Exit 0 on success, 1 if a crate is missing from the doc.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACTS_DIR="${REPO_ROOT}/contracts"
DOC="${REPO_ROOT}/docs/threat-model.md"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --doc) DOC="${2:?'--doc requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

FAILED=0

echo "==> Checking every contract crate is named in $(basename "$DOC")"

for dir in "${CONTRACTS_DIR}"/mux-*/; do
  name="$(basename "$dir")"

  if grep -q "$name" "$DOC"; then
    echo "  OK:   ${name}"
  else
    echo "  FAIL: ${name} — not mentioned in ${DOC}"
    FAILED=1
  fi
done

echo ""
if (( FAILED )); then
  echo "ERROR: docs/threat-model.md is missing one or more contract crates."
  echo "       Every contract that ships WASM must be documented (scope table,"
  echo "       trust boundaries, and a per-contract threat section) before the"
  echo "       Mux Soroban audit / mainnet readiness review."
  exit 1
fi

echo "All checks passed: threat-model.md lists every contract crate."
