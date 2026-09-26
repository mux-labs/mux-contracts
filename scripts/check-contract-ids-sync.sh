#!/usr/bin/env bash
# check-contract-ids-sync.sh
#
# Guards against #674-class drift: CONTRACT_IDS.md, docs/contract-ids.md,
# and config/addresses.json must all track the same set of contract keys.
# The canonical set is `bindings/src/types.ts`'s `MuxContractIds` interface
# — the TypeScript type that `bindings/src/addresses.ts` actually validates
# against and that client code consumes, so it is the closest thing this
# repo has to a single source of truth for "which contracts get an address
# entry".
#
# Checks:
#   1. Every key in MuxContractIds appears in config/addresses.json for
#      every network (localnet, testnet, mainnet) — no more, no less.
#   2. Every key in MuxContractIds is mentioned somewhere in CONTRACT_IDS.md
#      (the root overview doc).
#   3. Every key in MuxContractIds is mentioned somewhere in
#      docs/contract-ids.md (the detailed companion doc).
#
# Usage:
#   bash scripts/check-contract-ids-sync.sh
#
# Exit 0 on success, 1 on drift. Requires `jq`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TYPES_TS="${REPO_ROOT}/bindings/src/types.ts"
ADDRESSES_JSON="${REPO_ROOT}/config/addresses.json"
ROOT_DOC="${REPO_ROOT}/CONTRACT_IDS.md"
DETAIL_DOC="${REPO_ROOT}/docs/contract-ids.md"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --types) TYPES_TS="${2:?'--types requires a value'}"; shift 2 ;;
    --addresses) ADDRESSES_JSON="${2:?'--addresses requires a value'}"; shift 2 ;;
    --root-doc) ROOT_DOC="${2:?'--root-doc requires a value'}"; shift 2 ;;
    --detail-doc) DETAIL_DOC="${2:?'--detail-doc requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if ! command -v jq &>/dev/null; then
  echo "ERROR: jq is required but not installed." >&2
  exit 1
fi

FAILED=0

if [[ ! -f "$TYPES_TS" ]]; then
  echo "ERROR: $TYPES_TS not found." >&2
  exit 1
fi

# Extract the canonical key list from the MuxContractIds interface body,
# e.g. "  muxAccountFactory?: string;" -> "muxAccountFactory".
mapfile -t CANONICAL_KEYS < <(
  awk '/interface MuxContractIds/{p=1; next} p && /^}/{exit} p' "$TYPES_TS" \
    | grep -oE '^\s*mux[A-Za-z]+' \
    | sed -E 's/^\s*//'
)

if [[ "${#CANONICAL_KEYS[@]}" -eq 0 ]]; then
  echo "ERROR: found no keys in MuxContractIds — check $TYPES_TS." >&2
  exit 1
fi

echo "==> Canonical contract keys (from $(basename "$TYPES_TS")): ${CANONICAL_KEYS[*]}"
echo ""

echo "==> Checking $(basename "$ADDRESSES_JSON") against the canonical key set"
if [[ ! -f "$ADDRESSES_JSON" ]]; then
  echo "  FAIL: $ADDRESSES_JSON not found"
  FAILED=1
else
  for network in localnet testnet mainnet; do
    if ! jq -e --arg n "$network" 'has($n)' "$ADDRESSES_JSON" >/dev/null; then
      echo "  FAIL: ${network} — missing from addresses.json entirely"
      FAILED=1
      continue
    fi
    mapfile -t ACTUAL_KEYS < <(jq -r --arg n "$network" '.[$n] | keys[]' "$ADDRESSES_JSON")
    for key in "${CANONICAL_KEYS[@]}"; do
      if printf '%s\n' "${ACTUAL_KEYS[@]}" | grep -qx "$key"; then
        : # present, ok
      else
        echo "  FAIL: ${network}.${key} — missing from addresses.json"
        FAILED=1
      fi
    done
    for key in "${ACTUAL_KEYS[@]}"; do
      if printf '%s\n' "${CANONICAL_KEYS[@]}" | grep -qx "$key"; then
        : # known key, ok
      else
        echo "  FAIL: ${network}.${key} — present in addresses.json but not in MuxContractIds"
        FAILED=1
      fi
    done
  done
  if (( ! FAILED )); then
    echo "  OK:   all three networks carry exactly the canonical key set"
  fi
fi

echo ""
echo "==> Checking $(basename "$ROOT_DOC") mentions every canonical key"
if [[ ! -f "$ROOT_DOC" ]]; then
  echo "  FAIL: $ROOT_DOC not found"
  FAILED=1
else
  for key in "${CANONICAL_KEYS[@]}"; do
    if grep -q "$key" "$ROOT_DOC"; then
      echo "  OK:   ${key}"
    else
      echo "  FAIL: ${key} — not mentioned in $(basename "$ROOT_DOC")"
      FAILED=1
    fi
  done
fi

echo ""
echo "==> Checking $(basename "$DETAIL_DOC") mentions every canonical key"
if [[ ! -f "$DETAIL_DOC" ]]; then
  echo "  FAIL: $DETAIL_DOC not found"
  FAILED=1
else
  for key in "${CANONICAL_KEYS[@]}"; do
    if grep -q "$key" "$DETAIL_DOC"; then
      echo "  OK:   ${key}"
    else
      echo "  FAIL: ${key} — not mentioned in $(basename "$DETAIL_DOC")"
      FAILED=1
    fi
  done
fi

echo ""
if (( FAILED )); then
  echo "ERROR: CONTRACT_IDS.md / docs/contract-ids.md / config/addresses.json"
  echo "       have drifted from bindings/src/types.ts's MuxContractIds."
  echo "       Update whichever side is stale so all four stay in sync."
  exit 1
fi

echo "All checks passed: CONTRACT_IDS.md, docs/contract-ids.md, and"
echo "config/addresses.json all track the same contract key set."
