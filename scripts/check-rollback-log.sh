#!/usr/bin/env bash
# check-rollback-log.sh
#
# Guards the completion-tracking gap in docs/rollback-deploy.md: the
# "Pre-Rollback Checklist" and "Post-Rollback Steps" there were documented
# templates with no record of whether they were ever actually completed.
# Every entry in ops/rollback-log.md (a "## <scope> - <YYYY-MM-DD>" header)
# must record all required fields, and both checklist confirmations must be
# checked ([x], not [ ]).
#
# Usage:
#   bash scripts/check-rollback-log.sh [--log <path>]
#
# Exit 0 on success (including when the log has no entries yet), 1 if an
# entry is missing a required field or has an unchecked checklist box.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="${REPO_ROOT}/ops/rollback-log.md"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --log) LOG="${2:?'--log requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ ! -f "$LOG" ]]; then
  echo "ERROR: log file not found: $LOG" >&2
  exit 1
fi

echo "==> Checking rollback log entries (${LOG})"

# Only real entries match a literal YYYY-MM-DD date, so the template block's
# "YYYY-MM-DD" placeholder text is never mistaken for an entry (the
# placeholder itself is literal letters, not digits, so it can't match).
entries="$(awk '
  /^## .+ - [0-9]{4}-[0-9]{2}-[0-9]{2}$/ {
    if (header != "") { printf "%s\x01%s\x02", header, body }
    header = $0
    body = ""
    next
  }
  { if (header != "") body = body $0 "\n" }
  END { if (header != "") printf "%s\x01%s\x02", header, body }
' "$LOG")"

REQUIRED_FIELDS=(
  "Strategy used:"
  "Pre-Rollback Checklist completed:"
  "Post-Rollback Steps completed:"
  "Incident report:"
  "Follow-up issue:"
)

CHECKED=0
FAILED=0

while IFS=$'\x01' read -r -d $'\x02' header body; do
  [[ -z "$header" ]] && continue
  CHECKED=$((CHECKED + 1))
  entry_ok=1
  for field in "${REQUIRED_FIELDS[@]}"; do
    if ! grep -qF "$field" <<< "$body"; then
      echo "  FAIL: ${header#\#\# } is missing field '${field}'"
      entry_ok=0
    fi
  done
  if grep -qE '(Pre-Rollback Checklist completed|Post-Rollback Steps completed):[[:space:]]*\[ \]' <<< "$body"; then
    echo "  FAIL: ${header#\#\# } has an unchecked checklist confirmation"
    entry_ok=0
  fi
  if (( entry_ok )); then
    echo "  OK:   ${header#\#\# }"
  fi
  FAILED=$(( FAILED || !entry_ok ))
done <<< "$entries"

echo ""
if (( CHECKED == 0 )); then
  echo "SKIP: no rollback-log entries yet."
fi

if (( FAILED )); then
  echo "ERROR: one or more rollback-log entries are incomplete or unchecked."
  echo "       See ops/rollback-log.md for the required template."
  exit 1
fi

echo "All rollback-log entries are complete."
