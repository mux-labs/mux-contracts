#!/usr/bin/env bash
# Tests for check-mainnet-address-review.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-mainnet-address-review.sh"
PASS=0; FAIL=0

assert_exit() {
  local label="$1" expected="$2"; shift 2
  local actual=0
  "$@" >/dev/null 2>&1 || actual=$?
  if [[ "$actual" -eq "$expected" ]]; then
    echo "  PASS: $label (exit $actual)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (expected $expected, got $actual)"
    FAIL=$((FAIL + 1))
  fi
}

echo "Testing check-mainnet-address-review.sh..."

TMPDIR_REV="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_REV"' EXIT

# 1. Real config/addresses.json (all empty placeholders) passes
assert_exit "repo default addresses.json exits 0" 0 \
  bash "$SCRIPT"

# 2. Populated mainnet address without review approval marker exits 1
cat > "${TMPDIR_REV}/unreviewed.json" <<'EOF'
{
  "_review": { "mainnetApproval": null },
  "localnet": { "muxAccount": "" },
  "testnet": { "muxAccount": "" },
  "mainnet": { "muxAccount": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" }
}
EOF

assert_exit "unreviewed populated mainnet address exits 1" 1 \
  bash "$SCRIPT" --addresses "${TMPDIR_REV}/unreviewed.json"

# 3. Populated mainnet address with env approval marker exits 0
assert_exit "env approved mainnet address exits 0" 0 \
  env MAINNET_ADDRESS_REVIEW_APPROVED=true bash "$SCRIPT" --addresses "${TMPDIR_REV}/unreviewed.json"

# 4. Populated mainnet address with PR label exits 0
assert_exit "PR label approved mainnet address exits 0" 0 \
  env PR_LABELS="mainnet-approved,backend" bash "$SCRIPT" --addresses "${TMPDIR_REV}/unreviewed.json"

# 5. Populated mainnet address with inline _review.mainnetApproval marker exits 0
cat > "${TMPDIR_REV}/approved.json" <<'EOF'
{
  "_review": {
    "mainnetApproval": {
      "approvedBy": "security-lead",
      "pr": "100"
    }
  },
  "localnet": { "muxAccount": "" },
  "testnet": { "muxAccount": "" },
  "mainnet": { "muxAccount": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" }
}
EOF

assert_exit "inline approved mainnet address exits 0" 0 \
  bash "$SCRIPT" --addresses "${TMPDIR_REV}/approved.json"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
