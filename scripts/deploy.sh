#!/usr/bin/env bash
#
# deploy.sh
#
# Deploy Mux Protocol Soroban contracts to a Stellar network.
#
# Usage:
#   bash scripts/deploy.sh [OPTIONS]
#
# Options:
#   --network <name>     Stellar network (testnet|mainnet|localnet, default: testnet)
#   --contract <name>    Deploy a single contract by name (default: all contracts)
#   --dry-run            Simulate all deployment steps without executing on-chain transactions
#   --skip-build         Skip the WASM build step (assumes artifacts already exist)
#   --rpc-url <url>      Override the RPC URL
#   --rollback           Enable rollback mode with correlation ID logging
#   --rollback-strategy <strategy> Rollback strategy (address-repoint|deploy-wasm|admin-pause)
#   --help               Show this help message
#
# Environment variables:
#   DEPLOYER_PRIVATE_KEY  Deployer secret key (required unless --dry-run)
#   ADMIN_ADDRESS         Contract admin public key (required unless --dry-run)
#   SOROBAN_NETWORK       Override network (alternative to --network flag)
#   RPC_URL               Override RPC URL (alternative to --rpc-url flag)
#   CORRELATION_ID        Correlation ID for rollback operations (auto-generated if not set)
#   ENABLE_ROLLBACK       Enable rollback operations (default: false for mainnet)
#
# Dry-run mode:
#   When --dry-run is set, the script simulates every deployment step and logs
#   what would be executed. No on-chain transactions are submitted.
#   Missing environment variables are tolerated in dry-run mode.
#
# Rollback mode:
#   When --rollback is set, the script generates a correlation ID and logs all
#   operations to ops/logs/rollback-*.log following the rollback-log discipline.
#   Secrets are redacted before logging. See ops/rollback-log.md for details.
#
# Examples:
#   # Simulate a full deploy (no keys required)
#   bash scripts/deploy.sh --dry-run
#
#   # Simulate deploying a single contract
#   bash scripts/deploy.sh --dry-run --contract mux-account
#
#   # Real deploy to testnet
#   DEPLOYER_PRIVATE_KEY=S... ADMIN_ADDRESS=G... bash scripts/deploy.sh --network testnet
#
#   # Rollback with correlation ID logging
#   ENABLE_ROLLBACK=true bash scripts/deploy.sh --network mainnet --rollback --rollback-strategy address-repoint
#
# Exit codes:
#   0 - Success (or successful dry-run simulation)
#   1 - Deployment error
#   2 - Invalid arguments or missing required config
#   3 - Rollback authorization denied
#   4 - Rollback log failure

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_DIR="${REPO_ROOT}/target/wasm32-unknown-unknown/release"

# ─────────────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────────────

NETWORK="${SOROBAN_NETWORK:-testnet}"
DRY_RUN=false
SKIP_BUILD=false
TARGET_CONTRACT=""
RPC_URL_OVERRIDE="${RPC_URL:-}"
ROLLBACK_MODE=false
ROLLBACK_STRATEGY=""
ENABLE_ROLLBACK="${ENABLE_ROLLBACK:-false}"

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
# Rollback logging functions
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

log_safe() {
  local key="$1"
  local value="$2"
  case "$key" in
    *SECRET*|*KEY*|*TOKEN*|*PRIVATE*)
      echo "$key=$(redact_secret "$value")"
      ;;
    *)
      echo "$key=$value"
      ;;
  esac
}

init_rollback_logging() {
  local correlation_id="$1"
  local network="$2"
  local strategy="$3"

  ROLLBACK_LOG_DIR="${REPO_ROOT}/ops/logs"
  mkdir -p "$ROLLBACK_LOG_DIR"
  ROLLBACK_LOG_FILE="${ROLLBACK_LOG_DIR}/rollback-$(date +%Y-%m-%d).log"

  local git_commit
  git_commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ) operator=<REDACTED> network=${network} contract_name=${TARGET_CONTRACT:-all} rollback_strategy=${strategy} authz_method=multisig authz_id=<REDACTED> status=started git_commit=${git_commit}"

  if ! echo "$log_entry" >> "$ROLLBACK_LOG_FILE"; then
    log_error "Failed to write to rollback log. Operation blocked."
    exit 4
  fi

  log_info "Rollback logging initialized: correlation_id=$correlation_id"
}

