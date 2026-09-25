#!/usr/bin/env bash
# scripts/validate-localnet-env.sh
#
# Validates that .env.localnet.example is complete and secret-free.
#
# Checks:
#   1. Every required variable is declared in the file (key present, any value).
#   2. No real Stellar secret keys (S..., 56 chars) are present.
#   3. No real contract IDs with non-placeholder values appear.
#
# Usage:
#   bash scripts/validate-localnet-env.sh [--file <path>]
#
# Exit 0 on success, 1 on any violation.
#
# The canonical localnet example file is .env.localnet.example.
# Do NOT run this against .env.localnet (the live file) — it may contain
# real contract IDs, which is expected and correct for a deployed localnet.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${REPO_ROOT}/.env.localnet.example"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file) ENV_FILE="${2:?'--file requires a value'}"; shift 2 ;;
    --help|-h)
      echo "Usage: bash scripts/validate-localnet-env.sh [--file <path>]"
      exit 0
      ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: env file not found: $ENV_FILE" >&2
  exit 1
fi

echo "==> Validating ${ENV_FILE}"

FAILED=0

# ── 1. Required variables ────────────────────────────────────────────────────
REQUIRED_VARS=(
  "QUICKSTART_IMAGE_TAG"
  "PROTOCOL_VERSION"
  "NETWORK"
  "LOCALNET_NETWORK_PASSPHRASE"
  "SOROBAN_RPC_PORT"
  "HORIZON_PORT"
  "LOCALNET_RPC_URL"
  "SOROBAN_NETWORK"
  "QUICKSTART_CPUS"
  "QUICKSTART_MEMORY"
  "SOROBAN_DATA_DRIVER"
  "SOROBAN_DATA_SIZE"
  "LOCALNET_MUX_ACCOUNT_ID"
  "LOCALNET_MUX_BATCHER_ID"
  "LOCALNET_MUX_PERMISSIONS_ID"
  "LOCALNET_MUX_SPENDING_POLICY_ID"
  "LOCALNET_MUX_ACCOUNT_FACTORY_ID"
  "LOCALNET_MUX_REGISTRY_ID"
  "LOCALNET_MUX_WALLET_REGISTRY_ID"
  "LOCALNET_MUX_DELEGATION_ID"
  "LOCALNET_MUX_RECOVERY_ID"
  "LOCALNET_MUX_POLICY_ID"
)

echo ""
echo "  Checking required variables..."
for var in "${REQUIRED_VARS[@]}"; do
  # Match: KEY= (empty value) or KEY=value or # KEY= (commented out is OK for optional)
  if grep -qE "^#?${var}=" "$ENV_FILE"; then
    echo "  OK:   ${var} declared"
  else
    echo "  FAIL: ${var} NOT declared in ${ENV_FILE}"
    FAILED=1
  fi
done

# ── 2. No real Stellar secret keys ──────────────────────────────────────────
# Stellar secret keys start with S and are 56 characters (base32 Strkey).
echo ""
echo "  Checking for real Stellar secret keys..."
if grep -qE "^[^#]*=S[A-Z2-7]{55}" "$ENV_FILE" 2>/dev/null; then
  echo "  FAIL: Real Stellar secret key detected in ${ENV_FILE}" >&2
  echo "        Example files must never contain real private key material." >&2
  FAILED=1
else
  echo "  OK:   No real Stellar secret keys found"
fi

# ── 3. Contract ID values must be empty (example files use empty placeholders)
echo ""
echo "  Checking contract ID values are empty placeholders..."
while IFS= read -r line; do
  # Skip comments and blank lines
  [[ "$line" =~ ^# ]] && continue
  [[ -z "$line" ]] && continue

  # Extract key and value
  key="${line%%=*}"
  value="${line#*=}"

  if [[ "$key" == *"_MUX_"*"_ID" ]]; then
    # Value must be empty for example files
    if [[ -n "$value" ]]; then
      # Allow placeholder patterns like <FILL_IN> or example-style values
      if [[ "$value" =~ ^[Cc][A-Z2-7]{55}$ ]]; then
        echo "  FAIL: ${key} has a real-looking contract ID value in example file"
        FAILED=1
      else
        echo "  OK:   ${key} has non-empty placeholder: ${value}"
      fi
    else
      echo "  OK:   ${key} is empty (expected for example file)"
    fi
  fi
done < "$ENV_FILE"

echo ""
if (( FAILED )); then
  echo "ERROR: .env.localnet.example validation failed."
  echo "       See failures above. Fix before committing."
  exit 1
fi

echo "==> .env.localnet.example is complete and secret-free."
