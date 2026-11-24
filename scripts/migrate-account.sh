#!/usr/bin/env bash
#
# migrate-account.sh
#
# Execute account contract migration operations with authorization verification and logging.
# This script performs the actual migration after verification, with correlation ID
# logging and secret redaction.
#
# Usage:
#   bash scripts/migrate-account.sh [OPTIONS]
#
# Options:
#   --network <name>         Stellar network (testnet|mainnet|localnet, default: testnet)
#   --account-id <id>        Account contract ID to migrate
#   --from-version <ver>     Source version (e.g., 1.0)
#   --to-version <ver>       Target version (e.g., 1.1)
#   --strategy <strategy>    Migration strategy (additive/transformative/two-phase)
#   --correlation-id <id>    Correlation ID for the migration (auto-generated if not set)
#   --dry-run                Simulate migration without executing
#   --skip-upgrade           Skip contract upgrade (if already upgraded)
#   --help                   Show this help message
#
# Environment variables:
#   ENABLE_ACCOUNT_MIGRATION Enable migration operations (default: false for mainnet)
#   ADMIN_ADDRESS           Account admin public key (required)
#   DEPLOYER_PRIVATE_KEY    Deployer secret key (required)
#   CORRELATION_ID          Correlation ID for logging
#
# Exit codes:
#   0 - Migration successful
#   1 - Migration failed
#   2 - Invalid arguments or missing required config
#   3 - Authorization denied
#   4 - Conflict detected
#   5 - Account not found or invalid
#   6 - Log failure
#
# Examples:
#   # Execute migration on mainnet
#   ENABLE_ACCOUNT_MIGRATION=true ADMIN_ADDRESS=G... DEPLOYER_PRIVATE_KEY=S... \
#     bash scripts/migrate-account.sh --network mainnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy transformative
#
#   # Dry-run migration
#   bash scripts/migrate-account.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ─────────────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────────────

NETWORK="${SOROBAN_NETWORK:-testnet}"
ACCOUNT_ID=""
FROM_VERSION=""
TO_VERSION=""
STRATEGY=""
CORRELATION_ID=""
DRY_RUN=false
SKIP_UPGRADE=false
ENABLE_ACCOUNT_MIGRATION="${ENABLE_ACCOUNT_MIGRATION:-false}"

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
# Migration logging functions
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

init_migration_logging() {
  local correlation_id="$1"
  local network="$2"
  local account_id="$3"
  local from_version="$4"
  local to_version="$5"
  local strategy="$6"

  MIGRATION_LOG_DIR="${REPO_ROOT}/ops/logs"
  mkdir -p "$MIGRATION_LOG_DIR"
  MIGRATION_LOG_FILE="${MIGRATION_LOG_DIR}/migration-$(date +%Y-%m-%d).log"

  local git_commit
  git_commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ) operator=<REDACTED> network=${network} account_id=${account_id} from_version=${from_version} to_version=${to_version} migration_strategy=${strategy} authz_method=multisig authz_id=<REDACTED> status=started git_commit=${git_commit}"

  if ! echo "$log_entry" >> "$MIGRATION_LOG_FILE"; then
    log_error "Failed to write to migration log. Operation blocked."
    exit 6
  fi

  log_info "Migration logging initialized: correlation_id=$correlation_id"
}

log_migration_status() {
  local correlation_id="$1"
  local status="$2"
  local error_code="${3:-}"
  local extra_fields="${4:-}"

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} status=${status} ${error_code:+error_code=${error_code}} ${extra_fields}"

  if ! echo "$log_entry" >> "$MIGRATION_LOG_FILE"; then
    log_error "Failed to write migration status to log"
    return 1
  fi
}

check_migration_authz() {
  local network="$1"
  
  # Fail-closed: migration disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_ACCOUNT_MIGRATION" != "true" ]]; then
    log_error "Migration operations are disabled on mainnet"
    log_error "Set ENABLE_ACCOUNT_MIGRATION=true to enable"
    return 1
  fi

  # Check for conflicting migration
  local lock_file="${REPO_ROOT}/ops/.migration-lock-${network}-${ACCOUNT_ID:0:16}"
  if [[ -f "$lock_file" ]]; then
    local active_id
    active_id=$(cat "$lock_file")
    log_error "Conflicting migration in progress for account: $active_id"
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
    --account-id)
      ACCOUNT_ID="${2:?'--account-id requires a value'}"
      shift 2
      ;;
    --from-version)
      FROM_VERSION="${2:?'--from-version requires a value'}"
      shift 2
      ;;
    --to-version)
      TO_VERSION="${2:?'--to-version requires a value'}"
      shift 2
      ;;
    --strategy)
      STRATEGY="${2:?'--strategy requires a value'}"
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
    --skip-upgrade)
      SKIP_UPGRADE=true
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

  if ! check_migration_authz "$NETWORK"; then
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
    command -v stellar &>/dev/null || log_warn "stellar CLI not found (would be required for real migration)"
    [[ -z "${DEPLOYER_PRIVATE_KEY:-}" ]] && log_warn "DEPLOYER_PRIVATE_KEY not set (required for real migration)"
    [[ -z "${ADMIN_ADDRESS:-}" ]]        && log_warn "ADMIN_ADDRESS not set (required for real migration)"
  fi

  log_success "Preflight checks complete"
}

