#!/usr/bin/env bash
#
# deploy.sh
#
# Deploy Mux Protocol Soroban contracts to a Stellar network.
#
# Usage:
#   bash scripts/deploy.sh [options]
#
# Options:
#   --network <name>       Target network: testnet | mainnet | localnet (default: testnet)
#   --contract <name>      Deploy a single contract only (default: all contracts)
#   --dry-run              Simulate deployment — log all actions, skip on-chain submissions
#   --skip-build           Skip `cargo build`; use pre-built WASMs from target/
#   --deployer <key>       Deployer secret key (overrides DEPLOYER_SECRET_KEY env var)
#   --help                 Show this help message
#
# Environment Variables:
#   DEPLOYER_SECRET_KEY    Secret key of the funded deployer account
#   SOROBAN_RPC_URL        RPC endpoint URL (overrides per-network default)
#   STELLAR_NETWORK        Network passphrase (overrides per-network default)
#
# Examples:
#   # Dry-run on testnet (no transactions submitted)
#   bash scripts/deploy.sh --dry-run
#
#   # Deploy all contracts to testnet
#   DEPLOYER_SECRET_KEY=S... bash scripts/deploy.sh --network testnet
#
#   # Deploy a single contract to mainnet
#   DEPLOYER_SECRET_KEY=S... bash scripts/deploy.sh --network mainnet --contract mux-account
#
# Exit codes:
#   0  Success (or dry-run completed)
#   1  Missing required configuration or deployment error

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Colours
# ─────────────────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# ─────────────────────────────────────────────────────────────────────────────
# Logging helpers
# ─────────────────────────────────────────────────────────────────────────────

log_info()    { echo -e "${BLUE}ℹ️  ${NC}$*"; }
log_success() { echo -e "${GREEN}✓${NC} $*"; }
log_warning() { echo -e "${YELLOW}⚠️  ${NC}$*"; }
log_error()   { echo -e "${RED}✗${NC} $*" >&2; }

# Prefix every line with [DRY RUN] when --dry-run is active
dry_log() { echo -e "${CYAN}${BOLD}[DRY RUN]${NC} $*"; }

# ─────────────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────────────

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_DIR="${REPO_ROOT}/target/wasm32-unknown-unknown/release"

NETWORK="testnet"
TARGET_CONTRACT=""
DRY_RUN=false
SKIP_BUILD=false
DEPLOYER_KEY="${DEPLOYER_SECRET_KEY:-}"

ALL_CONTRACTS=(
  "mux-account"
  "mux-account-factory"
  "mux-batcher"
  "mux-permissions"
)

# Per-network RPC / passphrase defaults
declare -A NETWORK_RPC=(
  [testnet]="https://soroban-testnet.stellar.org"
  [mainnet]="https://soroban-mainnet.stellar.org"
  [localnet]="http://localhost:8000/rpc"
)
declare -A NETWORK_PASSPHRASE=(
  [testnet]="Test SDF Network ; September 2015"
  [mainnet]="Public Global Stellar Network ; September 2015"
  [localnet]="Standalone Network ; February 2017"
)

# ─────────────────────────────────────────────────────────────────────────────
# Argument parsing
# ─────────────────────────────────────────────────────────────────────────────

usage() {
  grep '^#' "$0" | sed 's/^# \{0,1\}//' | head -30
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --network)
      NETWORK="${2:?'--network requires a value (testnet|mainnet|localnet)'}"
      shift 2 ;;
    --contract)
      TARGET_CONTRACT="${2:?'--contract requires a contract name'}"
      shift 2 ;;
    --dry-run)
      DRY_RUN=true
      shift ;;
    --skip-build)
      SKIP_BUILD=true
      shift ;;
    --deployer)
      DEPLOYER_KEY="${2:?'--deployer requires a secret key'}"
      shift 2 ;;
    --help|-h)
      usage ;;
    *)
      log_error "Unknown argument: $1"
      exit 1 ;;
  esac
done

# ─────────────────────────────────────────────────────────────────────────────
# Validate network
# ─────────────────────────────────────────────────────────────────────────────

if [[ -z "${NETWORK_RPC[$NETWORK]+x}" ]]; then
  log_error "Unknown network '$NETWORK'. Use: testnet, mainnet, or localnet"
  exit 1
fi

RPC_URL="${SOROBAN_RPC_URL:-${NETWORK_RPC[$NETWORK]}}"
PASSPHRASE="${STELLAR_NETWORK:-${NETWORK_PASSPHRASE[$NETWORK]}}"

# ─────────────────────────────────────────────────────────────────────────────
# Determine contracts to deploy
# ─────────────────────────────────────────────────────────────────────────────

if [[ -n "$TARGET_CONTRACT" ]]; then
  CONTRACTS=("$TARGET_CONTRACT")
else
  CONTRACTS=("${ALL_CONTRACTS[@]}")
fi

# ─────────────────────────────────────────────────────────────────────────────
# Print configuration banner
# ─────────────────────────────────────────────────────────────────────────────

echo ""
if [[ "$DRY_RUN" == "true" ]]; then
  echo -e "${CYAN}${BOLD}╔══════════════════════════════════════╗${NC}"
  echo -e "${CYAN}${BOLD}║         DRY RUN MODE ACTIVE          ║${NC}"
  echo -e "${CYAN}${BOLD}║  No transactions will be submitted.  ║${NC}"
  echo -e "${CYAN}${BOLD}╚══════════════════════════════════════╝${NC}"
  echo ""
fi

