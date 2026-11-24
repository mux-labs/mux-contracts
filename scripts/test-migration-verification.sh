#!/usr/bin/env bash
#
# test-migration-verification.sh
#
# Test suite for migration verification script.
# Tests authorization, conflict detection, and safety validations.
#
# Usage:
#   bash scripts/test-migration-verification.sh
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
  rm -f ops/.migration-lock-*
}

teardown() {
  # Clean up test artifacts
  rm -f ops/.migration-lock-*
  rm -f ops/logs/migration-*.log
}

# ─────────────────────────────────────────────────────────────────────────────
# Tests
# ─────────────────────────────────────────────────────────────────────────────

test_invalid_network() {
  test_info "Test: Invalid network"
  
  if bash scripts/verify-migration.sh --network invalid-network --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run 2>&1 | grep -q "Invalid network"; then
    test_pass "Invalid network rejected"
  else
    test_fail "Invalid network not rejected"
  fi
}

test_valid_network() {
  test_info "Test: Valid networks"
  
  for network in testnet mainnet localnet; do
    if bash scripts/verify-migration.sh --network "$network" --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run &>/dev/null; then
      test_pass "Network '$network' accepted"
    else
      test_fail "Network '$network' rejected"
    fi
  done
}

test_invalid_strategy() {
  test_info "Test: Invalid migration strategy"
  
  if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy invalid --dry-run 2>&1 | grep -q "Invalid migration strategy"; then
    test_pass "Invalid strategy rejected"
  else
    test_fail "Invalid strategy not rejected"
  fi
}

test_valid_strategy() {
  test_info "Test: Valid migration strategies"
  
  for strategy in additive transformative two-phase; do
    if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy "$strategy" --dry-run &>/dev/null; then
      test_pass "Strategy '$strategy' accepted"
    else
      test_fail "Strategy '$strategy' rejected"
    fi
  done
}

test_migration_disabled_mainnet() {
  test_info "Test: Migration disabled on mainnet by default"
  
  if ENABLE_ACCOUNT_MIGRATION=false bash scripts/verify-migration.sh --network mainnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run 2>&1 | grep -q "Migration operations are disabled"; then
    test_pass "Migration correctly disabled on mainnet"
  else
    test_fail "Migration not disabled on mainnet"
  fi
}

test_migration_enabled_with_flag() {
  test_info "Test: Migration enabled with ENABLE_ACCOUNT_MIGRATION=true"
  
  if ENABLE_ACCOUNT_MIGRATION=true bash scripts/verify-migration.sh --network mainnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run &>/dev/null; then
    test_pass "Migration enabled with flag"
  else
    test_fail "Migration not enabled with flag"
  fi
}

test_migration_allowed_testnet() {
  test_info "Test: Migration allowed on testnet"
  
  if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run &>/dev/null; then
    test_pass "Migration allowed on testnet"
  else
    test_fail "Migration not allowed on testnet"
  fi
}

test_conflict_detection() {
  test_info "Test: Conflict detection"
  
  # Create a lock file
  mkdir -p ops
  echo "test-correlation-id" > ops/.migration-lock-testnet-CABCD1234567890
  
  if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run 2>&1 | grep -q "Conflicting migration"; then
    test_pass "Conflict detected"
  else
    test_fail "Conflict not detected"
  fi
  
  # Clean up
  rm -f ops/.migration-lock-testnet-CABCD1234567890
}

test_no_conflict_when_free() {
  test_info "Test: No conflict when no active migration"
  
  if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run &>/dev/null; then
    test_pass "No conflict when free"
  else
    test_fail "False positive conflict"
  fi
}

test_correlation_id_generation() {
  test_info "Test: Correlation ID generation"
  
  local output
  output=$(bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run 2>&1)
  
  if echo "$output" | grep -q "Correlation ID:"; then
    test_pass "Correlation ID generated"
  else
    test_fail "Correlation ID not generated"
  fi
}

test_custom_correlation_id() {
  test_info "Test: Custom correlation ID"
  
  local output
  output=$(bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --correlation-id custom-id-123 --dry-run 2>&1)
  
  if echo "$output" | grep -q "custom-id-123"; then
    test_pass "Custom correlation ID used"
  else
    test_fail "Custom correlation ID not used"
  fi
}

test_dry_run_mode() {
  test_info "Test: Dry-run mode"
  
  if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run &>/dev/null; then
    test_pass "Dry-run mode works"
  else
    test_fail "Dry-run mode failed"
  fi
}

test_help_flag() {
  test_info "Test: Help flag"
  
  if bash scripts/verify-migration.sh --help &>/dev/null; then
    test_pass "Help flag works"
  else
    test_fail "Help flag failed"
  fi
}

test_missing_account_id() {
  test_info "Test: Missing account-id argument"
  
  if bash scripts/verify-migration.sh --network testnet --from-version 1.0 --to-version 1.1 --strategy additive --dry-run 2>&1 | grep -q "required"; then
    test_pass "Missing account-id argument rejected"
  else
    test_fail "Missing account-id argument not rejected"
  fi
}

test_missing_version() {
  test_info "Test: Missing version arguments"
  
  if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --strategy additive --dry-run 2>&1 | grep -q "required"; then
    test_pass "Missing version arguments rejected"
  else
    test_fail "Missing version arguments not rejected"
  fi
}

test_missing_strategy() {
  test_info "Test: Missing strategy argument"
  
  if bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --dry-run 2>&1 | grep -q "required"; then
    test_pass "Missing strategy argument rejected"
  else
    test_fail "Missing strategy argument not rejected"
  fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Main test runner
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  echo "Running migration verification tests..."
  echo ""
  
  setup
  
  # Run all tests
  test_invalid_network
  test_valid_network
  test_invalid_strategy
  test_valid_strategy
  test_migration_disabled_mainnet
  test_migration_enabled_with_flag
  test_migration_allowed_testnet
  test_conflict_detection
  test_no_conflict_when_free
  test_correlation_id_generation
  test_custom_correlation_id
  test_dry_run_mode
  test_help_flag
  test_missing_account_id
  test_missing_version
  test_missing_strategy
  
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
