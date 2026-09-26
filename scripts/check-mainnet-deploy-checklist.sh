#!/usr/bin/env bash
# check-mainnet-deploy-checklist.sh
#
# Automated fail-closed verification script for docs/MAINNET_DEPLOY_CHECKLIST.md (issue #769).
#
# Verifies all automated Phase 1 Pre-Deploy checklist items:
#   1. Clean git working tree (uncommitted files block mainnet deploy)
#   2. Mainnet network configuration safety (passphrase, RPC, friendbot)
#   3. Mainnet address review rule compliance
#   4. Deployer key rotation log integrity
#   5. Rollback log completion
#   6. Deploy secret name consistency (.github/workflows/deploy.yml vs deploy.sh)
#   7. Git-ignored secret patterns (.env, deployment.env, etc.)
#   8. Compiler release profile verification
#   9. No testutils in production artifacts
#  10. Contract IDs sync across docs and addresses.json
#  11. Immutable mainnet deploy flag (when --enforce-flag is set)
#
# Usage:
#   bash scripts/check-mainnet-deploy-checklist.sh [OPTIONS]
#
# Options:
#   --enforce-flag     Enforce that MUX_MAINNET_DEPLOY_FLAG=I_ACKNOWLEDGE_MAINNET_DEPLOY
#   --skip-git-clean   Skip git working tree cleanliness check (for dry-run/testing)
#   --help, -h         Show this help message
#
# Exit codes:
#   0 - All checklist checks passed
#   1 - One or more checklist verification checks failed (fail-closed)
#   2 - Usage error

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENFORCE_FLAG=0
SKIP_GIT_CLEAN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --enforce-flag)
      ENFORCE_FLAG=1
      shift
      ;;
    --skip-git-clean)
      SKIP_GIT_CLEAN=1
      shift
      ;;
    --help|-h)
      sed -n '2,24p' "$0" | sed 's/^# //' | sed 's/^#//'
      exit 0
      ;;
    *)
      echo "ERROR: Unknown option: $1" >&2
      exit 2
      ;;
  esac
done

echo "========================================================================"
echo " Mux Protocol — Mainnet Deploy Checklist Verification (Fail-Closed)"
echo "========================================================================"
echo ""

FAILED=0

fail_step() {
  echo "  [FAIL] $1" >&2
  FAILED=1
}

pass_step() {
  echo "  [PASS] $1"
}

# 1. Clean working tree check
echo "==> Step 1: Checking git working tree cleanliness..."
if (( SKIP_GIT_CLEAN )); then
  echo "  [SKIP] Working tree cleanliness check skipped (--skip-git-clean)"
else
  if [[ -n "$(git -C "$REPO_ROOT" status --porcelain 2>/dev/null || true)" ]]; then
    fail_step "Working directory has uncommitted changes. Mainnet deployment requires a clean git status."
  else
    pass_step "Working directory is clean"
  fi
fi

# 2. Mainnet network configuration safety
echo "==> Step 2: Validating mainnet network configuration..."
source "${REPO_ROOT}/scripts/network-config.sh"
if load_network_config "mainnet" >/dev/null 2>&1; then
  if [[ "$NETWORK_PASSPHRASE" != "Public Global Stellar Network ; September 2015" ]]; then
    fail_step "Mainnet passphrase invalid: $NETWORK_PASSPHRASE"
  elif [[ -n "$NETWORK_FRIENDBOT_URL" ]]; then
    fail_step "Friendbot URL must be empty for mainnet"
  elif [[ "$NETWORK_RPC_URL" =~ (xycl|key=|secret|token) ]]; then
    fail_step "Mainnet RPC URL appears to contain private key or token material"
  else
    pass_step "Mainnet network configuration is secure ($NETWORK_RPC_URL)"
  fi
else
  fail_step "Failed to load mainnet configuration from config/networks.toml"
fi

