#!/usr/bin/env bash
# Tests for verify-wasm-hash.sh
set -euo pipefail
PASS=0; FAIL=0

assert_exit() {
  local label="$1" expected_code="$2"; shift 2
  actual_code=0
  "$@" >/dev/null 2>&1 || actual_code=$?
  if [ "$actual_code" = "$expected_code" ]; then
    echo "  PASS: $label (exit $actual_code)"
    ((PASS++))
  else
    echo "  FAIL: $label (expected exit $expected_code, got $actual_code)"
    ((FAIL++))
  fi
}

SCRIPT="$(dirname "$0")/verify-wasm-hash.sh"

TMPFILE=$(mktemp /tmp/test_wasm_XXXXXX.wasm)
echo "fake wasm content" > "$TMPFILE"

if command -v sha256sum >/dev/null 2>&1; then
  CORRECT_HASH=$(sha256sum "$TMPFILE" | awk '{print $1}')
else
  CORRECT_HASH=$(shasum -a 256 "$TMPFILE" | awk '{print $1}')
fi

WRONG_HASH="0000000000000000000000000000000000000000000000000000000000000000"

echo "Testing verify-wasm-hash.sh..."
assert_exit "correct hash exits 0"    0 bash "$SCRIPT" "$TMPFILE" "$CORRECT_HASH"
assert_exit "wrong hash exits 1"      1 bash "$SCRIPT" "$TMPFILE" "$WRONG_HASH"
assert_exit "missing file exits 2"    2 bash "$SCRIPT" "/nonexistent.wasm" "$CORRECT_HASH"
assert_exit "bad hash format exits 2" 2 bash "$SCRIPT" "$TMPFILE" "not-a-hex-hash"
assert_exit "compute-only exits 0"    0 bash "$SCRIPT" --compute-only "$TMPFILE"
assert_exit "no args exits 2"         2 bash "$SCRIPT"

rm -f "$TMPFILE"
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
