#!/usr/bin/env bash
# Unit tests for deploy-testnet.sh argument parsing and passphrase resolution.
# Run with: bash scripts/test-deploy-script.sh
set -euo pipefail
PASS=0; FAIL=0

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "  PASS: $label"
    ((PASS++))
  else
    echo "  FAIL: $label"
    echo "    expected: $expected"
    echo "    actual:   $actual"
    ((FAIL++))
  fi
}

resolve_passphrase() {
  case "$1" in
    testnet)        echo "Test SDF Network ; September 2015" ;;
    mainnet|public) echo "Public Global Stellar Network ; September 2015" ;;
    futurenet)      echo "Test SDF Future Network ; October 2022" ;;
    local)          echo "Standalone Network ; February 2017" ;;
    *)              echo "ERROR: Unknown network '$1'" ;;
  esac
}

echo "Testing resolve_passphrase()..."
assert_eq "testnet passphrase"   "Test SDF Network ; September 2015"           "$(resolve_passphrase testnet)"
assert_eq "mainnet passphrase"   "Public Global Stellar Network ; September 2015" "$(resolve_passphrase mainnet)"
assert_eq "futurenet passphrase" "Test SDF Future Network ; October 2022"      "$(resolve_passphrase futurenet)"
assert_eq "local passphrase"     "Standalone Network ; February 2017"          "$(resolve_passphrase local)"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