# 3. Mainnet address review rule
echo "==> Step 3: Checking mainnet address review rule..."
if bash "${REPO_ROOT}/scripts/check-mainnet-address-review.sh" >/dev/null 2>&1; then
  pass_step "Mainnet address review rule satisfied"
else
  fail_step "Mainnet address review rule check failed (check-mainnet-address-review.sh)"
fi

# 4. Deployer key rotation log
echo "==> Step 4: Checking deployer key rotation log..."
if bash "${REPO_ROOT}/scripts/check-deployer-key-rotation-log.sh" >/dev/null 2>&1; then
  pass_step "Deployer key rotation log is complete and verified"
else
  fail_step "Deployer key rotation log check failed (check-deployer-key-rotation-log.sh)"
fi

# 5. Rollback log completion
echo "==> Step 5: Checking rollback execution log..."
if bash "${REPO_ROOT}/scripts/check-rollback-log.sh" >/dev/null 2>&1; then
  pass_step "Rollback log is valid"
else
  fail_step "Rollback log check failed (check-rollback-log.sh)"
fi

# 6. Deploy secret name consistency
echo "==> Step 6: Checking deploy secret name consistency..."
if bash "${REPO_ROOT}/scripts/check-deploy-secret-name.sh" >/dev/null 2>&1; then
  pass_step "Deploy secret name consistency verified"
else
  fail_step "Deploy secret name check failed (check-deploy-secret-name.sh)"
fi

# 7. Git-ignored secret patterns
echo "==> Step 7: Checking gitignore secret patterns..."
if bash "${REPO_ROOT}/scripts/check-gitignore-secret-patterns.sh" >/dev/null 2>&1; then
  pass_step "Secret patterns properly ignored by git"
else
  fail_step "Gitignore secret patterns check failed (check-gitignore-secret-patterns.sh)"
fi

# 8. Release profile invariants
echo "==> Step 8: Checking compiler release profile..."
if bash "${REPO_ROOT}/scripts/check-release-profile.sh" >/dev/null 2>&1; then
  pass_step "Cargo release profile invariants verified"
else
  fail_step "Release profile check failed (check-release-profile.sh)"
fi

# 9. No testutils in release wasm
echo "==> Step 9: Checking for forbidden testutils..."
if bash "${REPO_ROOT}/scripts/check-no-testutils.sh" >/dev/null 2>&1; then
  pass_step "No testutils detected in release configuration"
else
  fail_step "Forbidden testutils detected (check-no-testutils.sh)"
fi

# 10. Contract IDs sync
echo "==> Step 10: Checking contract IDs synchronization..."
if bash "${REPO_ROOT}/scripts/check-contract-ids-sync.sh" >/dev/null 2>&1; then
  pass_step "Contract IDs synchronized across docs and config"
else
  fail_step "Contract IDs sync check failed (check-contract-ids-sync.sh)"
fi

# 11. Immutable mainnet deploy flag
if (( ENFORCE_FLAG )); then
  echo "==> Step 11: Enforcing immutable mainnet deploy flag..."
  FLAG_VAL="${MUX_MAINNET_DEPLOY_FLAG:-}"
  if [[ "$FLAG_VAL" == "I_ACKNOWLEDGE_MAINNET_DEPLOY" ]]; then
    pass_step "Immutable mainnet deploy flag present and verified"
  else
    fail_step "MUX_MAINNET_DEPLOY_FLAG must equal 'I_ACKNOWLEDGE_MAINNET_DEPLOY' (got: '$FLAG_VAL')"
  fi
fi

echo ""
if (( FAILED )); then
  echo "========================================================================"
  echo " ERROR: Mainnet deploy checklist verification FAILED."
  echo "        Address all failing checks above before deploying to mainnet."
  echo "========================================================================"
  exit 1
fi

echo "========================================================================"
echo " SUCCESS: All mainnet deploy checklist verification checks passed."
echo "========================================================================"
exit 0
