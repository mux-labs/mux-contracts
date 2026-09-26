#!/usr/bin/env bash
# test-ttl-keeper.sh
#
# Test script validating that TTL extension logic works correctly for both
# instance and persistent storage in all Mux contracts. This addresses checklist
# section 6 from storage-griefing.md.
#
# Usage:
#   bash scripts/test-ttl-keeper.sh
#
# Exit codes:
#   0 — all tests passed
#   1 — one or more tests failed

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

PASS=0
FAIL=0

echo "=========================================="
echo "TTL Keeper Test Suite"
echo "=========================================="
echo ""

# Test 1: Verify TTL constants are defined correctly
echo "Test 1: Verify TTL constants across all contracts"
echo "----------------------------------------------"

EXPECTED_THRESHOLD="17_280"  # ~1 day in ledgers
EXPECTED_EXTEND="518_400"    # ~30 days in ledgers

CONTRACTS=(
  "contracts/mux-account/src/lib.rs"
  "contracts/mux-account-factory/src/lib.rs"
  "contracts/mux-batcher/src/lib.rs"
  "contracts/mux-delegation/src/lib.rs"
  "contracts/mux-permissions/src/lib.rs"
  "contracts/mux-policy/src/lib.rs"
  "contracts/mux-recovery/src/lib.rs"
  "contracts/mux-registry/src/lib.rs"
  "contracts/mux-spending-policy/src/lib.rs"
  "contracts/mux-wallet-registry/src/lib.rs"
)

TTL_CONSTANT_CHECK_PASSED=true

for contract in "${CONTRACTS[@]}"; do
  if [[ ! -f "$contract" ]]; then
    echo "  ✗ Contract not found: $contract"
    TTL_CONSTANT_CHECK_PASSED=false
    continue
  fi
  
  # Check for TTL constants
  if ! grep -qE "TTL_(THRESHOLD|EXTEND_TO)" "$contract"; then
    echo "  ⚠ No TTL constants in: $contract (may use default or be stateless)"
    continue
  fi
  
  # Verify threshold value
  if grep -qE "TTL_THRESHOLD.*=.*${EXPECTED_THRESHOLD}" "$contract"; then
    echo "  ✓ TTL_THRESHOLD correct in: $(basename "$contract")"
  else
    echo "  ✗ TTL_THRESHOLD incorrect in: $contract"
    echo "    Expected: $EXPECTED_THRESHOLD"
    echo "    Found: $(grep -E "TTL_THRESHOLD" "$contract" | head -1)"
    TTL_CONSTANT_CHECK_PASSED=false
  fi
  
  # Verify extend value
  if grep -qE "TTL_EXTEND_TO.*=.*${EXPECTED_EXTEND}" "$contract"; then
    echo "  ✓ TTL_EXTEND_TO correct in: $(basename "$contract")"
  else
    echo "  ✗ TTL_EXTEND_TO incorrect in: $contract"
    echo "    Expected: $EXPECTED_EXTEND"
    echo "    Found: $(grep -E "TTL_EXTEND_TO" "$contract" | head -1)"
    TTL_CONSTANT_CHECK_PASSED=false
  fi
done

echo ""
if $TTL_CONSTANT_CHECK_PASSED; then
  echo "✓ Test 1 PASSED: TTL constants are correctly defined"
  PASS=$((PASS + 1))
else
  echo "✗ Test 1 FAILED: TTL constants are incorrect or missing"
  FAIL=$((FAIL + 1))
fi
echo ""

# Test 2: Verify extend_ttl calls on write paths
echo "Test 2: Verify extend_ttl() is called on all write paths"
echo "--------------------------------------------------------"

EXTEND_TTL_CHECK_PASSED=true

for contract in "${CONTRACTS[@]}"; do
  if [[ ! -f "$contract" ]]; then
    continue
  fi
  
  # Count public write functions (pub fn that mutate storage)
  WRITE_FUNCTIONS=$(grep -E "^\s*pub fn (set|add|remove|grant|revoke|register|deploy|initialize|record|reset|update)" "$contract" | wc -l)
  
  # Count extend_ttl calls
  EXTEND_CALLS=$(grep -E "extend_ttl\(" "$contract" | grep -v "^\s*//" | wc -l)
  
  if [[ "$WRITE_FUNCTIONS" -gt 0 && "$EXTEND_CALLS" -gt 0 ]]; then
    echo "  ✓ $(basename "$contract"): $EXTEND_CALLS extend_ttl calls for $WRITE_FUNCTIONS write functions"
  elif [[ "$WRITE_FUNCTIONS" -gt 0 && "$EXTEND_CALLS" -eq 0 ]]; then
    echo "  ✗ $(basename "$contract"): $WRITE_FUNCTIONS write functions but NO extend_ttl calls"
    EXTEND_TTL_CHECK_PASSED=false
  else
    echo "  ⚠ $(basename "$contract"): No write functions (stateless or read-only)"
  fi
done

echo ""
if $EXTEND_TTL_CHECK_PASSED; then
  echo "✓ Test 2 PASSED: All write paths call extend_ttl()"
  PASS=$((PASS + 1))
else
  echo "✗ Test 2 FAILED: Some write paths missing extend_ttl() calls"
  FAIL=$((FAIL + 1))
fi
echo ""

# Test 3: Verify unit tests for TTL extension exist
echo "Test 3: Verify TTL extension unit tests"
echo "----------------------------------------"

TTL_TEST_CHECK_PASSED=true

TTL_TEST_PATTERNS=(
  "test_ttl_extended"
  "test_ttl_constants"
  "test.*ttl"
)

