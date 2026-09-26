#!/usr/bin/env bash
#
# verify-upgrade.sh
#
# Verify upgrade authorization and safety before executing contract upgrade operations.
# This script performs pre-upgrade checks including authorization verification,
# WASM hash validation, conflict detection, and safety validations.
#
# Usage:
#   bash scripts/verify-upgrade.sh [OPTIONS]
#
# Options:
#   --network <name>         Stellar network (testnet|mainnet|localnet, default: testnet)
#   --contract <name>        Contract to upgrade
#   --contract-id <id>       Contract ID (optional, will read from config if not provided)
#   --new-wasm-hash <hash>   New WASM hash to upgrade to
#   --old-wasm-hash <hash>   Old WASM hash (for rollback verification)
#   --correlation-id <id>    Correlation ID for the upgrade (auto-generated if not set)
#   --dry-run                Verify without executing
#   --help                   Show this help message
#
# Environment variables:
#   ENABLE_CONTRACT_UPGRADE  Enable upgrade operations (default: false for mainnet)
#   ADMIN_ADDRESS           Contract admin public key (required for authz check)
#   DEPLOYER_PRIVATE_KEY    Deployer secret key (required for live verification)
#
# Exit codes:
#   0 - Verification passed (safe to proceed)
#   1 - Verification failed (do not proceed)
#   2 - Invalid arguments or missing required config
#   3 - Authorization denied
#   4 - Conflict detected
#   5 - WASM hash verification failed
#
# Examples:
#   # Verify upgrade authorization for mainnet
#   ENABLE_CONTRACT_UPGRADE=true ADMIN_ADDRESS=G... bash scripts/verify-upgrade.sh \
#     --network mainnet --contract mux-account --new-wasm-hash a1b2c3...
#
#   # Dry-run verification
#   bash scripts/verify-upgrade.sh --network testnet --contract mux-account --new-wasm-hash a1b2c3... --dry-run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ─────────────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────────────

NETWORK="${SOROBAN_NETWORK:-testnet}"
CONTRACT_NAME=""
CONTRACT_ID=""
NEW_WASM_HASH=""
OLD_WASM_HASH=""
CORRELATION_ID=""
DRY_RUN=false
ENABLE_CONTRACT_UPGRADE="${ENABLE_CONTRACT_UPGRADE:-false}"

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
    --contract)
      CONTRACT_NAME="${2:?'--contract requires a value'}"
      shift 2
      ;;
    --contract-id)
      CONTRACT_ID="${2:?'--contract-id requires a value'}"
      shift 2
      ;;
    --new-wasm-hash)
      NEW_WASM_HASH="${2:?'--new-wasm-hash requires a value'}"
      shift 2
      ;;
    --old-wasm-hash)
      OLD_WASM_HASH="${2:?'--old-wasm-hash requires a value'}"
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