log_info "Configuration:"
log_info "  Network    : $NETWORK"
log_info "  RPC URL    : $RPC_URL"
log_info "  Contracts  : ${CONTRACTS[*]}"
log_info "  Skip build : $SKIP_BUILD"
log_info "  Dry run    : $DRY_RUN"
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# Deployer key check (skip in dry-run)
# ─────────────────────────────────────────────────────────────────────────────

if [[ "$DRY_RUN" == "false" ]]; then
  if [[ -z "$DEPLOYER_KEY" ]]; then
    log_error "No deployer key provided."
    log_error "Set DEPLOYER_SECRET_KEY environment variable or pass --deployer <key>"
    exit 1
  fi
  log_success "Deployer key found (${#DEPLOYER_KEY} chars)"
else
  dry_log "Would validate DEPLOYER_SECRET_KEY — skipped in dry-run"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Utility: run a command or simulate it in dry-run mode
# ─────────────────────────────────────────────────────────────────────────────

# run_cmd CMD [ARGS...]
# In live mode: executes the command.
# In dry-run:   prints the command prefixed with [DRY RUN] and returns 0.
run_cmd() {
  if [[ "$DRY_RUN" == "true" ]]; then
    dry_log "Would run: $*"
    return 0
  fi
  "$@"
}

# ─────────────────────────────────────────────────────────────────────────────
# Step 1 — Build contracts
# ─────────────────────────────────────────────────────────────────────────────

if [[ "$SKIP_BUILD" == "false" ]]; then
  log_info "Step 1/3: Building Soroban contracts..."
  if [[ "$DRY_RUN" == "true" ]]; then
    dry_log "Would run: cargo build --target wasm32-unknown-unknown --release --workspace"
  else
    cargo build --target wasm32-unknown-unknown --release --workspace
    log_success "Build complete"
  fi
else
  log_info "Step 1/3: Skipping build (--skip-build set)"
fi
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# Step 2 — Upload + deploy each contract
# ─────────────────────────────────────────────────────────────────────────────

log_info "Step 2/3: Deploying contracts to $NETWORK..."
echo ""

declare -A DEPLOYED_IDS

for contract in "${CONTRACTS[@]}"; do
  wasm_name="${contract//-/_}.wasm"
  wasm_path="${WASM_DIR}/${wasm_name}"

  echo -e "  ${BOLD}→ $contract${NC}"

  # WASM existence check
  if [[ "$DRY_RUN" == "false" ]] && [[ ! -f "$wasm_path" ]]; then
    log_error "WASM not found: $wasm_path"
    log_error "Run without --skip-build or build first."
    exit 1
  fi

  if [[ "$DRY_RUN" == "true" ]]; then
    dry_log "  Would check WASM exists: $wasm_path"
    dry_log "  Would run: stellar contract upload --wasm $wasm_path --source <deployer> --rpc-url $RPC_URL --network-passphrase \"$PASSPHRASE\""
    dry_log "  Would run: stellar contract deploy --wasm-hash <WASM_HASH> --source <deployer> --rpc-url $RPC_URL --network-passphrase \"$PASSPHRASE\""
    dry_log "  Would record contract ID → <CONTRACT_ID>"
    DEPLOYED_IDS[$contract]="<dry-run-placeholder>"
  else
    log_info "  Uploading WASM: $wasm_path"
    WASM_HASH=$(stellar contract upload \
      --wasm "${wasm_path}" \
      --source "${DEPLOYER_KEY}" \
      --rpc-url "${RPC_URL}" \
      --network-passphrase "${PASSPHRASE}" \
      2>&1 | tail -1)
    log_success "  WASM hash: $WASM_HASH"

    log_info "  Deploying contract..."
    CONTRACT_ID=$(stellar contract deploy \
      --wasm-hash "${WASM_HASH}" \
      --source "${DEPLOYER_KEY}" \
      --rpc-url "${RPC_URL}" \
      --network-passphrase "${PASSPHRASE}" \
      2>&1 | tail -1)
    log_success "  Contract ID: $CONTRACT_ID"
    DEPLOYED_IDS[$contract]="$CONTRACT_ID"
  fi
  echo ""
done

# ─────────────────────────────────────────────────────────────────────────────
# Step 3 — Summary
# ─────────────────────────────────────────────────────────────────────────────

log_info "Step 3/3: Deployment summary"
echo ""

if [[ "$DRY_RUN" == "true" ]]; then
  echo -e "${CYAN}${BOLD}[DRY RUN] Actions that would have been taken:${NC}"
  echo ""
  echo -e "  1. Build contracts via cargo build --target wasm32-unknown-unknown --release"
  echo -e "  2. Upload each WASM binary to $NETWORK via stellar contract upload"
  echo -e "  3. Deploy each contract via stellar contract deploy"
  echo -e "  4. Record contract IDs for post-deploy configuration"
  echo ""
  echo -e "${CYAN}${BOLD}[DRY RUN] Contracts that would be deployed:${NC}"
  for contract in "${CONTRACTS[@]}"; do
    echo -e "  ${CYAN}[DRY RUN]${NC}   $contract → ${DEPLOYED_IDS[$contract]}"
  done
  echo ""
  echo -e "${CYAN}${BOLD}[DRY RUN] No transactions were submitted. Dry-run complete.${NC}"
else
  echo -e "${BOLD}Deployed contract IDs:${NC}"
  for contract in "${CONTRACTS[@]}"; do
    echo -e "  ${GREEN}✓${NC} $contract → ${DEPLOYED_IDS[$contract]}"
  done
  echo ""
  log_success "Deployment complete on $NETWORK!"
fi
