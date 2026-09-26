#!/usr/bin/env bash
# check-changelog.sh
#
# Validates CHANGELOG.md adherence to Keep a Changelog format:
# 1. CHANGELOG.md must exist and include an '## [Unreleased]' section.
# 2. Subsections must be one of the canonical categories:
#    Added, Changed, Deprecated, Removed, Fixed, Security, Release Artifacts.
# 3. No duplicate subsection headers within any release section.
# 4. Release artifacts validation passes for all tagged releases.
#
# Usage:
#   bash scripts/check-changelog.sh [--changelog <path>]
#
# Exit 0 on success, 1 on validation error.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHANGELOG="${REPO_ROOT}/CHANGELOG.md"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --changelog) CHANGELOG="${2:?'--changelog requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ ! -f "$CHANGELOG" ]]; then
  echo "ERROR: changelog file not found: $CHANGELOG" >&2
  exit 1
fi

echo "==> Validating changelog format (${CHANGELOG})"

# Ensure ## [Unreleased] exists
if ! grep -q '^## \[Unreleased\]' "$CHANGELOG"; then
  echo "ERROR: Missing '## [Unreleased]' section in ${CHANGELOG}" >&2
  exit 1
fi

# Check for canonical section headers and duplicate subsections within releases
python3 - "$CHANGELOG" <<'PYEOF'
import sys, re

changelog_path = sys.argv[1]
with open(changelog_path, 'r', encoding='utf-8') as f:
    lines = f.readlines()

valid_subsections = {
    "Added", "Changed", "Deprecated", "Removed", "Fixed", "Security", "Release Artifacts"
}

current_release = None
seen_subsections = set()
errors = []

for idx, line in enumerate(lines, 1):
    line_clean = line.strip()
    if line_clean.startswith("## ["):
        current_release = line_clean
        seen_subsections = set()
    elif line_clean.startswith("### "):
        subsection = line_clean[4:].strip()
        if subsection not in valid_subsections:
            errors.append(f"Line {idx}: Invalid subsection heading '{subsection}' in {current_release}")
        if subsection in seen_subsections:
            errors.append(f"Line {idx}: Duplicate subsection '{subsection}' in {current_release}")
        seen_subsections.add(subsection)

if errors:
    for err in errors:
        print(f"ERROR: {err}", file=sys.stderr)
    sys.exit(1)
print("Changelog structure and subsections validated successfully.")
PYEOF

# Verify release artifacts if tagged releases exist
if [[ -f "${REPO_ROOT}/scripts/check-changelog-release-artifacts.sh" ]]; then
  bash "${REPO_ROOT}/scripts/check-changelog-release-artifacts.sh" --changelog "$CHANGELOG"
fi

echo "All changelog automation checks passed."
