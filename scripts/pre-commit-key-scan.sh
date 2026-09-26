#!/usr/bin/env bash
# pre-commit-key-scan.sh
#
# Git pre-commit hook that scans staged files for secret keys.
# Blocks commits containing Stellar secret keys or common credential patterns.
#
# Installation:
#   bash scripts/install-git-hooks.sh
#   # or manually:
#   cp scripts/pre-commit-key-scan.sh .git/hooks/pre-commit
#   chmod +x .git/hooks/pre-commit
#
# Bypass (use with caution):
#   git commit --no-verify -m "message"
#
# Exit codes:
#   0 — no secrets found, commit allowed
#   1 — secrets detected, commit blocked

set -euo pipefail

# Secret patterns to detect
STELLAR_SECRET_PATTERN='S[A-Z0-9]{55}'
AWS_KEY_PATTERN='AKIA[A-Z0-9]{16}'
GENERIC_SECRET_PATTERNS=(
  'SECRET_KEY\s*=.*S[A-Z0-9]{55}'
  'PRIVATE_KEY\s*=.*S[A-Z0-9]{55}'
  'DEPLOYER_PRIVATE_KEY\s*=.*S[A-Z0-9]{55}'
  'API_KEY\s*=\s*["\047][A-Za-z0-9+/=]{20,}["\047]'
)

echo "Running pre-commit secret scan..."

# Get list of staged files
STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM)

if [[ -z "$STAGED_FILES" ]]; then
  echo "  No staged files to scan"
  exit 0
fi

FOUND=0

# Scan for Stellar secret keys
for file in $STAGED_FILES; do
  if [[ -f "$file" ]]; then
    # Check for Stellar secret keys (S followed by 55 base32 chars)
    if git diff --cached "$file" | grep -qE "$STELLAR_SECRET_PATTERN"; then
      echo "  ✗ ERROR: Potential Stellar secret key detected in: $file"
      echo "    Pattern: $STELLAR_SECRET_PATTERN"
      FOUND=1
    fi
    
    # Check for AWS access keys
    if git diff --cached "$file" | grep -qE "$AWS_KEY_PATTERN"; then
      echo "  ✗ ERROR: Potential AWS access key detected in: $file"
      echo "    Pattern: $AWS_KEY_PATTERN"
      FOUND=1
    fi
    
    # Check for generic secret patterns
    for pattern in "${GENERIC_SECRET_PATTERNS[@]}"; do
      if git diff --cached "$file" | grep -qE "$pattern"; then
        echo "  ✗ ERROR: Potential secret detected in: $file"
        echo "    Pattern: $pattern"
        FOUND=1
      fi
    done
  fi
done

if [[ "$FOUND" -eq 1 ]]; then
  echo ""
  echo "=========================================="
  echo "COMMIT BLOCKED: Secrets detected!"
  echo "=========================================="
  echo ""
  echo "Action required:"
  echo "1. Remove the secret from the staged file(s)"
  echo "2. Use environment variables instead (see docs/deployer-key-security-playbook.md)"
  echo "3. If this is a false positive (e.g., example file with placeholder):"
  echo "   - Use a clearly fake placeholder (SXXX...XXX)"
  echo "   - Or bypass with: git commit --no-verify"
  echo ""
  echo "NEVER commit real secret keys to version control!"
  echo ""
  exit 1
fi

echo "  ✓ No secrets detected in staged files"
exit 0
