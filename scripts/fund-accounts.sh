#!/usr/bin/env bash
#
# fund-accounts.sh
#
# Fund one or more Stellar accounts using Friendbot (testnet) or a local faucet endpoint.
#
# This script accepts target account addresses as command-line arguments or reads them from
# a config file. It gracefully handles accounts that are already funded.
#
# Usage:
#   # Fund a single account on testnet
#   bash scripts/fund-accounts.sh GBXYZ...
#
#   # Fund multiple accounts on testnet
#   bash scripts/fund-accounts.sh GBXYZ... GBXYZ...
#
#   # Fund accounts from a config file on localnet
#   SOROBAN_NETWORK=localnet bash scripts/fund-accounts.sh --from-file accounts.txt
#
#   # Fund accounts with custom localnet endpoint
#   LOCALNET_FAUCET_URL=http://localhost:8000 bash scripts/fund-accounts.sh GBXYZ...
#
# Configuration via Environment Variables:
#   SOROBAN_NETWORK         - Network to use (testnet|localnet, default: testnet)
#   FRIENDBOT_URL           - Friendbot endpoint for testnet (default: https://friendbot.stellar.org)
#   LOCALNET_FAUCET_URL     - Faucet endpoint for localnet (default: http://localhost:8000)
#
# Exit codes:
#   0 - All accounts funded successfully or already funded
#   1 - Invalid arguments or network error

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────

SOROBAN_NETWORK="${SOROBAN_NETWORK:-testnet}"
FRIENDBOT_URL="${FRIENDBOT_URL:-https://friendbot.stellar.org}"
LOCALNET_FAUCET_URL="${LOCALNET_FAUCET_URL:-http://localhost:8000}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# ─────────────────────────────────────────────────────────────────────────────
# Helper Functions
# ─────────────────────────────────────────────────────────────────────────────

log_info() {
  echo -e "${BLUE}ℹ️  ${NC}$*"
}

log_success() {
  echo -e "${GREEN}✓${NC} $*"
}

log_warning() {
  echo -e "${YELLOW}⚠️  ${NC}$*"
}

log_error() {
  echo -e "${RED}✗${NC} $*"
}

# Fund an account using Friendbot (testnet)
fund_with_friendbot() {
  local account="$1"

  log_info "Funding account on testnet: $account"

  # Make request to Friendbot
  local response=$(curl -s -X GET "${FRIENDBOT_URL}?addr=${account}")

  # Check for errors in response
  if echo "$response" | grep -q "\"status_code\": 400"; then
    # Likely already funded
    if echo "$response" | grep -q "already exists"; then
      log_warning "Account already funded (already exists in ledger): $account"
      return 0
    else
      log_error "Friendbot error for $account: $response"
      return 1
    fi
  fi

  if echo "$response" | grep -q "\"status_code\": 200\|\"type\": \"native\""; then
    log_success "Account funded successfully: $account"
    return 0
  fi

  # Check for hash in response (success indicator for newer Friendbot)
  if echo "$response" | grep -q "\"hash\""; then
    log_success "Account funded successfully: $account"
    return 0
  fi

  # If we got here, check for "ledger" field which indicates success
  if echo "$response" | grep -q "\"ledger\""; then
    log_success "Account funded successfully: $account"
    return 0
  fi

  log_error "Failed to fund account $account. Response: $response"
  return 1
}

# Fund an account using local faucet (localnet)
fund_with_localnet() {
  local account="$1"

  log_info "Funding account on localnet: $account"

  # Make request to local faucet
  # Most local faucets expect a POST or GET request like: /friendbot?addr=ACCOUNT
  local response=$(curl -s -X GET "${LOCALNET_FAUCET_URL}/friendbot?addr=${account}")

  # Check for success patterns
  if echo "$response" | grep -q "\"status_code\": 200\|\"type\": \"native\""; then
    log_success "Account funded successfully: $account"
    return 0
  fi

  # Check if account was already funded
  if echo "$response" | grep -q "already exists"; then
    log_warning "Account already funded: $account"
    return 0
  fi

  # Success might also be indicated by a transaction hash
  if echo "$response" | grep -q "\"hash\""; then
    log_success "Account funded successfully: $account"
    return 0
  fi

  if echo "$response" | grep -q "\"ledger\""; then
    log_success "Account funded successfully: $account"
    return 0
  fi

  # For some faucet implementations, a successful response might be empty
  # or minimal. Only fail if we get an explicit error.
  if echo "$response" | grep -q "error\|Error\|ERROR"; then
    log_error "Failed to fund account $account. Response: $response"
    return 1
  fi

  log_success "Account funding request sent: $account"
  return 0
}