log_rollback_status() {
  local correlation_id="$1"
  local status="$2"
  local error_code="${3:-}"
  local extra_fields="${4:-}"

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} status=${status} ${error_code:+error_code=${error_code}} ${extra_fields}"

  if ! echo "$log_entry" >> "$ROLLBACK_LOG_FILE"; then
    log_error "Failed to write rollback status to log"
    return 1
  fi
}

check_rollback_authz() {
  local network="$1"
  
  # Fail-closed: rollback disabled by default on mainnet
  if [[ "$network" == "mainnet" && "$ENABLE_ROLLBACK" != "true" ]]; then
    log_error "Rollback operations are disabled on mainnet. Set ENABLE_ROLLBACK=true to enable."
    return 1
  fi

  # Check for conflicting rollback (simplified - in production, check active lock file)
  local lock_file="${REPO_ROOT}/ops/.rollback-lock-${network}"
  if [[ -f "$lock_file" ]]; then
    local active_id
    active_id=$(cat "$lock_file")
    log_error "Conflicting rollback in progress: $active_id"
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
      TARGET_CONTRACT="${2:?'--contract requires a value'}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --skip-build)
      SKIP_BUILD=true
      shift
      ;;
    --rpc-url)
      RPC_URL_OVERRIDE="${2:?'--rpc-url requires a value'}"
      shift 2
      rollback)
      ROLLBACK_MODE=true
      shift
      ;;
    --rollback-strategy)
      ROLLBACK_STRATEGY="${2:?'--rollback-strategy requires a value'}"
      shift 2
      ;;
    --;;
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
    *)
      log_error "Unknown network: $1 (use testnet, mainnet, or localnet)"
      exit 2
      ;;
  esac
}

resolve_passphrase() {
  case "$1" in
    testnet)  echo "Test SDF Network ; September 2015" ;;
    mainnet)  echo "Public Global Stellar Network ; September 2015" ;;
    localnet) echo "Standalone Network ; February 2017" ;;
  esac
}

RPC_URL="${RPC_URL_OVERRIDE:-$(resolve_rpc_url "$NETWORK")}"
NETWORK_PASSPHRASE="$(resolve_passphrase "$NETWORK")"

# ─────────────────────────────────────────────────────────────────────────────
# Contracts to deploy
# ─────────────────────────────────────────────────────────────────────────────

ALL_CONTRACTS=(
  "mux-account"
  "mux-account-factory"
  "mux-batcher"
  "mux-permissions"
  "mux-registry"
)

if [[ -n "$TARGET_CONTRACT" ]]; then
  CONTRACTS=("$TARGET_CONTRACT")
else
  CONTRACTS=("${ALL_CONTRACTS[@]}")
fi

# ─────────────────────────────────────────────────────────────────────────────
# Preflight
# ─────────────────────────────────────────────────────────────────────────────

preflight_checks() {
  log_info "Running preflight checks..."

  if [[ "$ROLLBACK_MODE" == "true" ]]; then
    if ! check_rollback_authz "$NETWORK"; then
      exit 3
    fi
    if [[ -z "$ROLLBACK_STRATEGY" ]]; then
      log_error "--rollback-strategy is required when --rollback is set"
      exit 2
    fi
  fi

  if [[ "$DRY_RUN" == "false" ]]; then
    if ! command -v stellar &>/dev/null; then
      log_error "'stellar' CLI not found. Install: https://developers.stellar.org/docs/tools/stellar-cli"
      exit 1
    fi
    if ! command -v cargo &>/dev/null; then
      log_error "'cargo' not found. Install Rust: https://rustup.rs"
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
    # Dry-run: warn about missing vars but do not fail
    command -v stellar &>/dev/null || log_warn "stellar CLI not found (would be required for real deploy)"
    command -v cargo   &>/dev/null || log_warn "cargo not found (would be required for real deploy)"
    [[ -z "${DEPLOYER_PRIVATE_KEY:-}" ]] && log_warn "DEPLOYER_PRIVATE_KEY not set (required for real deploy)"
    [[ -z "${ADMIN_ADDRESS:-}" ]]        && log_warn "ADMIN_ADDRESS not set (required for real deploy)"
  fi

  log_success "Preflight checks complete"
}

# ─────────────────────────────────────────────────────────────────────────────
# Build
# ─────────────────────────────────────────────────────────────────────────────

