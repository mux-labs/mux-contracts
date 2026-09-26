#!/usr/bin/env bash
# Tests for check-mainnet-deploy-checklist.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-mainnet-deploy-checklist.sh"
PASS=0; FAIL=0

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

echo "Testing check-mainnet-deploy-checklist.sh..."

# 1. --help should exit 0
assert_exit "--help exits 0" 0 \
  bash "$SCRIPT" --help

# 2. Unknown argument should exit 2
assert_exit "unknown argument exits 2" 2 \
  bash "$SCRIPT" --invalid-flag

# 3. Running with --skip-git-clean should exit 0 on clean baseline
assert_exit "clean baseline exits 0 with --skip-git-clean" 0 \
  bash "$SCRIPT" --skip-git-clean

# 4. Running with --enforce-flag without MUX_MAINNET_DEPLOY_FLAG should exit 1
assert_exit "--enforce-flag without env var exits 1" 1 \
  env -u MUX_MAINNET_DEPLOY_FLAG bash "$SCRIPT" --skip-git-clean --enforce-flag

# 5. Running with --enforce-flag with invalid flag value should exit 1
assert_exit "--enforce-flag with invalid value exits 1" 1 \
  env MUX_MAINNET_DEPLOY_FLAG="true" bash "$SCRIPT" --skip-git-clean --enforce-flag

# 6. Running with --enforce-flag with exact sentinel should exit 0
assert_exit "--enforce-flag with exact sentinel value exits 0" 0 \
  env MUX_MAINNET_DEPLOY_FLAG="I_ACKNOWLEDGE_MAINNET_DEPLOY" bash "$SCRIPT" --skip-git-clean --enforce-flag

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
