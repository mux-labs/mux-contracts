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
#   --help               Show this help message
#
# Environment variables:
#   DEPLOYER_PRIVATE_KEY  Deployer secret key (required unless --dry-run)
#   ADMIN_ADDRESS         Contract admin public key (required unless --dry-run)
#   SOROBAN_NETWORK       Override network (alternative to --network flag)
#   RPC_URL               Override RPC URL (alternative to --rpc-url flag)
#
# Dry-run mode:
#   When --dry-run is set, the script simulates every deployment step and logs
#   what would be executed. No on-chain transactions are submitted.
#   Missing environment variables are tolerated in dry-run mode.
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
# Exit codes:
#   0 - Success (or successful dry-run simulation)
#   1 - Deployment error
#   2 - Invalid arguments or missing required config

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
  "mux-delegation"
  "mux-permissions"
  "mux-recovery"
  "mux-registry"
  "mux-spending-policy"
  "mux-wallet-registry"
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
    case "$name" in
      mux-account)
        log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize --owner \${OWNER_ADDRESS} --guardians '[]'"
        ;;
      mux-account-factory)
        log_dry "  Init      : (no explicit init — factory is used directly)"
        ;;
      mux-batcher)
        log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize"
        ;;
      mux-delegation)
        log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize --admin \${ADMIN_ADDRESS}"
        ;;
      mux-permissions)
        log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize --admin \${ADMIN_ADDRESS}"
        ;;
      mux-registry)
        log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize --admin \${ADMIN_ADDRESS}"
        ;;
      mux-wallet-registry)
        log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize --admin \${ADMIN_ADDRESS}"
        ;;
      *)
        log_dry "  Init      : stellar contract invoke --id <contract_id> -- initialize --admin \${ADMIN_ADDRESS}"
        ;;
    esac
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
  echo ""

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

  for contract in "${CONTRACTS[@]}"; do
    deploy_contract "$contract"
  done

  echo ""
  if [[ "$DRY_RUN" == "true" ]]; then
    log_success "Dry-run complete — no on-chain transactions were submitted"
  else
    log_success "Deployment complete"
    [[ -f "${REPO_ROOT}/deployment.env" ]] && log_info "Contract addresses written to deployment.env"
  fi
}

main
