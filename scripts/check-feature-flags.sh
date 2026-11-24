#!/usr/bin/env bash
#
# check-feature-flags.sh
#
# Check feature flags before executing money-path or mainnet-affecting operations.
# This script implements fail-closed behavior: if a flag is ambiguous or not set,
# the operation is blocked by default.
#
# Usage:
#   bash scripts/check-feature-flags.sh <flag_name>
#
# Arguments:
#   flag_name  Name of the feature flag to check
#
# Environment variables:
#   <FLAG_NAME>  The feature flag value (true/false)
#
# Exit codes:
#   0 - Flag is enabled (safe to proceed)
#   1 - Flag is disabled or not set (operation blocked)
#   2 - Invalid flag name
#
# Examples:
#   # Check if rollback is enabled
#   ENABLE_ROLLBACK=true bash scripts/check-feature-flags.sh ENABLE_ROLLBACK
#
#   # Check if contract upgrade is enabled
#   ENABLE_CONTRACT_UPGRADE=false bash scripts/check-feature-flags.sh ENABLE_CONTRACT_UPGRADE
#
#   # In deploy.sh, before executing money-path operation
#   if ! bash scripts/check-feature-flags.sh ENABLE_ROLLBACK; then
#     echo "Rollback is disabled"
#     exit 1
#   fi

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Valid feature flags
# ─────────────────────────────────────────────────────────────────────────────

VALID_FLAGS=(
  "ENABLE_ROLLBACK"
  "ENABLE_CONTRACT_UPGRADE"
  "ENABLE_NEW_BATCH_SIZE"
  "ENABLE_NEW_POLICY"
  "ENABLE_MONEY_PATH_CHANGE"
)

# ─────────────────────────────────────────────────────────────────────────────
# Colors
# ─────────────────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_info()  { echo -e "${GREEN}[OK]${NC}    $*"; }

# ─────────────────────────────────────────────────────────────────────────────
# Validation
# ─────────────────────────────────────────────────────────────────────────────

validate_flag_name() {
  local flag_name="$1"
  
  for valid_flag in "${VALID_FLAGS[@]}"; do
    if [[ "$flag_name" == "$valid_flag" ]]; then
      return 0
    fi
  done
  
  log_error "Invalid feature flag: $flag_name"
  log_error "Valid flags: ${VALID_FLAGS[*]}"
  return 1
}

# ─────────────────────────────────────────────────────────────────────────────
# Flag checking
# ─────────────────────────────────────────────────────────────────────────────

check_flag() {
  local flag_name="$1"
  local flag_value="${!flag_name:-}"
  
  # Fail-closed: if flag is not set, treat as disabled
  if [[ -z "$flag_value" ]]; then
    log_error "Feature flag $flag_name is not set"
    log_error "Set $flag_name=true to enable this feature"
    return 1
  fi
  
  # Normalize to lowercase
  flag_value=$(echo "$flag_value" | tr '[:upper:]' '[:lower:]')
  
  case "$flag_value" in
    true|1|yes)
      log_info "Feature flag $flag_name is enabled"
      return 0
      ;;
    false|0|no|"")
      log_error "Feature flag $flag_name is disabled"
      log_error "Set $flag_name=true to enable this feature"
      return 1
      ;;
    *)
      log_error "Feature flag $flag_name has invalid value: $flag_value"
      log_error "Valid values: true, false, 1, 0, yes, no"
      return 1
      ;;
  esac
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

main() {
  if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <flag_name>"
    echo ""
    echo "Valid feature flags:"
    for flag in "${VALID_FLAGS[@]}"; do
      echo "  - $flag"
    done
    exit 2
  fi
  
  local flag_name="$1"
  
  if ! validate_flag_name "$flag_name"; then
    exit 2
  fi
  
  if check_flag "$flag_name"; then
    exit 0
  else
    exit 1
  fi
}

main "$@"
