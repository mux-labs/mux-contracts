#!/usr/bin/env bash
# ==============================================================================
# scripts/deploy-testnet.sh
#
# Deploy Mux Protocol Soroban contracts to Stellar Testnet.
#
# Usage:
#   ./scripts/deploy-testnet.sh [--contract <name>] [--dry-run]
#
# Environment variables (required):
#   MUX_DEPLOYER_SECRET   — Stellar secret key for the deployer account
#   MUX_ADMIN_ADDRESS     — Stellar address that will become contract admin
#
# Environment variables (optional):
#   MUX_NETWORK           — Network alias (default: testnet)
#   MUX_RPC_URL           — Horizon RPC URL (default: Stellar testnet)
#   MUX_NETWORK_PASSPHRASE— Network passphrase (auto-resolved from network alias)
# ==============================================================================

set -euo pipefail

# ── Colour helpers ─────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; NC='\033[0m'
info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

# ── Defaults ───────────────────────────────────────────────────────────────────
NETWORK="${MUX_NETWORK:-testnet}"
DRY_RUN=false
SPECIFIC_CONTRACT=""
WASM_DIR="target/wasm32-unknown-unknown/release"

# ── Network passphrase map ─────────────────────────────────────────────────────
resolve_passphrase() {
  case "$1" in
    testnet)        echo "Test SDF Network ; September 2015" ;;
    mainnet|public) echo "Public Global Stellar Network ; September 2015" ;;
    futurenet)      echo "Test SDF Future Network ; October 2022" ;;
    local)          echo "Standalone Network ; February 2017" ;;
    *)              error "Unknown network '$1'. Set MUX_NETWORK_PASSPHRASE manually." ;;
  esac
}

NETWORK_PASSPHRASE="${MUX_NETWORK_PASSPHRASE:-$(resolve_passphrase "$NETWORK")}"

# ── RPC URL ────────────────────────────────────────────────────────────────────
if [ -z "${MUX_RPC_URL:-}" ]; then
  case "$NETWORK" in
    testnet)   MUX_RPC_URL="https://soroban-testnet.stellar.org" ;;
    mainnet)   MUX_RPC_URL="https://mainnet.stellar.validationcloud.io/v1/xycl7T9PGtSJBu8K7pPL9F47SxHgGPJG" ;;
    futurenet) MUX_RPC_URL="https://rpc-futurenet.stellar.org" ;;
    local)     MUX_RPC_URL="http://localhost:8000/soroban/rpc" ;;
  esac
fi

# ── Argument parsing ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --contract) SPECIFIC_CONTRACT="$2"; shift 2 ;;
    --dry-run)  DRY_RUN=true; shift ;;
    --network)  NETWORK="$2"; NETWORK_PASSPHRASE="$(resolve_passphrase "$2")"; shift 2 ;;
    --help|-h)
      grep '^#' "$0" | sed 's/^# //' | sed 's/^#//'
      exit 0 ;;
    *) error "Unknown argument: $1" ;;
  esac
done

# ── Preflight checks ───────────────────────────────────────────────────────────
info "Running preflight checks..."

command -v stellar   >/dev/null 2>&1 || error "'stellar' CLI not found. Install: https://developers.stellar.org/docs/tools/stellar-cli"
command -v cargo     >/dev/null 2>&1 || error "'cargo' not found. Install Rust: https://rustup.rs"
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || error "sha256sum / shasum not found"

[ -z "${MUX_DEPLOYER_SECRET:-}" ] && error "MUX_DEPLOYER_SECRET is not set"
[ -z "${MUX_ADMIN_ADDRESS:-}"   ] && error "MUX_ADMIN_ADDRESS is not set"

success "Preflight checks passed"

# ── Discover contracts ─────────────────────────────────────────────────────────
find_contracts() {
  if [ -n "$SPECIFIC_CONTRACT" ]; then
    echo "$SPECIFIC_CONTRACT"
  elif [ -f "Cargo.toml" ]; then
    find contracts -maxdepth 2 -name "Cargo.toml" -exec dirname {} \; 2>/dev/null \
      | xargs -I{} basename {} \
      | sort
  else
    error "No Cargo.toml found. Run from the repo root."
  fi
}

CONTRACTS=()
while IFS= read -r c; do CONTRACTS+=("$c"); done < <(find_contracts)

[ ${#CONTRACTS[@]} -eq 0 ] && error "No contracts found to deploy"

info "Contracts to deploy: ${CONTRACTS[*]}"
info "Network:    $NETWORK"
info "RPC URL:    $MUX_RPC_URL"
info "Admin:      $MUX_ADMIN_ADDRESS"
info "Dry run:    $DRY_RUN"
echo ""

# ── Build ──────────────────────────────────────────────────────────────────────
info "Building contracts (release, wasm32)..."

if [ "$DRY_RUN" = true ]; then
  warn "[DRY RUN] Would run: cargo build --target wasm32-unknown-unknown --release"
else
  cargo build --target wasm32-unknown-unknown --release 2>&1 \
    | grep -E '(Compiling|Finished|error|warning)' || true
  success "Build complete"
fi

# ── Deploy each contract ───────────────────────────────────────────────────────
DEPLOYMENT_LOG="deployment-$(date +%Y%m%d-%H%M%S).log"
declare -A DEPLOYED_IDS

deploy_contract() {
  local name="$1"
  local wasm_path="$WASM_DIR/${name//-/_}.wasm"

  [ -f "$wasm_path" ] || wasm_path="$WASM_DIR/${name}.wasm"
  [ -f "$wasm_path" ] || { warn "WASM not found for '$name' at $wasm_path — skipping"; return; }

  info "Deploying $name..."
  info "  WASM: $wasm_path"

  if command -v sha256sum >/dev/null 2>&1; then
    WASM_HASH=$(sha256sum "$wasm_path" | awk '{print $1}')
  else
    WASM_HASH=$(shasum -a 256 "$wasm_path" | awk '{print $1}')
  fi
  info "  SHA-256: $WASM_HASH"

  if [ "$DRY_RUN" = true ]; then
    warn "  [DRY RUN] Would upload WASM and deploy $name"
    DEPLOYED_IDS["$name"]="DRY_RUN_ID"
    return
  fi

  info "  Uploading WASM..."
  UPLOADED_HASH=$(stellar contract upload \
    --wasm "$wasm_path" \
    --source-account "$MUX_DEPLOYER_SECRET" \
    --rpc-url "$MUX_RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    2>&1 | tail -1)

  info "  On-chain hash: $UPLOADED_HASH"

  info "  Deploying contract instance..."
  CONTRACT_ID=$(stellar contract deploy \
    --wasm-hash "$UPLOADED_HASH" \
    --source-account "$MUX_DEPLOYER_SECRET" \
    --rpc-url "$MUX_RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    2>&1 | tail -1)

  DEPLOYED_IDS["$name"]="$CONTRACT_ID"
  success "  Deployed $name → $CONTRACT_ID"

  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) | $NETWORK | $name | $CONTRACT_ID | $WASM_HASH" \
    >> "$DEPLOYMENT_LOG"
}

for contract in "${CONTRACTS[@]}"; do
  deploy_contract "$contract"
done

# ── Summary ────────────────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
success "Deployment summary ($NETWORK)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
for contract in "${!DEPLOYED_IDS[@]}"; do
  echo "  $contract → ${DEPLOYED_IDS[$contract]}"
done
echo ""
[ "$DRY_RUN" = false ] && info "Deployment log: $DEPLOYMENT_LOG"
success "Done"
