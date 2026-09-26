#!/usr/bin/env bash
# check-changelog-release-artifacts.sh
#
# Guards against docs/#699-class drift: every tagged release section in
# CHANGELOG.md (`## [x.y.z] - YYYY-MM-DD`, i.e. everything except
# `## [Unreleased]`) must include a `### Release Artifacts` subsection
# listing WASM SHA-256 hashes and the bindings package version — see
# .github/CHANGELOG_TEMPLATE.md. Without this, a release entry can't be
# pinned to the exact on-chain bytecode and binding version it ships with.
#
# Usage:
#   bash scripts/check-changelog-release-artifacts.sh
#
# Exit 0 on success (including when there are no tagged releases yet),
# 1 if a tagged release is missing its Release Artifacts subsection.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHANGELOG="${REPO_ROOT}/CHANGELOG.md"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --changelog) CHANGELOG="${2:?'--changelog requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

FAILED=0
CHECKED=0

echo "==> Checking tagged CHANGELOG.md releases have a Release Artifacts section"

# Split into per-release blocks on top-level `## [` headers, skipping Unreleased.
releases="$(awk '
  /^## \[/ {
    if (version != "" && version != "Unreleased") {
      printf "%s\x01%s\x02", version, body
    }
    version = $0
    sub(/^## \[/, "", version)
    sub(/\].*/, "", version)
    body = ""
    next
  }
  { body = body $0 "\n" }
  END {
    if (version != "" && version != "Unreleased") {
      printf "%s\x01%s\x02", version, body
    }
  }
' "$CHANGELOG")"

while IFS=$'\x01' read -r -d $'\x02' version body; do
  [[ -z "$version" ]] && continue
  CHECKED=$((CHECKED + 1))
  if echo "$body" | grep -q '^### Release Artifacts'; then
    echo "  OK:   [$version] has a Release Artifacts section"
  else
    echo "  FAIL: [$version] is missing a ### Release Artifacts section"
    FAILED=1
  fi
done <<< "$releases"

echo ""
if (( CHECKED == 0 )); then
  echo "SKIP: no tagged releases in CHANGELOG.md yet (only Unreleased)."
fi

if (( FAILED )); then
  echo "ERROR: one or more tagged releases in CHANGELOG.md is missing a"
  echo "       ### Release Artifacts section (WASM hashes + bindings version)."
  echo "       See .github/CHANGELOG_TEMPLATE.md."
  exit 1
fi

echo "All checks passed: every tagged release has a Release Artifacts section."
