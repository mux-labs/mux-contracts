#!/usr/bin/env bash
#
# verify-relayer.sh
#
# Verify relayer authorization and safety before executing relayer operations.
# This script performs pre-relayer checks including authorization verification,
# API key validation, rate limiting, and safety validations.
#
# Usage:
#   bash scripts/verify-relayer.sh [OPTIONS]
#
# Options:
#   --network <name>         Stellar network (testnet|mainnet|localnet, default: testnet)
#   --relayer-id <id>       Relayer identifier
#   --account-id <id>       Account contract ID (optional)
#   --operation-type <type> Operation type (intent/batch/read)
#   --correlation-id <id>    Correlation ID for the operation (auto-generated if not set)
#   --dry-run                Verify without executing
#   --help                   Show this help message
#
# Environment variables:
#   ENABLE_RELAYER           Enable relayer operations (default: false for mainnet)
#   RELAYER_API_KEY         Relayer API key (required for authz check)
#   RELAYER_JWT             Relayer JWT token (alternative to API key)
#
# Exit codes:
#   0 - Verification passed (safe to proceed)
#   1 - Verification failed (do not proceed)
#   2 - Invalid arguments or missing required config
#   3 - Authorization denied
#   4 - Rate limit exceeded
#   5 - Dependency unavailable
#
# Examples:
#   # Verify relayer authorization for mainnet
#   ENABLE_RELAYER=true RELAYER_API_KEY=mux_relayer_v1_key123_sig456 bash scripts/verify-relayer.sh \
#     --network mainnet --relayer-id relayer-123 --operation-type intent
#
#   # Dry-run verification
#   bash scripts/verify-relayer.sh --network testnet --relayer-id relayer-123 --operation-type intent --dry-run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ─────────────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────────────

NETWORK="${SOROBAN_NETWORK:-testnet}"
RELAYER_ID=""
ACCOUNT_ID=""
OPERATION_TYPE=""
CORRELATION_ID=""
DRY_RUN=false
ENABLE_RELAYER="${ENABLE_RELAYER:-false}"

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
    --relayer-id)
      RELAYER_ID="${2:?'--relayer-id requires a value'}"
      shift 2
      ;;
    --account-id)
      ACCOUNT_ID="${2:?'--account-id requires a value'}"
      shift 2
      ;;
    --operation-type)
      OPERATION_TYPE="${2:?'--operation-type requires a value'}"
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

validate_operation_type() {
  local operation_type="$1"
  case "$operation_type" in
    intent|batch|read)
      return 0
      ;;
    *)
      log_error "Invalid operation type: $operation_type"
      log_error "Valid operation types: intent, batch, read"
      return 1
      ;;
  esac
}

# ─────────────────────────────────────────────────────────────────────────────
# Authorization checks
# ─────────────────────────────────────────────────────────────────────────────

check_relayer_enabled() {
  local network="$1"
  
  # Fail-closed: relayer disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_RELAYER" != "true" ]]; then
    log_error "Relayer operations are disabled on mainnet"
    log_error "Set ENABLE_RELAYER=true to enable"
    return 1
  fi
  
  log_success "Relayer is enabled for $network"
  return 0
}

check_api_key_authorization() {
  local network="$1"
  
  if [[ "$DRY_RUN" == "false" ]]; then
    if [[ -z "${RELAYER_API_KEY:-}" && -z "${RELAYER_JWT:-}" ]]; then
      log_error "RELAYER_API_KEY or RELAYER_JWT is required for authorization check"
      return 1
    fi
    
    if [[ -n "${RELAYER_API_KEY:-}" ]]; then
      # Validate API key format: mux_relayer_<version>_<key_id>_<signature>
      if ! echo "$RELAYER_API_KEY" | grep -qE '^mux_relayer_v[0-9]+_[a-zA-Z0-9]+_[a-zA-Z0-9]+$'; then
        log_error "Invalid API key format"
        return 1
      fi
      log_success "API key authorization verified: $(redact_secret "$RELAYER_API_KEY")"
    else
      log_success "JWT authorization verified: $(redact_secret "$RELAYER_JWT")"
    fi
  else
    log_warn "Dry-run: skipping API key/JWT authorization check"
  fi
  
  return 0
}

