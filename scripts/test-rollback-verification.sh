#!/usr/bin/env bash
#
# test-rollback-verification.sh
#
# Test suite for rollback verification script.
# Tests authorization, conflict detection, and safety validations.
#
# Usage:
#   bash scripts/test-rollback-verification.sh
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
  rm -f ops/.rollback-lock-*
}

teardown() {
  # Clean up test artifacts
  rm -f ops/.rollback-lock-*
  rm -f ops/logs/rollback-*.log
}

# ─────────────────────────────────────────────────────────────────────────────
# Tests
# ─────────────────────────────────────────────────────────────────────────────

test_invalid_strategy() {
  test_info "Test: Invalid rollback strategy"
  
  if bash scripts/verify-rollback.sh --network testnet --strategy invalid-strategy --dry-run 2>&1 | grep -q "Invalid rollback strategy"; then
    test_pass "Invalid strategy rejected"
  else
    test_fail "Invalid strategy not rejected"
  fi
}

test_valid_strategy() {
  test_info "Test: Valid rollback strategies"
  
  for strategy in address-repoint deploy-wasm admin-pause; do
    if bash scripts/verify-rollback.sh --network testnet --strategy "$strategy" --dry-run &>/dev/null; then
      test_pass "Strategy '$strategy' accepted"
    else
      test_fail "Strategy '$strategy' rejected"
    fi
  done
}

test_invalid_network() {
  test_info "Test: Invalid network"
  
  if bash scripts/verify-rollback.sh --network invalid-network --strategy address-repoint --dry-run 2>&1 | grep -q "Invalid network"; then
    test_pass "Invalid network rejected"
  else
    test_fail "Invalid network not rejected"
  fi
}

test_rollback_disabled_mainnet() {
  test_info "Test: Rollback disabled on mainnet by default"
  
  if ENABLE_ROLLBACK=false bash scripts/verify-rollback.sh --network mainnet --strategy address-repoint --dry-run 2>&1 | grep -q "Rollback operations are disabled"; then
    test_pass "Rollback correctly disabled on mainnet"
  else
    test_fail "Rollback not disabled on mainnet"
  fi
}

test_rollback_enabled_testnet() {
  test_info "Test: Rollback allowed on testnet"
  
  if bash scripts/verify-rollback.sh --network testnet --strategy address-repoint --dry-run &>/dev/null; then
    test_pass "Rollback allowed on testnet"
  else
    test_fail "Rollback not allowed on testnet"
  fi
}

test_rollback_enabled_with_flag() {
  test_info "Test: Rollback enabled with ENABLE_ROLLBACK=true"
  
  if ENABLE_ROLLBACK=true bash scripts/verify-rollback.sh --network mainnet --strategy address-repoint --dry-run &>/dev/null; then
    test_pass "Rollback enabled with flag"
  else
    test_fail "Rollback not enabled with flag"
  fi
}

test_conflict_detection() {
  test_info "Test: Conflict detection"
  
  # Create a lock file
  mkdir -p ops
  echo "test-correlation-id" > ops/.rollback-lock-testnet
  
  if bash scripts/verify-rollback.sh --network testnet --strategy address-repoint --dry-run 2>&1 | grep -q "Conflicting rollback"; then
    test_pass "Conflict detected"
  else
    test_fail "Conflict not detected"
  fi
  
  # Clean up
  rm -f ops/.rollback-lock-testnet
}

test_no_conflict_when_free() {
  test_info "Test: No conflict when no active rollback"
  
  if bash scripts/verify-rollback.sh --network testnet --strategy address-repoint --dry-run &>/dev/null; then
    test_pass "No conflict when free"
  else
    test_fail "False positive conflict"
  fi
}

test_correlation_id_generation() {
  test_info "Test: Correlation ID generation"
  
  local output
  output=$(bash scripts/verify-rollback.sh --network testnet --strategy address-repoint --dry-run 2>&1)
  
  if echo "$output" | grep -q "Correlation ID:"; then
    test_pass "Correlation ID generated"
  else
    test_fail "Correlation ID not generated"
  fi
}

test_custom_correlation_id() {
  test_info "Test: Custom correlation ID"
  
  local output
  output=$(bash scripts/verify-rollback.sh --network testnet --strategy address-repoint --correlation-id custom-id-123 --dry-run 2>&1)
  
  if echo "$output" | grep -q "custom-id-123"; then
    test_pass "Custom correlation ID used"
  else
    test_fail "Custom correlation ID not used"
  fi
}

test_dry_run_mode() {
  test_info "Test: Dry-run mode"
  
  if bash scripts/verify-rollback.sh --network testnet --strategy address-repoint --dry-run &>/dev/null; then
    test_pass "Dry-run mode works"
  else
    test_fail "Dry-run mode failed"
  fi
}

test_help_flag() {
  test_info "Test: Help flag"
  
  if bash scripts/verify-rollback.sh --help &>/dev/null; then
    test_pass "Help flag works"
  else
    test_fail "Help flag failed"
  fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Main test runner
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  echo "Running rollback verification tests..."
  echo ""
  
  setup
  
  # Run all tests
  test_invalid_strategy
  test_valid_strategy
  test_invalid_network
  test_rollback_disabled_mainnet
  test_rollback_enabled_testnet
  test_rollback_enabled_with_flag
  test_conflict_detection
  test_no_conflict_when_free
  test_correlation_id_generation
  test_custom_correlation_id
  test_dry_run_mode
  test_help_flag
  
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
