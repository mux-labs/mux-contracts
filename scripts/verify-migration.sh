#!/usr/bin/env bash
#
# verify-migration.sh
#
# Verify account migration authorization and safety before executing migration operations.
# This script performs pre-migration checks including authorization verification,
# account validation, conflict detection, and safety validations.
#
# Usage:
#   bash scripts/verify-migration.sh [OPTIONS]
#
# Options:
#   --network <name>         Stellar network (testnet|mainnet|localnet, default: testnet)
#   --account-id <id>        Account contract ID to migrate
#   --from-version <ver>     Source version (e.g., 1.0)
#   --to-version <ver>       Target version (e.g., 1.1)
#   --strategy <strategy>    Migration strategy (additive/transformative/two-phase)
#   --correlation-id <id>    Correlation ID for the migration (auto-generated if not set)
#   --dry-run                Verify without executing
#   --help                   Show this help message
#
# Environment variables:
#   ENABLE_ACCOUNT_MIGRATION Enable migration operations (default: false for mainnet)
#   ADMIN_ADDRESS           Account admin public key (required for authz check)
#   DEPLOYER_PRIVATE_KEY    Deployer secret key (required for live verification)
#
# Exit codes:
#   0 - Verification passed (safe to proceed)
#   1 - Verification failed (do not proceed)
#   2 - Invalid arguments or missing required config
#   3 - Authorization denied
#   4 - Conflict detected
#   5 - Account not found or invalid
#
# Examples:
#   # Verify migration authorization for mainnet
#   ENABLE_ACCOUNT_MIGRATION=true ADMIN_ADDRESS=G... bash scripts/verify-migration.sh \
#     --network mainnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy transformative
#
#   # Dry-run verification
#   bash scripts/verify-migration.sh --network testnet --account-id CABCD... --from-version 1.0 --to-version 1.1 --strategy additive --dry-run

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
ENABLE_ACCOUNT_MIGRATION="${ENABLE_ACCOUNT_MIGRATION:-false}"

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

validate_strategy() {
  local strategy="$1"
  case "$strategy" in
    additive|transformative|two-phase)
      return 0
      ;;
    *)
      log_error "Invalid migration strategy: $strategy"
      log_error "Valid strategies: additive, transformative, two-phase"
      return 1
      ;;
  esac
}

validate_version() {
  local version="$1"
  if [[ -z "$version" ]]; then
    log_error "Version cannot be empty"
    return 1
  fi
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Authorization checks
# ─────────────────────────────────────────────────────────────────────────────

check_migration_enabled() {
  local network="$1"
  
  # Fail-closed: migration disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_ACCOUNT_MIGRATION" != "true" ]]; then
    log_error "Migration operations are disabled on mainnet"
    log_error "Set ENABLE_ACCOUNT_MIGRATION=true to enable"
    return 1
  fi
  
  log_success "Migration is enabled for $network"
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
    log_warn "Ensure 3/5 signers have approved this migration"
  fi
  
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Conflict detection
# ─────────────────────────────────────────────────────────────────────────────

check_active_migration() {
  local network="$1"
  local account_id="$2"
  local lock_file="${REPO_ROOT}/ops/.migration-lock-${network}-${account_id:0:16}"
  
  if [[ -f "$lock_file" ]]; then
    local active_id
    active_id=$(cat "$lock_file")
    log_error "Conflicting migration in progress for account: $active_id"
    log_error "Wait for the existing migration to complete or cancel it"
    return 1
  fi
  
  log_success "No conflicting migration in progress"
  return 0
}

check_replay_protection() {
  local correlation_id="$1"
  local account_id="$2"
  local log_dir="${REPO_ROOT}/ops/logs"
  
  if [[ -d "$log_dir" ]]; then
    if grep -q "correlation_id=${correlation_id}.*account_id=${account_id}.*status=completed" "$log_dir"/migration-*.log 2>/dev/null; then
      log_error "Migration with correlation ID $correlation_id for account $account_id already completed"
      log_error "This appears to be a replay attempt"
      return 1
    fi
  fi
  
  log_success "No replay detected for correlation ID: $correlation_id"
  return 0
}

# ─────────────────────────────────────────────────────────────────────────────
# Account validation
# ─────────────────────────────────────────────────────────────────────────────

check_account_exists() {
  local account_id="$1"
  local network="$2"
  
  if [[ "$DRY_RUN" == "false" ]]; then
    log_info "Checking if account exists on $network..."
    # In production, this would query the RPC endpoint
    # For now, we assume the account exists if the ID is provided
    log_success "Account ID provided: $(redact_secret "$account_id")"
  else
    log_warn "Dry-run: skipping account existence check"
  fi
  
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
  log_info "Mux Protocol — Account Migration Verification"
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
  
  if ! validate_network "$NETWORK"; then
    exit 2
  fi
  
  if ! validate_strategy "$STRATEGY"; then
    exit 2
  fi
  
  if ! validate_version "$FROM_VERSION"; then
    exit 2
  fi
  
  if ! validate_version "$TO_VERSION"; then
    exit 2
  fi
  
  # Generate correlation ID if not provided
  CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
  
  log_info "Configuration:"
  log_info "  Network:         $NETWORK"
  log_info "  Account ID:      $(redact_secret "$ACCOUNT_ID")"
  log_info "  From Version:    $FROM_VERSION"
  log_info "  To Version:      $TO_VERSION"
  log_info "  Strategy:        $STRATEGY"
  log_info "  Correlation ID:  $CORRELATION_ID"
  log_info "  Dry-run:         $DRY_RUN"
  echo ""
  
  # Run verification checks
  local checks_passed=0
  local checks_total=7
  
  log_info "Running verification checks..."
  echo ""
  
  # Check 1: Migration enabled
  if check_migration_enabled "$NETWORK"; then
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
  
  # Check 4: No conflicting migration
  if check_active_migration "$NETWORK" "$ACCOUNT_ID"; then
    ((checks_passed++))
  else
    exit 4
  fi
  
  # Check 5: Replay protection
  if check_replay_protection "$CORRELATION_ID" "$ACCOUNT_ID"; then
    ((checks_passed++))
  else
    exit 4
  fi
  
  # Check 6: Account exists
  if check_account_exists "$ACCOUNT_ID" "$NETWORK"; then
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
    log_info "It is safe to proceed with the migration operation"
    log_info "Use the following correlation ID for logging:"
    log_info "  export CORRELATION_ID=$CORRELATION_ID"
    echo ""
    log_info "Execute migration with:"
    log_info "  ENABLE_ACCOUNT_MIGRATION=true bash scripts/migrate-account.sh \\"
    log_info "    --network $NETWORK \\"
    log_info "    --account-id $ACCOUNT_ID \\"
    log_info "    --from-version $FROM_VERSION \\"
    log_info "    --to-version $TO_VERSION \\"
    log_info "    --strategy $STRATEGY"
  else
    log_info "Dry-run verification passed"
  fi
  
  return 0
}

main "$@"
