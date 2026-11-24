#!/usr/bin/env bash
# Validate the repository's ABI breaking-change process.
# Usage: bash scripts/check-breaking-change.sh [--changelog FILE] [--migration FILE] [--require-ack]
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHANGELOG="$ROOT/CHANGELOG.md"
MIGRATION="$ROOT/docs/abi-breaking-change-checklist.md"
REQUIRE_ACK=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --changelog) CHANGELOG="${2:?--changelog requires a file}"; shift 2 ;;
    --migration) MIGRATION="${2:?--migration requires a file}"; shift 2 ;;
    --require-ack) REQUIRE_ACK=true; shift ;;
    --help|-h) echo "Usage: $0 [--changelog FILE] [--migration FILE]"; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done
fail=0
require_file() {
  if [[ ! -s "$1" ]]; then echo "FAIL: required file is missing or empty: $1" >&2; fail=1; else echo "PASS: file exists: ${1#$ROOT/}"; fi
}
require_text() {
  local file="$1" pattern="$2" label="$3"
  if grep -Fq -- "$pattern" "$file"; then echo "PASS: $label"; else echo "FAIL: $label" >&2; fail=1; fi
}
require_file "$CHANGELOG"
require_file "$MIGRATION"
if [[ -s "$CHANGELOG" ]]; then
  require_text "$CHANGELOG" "## [Unreleased]" "CHANGELOG has an Unreleased section"
  require_text "$CHANGELOG" "### ⚠️ BREAKING CHANGES" "CHANGELOG has a breaking-change subsection"
  require_text "$CHANGELOG" "BREAKING CHANGE:" "CHANGELOG records the release/PR marker"
fi
if [[ -s "$MIGRATION" ]]; then
  for heading in "## Classification" "## Compatibility and migration" "## Security gates" "## Rollback" "## Review checklist"; do
    require_text "$MIGRATION" "$heading" "migration guide contains $heading"
  done
  require_text "$MIGRATION" "BREAKING_CHANGE_ACK" "migration guide defines an explicit acknowledgement gate"
  require_text "$MIGRATION" "fail closed" "migration guide requires fail-closed behavior"
fi
if [[ "$REQUIRE_ACK" == "true" ]]; then
  if [[ "${BREAKING_CHANGE_ACK:-}" == "1" ]]; then
    echo "PASS: BREAKING_CHANGE_ACK=1 release acknowledgement"
  else
    echo "FAIL: BREAKING_CHANGE_ACK=1 is required in acknowledgement mode" >&2
    fail=1
  fi
fi
if (( fail )); then
  echo "ABI breaking-change process validation failed." >&2
  exit 1
fi
echo "ABI breaking-change process validation passed."