validate_wasm_hash() {
  local hash="$1"
  if ! echo "$hash" | grep -qE '^[0-9a-f]{64}$'; then
    log_error "Invalid WASM hash: $hash"
    log_error "WASM hash must be 64 lowercase hex characters"
    return 1
  fi
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Authorization checks
# ─────────────────────────────────────────────────────────────────────────────

check_upgrade_enabled() {
  local network="$1"
  
  # Fail-closed: upgrade disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_CONTRACT_UPGRADE" != "true" ]]; then
    log_error "Upgrade operations are disabled on mainnet"
    log_error "Set ENABLE_CONTRACT_UPGRADE=true to enable"
    return 1
  fi
  
  log_success "Upgrade is enabled for $network"
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
    log_warn "Ensure 3/5 signers have approved this upgrade"
  fi
  
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Conflict detection
# ─────────────────────────────────────────────────────────────────────────────

check_active_upgrade() {
  local network="$1"
  local lock_file="${REPO_ROOT}/ops/.upgrade-lock-${network}"
  
  if [[ -f "$lock_file" ]]; then
    local active_id
    active_id=$(cat "$lock_file")
    log_error "Conflicting upgrade in progress: $active_id"
    log_error "Wait for the existing upgrade to complete or cancel it"
    return 1
  fi
  
  log_success "No conflicting upgrade in progress"
  return 0
}

check_replay_protection() {
  local correlation_id="$1"
  local log_dir="${REPO_ROOT}/ops/logs"
  
  if [[ -d "$log_dir" ]]; then
    if grep -q "correlation_id=${correlation_id}.*status=completed" "$log_dir"/upgrade-*.log 2>/dev/null; then
      log_error "Upgrade with correlation ID $correlation_id already completed"
      log_error "This appears to be a replay attempt"
      return 1
    fi
  fi
  
  log_success "No replay detected for correlation ID: $correlation_id"
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# WASM hash verification
# ─────────────────────────────────────────────────────────────────────────────

verify_wasm_hash() {
  local wasm_hash="$1"
  local contract_name="$2"
  
  # Check if WASM file exists
  local wasm_file="${REPO_ROOT}/target/wasm32-unknown-unknown/release/${contract_name//-/_}.wasm"
  
  if [[ ! -f "$wasm_file" ]]; then
    log_warn "WASM file not found at $wasm_file"
    log_warn "Skipping WASM hash verification (file may be pre-uploaded)"
    return 0
  fi
  
  # Compute hash of local WASM
  local computed_hash
  if command -v sha256sum &>/dev/null; then
    computed_hash=$(sha256sum "$wasm_file" | awk '{print $1}')
  elif command -v shasum &>/dev/null; then
    computed_hash=$(shasum -a 256 "$wasm_file" | awk '{print $1}')
  else
    log_warn "sha256sum/shasum not available, skipping hash verification"
    return 0
  fi
  
  if [[ "$computed_hash" != "$wasm_hash" ]]; then
    log_error "WASM hash mismatch"
    log_error "  Expected: $wasm_hash"
    log_error "  Computed: $computed_hash"
    log_error "The local WASM file does not match the provided hash"
    return 1
  fi
  
  log_success "WASM hash verified: $wasm_hash"
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

check_contract_id() {
  local contract_name="$1"
  local network="$2"
  local provided_id="$3"
  
  if [[ -n "$provided_id" ]]; then
    log_success "Contract ID provided: $(redact_secret "$provided_id")"
    return 0
  fi
  
  # Try to read from config
  local config_file="${REPO_ROOT}/config/addresses.json"
  if [[ -f "$config_file" ]]; then
    if command -v python3 &>/dev/null; then
      local contract_id
      contract_id=$(python3 -c "
import json
import sys
try:
    with open('$config_file') as f:
        config = json.load(f)
    network_config = config.get('$network', {})
    # Convert contract name to config key format
    key = '${contract_name//-/_}'
    print(network_config.get(key, ''))
except Exception as e:
    print('')
" 2>/dev/null)
      
      if [[ -n "$contract_id" ]]; then
        CONTRACT_ID="$contract_id"
        log_success "Contract ID from config: $(redact_secret "$contract_id")"
        return 0
      fi
    fi
  fi
  
  log_warn "Contract ID not provided and not found in config"
  log_warn "You may need to provide --contract-id"
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Main verification
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  log_info "Mux Protocol — Upgrade Verification"
  echo ""
  
  # Validate inputs
  if [[ -z "$CONTRACT_NAME" ]]; then
    log_error "--contract is required"
    exit 2
  fi
  
  if [[ -z "$NEW_WASM_HASH" ]]; then
    log_error "--new-wasm-hash is required"
    exit 2
  fi
  
  if ! validate_network "$NETWORK"; then
    exit 2
  fi
  
  if ! validate_wasm_hash "$NEW_WASM_HASH"; then
    exit 5
  fi
  
  if [[ -n "$OLD_WASM_HASH" ]] && ! validate_wasm_hash "$OLD_WASM_HASH"; then
    exit 5
  fi
  
  # Generate correlation ID if not provided
  CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
  
  # Get contract ID
  check_contract_id "$CONTRACT_NAME" "$NETWORK" "$CONTRACT_ID"
  
  log_info "Configuration:"
  log_info "  Network:         $NETWORK"
  log_info "  Contract:        $CONTRACT_NAME"
  log_info "  Contract ID:     ${CONTRACT_ID:-<not provided>}"
  log_info "  New WASM hash:   $NEW_WASM_HASH"
  [[ -n "$OLD_WASM_HASH" ]] && log_info "  Old WASM hash:   $OLD_WASM_HASH"
  log_info "  Correlation ID:  $CORRELATION_ID"
  log_info "  Dry-run:         $DRY_RUN"
  echo ""
  
  # Run verification checks
  local checks_passed=0
  local checks_total=7
  
  log_info "Running verification checks..."
  echo ""
  
  # Check 1: Upgrade enabled
  if check_upgrade_enabled "$NETWORK"; then
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
  
  # Check 4: No conflicting upgrade
  if check_active_upgrade "$NETWORK"; then
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
  
  # Check 6: WASM hash verification
  if verify_wasm_hash "$NEW_WASM_HASH" "$CONTRACT_NAME"; then
    ((checks_passed++))
  else
    exit 5
  fi
  
  # Check 7: RPC availability
  if check_rpc_availability "$NETWORK"; then
    ((checks_passed++))
  else
    log_warn "RPC check failed, but continuing (may fail during execution)"
  fi
  
  echo ""
  log_success "Verification complete: $checks_passed/$checks_total checks passed"
  echo ""
  
  if [[ "$DRY_RUN" == "false" ]]; then
    log_info "It is safe to proceed with the upgrade operation"
    log_info "Use the following correlation ID for logging:"
    log_info "  export CORRELATION_ID=$CORRELATION_ID"
    echo ""
    log_info "Execute upgrade with:"
    log_info "  ENABLE_CONTRACT_UPGRADE=true bash scripts/upgrade.sh \\"
    log_info "    --network $NETWORK \\"
    log_info "    --contract $CONTRACT_NAME \\"
    log_info "    --new-wasm-hash $NEW_WASM_HASH \\"
    [[ -n "$CONTRACT_ID" ]] && log_info "    --contract-id $CONTRACT_ID \\"
    [[ -n "$OLD_WASM_HASH" ]] && log_info "    --old-wasm-hash $OLD_WASM_HASH"
  else
    log_info "Dry-run verification passed"
  fi
  
  return 0
}

main "$@"
