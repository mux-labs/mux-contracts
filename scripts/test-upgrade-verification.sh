#!/usr/bin/env bash
#
# test-upgrade-verification.sh
#
# Test suite for upgrade verification script.
# Tests authorization, conflict detection, and safety validations.
#
# Usage:
#   bash scripts/test-upgrade-verification.sh
#
# Exit codes:
#   0 - All tests passed
#   1 - One or more tests failed

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# ─────────────────────────────────────────────────────────────────────────────
# Test utilities
# ─────────────────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

TESTS_PASSED=0
TESTS_FAILED=0

test_pass() {
  echo -e "${GREEN}✓${NC} $1"
  ((TESTS_PASSED++))
}

test_fail() {
  echo -e "${RED}✗${NC} $1"
  ((TESTS_FAILED++))
}

test_info() {
  echo -e "${YELLOW}→${NC} $1"
}

# ─────────────────────────────────────────────────────────────────────────────
# Setup/teardown
# ─────────────────────────────────────────────────────────────────────────────

setup() {
  # Create ops directory if it doesn't exist
  mkdir -p ops/logs
  
  # Clean up any existing lock files
  rm -f ops/.upgrade-lock-*
}

teardown() {
  # Clean up test artifacts
  rm -f ops/.upgrade-lock-*
  rm -f ops/logs/upgrade-*.log
}

# ─────────────────────────────────────────────────────────────────────────────
# Tests
# ─────────────────────────────────────────────────────────────────────────────

test_invalid_network() {
  test_info "Test: Invalid network"
  
  if bash scripts/verify-upgrade.sh --network invalid-network --contract mux-account --new-wasm-hash a1b2c3d4e5f6... --dry-run 2>&1 | grep -q "Invalid network"; then
    test_pass "Invalid network rejected"
  else
    test_fail "Invalid network not rejected"
  fi
}

test_valid_network() {
  test_info "Test: Valid networks"
  
  for network in testnet mainnet localnet; do
    if bash scripts/verify-upgrade.sh --network "$network" --contract mux-account --new-wasm-hash a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2 --dry-run &>/dev/null; then
      test_pass "Network '$network' accepted"
    else
      test_fail "Network '$network' rejected"
    fi
  done
}

test_invalid_wasm_hash() {
  test_info "Test: Invalid WASM hash"
  
  if bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash invalid --dry-run 2>&1 | grep -q "Invalid WASM hash"; then
    test_pass "Invalid WASM hash rejected"
  else
    test_fail "Invalid WASM hash not rejected"
  fi
}

test_valid_wasm_hash() {
  test_info "Test: Valid WASM hash"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  if bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run &>/dev/null; then
    test_pass "Valid WASM hash accepted"
  else
    test_fail "Valid WASM hash rejected"
  fi
}

test_upgrade_disabled_mainnet() {
  test_info "Test: Upgrade disabled on mainnet by default"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  if ENABLE_CONTRACT_UPGRADE=false bash scripts/verify-upgrade.sh --network mainnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run 2>&1 | grep -q "Upgrade operations are disabled"; then
    test_pass "Upgrade correctly disabled on mainnet"
  else
    test_fail "Upgrade not disabled on mainnet"
  fi
}

test_upgrade_enabled_with_flag() {
  test_info "Test: Upgrade enabled with ENABLE_CONTRACT_UPGRADE=true"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  if ENABLE_CONTRACT_UPGRADE=true bash scripts/verify-upgrade.sh --network mainnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run &>/dev/null; then
    test_pass "Upgrade enabled with flag"
  else
    test_fail "Upgrade not enabled with flag"
  fi
}

test_upgrade_allowed_testnet() {
  test_info "Test: Upgrade allowed on testnet"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  if bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run &>/dev/null; then
    test_pass "Upgrade allowed on testnet"
  else
    test_fail "Upgrade not allowed on testnet"
  fi
}

test_conflict_detection() {
  test_info "Test: Conflict detection"
  
  # Create a lock file
  mkdir -p ops
  echo "test-correlation-id" > ops/.upgrade-lock-testnet
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  if bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run 2>&1 | grep -q "Conflicting upgrade"; then
    test_pass "Conflict detected"
  else
    test_fail "Conflict not detected"
  fi
  
  # Clean up
  rm -f ops/.upgrade-lock-testnet
}

test_no_conflict_when_free() {
  test_info "Test: No conflict when no active upgrade"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  if bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run &>/dev/null; then
    test_pass "No conflict when free"
  else
    test_fail "False positive conflict"
  fi
}

test_correlation_id_generation() {
  test_info "Test: Correlation ID generation"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  local output
  output=$(bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run 2>&1)
  
  if echo "$output" | grep -q "Correlation ID:"; then
    test_pass "Correlation ID generated"
  else
    test_fail "Correlation ID not generated"
  fi
}

test_custom_correlation_id() {
  test_info "Test: Custom correlation ID"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  local output
  output=$(bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash "$valid_hash" --correlation-id custom-id-123 --dry-run 2>&1)
  
  if echo "$output" | grep -q "custom-id-123"; then
    test_pass "Custom correlation ID used"
  else
    test_fail "Custom correlation ID not used"
  fi
}

test_dry_run_mode() {
  test_info "Test: Dry-run mode"
  
  local valid_hash="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
  if bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash "$valid_hash" --dry-run &>/dev/null; then
    test_pass "Dry-run mode works"
  else
    test_fail "Dry-run mode failed"
  fi
}

test_help_flag() {
  test_info "Test: Help flag"
  
  if bash scripts/verify-upgrade.sh --help &>/dev/null; then
    test_pass "Help flag works"
  else
    test_fail "Help flag failed"
  fi
}

test_missing_contract() {
  test_info "Test: Missing contract argument"
  
  if bash scripts/verify-upgrade.sh --network testnet --new-wasm-hash a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2 --dry-run 2>&1 | grep -q "required"; then
    test_pass "Missing contract argument rejected"
  else
    test_fail "Missing contract argument not rejected"
  fi
}

test_missing_wasm_hash() {
  test_info "Test: Missing WASM hash argument"
  
  if bash scripts/verify-upgrade.sh --network testnet --contract mux-account --dry-run 2>&1 | grep -q "required"; then
    test_pass "Missing WASM hash argument rejected"
  else
    test_fail "Missing WASM hash argument not rejected"
  fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Main test runner
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  echo "Running upgrade verification tests..."
  echo ""
  
  setup
  
  # Run all tests
  test_invalid_network
  test_valid_network
  test_invalid_wasm_hash
  test_valid_wasm_hash
  test_upgrade_disabled_mainnet
  test_upgrade_enabled_with_flag
  test_upgrade_allowed_testnet
  test_conflict_detection
  test_no_conflict_when_free
  test_correlation_id_generation
  test_custom_correlation_id
  test_dry_run_mode
  test_help_flag
  test_missing_contract
  test_missing_wasm_hash
  
  teardown
  
  # Summary
  echo ""
  echo "Test Results:"
  echo "  Passed: $TESTS_PASSED"
  echo "  Failed: $TESTS_FAILED"
  echo ""
  
  if [[ $TESTS_FAILED -eq 0 ]]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
  else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
  fi
}

main "$@"
