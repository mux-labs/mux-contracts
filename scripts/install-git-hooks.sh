#!/usr/bin/env bash
# install-git-hooks.sh
#
# Install git hooks for the Mux contracts repository.
# Currently installs: pre-commit secret key scanner
#
# Usage:
#   bash scripts/install-git-hooks.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"
SCRIPTS_DIR="$REPO_ROOT/scripts"

echo "Installing git hooks..."

# Check if .git directory exists
if [[ ! -d "$REPO_ROOT/.git" ]]; then
  echo "Error: .git directory not found. Are you in a git repository?" >&2
  exit 1
fi

# Install pre-commit hook
PRE_COMMIT_SOURCE="$SCRIPTS_DIR/pre-commit-key-scan.sh"
PRE_COMMIT_TARGET="$HOOKS_DIR/pre-commit"

if [[ ! -f "$PRE_COMMIT_SOURCE" ]]; then
  echo "Error: pre-commit script not found at $PRE_COMMIT_SOURCE" >&2
  exit 1
fi

# Backup existing pre-commit hook if it exists
if [[ -f "$PRE_COMMIT_TARGET" ]]; then
  echo "  Backing up existing pre-commit hook..."
  cp "$PRE_COMMIT_TARGET" "$PRE_COMMIT_TARGET.backup.$(date +%Y%m%d%H%M%S)"
fi

# Install new pre-commit hook
echo "  Installing pre-commit secret scanner..."
cp "$PRE_COMMIT_SOURCE" "$PRE_COMMIT_TARGET"
chmod +x "$PRE_COMMIT_TARGET"

echo ""
echo "✓ Git hooks installed successfully"
echo ""
echo "Installed hooks:"
echo "  - pre-commit: Scans for secret keys in staged files"
echo ""
echo "To bypass the hook (use with caution):"
echo "  git commit --no-verify -m \"message\""
echo ""
echo "To test the hook:"
echo "  echo 'SECRET=SXXXXX...' > test.txt"
echo "  git add test.txt"
echo "  git commit -m 'test'"
echo "  # Should block the commit"
echo ""