build_contracts() {
  if [[ "$SKIP_BUILD" == "true" ]]; then
    log_info "Skipping WASM build (--skip-build)"
    return
  fi

  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Would run: cargo build --target wasm32-unknown-unknown --release --workspace"
    return
  fi

  log_info "Building contracts..."
  cd "$REPO_ROOT"
  cargo build --target wasm32-unknown-unknown --release --workspace
  log_success "Build complete"
}

# ─────────────────────────────────────────────────────────────────────────────
# Deploy a single contract
# ─────────────────────────────────────────────────────────────────────────────

deploy_contract() {
  local name="$1"
  local wasm_name="${name//-/_}.wasm"
  local wasm_path="${WASM_DIR}/${wasm_name}"

  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Contract: $name"
    log_dry "  WASM path : $wasm_path"
    log_dry "  Upload    : stellar contract upload --wasm $wasm_path --network-passphrase \"$NETWORK_PASSPHRASE\" --rpc-url $RPC_URL"
    log_dry "  Deploy    : stellar contract deploy --wasm-hash <hash> --network-passphrase \"$NETWORK_PASSPHRASE\" --rpc-url $RPC_URL"
    log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize --admin \${ADMIN_ADDRESS}"
    return
  fi

  if [[ ! -f "$wasm_path" ]]; then
    log_warn "WASM not found for $name at $wasm_path — skipping"
    return
  fi

  log_info "Deploying $name..."

  local wasm_hash
  wasm_hash=$(stellar contract upload \
    --wasm "$wasm_path" \
    --source-account "$DEPLOYER_PRIVATE_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" 2>&1 | tail -1)

  log_info "  WASM hash: $wasm_hash"

  local contract_id
  contract_id=$(stellar contract deploy \
    --wasm-hash "$wasm_hash" \
    --source-account "$DEPLOYER_PRIVATE_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" 2>&1 | tail -1)

  log_success "  $name deployed → $contract_id"
  echo "$name=$contract_id" >> "${REPO_ROOT}/deployment.env"
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

main() {
  echo ""
  log_info "Mux Protocol — Contract Deployment"
  log_info "Network:  $NETWORK"
  log_info "RPC URL:  $RPC_URL"
  log_info "Dry-run:  $DRY_RUN"
  [[ -n "$TARGET_CONTRACT" ]] && log_info "Contract: $TARGET_CONTRACT"
  [[ "$ROLLBACK_MODE" == "true" ]] && log_info "Rollback: $ROLLBACK_STRATEGY"
  echo ""

  # Initialize rollback logging if in rollback mode
  if [[ "$ROLLBACK_MODE" == "true" ]]; then
    CORRELATION_ID="${CORRELATION_ID:-$(generate_correlation_id)}"
    export CORRELATION_ID
    init_rollback_logging "$CORRELATION_ID" "$NETWORK" "$ROLLBACK_STRATEGY"
    
    # Create lock file
    local lock_file="${REPO_ROOT}/ops/.rollback-lock-${NETWORK}"
    echo "$CORRELATION_ID" > "$lock_file"
    trap "rm -f '$lock_file'" EXIT
  fi

  preflight_checks
  build_contracts

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_dry "Simulating deployment of ${#CONTRACTS[@]} contract(s):"
  else
    log_info "Deploying ${#CONTRACTS[@]} contract(s)..."
    [[ -f "${REPO_ROOT}/deployment.env" ]] && rm "${REPO_ROOT}/deployment.env"
  fi
  echo ""

  local deploy_success=true
  for contract in "${CONTRACTS[@]}"; do
    if ! deploy_contract "$contract"; then
      deploy_success=false
      break
    fi
  done

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_success "Dry-run complete — no on-chain transactions were submitted"
  elif [[ "$deploy_success" == "true" ]]; then
    log_success "Deployment complete"
    [[ -f "${REPO_ROOT}/deployment.env" ]] && log_info "Contract addresses written to deployment.env"
  else
    log_error "Deployment failed"
    if [[ "$ROLLBACK_MODE" == "true" ]]; then
      log_rollback_status "$CORRELATION_ID" "failed" "DEPLOYMENT_ERROR"
    fi
    exit 1
  fi

  # Log rollback completion
  if [[ "$ROLLBACK_MODE" == "true" ]]; then
    log_rollback_status "$CORRELATION_ID" "completed"
    log_info "Rollback operation logged: $CORRELATION_ID"
  fi
}

main