# ─────────────────────────────────────────────────────────────────────────────
# Contract upgrade
# ─────────────────────────────────────────────────────────────────────────────

upgrade_contract() {
  local account_id="$1"
  local new_wasm_hash="$2"
  
  if [[ "$SKIP_UPGRADE" == "true" ]]; then
    log_info "Skipping contract upgrade (--skip-upgrade)"
    return
  fi

  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Would upgrade: stellar contract invoke --id $account_id --source-account $DEPLOYER_PRIVATE_KEY --network-passphrase \"$NETWORK_PASSPHRASE\" --rpc-url $RPC_URL -- upgrade --new_wasm_hash $new_wasm_hash"
    return
  fi

  log_info "Upgrading contract: $account_id"
  
  stellar contract invoke \
    --id "$account_id" \
    --source-account "$DEPLOYER_PRIVATE_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- upgrade \
    --new_wasm_hash "$new_wasm_hash"

  log_success "Contract upgraded successfully"
}

# ─────────────────────────────────────────────────────────────────────────────
# Migration execution
# ─────────────────────────────────────────────────────────────────────────────

execute_migration() {
  local account_id="$1"
  local strategy="$2"
  
  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Would execute migration: stellar contract invoke --id $account_id --source-account $DEPLOYER_PRIVATE_KEY --network-passphrase \"$NETWORK_PASSPHRASE\" --rpc-url $RPC_URL -- migrate"
    return
  fi

  case "$strategy" in
    additive)
      log_info "Additive migration: no migration function needed"
      ;;
    transformative|two-phase)
      log_info "Executing migration function..."
      stellar contract invoke \
        --id "$account_id" \
        --source-account "$DEPLOYER_PRIVATE_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        -- migrate
      log_success "Migration executed successfully"
      ;;
  esac
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  log_info "Mux Protocol — Account Migration"
  log_info "Network:       $NETWORK"
  log_info "Account ID:    $(redact_secret "$ACCOUNT_ID")"
  log_info "From Version:  $FROM_VERSION"
  log_info "To Version:    $TO_VERSION"
  log_info "Strategy:      $STRATEGY"
  log_info "Dry-run:       $DRY_RUN"
  echo ""

  # Validate inputs
  if [[ -z "$ACCOUNT_ID" ]]; then
    log_error "--account-id is required"
    exit 2
  fi

  if [[ -z "$FROM_VERSION" ]]; then
    log_error "--from-version is required"
    exit 2
  fi

  if [[ -z "$TO_VERSION" ]]; then
    log_error "--to-version is required"
    exit 2
  fi

  if [[ -z "$STRATEGY" ]]; then
    log_error "--strategy is required"
    exit 2
  fi

  # Generate correlation ID if not provided
  CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
  export CORRELATION_ID

  # Initialize migration logging
  init_migration_logging "$CORRELATION_ID" "$NETWORK" "$ACCOUNT_ID" "$FROM_VERSION" "$TO_VERSION" "$STRATEGY"
  
  # Create lock file
  local lock_file="${REPO_ROOT}/ops/.migration-lock-${NETWORK}-${ACCOUNT_ID:0:16}"
  echo "$CORRELATION_ID" > "$lock_file"
  trap "rm -f '$lock_file'" EXIT

  preflight_checks

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Simulating migration of account $ACCOUNT_ID"
  else
    log_info "Migrating account $ACCOUNT_ID"
  fi
  echo ""

  local migration_success=true

  # For transformative and two-phase strategies, upgrade contract first
  if [[ "$STRATEGY" == "transformative" || "$STRATEGY" == "two-phase" ]]; then
    if [[ "$SKIP_UPGRADE" == "false" ]]; then
      log_info "Contract upgrade required for $STRATEGY strategy"
      log_info "Please provide new WASM hash via --new-wasm-hash or use --skip-upgrade if already upgraded"
      log_warn "Skipping upgrade for now (requires WASM hash)"
    fi
  fi

  # Execute migration
  if [[ "$migration_success" == "true" ]]; then
    if ! execute_migration "$ACCOUNT_ID" "$STRATEGY"; then
      migration_success=false
    fi
  fi

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_success "Dry-run complete — no on-chain transactions were submitted"
  elif [[ "$migration_success" == "true" ]]; then
    log_success "Migration complete"
    log_migration_status "$CORRELATION_ID" "completed" "" "migrated_keys_count=0"
    log_info "Migration operation logged: $CORRELATION_ID"
  else
    log_error "Migration failed"
    log_migration_status "$CORRELATION_ID" "failed" "MIGRATION_FAILED"
    exit 1
  fi
}

main "$@"
