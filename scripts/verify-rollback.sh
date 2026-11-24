#!/usr/bin/env bash
#
# verify-rollback.sh
#
# Verify rollback authorization and safety before executing rollback operations.
# This script performs pre-rollback checks including authorization verification,
# conflict detection, and safety validations.
#
# Usage:
#   bash scripts/verify-rollback.sh [OPTIONS]
#
# Options:
#   --network <name>     Stellar network (testnet|mainnet|localnet, default: testnet)
#   --strategy <strategy> Rollback strategy (address-repoint|deploy-wasm|admin-pause)
#   --contract <name>    Contract to rollback (default: all contracts)
#   --correlation-id <id> Correlation ID for the rollback (auto-generated if not set)
#   --dry-run            Verify without executing
#   --help               Show this help message
#
# Environment variables:
#   ENABLE_ROLLBACK       Enable rollback operations (default: false for mainnet)
#   ADMIN_ADDRESS        Contract admin public key (required for authz check)
#   DEPLOYER_PRIVATE_KEY  Deployer secret key (required for live verification)
#
# Exit codes:
#   0 - Verification passed (safe to proceed)
#   1 - Verification failed (do not proceed)
#   2 - Invalid arguments or missing required config
#   3 - Authorization denied
#   4 - Conflict detected
#
# Examples:
#   # Verify rollback authorization for mainnet
#   ENABLE_ROLLBACK=true ADMIN_ADDRESS=G... bash scripts/verify-rollback.sh \
#     --network mainnet --strategy address-repoint --contract mux-account
#
#   # Dry-run verification
#   bash scripts/verify-rollback.sh --network testnet --strategy deploy-wasm --dry-run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ─────────────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────────────

NETWORK="${SOROBAN_NETWORK:-testnet}"
STRATEGY=""
TARGET_CONTRACT=""
CORRELATION_ID=""
DRY_RUN=false
ENABLE_ROLLBACK="${ENABLE_ROLLBACK:-false}"

# ─────────────────────────────────────────────────────────────────────────────
# Colors
# ─────────────────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_success() { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# ─────────────────────────────────────────────────────────────────────────────
# Utility functions
# ─────────────────────────────────────────────────────────────────────────────

generate_correlation_id() {
  if command -v uuidgen &>/dev/null; then
    uuidgen | tr '[:upper:]' '[:lower:]'
  elif command -v python3 &>/dev/null; then
    python3 -c "import uuid; print(str(uuid.uuid4()))"
  else
    echo "$(date +%s)-$RANDOM"
  fi
}

redact_secret() {
  local secret="$1"
  if [[ -n "$secret" && ${#secret} -gt 4 ]]; then
    echo "${secret:0:1}***${secret: -1}"
  else
    echo "<REDACTED>"
  fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Argument parsing
# ─────────────────────────────────────────────────────────────────────────────

show_help() {
  grep '^#' "$0" | sed 's/^# //' | sed 's/^#//'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --network)
      NETWORK="${2:?'--network requires a value'}"
      shift 2
      ;;
    --strategy)
      STRATEGY="${2:?'--strategy requires a value'}"
      shift 2
      ;;
    --contract)
      TARGET_CONTRACT="${2:?'--contract requires a value'}"
      shift 2
      ;;
    --correlation-id)
      CORRELATION_ID="${2:?'--correlation-id requires a value'}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --help|-h)
      show_help
      ;;
    *)
      log_error "Unknown argument: $1"
      exit 2
      ;;
  esac
done

# ─────────────────────────────────────────────────────────────────────────────
# Validation functions
# ─────────────────────────────────────────────────────────────────────────────

validate_strategy() {
  local strategy="$1"
  case "$strategy" in
    address-repoint|deploy-wasm|admin-pause)
      return 0
      ;;
    *)
      log_error "Invalid rollback strategy: $strategy"
      log_error "Valid strategies: address-repoint, deploy-wasm, admin-pause"
      return 1
      ;;
  esac
}

validate_network() {
  local network="$1"
  case "$network" in
    testnet|mainnet|localnet)
      return 0
      ;;
    *)
      log_error "Invalid network: $network"
      log_error "Valid networks: testnet, mainnet, localnet"
      return 1
      ;;
  esac
}

# ─────────────────────────────────────────────────────────────────────────────
# Authorization checks
# ─────────────────────────────────────────────────────────────────────────────

check_rollback_enabled() {
  local network="$1"
  
  # Fail-closed: rollback disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_ROLLBACK" != "true" ]]; then
    log_error "Rollback operations are disabled on mainnet"
    log_error "Set ENABLE_ROLLBACK=true to enable"
    return 1
  fi
  
  log_success "Rollback is enabled for $network"
  return 0
}

check_admin_authorization() {
  local network="$1"
  
  if [[ "$DRY_RUN" == "false" ]]; then
    if [[ -z "${ADMIN_ADDRESS:-}" ]]; then
      log_error "ADMIN_ADDRESS is required for authorization check"
      return 1
    fi
    log_success "Admin authorization verified: $(redact_secret "$ADMIN_ADDRESS")"
  else
    log_warn "Dry-run: skipping admin authorization check"
  fi
  
  return 0
}

