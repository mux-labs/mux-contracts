#!/usr/bin/env bash
# check-deployer-key-rotation-log.sh
#
# Guards the "deployer key is drained and rotated after each mainnet
# deployment cycle" checklist item in docs/deployer-key-requirements.md and
# docs/funded-deployer-key.md. Every entry in ops/deployer-key-rotation-log.md
# (a "## <network> - <YYYY-MM-DD>" header) must record all required fields,
# and the drain/archive confirmations must be checked ([x], not [ ]).
#
# Usage:
#   bash scripts/check-deployer-key-rotation-log.sh [--log <path>]
#
# Exit 0 on success (including when the log has no entries yet), 1 if an
# entry is missing a required field or has an unchecked confirmation box.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="${REPO_ROOT}/ops/deployer-key-rotation-log.md"

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

echo "==> Checking deployer-key rotation log entries (${LOG})"

# Only real entries match a literal YYYY-MM-DD date, so the template block's
# "<YYYY-MM-DD>" placeholder is never mistaken for an entry.
entries="$(awk '
  /^## [A-Za-z]+ - [0-9]{4}-[0-9]{2}-[0-9]{2}$/ {
    if (header != "") { printf "%s\x01%s\x02", header, body }
    header = $0
    body = ""
    next
  }
  { if (header != "") body = body $0 "\n" }
  END { if (header != "") printf "%s\x01%s\x02", header, body }
' "$LOG")"

REQUIRED_FIELDS=(
  "Deploy run:"
  "Deployer public key:"
  "Drained to treasury:"
  "Old key archived/revoked in secrets manager:"
  "Verified by:"
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
  if grep -qE '(Drained to treasury|Old key archived/revoked in secrets manager):[[:space:]]*\[ \]' <<< "$body"; then
    echo "  FAIL: ${header#\#\# } has an unchecked drain/archive confirmation"
    entry_ok=0
  fi
  if (( entry_ok )); then
    echo "  OK:   ${header#\#\# }"
  fi
  FAILED=$(( FAILED || !entry_ok ))
done <<< "$entries"

echo ""
if (( CHECKED == 0 )); then
  echo "SKIP: no rotation-log entries yet."
fi

if (( FAILED )); then
  echo "ERROR: one or more rotation-log entries are incomplete or unchecked."
  echo "       See ops/deployer-key-rotation-log.md for the required template."
  exit 1
fi

echo "All rotation-log entries are complete."
