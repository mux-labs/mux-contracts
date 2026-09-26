#!/usr/bin/env bash
#
# test-relayer-verification.sh
#
# Test suite for relayer verification script.
# Tests authorization, rate limiting, and safety validations.
#
# Usage:
#   bash scripts/test-relayer-verification.sh
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
  
  # Clean up any existing rate limit files
  rm -f ops/.relayer-rate-limit-*
}

teardown() {
  # Clean up test artifacts
  rm -f ops/.relayer-rate-limit-*
  rm -f ops/logs/relayer-*.log
}

# ─────────────────────────────────────────────────────────────────────────────
# Tests
# ─────────────────────────────────────────────────────────────────────────────

test_invalid_network() {
  test_info "Test: Invalid network"
  
  if bash scripts/verify-relayer.sh --network invalid-network --relayer-id relayer-123 --operation-type intent --dry-run 2>&1 | grep -q "Invalid network"; then
    test_pass "Invalid network rejected"
  else
    test_fail "Invalid network not rejected"
  fi
}

test_valid_network() {
  test_info "Test: Valid networks"
  
  for network in testnet mainnet localnet; do
    if bash scripts/verify-relayer.sh --network "$network" --relayer-id relayer-123 --operation-type intent --dry-run &>/dev/null; then
      test_pass "Network '$network' accepted"
    else
      test_fail "Network '$network' rejected"
    fi
  done
}

test_invalid_operation_type() {
  test_info "Test: Invalid operation type"
  
  if bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type invalid --dry-run 2>&1 | grep -q "Invalid operation type"; then
    test_pass "Invalid operation type rejected"
  else
    test_fail "Invalid operation type not rejected"
  fi
}

test_valid_operation_type() {
  test_info "Test: Valid operation types"
  
  for op_type in intent batch read; do
    if bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type "$op_type" --dry-run &>/dev/null; then
      test_pass "Operation type '$op_type' accepted"
    else
      test_fail "Operation type '$op_type' rejected"
    fi
  done
}

test_relayer_disabled_mainnet() {
  test_info "Test: Relayer disabled on mainnet by default"
  
  if ENABLE_RELAYER=false bash scripts/verify-relayer.sh --network mainnet --relayer-id relayer-123 --operation-type intent --dry-run 2>&1 | grep -q "Relayer operations are disabled"; then
    test_pass "Relayer correctly disabled on mainnet"
  else
    test_fail "Relayer not disabled on mainnet"
  fi
}

test_relayer_enabled_with_flag() {
  test_info "Test: Relayer enabled with ENABLE_RELAYER=true"
  
  if ENABLE_RELAYER=true bash scripts/verify-relayer.sh --network mainnet --relayer-id relayer-123 --operation-type intent --dry-run &>/dev/null; then
    test_pass "Relayer enabled with flag"
  else
    test_fail "Relayer not enabled with flag"
  fi
}

test_relayer_allowed_testnet() {
  test_info "Test: Relayer allowed on testnet"
  
  if bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run &>/dev/null; then
    test_pass "Relayer allowed on testnet"
  else
    test_fail "Relayer not allowed on testnet"
  fi
}

test_api_key_validation() {
  test_info "Test: Invalid API key format"
  
  if RELAYER_API_KEY="invalid_key" bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run 2>&1 | grep -q "Invalid API key format"; then
    test_pass "Invalid API key format rejected"
  else
    test_fail "Invalid API key format not rejected"
  fi
}

test_valid_api_key_format() {
  test_info "Test: Valid API key format"
  
  if RELAYER_API_KEY="mux_relayer_v1_key123_signature456" bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run &>/dev/null; then
    test_pass "Valid API key format accepted"
  else
    test_fail "Valid API key format rejected"
  fi
}

test_rate_limit() {
  test_info "Test: Rate limit enforcement"
  
  # Set up rate limit file with high count
  mkdir -p ops
  echo "$(date +%s) 1000" > ops/.relayer-rate-limit-testnet-relayer-123
  
  if bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run 2>&1 | grep -q "Rate limit exceeded"; then
    test_pass "Rate limit enforced"
  else
    test_fail "Rate limit not enforced"
  fi
  
  # Clean up
  rm -f ops/.relayer-rate-limit-testnet-relayer-123
}

test_rate_limit_reset() {
  test_info "Test: Rate limit reset after time window"
  
  # Set up rate limit file with old timestamp
  mkdir -p ops
  echo "$(( $(date +%s) - 120 )) 1000" > ops/.relayer-rate-limit-testnet-relayer-123
  
  if bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run &>/dev/null; then
    test_pass "Rate limit reset after time window"
  else
    test_fail "Rate limit not reset"
  fi
  
  # Clean up
  rm -f ops/.relayer-rate-limit-testnet-relayer-123
}

test_correlation_id_generation() {
  test_info "Test: Correlation ID generation"
  
  local output
  output=$(bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run 2>&1)
  
  if echo "$output" | grep -q "Correlation ID:"; then
    test_pass "Correlation ID generated"
  else
    test_fail "Correlation ID not generated"
  fi
}

test_custom_correlation_id() {
  test_info "Test: Custom correlation ID"
  
  local output
  output=$(bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --correlation-id custom-id-123 --dry-run 2>&1)
  
  if echo "$output" | grep -q "custom-id-123"; then
    test_pass "Custom correlation ID used"
  else
    test_fail "Custom correlation ID not used"
  fi
}

test_dry_run_mode() {
  test_info "Test: Dry-run mode"
  
  if bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run &>/dev/null; then
    test_pass "Dry-run mode works"
  else
    test_fail "Dry-run mode failed"
  fi
}

test_help_flag() {
  test_info "Test: Help flag"
  
  if bash scripts/verify-relayer.sh --help &>/dev/null; then
    test_pass "Help flag works"
  else
    test_fail "Help flag failed"
  fi
}

test_missing_relayer_id() {
  test_info "Test: Missing relayer-id argument"
  
  if bash scripts/verify-relayer.sh --network testnet --operation-type intent --dry-run 2>&1 | grep -q "required"; then
    test_pass "Missing relayer-id argument rejected"
  else
    test_fail "Missing relayer-id argument not rejected"
  fi
}

test_missing_operation_type() {
  test_info "Test: Missing operation-type argument"
  
  if bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --dry-run 2>&1 | grep -q "required"; then
    test_pass "Missing operation-type argument rejected"
  else
    test_fail "Missing operation-type argument not rejected"
  fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Main test runner
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  echo "Running relayer verification tests..."
  echo ""
  
  setup
  
  # Run all tests
  test_invalid_network
  test_valid_network
  test_invalid_operation_type
  test_valid_operation_type
  test_relayer_disabled_mainnet
  test_relayer_enabled_with_flag
  test_relayer_allowed_testnet
  test_api_key_validation
  test_valid_api_key_format
  test_rate_limit
  test_rate_limit_reset
  test_correlation_id_generation
  test_custom_correlation_id
  test_dry_run_mode
  test_help_flag
  test_missing_relayer_id
  test_missing_operation_type
  
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