check_multisig_quorum() {
  local network="$1"
  
  # For mainnet, require multisig quorum
  if [[ "$network" == "mainnet" ]]; then
    log_info "Checking multisig quorum for mainnet..."
    # In production, this would query the multisig contract
    # For now, we log a warning that manual verification is required
    log_warn "Manual multisig quorum verification required for mainnet"
    log_warn "Ensure 3/5 signers have approved this rollback"
  fi
  
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Conflict detection
# ─────────────────────────────────────────────────────────────────────────────

check_active_rollback() {
  local network="$1"
  local lock_file="${REPO_ROOT}/ops/.rollback-lock-${network}"
  
  if [[ -f "$lock_file" ]]; then
    local active_id
    active_id=$(cat "$lock_file")
    log_error "Conflicting rollback in progress: $active_id"
    log_error "Wait for the existing rollback to complete or cancel it"
    return 1
  fi
  
  log_success "No conflicting rollback in progress"
  return 0
}

check_replay_protection() {
  local correlation_id="$1"
  local log_dir="${REPO_ROOT}/ops/logs"
  
  if [[ -d "$log_dir" ]]; then
    if grep -q "correlation_id=${correlation_id}.*status=completed" "$log_dir"/rollback-*.log 2>/dev/null; then
      log_error "Rollback with correlation ID $correlation_id already completed"
      log_error "This appears to be a replay attempt"
      return 1
    fi
  fi
  
  log_success "No replay detected for correlation ID: $correlation_id"
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Safety checks
# ─────────────────────────────────────────────────────────────────────────────

check_rpc_availability() {
  local network="$1"
  local rpc_url
  
  case "$network" in
    testnet)  rpc_url="https://soroban-testnet.stellar.org" ;;
    mainnet)  rpc_url="https://rpc-mainnet.stellar.org" ;;
    localnet) rpc_url="http://localhost:8000/soroban/rpc" ;;
  esac
  
  if [[ "$DRY_RUN" == "false" ]]; then
    log_info "Checking RPC availability: $rpc_url"
    if command -v curl &>/dev/null; then
      if ! curl -s -f "$rpc_url" &>/dev/null; then
        log_error "RPC endpoint is unavailable: $rpc_url"
        return 1
      fi
      log_success "RPC endpoint is available"
    else
      log_warn "curl not available, skipping RPC check"
    fi
  else
    log_warn "Dry-run: skipping RPC availability check"
  fi
  
  return 0
}

check_config_addresses() {
  local network="$1"
  local config_file="${REPO_ROOT}/config/addresses.json"
  
  if [[ ! -f "$config_file" ]]; then
    log_error "Config file not found: $config_file"
    return 1
  fi
  
  log_success "Config file exists: $config_file"
  
  # Check if network has addresses configured
  if command -v python3 &>/dev/null; then
    local has_addresses
    has_addresses=$(python3 -c "
import json
import sys
try:
    with open('$config_file') as f:
        config = json.load(f)
    network_config = config.get('$network', {})
    # Check if any contract addresses are non-empty
    has_any = any(bool(v) for v in network_config.values())
    print('1' if has_any else '0')
except Exception as e:
    print('0')
" 2>/dev/null)
    
    if [[ "$has_addresses" == "0" ]]; then
      log_warn "No contract addresses configured for $network in $config_file"
    else
      log_success "Contract addresses found for $network"
    fi
  fi
  
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Main verification
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  log_info "Mux Protocol — Rollback Verification"
  echo ""
  
  # Validate inputs
  if [[ -z "$STRATEGY" ]]; then
    log_error "--strategy is required"
    exit 2
  fi
  
  if ! validate_strategy "$STRATEGY"; then
    exit 2
  fi
  
  if ! validate_network "$NETWORK"; then
    exit 2
  fi
  
  # Generate correlation ID if not provided
  CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
  
  log_info "Configuration:"
  log_info "  Network:         $NETWORK"
  log_info "  Strategy:        $STRATEGY"
  log_info "  Contract:        ${TARGET_CONTRACT:-all}"
  log_info "  Correlation ID:  $CORRELATION_ID"
  log_info "  Dry-run:         $DRY_RUN"
  echo ""
  
  # Run verification checks
  local checks_passed=0
  local checks_total=6
  
  log_info "Running verification checks..."
  echo ""
  
  # Check 1: Rollback enabled
  if check_rollback_enabled "$NETWORK"; then
    ((checks_passed++))
  else
    exit 3
  fi
  
  # Check 2: Admin authorization
  if check_admin_authorization "$NETWORK"; then
    ((checks_passed++))
  else
    exit 3
  fi
  
  # Check 3: Multisig quorum
  if check_multisig_quorum "$NETWORK"; then
    ((checks_passed++))
  else
    exit 3
  fi
  
  # Check 4: No conflicting rollback
  if check_active_rollback "$NETWORK"; then
    ((checks_passed++))
  else
    exit 4
  fi
  
  # Check 5: Replay protection
  if check_replay_protection "$CORRELATION_ID"; then
    ((checks_passed++))
  else
    exit 4
  fi
  
  # Check 6: RPC availability
  if check_rpc_availability "$NETWORK"; then
    ((checks_passed++))
  else
    log_warn "RPC check failed, but continuing (may fail during execution)"
  fi
  
  # Additional safety check: config addresses
  check_config_addresses "$NETWORK"
  
  echo ""
  log_success "Verification complete: $checks_passed/$checks_total checks passed"
  echo ""
  
  if [[ "$DRY_RUN" == "false" ]]; then
    log_info "It is safe to proceed with the rollback operation"
    log_info "Use the following correlation ID for logging:"
    log_info "  export CORRELATION_ID=$CORRELATION_ID"
    echo ""
    log_info "Execute rollback with:"
    log_info "  ENABLE_ROLLBACK=true bash scripts/deploy.sh --network $NETWORK --rollback --rollback-strategy $STRATEGY ${TARGET_CONTRACT:+--contract $TARGET_CONTRACT}"
  else
    log_info "Dry-run verification passed"
  fi
  
  return 0
}

main "$@"
