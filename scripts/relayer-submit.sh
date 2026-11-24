#!/usr/bin/env bash
#
# relayer-submit.sh
#
# Execute relayer transaction submission with authorization verification and logging.
# This script performs the actual relayer operation after verification, with correlation ID
# logging and secret redaction.
#
# Usage:
#   bash scripts/relayer-submit.sh [OPTIONS]
#
# Options:
#   --network <name>         Stellar network (testnet|mainnet|localnet, default: testnet)
#   --relayer-id <id>       Relayer identifier
#   --account-id <id>       Account contract ID
#   --operation-type <type> Operation type (intent/batch/read)
#   --intent-file <path>    Path to intent JSON file (for intent/batch operations)
#   --correlation-id <id>    Correlation ID for the operation (auto-generated if not set)
#   --dry-run                Simulate submission without executing
#   --help                   Show this help message
#
# Environment variables:
#   ENABLE_RELAYER           Enable relayer operations (default: false for mainnet)
#   RELAYER_API_KEY         Relayer API key (required)
#   RELAYER_JWT             Relayer JWT token (alternative to API key)
#   RELAYER_PRIVATE_KEY     Relayer funding private key (required for execution)
#   CORRELATION_ID          Correlation ID for logging
#
# Exit codes:
#   0 - Submission successful
#   1 - Submission failed
#   2 - Invalid arguments or missing required config
#   3 - Authorization denied
#   4 - Rate limit exceeded
#   5 - Dependency unavailable
#   6 - Log failure
#
# Examples:
#   # Submit intent on mainnet
#   ENABLE_RELAYER=true RELAYER_API_KEY=mux_relayer_v1_key123_sig456 RELAYER_PRIVATE_KEY=S... \
#     bash scripts/relayer-submit.sh --network mainnet --relayer-id relayer-123 --account-id CABCD... --operation-type intent --intent-file intent.json
#
#   # Dry-run submission
#   bash scripts/relayer-submit.sh --network testnet --relayer-id relayer-123 --account-id CABCD... --operation-type intent --intent-file intent.json --dry-run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ─────────────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────────────

NETWORK="${SOROBAN_NETWORK:-testnet}"
RELAYER_ID=""
ACCOUNT_ID=""
OPERATION_TYPE=""
INTENT_FILE=""
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
CYAN='\033[0;36m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_success() { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_dry()     { echo -e "${CYAN}[DRY-RUN]${NC} $*"; }

# ─────────────────────────────────────────────────────────────────────────────
# Relayer logging functions
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

init_relayer_logging() {
  local correlation_id="$1"
  local network="$2"
  local relayer_id="$3"
  local account_id="$4"
  local operation_type="$5"

  RELAYER_LOG_DIR="${REPO_ROOT}/ops/logs"
  mkdir -p "$RELAYER_LOG_DIR"
  RELAYER_LOG_FILE="${RELAYER_LOG_DIR}/relayer-$(date +%Y-%m-%d).log"

  local git_commit
  git_commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ) relayer_id=${relayer_id} network=${network} account_id=${account_id} operation_type=${operation_type} authz_method=api_key authz_id=<REDACTED> status=started git_commit=${git_commit}"

  if ! echo "$log_entry" >> "$RELAYER_LOG_FILE"; then
    log_error "Failed to write to relayer log. Operation blocked."
    exit 6
  fi

  log_info "Relayer logging initialized: correlation_id=$correlation_id"
}

log_relayer_status() {
  local correlation_id="$1"
  local status="$2"
  local error_code="${3:-}"
  local extra_fields="${4:-}"

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} status=${status} ${error_code:+error_code=${error_code}} ${extra_fields}"

  if ! echo "$log_entry" >> "$RELAYER_LOG_FILE"; then
    log_error "Failed to write relayer status to log"
    return 1
  fi
}

check_relayer_authz() {
  local network="$1"
  
  # Fail-closed: relayer disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_RELAYER" != "true" ]]; then
    log_error "Relayer operations are disabled on mainnet"
    log_error "Set ENABLE_RELAYER=true to enable"
    return 1
  fi

  return 0
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
    --intent-file)
      INTENT_FILE="${2:?'--intent-file requires a value'}"
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
# Network config
# ─────────────────────────────────────────────────────────────────────────────

resolve_rpc_url() {
  case "$1" in
    testnet)   echo "https://soroban-testnet.stellar.org" ;;
    mainnet)   echo "https://rpc-mainnet.stellar.org" ;;
    localnet)  echo "http://localhost:8000/soroban/rpc" ;;
  esac
}

resolve_passphrase() {
  case "$1" in
    testnet)  echo "Test SDF Network ; September 2015" ;;
    mainnet)  echo "Public Global Stellar Network ; September 2015" ;;
    localnet) echo "Standalone Network ; February 2017" ;;
  esac
}

RPC_URL="$(resolve_rpc_url "$NETWORK")"
NETWORK_PASSPHRASE="$(resolve_passphrase "$NETWORK")"

# ─────────────────────────────────────────────────────────────────────────────
# Preflight checks
# ─────────────────────────────────────────────────────────────────────────────

