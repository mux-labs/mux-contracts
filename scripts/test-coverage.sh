#!/usr/bin/env bash
# test-coverage.sh  (#662)
#
# Validates the coverage script without running instrumented cargo tests:
#   1. --stub prints the COVERAGE REPORT STUB banner
#   2. Every contracts/mux-* crate is named in the stub
#   3. --help documents --stub, --lcov, and cargo-llvm-cov install instructions
#   4. --stub exits 0

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COV_SCRIPT="${REPO_ROOT}/scripts/coverage.sh"
CONTRACTS_DIR="${REPO_ROOT}/contracts"

PASS=0
FAIL=0

assert_exit() {
  local label="$1" expected_code="$2"; shift 2
  local actual_code=0
  "$@" >/dev/null 2>&1 || actual_code=$?
  if [[ "$actual_code" -eq "$expected_code" ]]; then
    echo "  PASS: $label (exit $actual_code)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (expected exit $expected_code, got $actual_code)"
    FAIL=$((FAIL + 1))
  fi
}

assert_output_contains() {
  local label="$1" pattern="$2"; shift 2
  local out
  out=$("$@" 2>&1) || true
  if echo "$out" | grep -q -- "$pattern"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (pattern '$pattern' not found)"
    FAIL=$((FAIL + 1))
  fi
}

echo "Part 1: Stub banner, exit code, and help"
assert_exit "stub exits 0" 0 \
  bash "${COV_SCRIPT}" --stub
assert_output_contains "stub banner present" "COVERAGE REPORT STUB" \
  bash "${COV_SCRIPT}" --stub
assert_output_contains "help documents --stub" -- "--stub" \
  bash "${COV_SCRIPT}" --help
assert_output_contains "help documents --lcov" -- "--lcov" \
  bash "${COV_SCRIPT}" --help
assert_output_contains "help documents cargo-llvm-cov install" "cargo install cargo-llvm-cov" \
  bash "${COV_SCRIPT}" --help

echo "Part 2: Stub lists every mux-* contract crate"
STUB_OUT="$(bash "${COV_SCRIPT}" --stub 2>&1)"
for dir in "${CONTRACTS_DIR}"/mux-*/; do
  name="$(basename "$dir")"
  if echo "$STUB_OUT" | grep -q "• ${name}"; then
    echo "  PASS: stub lists ${name}"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: stub missing ${name}"
    FAIL=$((FAIL + 1))
  fi
done

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"
if (( FAIL > 0 )); then
  exit 1
fi
exit 0
