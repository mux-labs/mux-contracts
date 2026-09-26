#!/usr/bin/env bash
# Tests for check-contract-ids-sync.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-contract-ids-sync.sh"

PASS=0
FAIL=0

assert_exit() {
  local label="$1" expected="$2"; shift 2
  local actual=0
  "$@" >/dev/null 2>&1 || actual=$?
  if [[ "$actual" -eq "$expected" ]]; then
    echo "  PASS: $label (exit $actual)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (expected exit $expected, got $actual)"
    FAIL=$((FAIL + 1))
  fi
}

assert_output_contains() {
  local label="$1" pattern="$2"; shift 2
  local out
  out=$("$@" 2>&1) || true
  if echo "$out" | grep -q "$pattern"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (pattern '$pattern' not found)"
    FAIL=$((FAIL + 1))
  fi
}

echo "Testing check-contract-ids-sync.sh..."

# The real repo state must pass (this is the #674 fix itself).
assert_exit "current repo state passes" 0 bash "$SCRIPT"

TMPDIR_SYNC="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_SYNC"' EXIT

REAL_TYPES="${REPO_ROOT}/bindings/src/types.ts"
REAL_ADDRESSES="${REPO_ROOT}/config/addresses.json"
REAL_ROOT_DOC="${REPO_ROOT}/CONTRACT_IDS.md"
REAL_DETAIL_DOC="${REPO_ROOT}/docs/contract-ids.md"

# --- addresses.json missing a key that MuxContractIds has ----------------
cat > "${TMPDIR_SYNC}/stale-addresses.json" <<'EOF'
{
  "localnet": { "muxAccount": "", "muxBatcher": "", "muxDelegation": "", "muxPermissions": "", "muxWalletRegistry": "", "muxAccountFactory": "", "muxRegistry": "", "muxPolicy": "" },
  "testnet":  { "muxAccount": "", "muxBatcher": "", "muxPermissions": "" },
  "mainnet":  { "muxAccount": "", "muxBatcher": "", "muxDelegation": "", "muxPermissions": "", "muxWalletRegistry": "", "muxAccountFactory": "", "muxRegistry": "", "muxPolicy": "" }
}
EOF

assert_exit "addresses.json missing a testnet key fails" 1 \
  bash "$SCRIPT" --addresses "${TMPDIR_SYNC}/stale-addresses.json"
assert_output_contains "reports the missing key" "testnet.muxDelegation" \
  bash "$SCRIPT" --addresses "${TMPDIR_SYNC}/stale-addresses.json"

# --- addresses.json with an extra, unknown key ----------------------------
cat > "${TMPDIR_SYNC}/extra-key-addresses.json" <<'EOF'
{
  "localnet": { "muxAccount": "", "muxBatcher": "", "muxDelegation": "", "muxPermissions": "", "muxWalletRegistry": "", "muxAccountFactory": "", "muxRegistry": "", "muxPolicy": "" },
  "testnet":  { "muxAccount": "", "muxBatcher": "", "muxDelegation": "", "muxPermissions": "", "muxWalletRegistry": "", "muxAccountFactory": "", "muxRegistry": "", "muxPolicy": "", "muxTypo": "" },
  "mainnet":  { "muxAccount": "", "muxBatcher": "", "muxDelegation": "", "muxPermissions": "", "muxWalletRegistry": "", "muxAccountFactory": "", "muxRegistry": "", "muxPolicy": "" }
}
EOF

assert_exit "addresses.json with an unknown key fails" 1 \
  bash "$SCRIPT" --addresses "${TMPDIR_SYNC}/extra-key-addresses.json"
assert_output_contains "reports the unknown key" "testnet.muxTypo" \
  bash "$SCRIPT" --addresses "${TMPDIR_SYNC}/extra-key-addresses.json"

# --- CONTRACT_IDS.md missing a mention of a canonical key -----------------
cat > "${TMPDIR_SYNC}/stale-root-doc.md" <<'EOF'
# Contract IDs

Tracks muxAccount, muxBatcher, muxPermissions only.
EOF

assert_exit "root doc missing a key fails" 1 \
  bash "$SCRIPT" --root-doc "${TMPDIR_SYNC}/stale-root-doc.md"
assert_output_contains "reports the missing key in root doc" "muxDelegation" \
  bash "$SCRIPT" --root-doc "${TMPDIR_SYNC}/stale-root-doc.md"

# --- docs/contract-ids.md missing a mention of a canonical key -------------
cat > "${TMPDIR_SYNC}/stale-detail-doc.md" <<'EOF'
# Contract IDs

Tracks muxAccount, muxBatcher, muxPermissions only.
EOF

assert_exit "detail doc missing a key fails" 1 \
  bash "$SCRIPT" --detail-doc "${TMPDIR_SYNC}/stale-detail-doc.md"

# --- Sanity: passing all four real files explicitly still passes ----------
assert_exit "explicit real paths still pass" 0 \
  bash "$SCRIPT" \
    --types "$REAL_TYPES" \
    --addresses "$REAL_ADDRESSES" \
    --root-doc "$REAL_ROOT_DOC" \
    --detail-doc "$REAL_DETAIL_DOC"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
