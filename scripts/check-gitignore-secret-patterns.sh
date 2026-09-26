#!/usr/bin/env bash
# check-gitignore-secret-patterns.sh
#
# Guards the secret-handling checklists in docs/deployer-key-requirements.md
# and docs/funded-deployer-key.md: certain locally-generated, secret-bearing
# filenames must never be trackable by git (e.g. `deployment.env`, which
# scripts/deploy.sh writes contract IDs to on every deploy). Rather than
# grep .gitignore for literal lines — which breaks silently if the pattern
# syntax changes — this asks git itself whether a sample file with each
# name would be ignored.
#
# Usage:
#   bash scripts/check-gitignore-secret-patterns.sh [--gitignore <path>]
#
# Exit 0 if every required filename is git-ignored, 1 otherwise.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GITIGNORE="${REPO_ROOT}/.gitignore"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --gitignore) GITIGNORE="${2:?'--gitignore requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ ! -f "$GITIGNORE" ]]; then
  echo "ERROR: gitignore file not found: $GITIGNORE" >&2
  exit 1
fi

# Sample filenames that scripts/deploy.sh writes, or that the security
# checklists in docs/deployer-key-requirements.md and
# docs/funded-deployer-key.md require to be excluded from version control.
REQUIRED_IGNORED=(
  ".env"
  "example.secret"
  "deployment.env"
  "deployer.json"
)

echo "==> Checking that secret-bearing filenames are git-ignored (via ${GITIGNORE})"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
git -C "$WORKDIR" init -q
cp "$GITIGNORE" "$WORKDIR/.gitignore"

FAILED=0
for name in "${REQUIRED_IGNORED[@]}"; do
  touch "$WORKDIR/$name"
  if git -C "$WORKDIR" check-ignore -q "$name"; then
    echo "  OK:   $name is git-ignored"
  else
    echo "  FAIL: $name is NOT git-ignored"
    FAILED=1
  fi
done

echo ""
if (( FAILED )); then
  echo "ERROR: one or more required filenames are not covered by .gitignore."
  echo "       See the secret-handling checklists in"
  echo "       docs/deployer-key-requirements.md and docs/funded-deployer-key.md."
  exit 1
fi

echo "All required filenames are git-ignored."
