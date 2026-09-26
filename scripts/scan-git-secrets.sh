#!/usr/bin/env bash
# scan-git-secrets.sh
#
# Scan git history for Stellar secret keys and common credential patterns.
# Used in CI to prevent secret leaks; can also be run locally.
#
# Usage:
#   bash scripts/scan-git-secrets.sh
#   bash scripts/scan-git-secrets.sh --depth 100
#   bash scripts/scan-git-secrets.sh --depth 0  # scan all history
#
# Exit codes:
#   0 — no secrets found
#   1 — secrets detected (prints offending commits and files)
#   2 — usage error

set -euo pipefail

DEPTH="${1:-100}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: bash scripts/scan-git-secrets.sh [--depth N]

Options:
  --depth N    Number of commits to scan (default: 100, 0 = all history)
  --help       Show this help

Secret patterns detected:
  - Stellar secret keys: S[A-Z0-9]{55}
  - AWS access keys: AKIA[A-Z0-9]{16}
  - Generic secret vars: SECRET_KEY=, PRIVATE_KEY=, API_KEY=

Exit codes:
  0 — no secrets found
  1 — secrets detected
  2 — usage error
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ "${1:-}" == "--depth" ]]; then
  DEPTH="${2:-100}"
fi

cd "$REPO_ROOT"

echo "Scanning git history for leaked secrets (depth: ${DEPTH})..."
echo ""

# Patterns to detect
PATTERNS=(
  'S[A-Z0-9]{55}'                    # Stellar secret key
  'AKIA[A-Z0-9]{16}'                 # AWS access key
  'SECRET_KEY\s*=\s*["\047][^"\047]+["\047]'  # SECRET_KEY="..."
  'PRIVATE_KEY\s*=\s*["\047][^"\047]+["\047]' # PRIVATE_KEY="..."
  'DEPLOYER_PRIVATE_KEY\s*=\s*["\047]S[A-Z0-9]{55}["\047]' # DEPLOYER_PRIVATE_KEY="S..."
  'API_KEY\s*=\s*["\047][^"\047]+["\047]'     # API_KEY="..."
)

FOUND=0

for pattern in "${PATTERNS[@]}"; do
  echo "Checking pattern: $pattern"
  
  if [[ "$DEPTH" -eq 0 ]]; then
    # Scan all history
    MATCHES=$(git log --all -p --pretty=format:"%H %an %ae %s" -G"$pattern" | grep -E "$pattern" || true)
  else
    # Scan last N commits
    MATCHES=$(git log -p --pretty=format:"%H %an %ae %s" -"$DEPTH" -G"$pattern" | grep -E "$pattern" || true)
  fi
  
  if [[ -n "$MATCHES" ]]; then
    echo "  ✗ FOUND SECRETS matching pattern: $pattern"
    echo "$MATCHES" | head -20
    echo ""
    FOUND=1
  else
    echo "  ✓ No matches for pattern: $pattern"
  fi
done

echo ""
if [[ "$FOUND" -eq 1 ]]; then
  echo "=========================================="
  echo "ERROR: Secrets detected in git history!"
  echo "=========================================="
  echo ""
  echo "Action required:"
  echo "1. Identify the commits containing secrets (see output above)"
  echo "2. Follow the remediation procedure in docs/deployer-key-security-playbook.md"
  echo "3. Rotate any compromised keys immediately"
  echo ""
  exit 1
fi

echo "=========================================="
echo "✓ No secrets detected in git history"
echo "=========================================="
exit 0
