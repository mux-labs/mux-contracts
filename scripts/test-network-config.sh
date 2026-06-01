#!/usr/bin/env bash
# Tests for network-config.sh
set -euo pipefail
PASS=0; FAIL=0

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "  PASS: $label"
    ((PASS++))
  else
    echo "  FAIL: $label"
    echo "    expected: [$expected]"
    echo "    actual:   [$actual]"
    ((FAIL++))
  fi
}

source "$(dirname "$0")/network-config.sh"

echo "Testing load_network_config()..."

load_network_config testnet
assert_eq "testnet passphrase" "Test SDF Network ; September 2015"              "$NETWORK_PASSPHRASE"
assert_eq "testnet rpc_url"    "https://soroban-testnet.stellar.org"             "$NETWORK_RPC_URL"

load_network_config mainnet
assert_eq "mainnet passphrase" "Public Global Stellar Network ; September 2015" "$NETWORK_PASSPHRASE"

load_network_config futurenet
assert_eq "futurenet passphrase" "Test SDF Future Network ; October 2022"       "$NETWORK_PASSPHRASE"

load_network_config local
assert_eq "local passphrase"  "Standalone Network ; February 2017"              "$NETWORK_PASSPHRASE"
assert_eq "local rpc_url"     "http://localhost:8000/soroban/rpc"               "$NETWORK_RPC_URL"

load_unknown_exit=0
load_network_config unknown_net >/dev/null 2>&1 || load_unknown_exit=$?
assert_eq "unknown network returns error" "1" "$load_unknown_exit"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