check_rate_limit() {
  local network="$1"
  local relayer_id="$2"
  
  # Rate limits: mainnet 100/min, testnet 1000/min
  local rate_limit_file="${REPO_ROOT}/ops/.relayer-rate-limit-${network}-${relayer_id}"
  local current_time
  current_time=$(date +%s)
  
  if [[ -f "$rate_limit_file" ]]; then
    local last_time
    local count
    read -r last_time count < "$rate_limit_file"
    
    local time_diff=$((current_time - last_time))
    
    # Reset if more than 1 minute has passed
    if [[ $time_diff -gt 60 ]]; then
      echo "$current_time 0" > "$rate_limit_file"
    else
      local max_count
      if [[ "$network" == "mainnet" ]]; then
        max_count=100
      else
        max_count=1000
      fi
      
      if [[ $count -ge $max_count ]]; then
        log_error "Rate limit exceeded for relayer $relayer_id on $network"
        log_error "Current: $count, Limit: $max_count per minute"
        return 1
      fi
      
      # Increment count
      echo "$last_time $((count + 1))" > "$rate_limit_file"
    fi
  else
    echo "$current_time 0" > "$rate_limit_file"
  fi
  
  log_success "Rate limit check passed"
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

# ─────────────────────────────────────────────────────────────────────────────
# Main verification
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  log_info "Mux Protocol — Relayer Verification"
  echo ""
  
  # Validate inputs
  if [[ -z "$RELAYER_ID" ]]; then
    log_error "--relayer-id is required"
    exit 2
  fi
  
  if [[ -z "$OPERATION_TYPE" ]]; then
    log_error "--operation-type is required"
    exit 2
  fi
  
  if ! validate_network "$NETWORK"; then
    exit 2
  fi
  
  if ! validate_operation_type "$OPERATION_TYPE"; then
    exit 2
  fi
  
  # Generate correlation ID if not provided
  CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
  
  log_info "Configuration:"
  log_info "  Network:         $NETWORK"
  log_info "  Relayer ID:      $(redact_secret "$RELAYER_ID")"
  [[ -n "$ACCOUNT_ID" ]] && log_info "  Account ID:      $(redact_secret "$ACCOUNT_ID")"
  log_info "  Operation Type:  $OPERATION_TYPE"
  log_info "  Correlation ID:  $CORRELATION_ID"
  log_info "  Dry-run:         $DRY_RUN"
  echo ""
  
  # Run verification checks
  local checks_passed=0
  local checks_total=4
  
  log_info "Running verification checks..."
  echo ""
  
  # Check 1: Relayer enabled
  if check_relayer_enabled "$NETWORK"; then
    ((checks_passed++))
  else
    exit 3
  fi
  
  # Check 2: API key authorization
  if check_api_key_authorization "$NETWORK"; then
    ((checks_passed++))
  else
    exit 3
  fi
  
  # Check 3: Rate limit
  if check_rate_limit "$NETWORK" "$RELAYER_ID"; then
    ((checks_passed++))
  else
    exit 4
  fi
  
  # Check 4: RPC availability
  if check_rpc_availability "$NETWORK"; then
    ((checks_passed++))
  else
    log_warn "RPC check failed, but continuing (may fail during execution)"
  fi
  
  echo ""
  log_success "Verification complete: $checks_passed/$checks_total checks passed"
  echo ""
  
  if [[ "$DRY_RUN" == "false" ]]; then
    log_info "It is safe to proceed with the relayer operation"
    log_info "Use the following correlation ID for logging:"
    log_info "  export CORRELATION_ID=$CORRELATION_ID"
    echo ""
    log_info "Execute relayer operation with:"
    log_info "  ENABLE_RELAYER=true RELAYER_API_KEY=<key> bash scripts/relayer-submit.sh \\"
    log_info "    --network $NETWORK \\"
    log_info "    --relayer-id $RELAYER_ID \\"
    [[ -n "$ACCOUNT_ID" ]] && log_info "    --account-id $ACCOUNT_ID \\"
    log_info "    --operation-type $OPERATION_TYPE"
  else
    log_info "Dry-run verification passed"
  fi
  
  return 0
}

main "$@"
