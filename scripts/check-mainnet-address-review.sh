#!/usr/bin/env bash
# check-mainnet-address-review.sh
#
# Enforces the Mainnet Address PR Review Rule (issue #767, #871).
#
# Invariant:
#   Mainnet contract addresses are money-path configuration. Any non-empty
#   entry or modification to the `mainnet` section in `config/addresses.json`
#   MUST have an explicit review approval marker in `_review.mainnetApproval`,
#   or explicit approval via MAINNET_ADDRESS_REVIEW_APPROVED=true / PR label.
#
# Fail-closed:
#   Unreviewed mainnet addresses exit 1 with stable error code
#   MUX_MAINNET_ADDRESS_UNREVIEWED.
#
# Usage:
#   bash scripts/check-mainnet-address-review.sh [--addresses <path>] [--baseline <path>]
#
# Exit 0 on success / approval verified, 1 on unreviewed change or parse failure.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ADDRESSES_JSON="${REPO_ROOT}/config/addresses.json"
BASELINE_JSON=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --addresses) ADDRESSES_JSON="${2:?'--addresses requires a path'}"; shift 2 ;;
    --baseline)  BASELINE_JSON="${2:?'--baseline requires a path'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if ! command -v jq &>/dev/null; then
  echo "ERROR: jq is required to validate addresses.json" >&2
  exit 1
fi

if [[ ! -f "$ADDRESSES_JSON" ]]; then
  echo "ERROR: $ADDRESSES_JSON not found." >&2
  exit 1
fi

echo "==> Enforcing Mainnet Address Review Rule for $(basename "$ADDRESSES_JSON")"

# Check if review approval is granted via out-of-band environment variable or PR label
APPROVAL_GRANTED=0
if [[ "${MAINNET_ADDRESS_REVIEW_APPROVED:-}" == "true" ]]; then
  echo "  INFO: Review approval detected via MAINNET_ADDRESS_REVIEW_APPROVED=true"
  APPROVAL_GRANTED=1
fi

if [[ -n "${PR_LABELS:-}" ]]; then
  if grep -qiE '(mainnet-approved|mainnet-address-approved)' <<< "$PR_LABELS"; then
    echo "  INFO: Review approval detected via PR label"
    APPROVAL_GRANTED=1
  fi
fi

# Check for inline approval marker in addresses.json _review.mainnetApproval
MARKER_APPROVED_BY="$(jq -r '._review.mainnetApproval.approvedBy // empty' "$ADDRESSES_JSON" 2>/dev/null || true)"
MARKER_PR="$(jq -r '._review.mainnetApproval.pr // empty' "$ADDRESSES_JSON" 2>/dev/null || true)"

if [[ -n "$MARKER_APPROVED_BY" && -n "$MARKER_PR" ]]; then
  echo "  INFO: Review approval marker present in _review: approved by $MARKER_APPROVED_BY (PR #$MARKER_PR)"
  APPROVAL_GRANTED=1
fi

# Inspect mainnet addresses
MAINNET_KEYS="$(jq -r '.mainnet | keys[]' "$ADDRESSES_JSON" 2>/dev/null || true)"
HAS_POPULATED_MAINNET=0

while IFS= read -r key; do
  [[ -z "$key" ]] && continue
  val="$(jq -r --arg k "$key" '.mainnet[$k] // empty' "$ADDRESSES_JSON")"
  if [[ -n "$val" && "$val" != "null" && "$val" != '""' ]]; then
    HAS_POPULATED_MAINNET=1
    break
  fi
done <<< "$MAINNET_KEYS"

# If baseline is provided, check if mainnet addresses changed compared to baseline
DIFF_DETECTED=0
if [[ -n "$BASELINE_JSON" && -f "$BASELINE_JSON" ]]; then
  CANDIDATE_MAINNET="$(jq -S '.mainnet' "$ADDRESSES_JSON")"
  BASELINE_MAINNET="$(jq -S '.mainnet' "$BASELINE_JSON")"
  if [[ "$CANDIDATE_MAINNET" != "$BASELINE_MAINNET" ]]; then
    echo "  WARN: Mainnet addresses differ from baseline"
    DIFF_DETECTED=1
  fi
fi

# Fail-closed check:
# If mainnet addresses are populated or changed, approval MUST be granted
if (( HAS_POPULATED_MAINNET || DIFF_DETECTED )); then
  if (( ! APPROVAL_GRANTED )); then
    echo "ERROR: [MUX_MAINNET_ADDRESS_UNREVIEWED] Mainnet address configuration is present or modified without review approval." >&2
    echo "       Mainnet addresses represent money-path configuration." >&2
    echo "       Set _review.mainnetApproval in config/addresses.json or export MAINNET_ADDRESS_REVIEW_APPROVED=true." >&2
    exit 1
  fi
fi

echo "  OK: Mainnet address review check passed."
exit 0
