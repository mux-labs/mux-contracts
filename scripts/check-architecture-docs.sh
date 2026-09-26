#!/usr/bin/env bash
# check-architecture-docs.sh
#
# Guards against docs/#701-class drift: every WASM contract crate under
# contracts/ (i.e. every mux-* directory except the non-WASM test-helpers
# crate) must be named in docs/architecture-overview.md. Catches the case
# where a new contract crate is added to the workspace but the architecture
# doc is never updated to mention it.
#
# Usage:
#   bash scripts/check-architecture-docs.sh
#
# Exit 0 on success, 1 if a crate is missing from the doc.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACTS_DIR="${REPO_ROOT}/contracts"
DOC="${REPO_ROOT}/docs/architecture-overview.md"

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
  echo "ERROR: docs/architecture-overview.md is missing one or more contract"
  echo "       crates. Update it (and contracts/README.md if relevant) so the"
  echo "       architecture doc stays canonical for the current workspace."
  exit 1
fi

echo "All checks passed: architecture-overview.md lists every contract crate."