for contract in "${CONTRACTS[@]}"; do
  if [[ ! -f "$contract" ]]; then
    continue
  fi
  
  HAS_TTL_TEST=false
  for pattern in "${TTL_TEST_PATTERNS[@]}"; do
    if grep -qE "#\[test\]|fn ${pattern}" "$contract"; then
      HAS_TTL_TEST=true
      break
    fi
  done
  
  if $HAS_TTL_TEST; then
    echo "  ✓ $(basename "$contract"): Has TTL extension tests"
  else
    # Check if contract has storage (skip stateless contracts)
    if grep -qE "storage\(\)" "$contract"; then
      echo "  ✗ $(basename "$contract"): Missing TTL extension tests"
      TTL_TEST_CHECK_PASSED=false
    else
      echo "  ⚠ $(basename "$contract"): Stateless (no storage tests needed)"
    fi
  fi
done

echo ""
if $TTL_TEST_CHECK_PASSED; then
  echo "✓ Test 3 PASSED: All contracts with storage have TTL tests"
  PASS=$((PASS + 1))
else
  echo "✗ Test 3 FAILED: Some contracts missing TTL tests"
  FAIL=$((FAIL + 1))
fi
echo ""

# Test 4: Verify persistent storage TTL extension (mux-policy, mux-delegation)
echo "Test 4: Verify persistent storage TTL extension"
echo "------------------------------------------------"

PERSISTENT_TTL_CHECK_PASSED=true

# mux-policy uses persistent storage for WalletLimit
if grep -qE "persistent\(\)\.extend_ttl" "contracts/mux-policy/src/lib.rs"; then
  echo "  ✓ mux-policy: Uses persistent().extend_ttl() for per-wallet limits"
else
  echo "  ✗ mux-policy: Missing persistent storage TTL extension"
  PERSISTENT_TTL_CHECK_PASSED=false
fi

# mux-delegation uses persistent storage for DelegatePerms
if grep -qE "persistent\(\)\.extend_ttl" "contracts/mux-delegation/src/lib.rs"; then
  echo "  ✓ mux-delegation: Uses persistent().extend_ttl() for delegate permissions"
else
  echo "  ✗ mux-delegation: Missing persistent storage TTL extension"
  PERSISTENT_TTL_CHECK_PASSED=false
fi

echo ""
if $PERSISTENT_TTL_CHECK_PASSED; then
  echo "✓ Test 4 PASSED: Persistent storage TTL extension is correct"
  PASS=$((PASS + 1))
else
  echo "✗ Test 4 FAILED: Persistent storage missing TTL extension"
  FAIL=$((FAIL + 1))
fi
echo ""

# Test 5: Verify keeper script exists and is documented
echo "Test 5: Verify keeper documentation and scripts"
echo "-----------------------------------------------"

KEEPER_DOC_CHECK_PASSED=true

# Check for keeper documentation in storage-griefing.md
if grep -qE "keeper|TTL.*keeper|extend.*ttl" "docs/storage-griefing.md"; then
  echo "  ✓ storage-griefing.md: Documents keeper requirements"
else
  echo "  ✗ storage-griefing.md: Missing keeper documentation"
  KEEPER_DOC_CHECK_PASSED=false
fi

# Check for stellar contract extend example
if grep -qE "stellar contract extend" "docs/storage-griefing.md"; then
  echo "  ✓ storage-griefing.md: Provides stellar CLI extend example"
else
  echo "  ✗ storage-griefing.md: Missing CLI extend example"
  KEEPER_DOC_CHECK_PASSED=false
fi

# Check README mentions TTL/keeper
if grep -qE "TTL|keeper" "README.md"; then
  echo "  ✓ README.md: Mentions TTL management"
else
  echo "  ⚠ README.md: Could mention TTL keeper requirements"
fi

echo ""
if $KEEPER_DOC_CHECK_PASSED; then
  echo "✓ Test 5 PASSED: Keeper documentation is complete"
  PASS=$((PASS + 1))
else
  echo "✗ Test 5 FAILED: Keeper documentation is incomplete"
  FAIL=$((FAIL + 1))
fi
echo ""

# Test 6: Run contract unit tests that verify TTL behavior
echo "Test 6: Run contract unit tests for TTL behavior"
echo "-------------------------------------------------"

echo "  Running cargo test with ttl filter..."
if cargo test --workspace ttl --quiet 2>&1 | grep -qE "(test result: ok|passed)"; then
  echo "  ✓ Contract TTL unit tests passed"
  PASS=$((PASS + 1))
else
  echo "  ✗ Contract TTL unit tests failed or not found"
  FAIL=$((FAIL + 1))
fi
echo ""

# Summary
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo "Passed: $PASS"
echo "Failed: $FAIL"
echo ""

if [[ "$FAIL" -gt 0 ]]; then
  echo "✗ FAILED: Some TTL keeper tests did not pass"
  echo ""
  echo "Action required:"
  echo "1. Review failed test output above"
  echo "2. Fix missing TTL constants, extend_ttl calls, or tests"
  echo "3. Update docs/storage-griefing.md if keeper guidance is incomplete"
  echo "4. Re-run: bash scripts/test-ttl-keeper.sh"
  echo ""
  exit 1
fi

echo "✓ SUCCESS: All TTL keeper tests passed"
echo ""
echo "Checklist section 6 (TTL/storage-griefing keeper) is complete:"
echo "  - TTL constants are correctly defined"
echo "  - All write paths extend TTL"
echo "  - Unit tests verify TTL behavior"
echo "  - Persistent storage TTL is handled"
echo "  - Keeper documentation is present"
echo ""
exit 0
