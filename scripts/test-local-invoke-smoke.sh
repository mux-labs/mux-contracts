#!/usr/bin/env bash
# Tests for local-invoke-smoke.sh (dry-run / CLI validation only — no RPC).
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/local-invoke-smoke.sh"
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
    echo "  FAIL: $label (expected $expected, got $actual)"
    FAIL=$((FAIL + 1))
  fi
}

assert_output_contains() {
  local label="$1" pattern="$2"; shift 2
  local out
  out=$("$@" 2>&1) || true
  if echo "$out" | grep -q -F -- "$pattern"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (pattern '$pattern' not found)"
    FAIL=$((FAIL + 1))
  fi
}

echo "Testing local-invoke-smoke.sh..."

# Missing secret key should exit 2
assert_exit "missing secret-key exits 2" 2 \
  env -u SECRET_KEY -u DEPLOYER_PRIVATE_KEY bash "$SCRIPT"

# --help exits 0
assert_exit "help exits 0" 0 \
  bash "$SCRIPT" --help

# Dry-run exits 0 without secret key
assert_exit "dry-run exits 0 without secret" 0 \
  env -u SECRET_KEY -u DEPLOYER_PRIVATE_KEY bash "$SCRIPT" --dry-run

# Dry-run lists planned mux-account owner check
assert_output_contains "dry-run plans mux-account::owner" "mux-account::owner" \
  bash "$SCRIPT" --dry-run

# Dry-run with contract filter only plans that contract
assert_output_contains "dry-run --contract mux-account mentions account" "mux-account::owner" \
  bash "$SCRIPT" --dry-run --contract mux-account

# Unknown flag exits 2
assert_exit "unknown flag exits 2" 2 \
  bash "$SCRIPT" --dry-run --not-a-real-flag

echo ""
echo "Results: $PASS passed, $FAIL failed"
if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
exit 0
