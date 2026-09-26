#!/usr/bin/env bash
# Tests for check-no-testutils.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-no-testutils.sh"

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

echo "Testing check-no-testutils.sh..."

# Clean repo should pass Cargo.toml checks (WASM scan may SKIP).
assert_exit "clean tree exits 0" 0 bash "$SCRIPT" --wasm-dir /tmp/mux-no-wasm-$$

assert_output_contains "reports OK for mux-account" "mux-account" \
  bash "$SCRIPT" --wasm-dir /tmp/mux-no-wasm-$$

assert_output_contains "reports helpers are rlib-only" "rlib-only" \
  bash "$SCRIPT" --wasm-dir /tmp/mux-no-wasm-$$

assert_output_contains "skips missing WASM dir" "SKIP" \
  bash "$SCRIPT" --wasm-dir /tmp/mux-no-wasm-$$

# Synthetic WASM containing the string "testutils" must fail.
TMPDIR_WASM="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_WASM"' EXIT
printf 'padding testutils padding' > "${TMPDIR_WASM}/mux_account.wasm"

assert_exit "WASM with testutils string exits 1" 1 \
  bash "$SCRIPT" --wasm-dir "$TMPDIR_WASM"

assert_output_contains "flags contaminated WASM" "FAIL" \
  bash "$SCRIPT" --wasm-dir "$TMPDIR_WASM"

# Clean synthetic WASM passes the string scan.
printf 'clean wasm bytes with no banned marker' > "${TMPDIR_WASM}/mux_account.wasm"
assert_exit "clean synthetic WASM exits 0" 0 \
  bash "$SCRIPT" --wasm-dir "$TMPDIR_WASM"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
