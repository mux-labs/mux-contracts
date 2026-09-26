#!/usr/bin/env bash
# check-doc-examples.sh
#
# Guards against docs/#701-#700-class drift: CONTRIBUTING.md example code
# blocks referencing `client.<method>(...)` calls must name entrypoints that
# actually exist as `pub fn <method>` somewhere in a contract crate. This
# catches the case where a contributor-facing example calls a function that
# was renamed or never existed (e.g. a phantom `is_session_key_valid`).
#
# Usage:
#   bash scripts/check-doc-examples.sh
#
# Exit 0 on success, 1 if an example calls a nonexistent entrypoint.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOC="${REPO_ROOT}/CONTRIBUTING.md"
CONTRACTS_DIR="${REPO_ROOT}/contracts"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --doc) DOC="${2:?'--doc requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

FAILED=0

echo "==> Checking client.<method>() calls in CONTRIBUTING.md example code"

methods="$(grep -oE 'client\.[a-zA-Z_][a-zA-Z0-9_]*\(' "$DOC" | sed -E 's/^client\.(.*)\($/\1/' | sort -u || true)"

if [[ -z "$methods" ]]; then
  echo "  SKIP: no client.<method>() calls found in ${DOC}"
  exit 0
fi

while IFS= read -r method; do
  [[ -z "$method" ]] && continue
  if grep -rEq "pub fn ${method}\b" "${CONTRACTS_DIR}"/mux-*/src/lib.rs; then
    echo "  OK:   ${method}"
  else
    echo "  FAIL: ${method} — no 'pub fn ${method}' found in any contracts/mux-*/src/lib.rs"
    FAILED=1
  fi
done <<< "$methods"

echo ""
if (( FAILED )); then
  echo "ERROR: CONTRIBUTING.md references an entrypoint that does not exist."
  echo "       Update the example to use a real, current contract method."
  exit 1
fi

echo "All checks passed: CONTRIBUTING.md examples reference real entrypoints."
