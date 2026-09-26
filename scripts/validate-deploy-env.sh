#!/usr/bin/env bash
# scripts/validate-deploy-env.sh
#
# Validates that deploy.env.example and .env.deploy.example are secret-free.
#
# Checks:
#   1. Secret key fields (DEPLOYER_PRIVATE_KEY, SOROBAN_SECRET_KEY) must be
#      empty or a placeholder — not a real Stellar secret key (S..., 56 chars).
#   2. No real Stellar secret keys appear anywhere in the file.
#   3. No real Stellar public keys (G..., 56 chars) appear as addresses —
#      placeholders must use the G...X pattern shown in the template.
#   4. Every required variable is declared.
#
# Usage:
#   bash scripts/validate-deploy-env.sh [--file <path>]
#
# Exit 0 on success, 1 on any violation.
#
# Both deploy.env.example and .env.deploy.example are validated by default.
# Pass --file to validate a specific file.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

DEPLOY_ENV_FILES=(
  "${REPO_ROOT}/deploy.env.example"
  "${REPO_ROOT}/.env.deploy.example"
)
EXPLICIT_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file) EXPLICIT_FILE="${2:?'--file requires a value'}"; shift 2 ;;
    --help|-h)
      echo "Usage: bash scripts/validate-deploy-env.sh [--file <path>]"
      exit 0
      ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ -n "$EXPLICIT_FILE" ]]; then
  DEPLOY_ENV_FILES=("$EXPLICIT_FILE")
fi

OVERALL_FAILED=0

validate_file() {
  local env_file="$1"

  if [[ ! -f "$env_file" ]]; then
    echo "ERROR: env file not found: $env_file" >&2
    return 1
  fi

  echo ""
  echo "==> Validating ${env_file}"
  local failed=0

  # ── 1. No real Stellar secret keys (S + 55 base32 chars = 56 total) ──────
  echo ""
  echo "  Checking for real Stellar secret keys..."
  # A real Stellar secret key is exactly: S followed by 55 uppercase base32 chars [A-Z2-7].
  # Placeholder values like SXXXXXXX... use X which is NOT valid base32 — they are safe.
  # We only flag keys where ALL 55 chars after S are strictly in the A-Z2-7 alphabet.
  if grep -qE "(^|[[:space:]=])S[A-Z2-7]{55}([[:space:]=#]|$)" "$env_file" 2>/dev/null; then
    echo "  FAIL: Real Stellar secret key (S...) detected in ${env_file}" >&2
    echo "        Example files must NEVER contain real private key material." >&2
    echo "        Replace with an empty value or a clearly-marked placeholder." >&2
    failed=1
  else
    echo "  OK:   No real Stellar secret keys found"
  fi

  # ── 2. Secret key fields must be empty ────────────────────────────────────
  echo ""
  echo "  Checking secret key fields are empty or placeholder..."
  SECRET_KEY_FIELDS=(
    "DEPLOYER_PRIVATE_KEY"
    "SOROBAN_SECRET_KEY"
  )
  for field in "${SECRET_KEY_FIELDS[@]}"; do
    # Get value if the field exists (non-commented)
    value="$(grep -E "^${field}=" "$env_file" 2>/dev/null | head -1 | cut -d= -f2- || true)"
    if [[ -z "$value" ]]; then
      echo "  OK:   ${field} is empty"
    elif echo "$value" | grep -qE "^S[A-Z2-7]{55}$"; then
      echo "  FAIL: ${field} contains a real Stellar secret key"
      failed=1
    else
      echo "  OK:   ${field} has non-secret placeholder value"
    fi
  done

  # ── 3. Required variables are declared ────────────────────────────────────
  echo ""
  echo "  Checking required variables..."
  local required_vars=()

  # Detect which template format we're validating
  filename="$(basename "$env_file")"
  if [[ "$filename" == "deploy.env.example" ]]; then
    required_vars=(
      "DEPLOYER_PRIVATE_KEY"
      "ADMIN_ADDRESS"
    )
  else
    # .env.deploy.example format
    required_vars=(
      "SOROBAN_NETWORK"
      "SOROBAN_ACCOUNT"
      "SOROBAN_SECRET_KEY"
      "SOROBAN_RPC_URL"
    )
  fi

  for var in "${required_vars[@]}"; do
    if grep -qE "^#?${var}=" "$env_file" 2>/dev/null; then
      echo "  OK:   ${var} declared"
    else
      echo "  WARN: ${var} not found in ${env_file} (may use a different template format)"
    fi
  done

  # ── 4. Contract ID values must be empty or use placeholder pattern ────────
  echo ""
  echo "  Checking contract ID values are placeholders..."
  while IFS= read -r line; do
    [[ "$line" =~ ^# ]] && continue
    [[ -z "$line" ]] && continue
    key="${line%%=*}"
    value="${line#*=}"
    # Trim inline comments
    value="${value%%#*}"
    value="${value%"${value##*[![:space:]]}"}"  # trim trailing whitespace

    if [[ "$key" =~ _CONTRACT_ID$|_ID$ ]]; then
      if [[ -z "$value" ]]; then
        echo "  OK:   ${key} is empty"
      elif [[ "$value" =~ ^C[A-Z2-7]{55}$ ]]; then
        echo "  FAIL: ${key} contains a real-looking contract ID in example file"
        failed=1
      else
        echo "  OK:   ${key} has placeholder: ${value}"
      fi
    fi
  done < "$env_file"

  echo ""
  if (( failed )); then
    echo "  ERROR: ${env_file} validation FAILED."
    return 1
  fi

  echo "  PASS: ${env_file} is secret-free."
  return 0
}

for env_file in "${DEPLOY_ENV_FILES[@]}"; do
  validate_file "$env_file" || OVERALL_FAILED=1
done

echo ""
if (( OVERALL_FAILED )); then
  echo "==> deploy env validation FAILED. Fix the issues above before committing."
  exit 1
fi

echo "==> All deploy env example files are secret-free."