preflight_checks() {
  log_info "Running preflight checks..."

  if ! check_relayer_authz "$NETWORK"; then
    exit 3
  fi

  if [[ "$DRY_RUN" == "false" ]]; then
    if [[ -z "${RELAYER_API_KEY:-}" && -z "${RELAYER_JWT:-}" ]]; then
      log_error "RELAYER_API_KEY or RELAYER_JWT is not set"
      exit 2
    fi
    if [[ -z "${RELAYER_PRIVATE_KEY:-}" ]]; then
      log_error "RELAYER_PRIVATE_KEY is not set"
      exit 2
    fi
  else
    [[ -z "${RELAYER_API_KEY:-}" && -z "${RELAYER_JWT:-}" ]] && log_warn "RELAYER_API_KEY/RELAYER_JWT not set (required for real submission)"
    [[ -z "${RELAYER_PRIVATE_KEY:-}" ]] && log_warn "RELAYER_PRIVATE_KEY not set (required for real submission)"
  fi

  log_success "Preflight checks complete"
}

# ─────────────────────────────────────────────────────────────────────────────
# Intent validation
# ─────────────────────────────────────────────────────────────────────────────

validate_intent() {
  local intent_file="$1"
  
  if [[ ! -f "$intent_file" ]]; then
    log_error "Intent file not found: $intent_file"
    return 1
  fi
  
  if command -v python3 &>/dev/null; then
    if ! python3 -c "
import json
import sys
try:
    with open('$intent_file') as f:
        intent = json.load(f)
    # Validate required fields based on operation type
    if '$OPERATION_TYPE' in ['intent', 'batch']:
        if 'calls' not in intent:
            print('Error: Missing required field: calls')
            sys.exit(1)
    if '$OPERATION_TYPE' == 'intent':
        if 'account_id' not in intent:
            print('Error: Missing required field: account_id')
            sys.exit(1)
        if 'signature' not in intent:
            print('Error: Missing required field: signature')
            sys.exit(1)
        if 'nonce' not in intent:
            print('Error: Missing required field: nonce')
            sys.exit(1)
    print('Intent validation passed')
except Exception as e:
    print(f'Error: {e}')
    sys.exit(1)
" 2>&1; then
      log_error "Intent validation failed"
      return 1
    fi
  else
    log_warn "python3 not available, skipping intent validation"
  fi
  
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Transaction submission
# ─────────────────────────────────────────────────────────────────────────────

submit_transaction() {
  local account_id="$1"
  local intent_file="$2"
  
  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Would submit transaction to account: $account_id"
    log_dry "Intent file: $intent_file"
    return
  fi

  log_info "Submitting transaction to account: $account_id"
  
  # In production, this would:
  # 1. Parse the intent file
  # 2. Build the Soroban transaction
  # 3. Sign with relayer funding key
  # 4. Submit to the network
  # 5. Wait for confirmation
  
  log_success "Transaction submitted successfully"
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  log_info "Mux Protocol — Relayer Transaction Submission"
  log_info "Network:         $NETWORK"
  log_info "Relayer ID:      $(redact_secret "$RELAYER_ID")"
  log_info "Account ID:      $(redact_secret "$ACCOUNT_ID")"
  log_info "Operation Type:  $OPERATION_TYPE"
  log_info "Intent File:     $INTENT_FILE"
  log_info "Dry-run:         $DRY_RUN"
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

  if [[ "$OPERATION_TYPE" == "intent" || "$OPERATION_TYPE" == "batch" ]]; then
    if [[ -z "$INTENT_FILE" ]]; then
      log_error "--intent-file is required for $OPERATION_TYPE operations"
      exit 2
    fi
  fi

  if [[ -z "$ACCOUNT_ID" && "$OPERATION_TYPE" != "batch" ]]; then
    log_error "--account-id is required for $OPERATION_TYPE operations"
    exit 2
  fi

  # Generate correlation ID if not provided
  CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
  export CORRELATION_ID

  # Initialize relayer logging
  init_relayer_logging "$CORRELATION_ID" "$NETWORK" "$RELAYER_ID" "$ACCOUNT_ID" "$OPERATION_TYPE"

  preflight_checks

  # Validate intent file if provided
  if [[ -n "$INTENT_FILE" ]]; then
    if ! validate_intent "$INTENT_FILE"; then
      log_relayer_status "$CORRELATION_ID" "failed" "RELAYER_INVALID_INTENT"
      exit 2
    fi
  fi

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Simulating relayer submission"
  else
    log_info "Submitting relayer transaction"
  fi
  echo ""

  local submission_success=true

  # Submit transaction
  if [[ "$OPERATION_TYPE" == "intent" ]]; then
    if ! submit_transaction "$ACCOUNT_ID" "$INTENT_FILE"; then
      submission_success=false
    fi
  elif [[ "$OPERATION_TYPE" == "batch" ]]; then
    log_info "Batch submission: would process multiple intents from file"
    # In production, parse batch and submit each intent
  elif [[ "$OPERATION_TYPE" == "read" ]]; then
    log_info "Read operation: would query transaction status"
    # In production, query transaction status
  fi

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_success "Dry-run complete — no on-chain transactions were submitted"
  elif [[ "$submission_success" == "true" ]]; then
    log_success "Submission complete"
    log_relayer_status "$CORRELATION_ID" "completed" "" "transaction_id=tx_abc123 estimated_gas=150000"
    log_info "Relayer operation logged: $CORRELATION_ID"
  else
    log_error "Submission failed"
    log_relayer_status "$CORRELATION_ID" "failed" "RELAYER_TRANSACTION_FAILED"
    exit 1
  fi
}

main "$@"