# Validate a Stellar account address format
validate_account() {
  local account="$1"

  # Stellar public keys start with 'G' and are 56 characters
  if [[ $account =~ ^G[A-Z0-9]{55}$ ]]; then
    return 0
  fi

  log_error "Invalid Stellar account address format: $account"
  return 1
}

# ─────────────────────────────────────────────────────────────────────────────
# Main Script
# ─────────────────────────────────────────────────────────────────────────────

# Collect accounts from arguments or file
accounts=()

# Check for --from-file flag
if [[ $# -gt 0 && "$1" == "--from-file" ]]; then
  if [[ $# -lt 2 ]]; then
    log_error "Usage: --from-file <path-to-accounts-file>"
    exit 1
  fi

  config_file="$2"
  if [[ ! -f "$config_file" ]]; then
    log_error "Config file not found: $config_file"
    exit 1
  fi

  # Read accounts from file (one per line, skip empty lines and comments)
  while IFS= read -r line; do
    line=$(echo "$line" | xargs) # Trim whitespace
    if [[ -n "$line" && ! "$line" =~ ^# ]]; then
      accounts+=("$line")
    fi
  done <"$config_file"
else
  # Use command-line arguments as accounts
  accounts=("$@")
fi

# Validate we have at least one account
if [[ ${#accounts[@]} -eq 0 ]]; then
  log_error "No accounts specified"
  echo "Usage:"
  echo "  bash scripts/fund-accounts.sh GBXYZ... [GBXYZ...]"
  echo "  bash scripts/fund-accounts.sh --from-file accounts.txt"
  echo ""
  echo "Environment variables:"
  echo "  SOROBAN_NETWORK     - testnet or localnet (default: testnet)"
  echo "  FRIENDBOT_URL       - Friendbot endpoint (default: https://friendbot.stellar.org)"
  echo "  LOCALNET_FAUCET_URL - Local faucet endpoint (default: http://localhost:8000)"
  exit 1
fi

# Print configuration
log_info "Configuration:"
log_info "  Network: $SOROBAN_NETWORK"
if [[ "$SOROBAN_NETWORK" == "testnet" ]]; then
  log_info "  Friendbot URL: $FRIENDBOT_URL"
else
  log_info "  Faucet URL: $LOCALNET_FAUCET_URL"
fi
log_info "  Accounts to fund: ${#accounts[@]}"
echo ""

# Fund each account
failed_accounts=()
for account in "${accounts[@]}"; do
  # Validate account format
  if ! validate_account "$account"; then
    failed_accounts+=("$account")
    continue
  fi

  # Fund based on network
  if [[ "$SOROBAN_NETWORK" == "testnet" ]]; then
    if ! fund_with_friendbot "$account"; then
      failed_accounts+=("$account")
    fi
  elif [[ "$SOROBAN_NETWORK" == "localnet" ]]; then
    if ! fund_with_localnet "$account"; then
      failed_accounts+=("$account")
    fi
  else
    log_error "Unknown network: $SOROBAN_NETWORK (use testnet or localnet)"
    exit 1
  fi

  # Small delay between requests to avoid rate limiting
  sleep 0.5
done

# Summary
echo ""
if [[ ${#failed_accounts[@]} -eq 0 ]]; then
  log_success "All ${#accounts[@]} account(s) processed successfully!"
  exit 0
else
  log_error "Failed to fund ${#failed_accounts[@]} account(s):"
  for account in "${failed_accounts[@]}"; do
    echo "  - $account"
  done
  exit 1
fi
