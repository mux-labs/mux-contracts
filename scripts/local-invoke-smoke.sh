#!/usr/bin/env bash
# local-invoke-smoke.sh
#
# Run simulate-only smoke checks against deployed Mux contracts via the
# local-invoke helper. Useful after a localnet/testnet deploy to confirm
# public read entrypoints are reachable without submitting transactions.
#
# Usage:
#   bash scripts/local-invoke-smoke.sh --secret-key S...
#   bash scripts/local-invoke-smoke.sh --secret-key S... --network testnet
#   bash scripts/local-invoke-smoke.sh --secret-key S... --contract mux-account
#
# Environment:
#   SECRET_KEY / DEPLOYER_PRIVATE_KEY — signer secret (overridden by --secret-key)
#   SOROBAN_NETWORK — default network when --network is omitted
#
# Exit codes:
#   0 — all smoke checks passed (or dry-run listed checks)
#   1 — one or more smoke checks failed
#   2 — usage / dependency error

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INVOKE="${REPO_ROOT}/scripts/local-invoke.sh"

NETWORK="${SOROBAN_NETWORK:-localnet}"
SECRET_KEY="${SECRET_KEY:-${DEPLOYER_PRIVATE_KEY:-}}"
CONTRACT_FILTER=""
DRY_RUN=0
PASS=0
FAIL=0

usage() {
  cat <<'EOF'
Usage: bash scripts/local-invoke-smoke.sh --secret-key S... [options]

Options:
  --secret-key <secret>   Signer secret key (required unless SECRET_KEY is set)
  --network <network>     localnet|testnet|mainnet (default: localnet)
  --contract <name>       Run checks for a single named contract only
  --dry-run               Print planned checks without invoking
  --help                  Show this help

Smoke checks (simulate-only reads):
  mux-account         owner
  mux-batcher         max_batch_size
  mux-permissions     get_pending_admins
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --secret-key)
      SECRET_KEY="${2:-}"
      shift 2
      ;;
    --network)
      NETWORK="${2:-}"
      shift 2
      ;;
    --contract)
      CONTRACT_FILTER="${2:-}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$SECRET_KEY" && "$DRY_RUN" -eq 0 ]]; then
  echo "Error: --secret-key is required (or set SECRET_KEY / DEPLOYER_PRIVATE_KEY)." >&2
  usage >&2
  exit 2
fi

if [[ ! -x "$INVOKE" && ! -f "$INVOKE" ]]; then
  echo "Error: missing local-invoke helper at $INVOKE" >&2
  exit 2
fi

# contract_name|function_name
# Only no-arg public reads that succeed on a healthy initialized deploy.
SMOKE_CHECKS=(
  "mux-account|owner"
  "mux-batcher|max_batch_size"
  "mux-permissions|get_pending_admins"
)

run_check() {
  local contract="$1"
  local function="$2"
  local label="${contract}::${function}"

  if [[ -n "$CONTRACT_FILTER" && "$contract" != "$CONTRACT_FILTER" ]]; then
    return 0
  fi

  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "  PLAN: $label (--simulate-only --network $NETWORK)"
    PASS=$((PASS + 1))
    return 0
  fi

  echo "  RUN:  $label"
  if bash "$INVOKE" \
    --network "$NETWORK" \
    --contract-name "$contract" \
    --function "$function" \
    --secret-key "$SECRET_KEY" \
    --simulate-only; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label"
    FAIL=$((FAIL + 1))
  fi
}

echo "Local invoke smoke checks (network=$NETWORK)..."
for entry in "${SMOKE_CHECKS[@]}"; do
  IFS='|' read -r contract function <<<"$entry"
  run_check "$contract" "$function"
done

echo ""
echo "Smoke summary: $PASS passed, $FAIL failed"
if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
exit 0
