#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/bindings/scripts/check-funded-deployer.mjs"
NODE_BIN="${NODE_BIN:-node}"
PASS=0
FAIL=0

assert_exit() {
  local label="$1" expected="$2"; shift 2
  local actual=0
  "$@" >/dev/null 2>&1 || actual=$?
  if [[ "$actual" -eq "$expected" ]]; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (expected $expected, got $actual)"
    FAIL=$((FAIL + 1))
  fi
}

assert_output() {
  local label="$1" pattern="$2"; shift 2
  local output
  output=$("$@" 2>&1) || true
  if grep -qF -- "$pattern" <<<"$output"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (missing '$pattern')"
    FAIL=$((FAIL + 1))
  fi
}

echo "Testing funded deployer automation checks..."
assert_exit "dry-run succeeds without a secret" 0 "$NODE_BIN" "$SCRIPT" --dry-run
assert_output "dry-run reports no secret" "dry run" "$NODE_BIN" "$SCRIPT" --dry-run
assert_exit "live check rejects a missing secret" 1 env -u DEPLOYER_PRIVATE_KEY "$NODE_BIN" "$SCRIPT"
assert_exit "invalid minimum balance is rejected" 1 env MIN_DEPLOYER_BALANCE_XLM=-1 "$NODE_BIN" "$SCRIPT" --dry-run

echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
