#!/usr/bin/env bash
# Tests for check-architecture-docs.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-architecture-docs.sh"

PASS=0
FAIL=0

assert_exit() {
  local label="$1" expected="$2"; shift 2
  local actual=0
  "$@" >/dev/null 2>&1 || actual=$?
  if [[ "$actual" -eq "$expected" ]]; then
    echo "  PASS: $label (exit $actual)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (expected exit $expected, got $actual)"
    FAIL=$((FAIL + 1))
  fi
}

assert_output_contains() {
  local label="$1" pattern="$2"; shift 2
  local out
  out=$("$@" 2>&1) || true
  if echo "$out" | grep -q "$pattern"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (pattern '$pattern' not found)"
    FAIL=$((FAIL + 1))
  fi
}

echo "Testing check-architecture-docs.sh..."

# The real architecture-overview.md must pass (all crates are now listed).
assert_exit "current architecture-overview.md passes" 0 bash "$SCRIPT"

TMPDIR_DOC="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_DOC"' EXIT

# A doc missing a crate (reproduces the pre-fix #701 gap) must fail.
cat > "${TMPDIR_DOC}/stale.md" <<'EOF'
# Architecture Overview

- mux-account
- mux-account-factory
- mux-batcher
- mux-permissions
- mux-registry
- mux-wallet-registry
- mux-recovery
- mux-delegation
EOF

assert_exit "doc missing mux-policy/mux-spending-policy fails" 1 \
  bash "$SCRIPT" --doc "${TMPDIR_DOC}/stale.md"

assert_output_contains "reports the missing crate by name" "mux-policy" \
  bash "$SCRIPT" --doc "${TMPDIR_DOC}/stale.md"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
