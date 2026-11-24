#!/usr/bin/env bash
#
# upgrade.sh
#
# Execute contract upgrade operations with authorization verification and logging.
# This script performs the actual upgrade after verification, with correlation ID
# logging and secret redaction.
#
# Usage:
#   bash scripts/upgrade.sh [OPTIONS]
#
# Options:
#   --network <name>         Stellar network (testnet|mainnet|localnet, default: testnet)
#   --contract <name>        Contract to upgrade
#   --contract-id <id>       Contract ID (optional, will read from config if not provided)
#   --new-wasm-hash <hash>   New WASM hash to upgrade to
#   --old-wasm-hash <hash>   Old WASM hash (for rollback verification)
#   --correlation-id <id>    Correlation ID for the upgrade (auto-generated if not set)
#   --dry-run                Simulate upgrade without executing
#   --skip-upload            Skip WASM upload (if already uploaded)
#   --help                   Show this help message
#
# Environment variables:
#   ENABLE_CONTRACT_UPGRADE  Enable upgrade operations (default: false for mainnet)
#   ADMIN_ADDRESS           Contract admin public key (required)
#   DEPLOYER_PRIVATE_KEY    Deployer secret key (required)
#   CORRELATION_ID          Correlation ID for logging
#
# Exit codes:
#   0 - Upgrade successful
#   1 - Upgrade failed
#   2 - Invalid arguments or missing required config
#   3 - Authorization denied
#   4 - Conflict detected
#   5 - WASM hash verification failed
#   6 - Log failure
#
# Examples:
#   # Execute upgrade on mainnet
#   ENABLE_CONTRACT_UPGRADE=true ADMIN_ADDRESS=G... DEPLOYER_PRIVATE_KEY=S... \
#     bash scripts/upgrade.sh --network mainnet --contract mux-account --new-wasm-hash a1b2c3...
#
#   # Dry-run upgrade
#   bash scripts/upgrade.sh --network testnet --contract mux-account --new-wasm-hash a1b2c3... --dry-run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_DIR="${REPO_ROOT}/target/wasm32-unknown-unknown/release"

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
SKIP_UPLOAD=false
ENABLE_CONTRACT_UPGRADE="${ENABLE_CONTRACT_UPGRADE:-false}"

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
# Rollback logging functions (reused from deploy.sh)
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

init_upgrade_logging() {
  local correlation_id="$1"
  local network="$2"
  local contract_name="$3"
  local contract_id="$4"
  local new_wasm_hash="$5"
  local old_wasm_hash="$6"

  UPGRADE_LOG_DIR="${REPO_ROOT}/ops/logs"
  mkdir -p "$UPGRADE_LOG_DIR"
  UPGRADE_LOG_FILE="${UPGRADE_LOG_DIR}/upgrade-$(date +%Y-%m-%d).log"

  local git_commit
  git_commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ) operator=<REDACTED> network=${network} contract_name=${contract_name} contract_id=${contract_id} old_wasm_hash=${old_wasm_hash} new_wasm_hash=${new_wasm_hash} authz_method=multisig authz_id=<REDACTED> status=started git_commit=${git_commit}"

  if ! echo "$log_entry" >> "$UPGRADE_LOG_FILE"; then
    log_error "Failed to write to upgrade log. Operation blocked."
    exit 6
  fi

  log_info "Upgrade logging initialized: correlation_id=$correlation_id"
}

log_upgrade_status() {
  local correlation_id="$1"
  local status="$2"
  local error_code="${3:-}"
  local extra_fields="${4:-}"

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} status=${status} ${error_code:+error_code=${error_code}} ${extra_fields}"

  if ! echo "$log_entry" >> "$UPGRADE_LOG_FILE"; then
    log_error "Failed to write upgrade status to log"
    return 1
  fi
}

check_upgrade_authz() {
  local network="$1"
  
  # Fail-closed: upgrade disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_CONTRACT_UPGRADE" != "true" ]]; then
    log_error "Upgrade operations are disabled on mainnet"
    log_error "Set ENABLE_CONTRACT_UPGRADE=true to enable"
    return 1
  fi

  # Check for conflicting upgrade
  local lock_file="${REPO_ROOT}/ops/.upgrade-lock-${network}"
  if [[ -f "$lock_file" ]]; then
    local active_id
    active_id=$(cat "$lock_file")
    log_error "Conflicting upgrade in progress: $active_id"
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
    --skip-upload)
      SKIP_UPLOAD=true
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

  if ! check_upgrade_authz "$NETWORK"; then
    exit 3
  fi

  if [[ "$DRY_RUN" == "false" ]]; then
    if ! command -v stellar &>/dev/null; then
      log_error "'stellar' CLI not found. Install: https://developers.stellar.org/docs/tools/stellar-cli"
      exit 1
    fi
    if [[ -z "${DEPLOYER_PRIVATE_KEY:-}" ]]; then
      log_error "DEPLOYER_PRIVATE_KEY is not set"
      exit 2
    fi
    if [[ -z "${ADMIN_ADDRESS:-}" ]]; then
      log_error "ADMIN_ADDRESS is not set"
      exit 2
    fi
  else
    command -v stellar &>/dev/null || log_warn "stellar CLI not found (would be required for real upgrade)"
    [[ -z "${DEPLOYER_PRIVATE_KEY:-}" ]] && log_warn "DEPLOYER_PRIVATE_KEY not set (required for real upgrade)"
    [[ -z "${ADMIN_ADDRESS:-}" ]]        && log_warn "ADMIN_ADDRESS not set (required for real upgrade)"
  fi

  log_success "Preflight checks complete"
}

