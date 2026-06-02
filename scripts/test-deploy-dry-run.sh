#!/usr/bin/env bash
# Tests for deploy.sh --dry-run flag
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/deploy.sh"
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
  if echo "$out" | grep -q "$pattern"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (pattern '$pattern' not found in output)"
    FAIL=$((FAIL + 1))
  fi
}

echo "Testing deploy.sh --dry-run flag..."

# Dry-run should exit 0 even with no env vars set
assert_exit "dry-run exits 0 with no env vars" 0 \
  bash "$SCRIPT" --dry-run

# Dry-run with --network testnet should exit 0
assert_exit "dry-run --network testnet exits 0" 0 \
  bash "$SCRIPT" --dry-run --network testnet

# Dry-run with --network localnet should exit 0
assert_exit "dry-run --network localnet exits 0" 0 \
  bash "$SCRIPT" --dry-run --network localnet

# Dry-run with unknown network should exit 2
assert_exit "dry-run unknown network exits 2" 2 \
  bash "$SCRIPT" --dry-run --network unknown_net

# Dry-run with unknown flag should exit 2
assert_exit "unknown flag exits 2" 2 \
  bash "$SCRIPT" --dry-run --not-a-real-flag

# Dry-run output must contain [DRY-RUN] marker
assert_output_contains "dry-run output contains DRY-RUN marker" "DRY-RUN" \
  bash "$SCRIPT" --dry-run

# Dry-run with --contract single contract should exit 0
assert_exit "dry-run --contract single exits 0" 0 \
  bash "$SCRIPT" --dry-run --contract mux-account

# Dry-run output must warn about missing DEPLOYER_PRIVATE_KEY
assert_output_contains "dry-run warns about missing DEPLOYER_PRIVATE_KEY" "DEPLOYER_PRIVATE_KEY" \
  bash "$SCRIPT" --dry-run

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