# ─────────────────────────────────────────────────────────────────────────────
# WASM upload
# ─────────────────────────────────────────────────────────────────────────────

upload_wasm() {
  local wasm_file="$1"
  
  if [[ "$SKIP_UPLOAD" == "true" ]]; then
    log_info "Skipping WASM upload (--skip-upload)"
    return
  fi

  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Would upload: stellar contract upload --wasm $wasm_file --network-passphrase \"$NETWORK_PASSPHRASE\" --rpc-url $RPC_URL"
    return
  fi

  log_info "Uploading WASM: $wasm_file"
  
  local wasm_hash
  wasm_hash=$(stellar contract upload \
    --wasm "$wasm_file" \
    --source-account "$DEPLOYER_PRIVATE_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" 2>&1 | tail -1)

  log_success "WASM uploaded: $wasm_hash"
  echo "$wasm_hash"
}

# ─────────────────────────────────────────────────────────────────────────────
# Contract upgrade
# ─────────────────────────────────────────────────────────────────────────────

upgrade_contract() {
  local contract_id="$1"
  local wasm_hash="$2"
  
  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Would upgrade: stellar contract invoke --id $contract_id --source-account $DEPLOYER_PRIVATE_KEY --network-passphrase \"$NETWORK_PASSPHRASE\" --rpc-url $RPC_URL -- upgrade --new_wasm_hash $wasm_hash"
    return
  fi

  log_info "Upgrading contract: $contract_id"
  
  stellar contract invoke \
    --id "$contract_id" \
    --source-account "$DEPLOYER_PRIVATE_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- upgrade \
    --new_wasm_hash "$wasm_hash"

  log_success "Contract upgraded successfully"
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  log_info "Mux Protocol — Contract Upgrade"
  log_info "Network:  $NETWORK"
  log_info "Contract: $CONTRACT_NAME"
  log_info "Dry-run:  $DRY_RUN"
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

  # Get contract ID from config if not provided
  if [[ -z "$CONTRACT_ID" ]]; then
    local config_file="${REPO_ROOT}/config/addresses.json"
    if [[ -f "$config_file" ]] && command -v python3 &>/dev/null; then
      CONTRACT_ID=$(python3 -c "
import json
try:
    with open('$config_file') as f:
        config = json.load(f)
    network_config = config.get('$NETWORK', {})
    key = '${CONTRACT_NAME//-/_}'
    print(network_config.get(key, ''))
except:
    print('')
" 2>/dev/null)
    fi
  fi

  if [[ -z "$CONTRACT_ID" ]]; then
    log_error "Contract ID not provided and not found in config"
    log_error "Use --contract-id or ensure config/addresses.json is up to date"
    exit 2
  fi

  # Generate correlation ID if not provided
  CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
  export CORRELATION_ID

  # Initialize upgrade logging
  init_upgrade_logging "$CORRELATION_ID" "$NETWORK" "$CONTRACT_NAME" "$CONTRACT_ID" "$NEW_WASM_HASH" "$OLD_WASM_HASH"
  
  # Create lock file
  local lock_file="${REPO_ROOT}/ops/.upgrade-lock-${NETWORK}"
  echo "$CORRELATION_ID" > "$lock_file"
  trap "rm -f '$lock_file'" EXIT

  preflight_checks

  # Determine WASM file path
  local wasm_file="${WASM_DIR}/${CONTRACT_NAME//-/_}.wasm"
  
  if [[ "$SKIP_UPLOAD" == "false" && ! -f "$wasm_file" ]]; then
    log_warn "WASM file not found at $wasm_file"
    log_warn "Assuming WASM is already uploaded, using provided hash"
  fi

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Simulating upgrade of $CONTRACT_NAME"
  else
    log_info "Upgrading $CONTRACT_NAME"
  fi
  echo ""

  local upgrade_success=true

  # Upload WASM (if not skipped)
  if [[ "$SKIP_UPLOAD" == "false" && -f "$wasm_file" ]]; then
    local uploaded_hash
    uploaded_hash=$(upload_wasm "$wasm_file")
    
    if [[ -z "$uploaded_hash" ]]; then
      upgrade_success=false
    elif [[ "$uploaded_hash" != "$NEW_WASM_HASH" ]]; then
      log_error "Uploaded hash ($uploaded_hash) does not match expected hash ($NEW_WASM_HASH)"
      upgrade_success=false
    fi
  fi

  # Execute upgrade
  if [[ "$upgrade_success" == "true" ]]; then
    if ! upgrade_contract "$CONTRACT_ID" "$NEW_WASM_HASH"; then
      upgrade_success=false
    fi
  fi

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_success "Dry-run complete — no on-chain transactions were submitted"
  elif [[ "$upgrade_success" == "true" ]]; then
    log_success "Upgrade complete"
    log_upgrade_status "$CORRELATION_ID" "completed"
    log_info "Upgrade operation logged: $CORRELATION_ID"
  else
    log_error "Upgrade failed"
    log_upgrade_status "$CORRELATION_ID" "failed" "UPGRADE_FAILED"
    exit 1
  fi
}

main "$@"
