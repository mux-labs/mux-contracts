#!/usr/bin/env bash
# Tests for check-deploy-secret-name.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-deploy-secret-name.sh"

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

echo "Testing check-deploy-secret-name.sh..."

# The real deploy.yml / deploy.sh pair must agree.
assert_exit "current deploy.yml matches deploy.sh" 0 bash "$SCRIPT"

TMPDIR_FIXTURES="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_FIXTURES"' EXIT

# Reproduce the pre-fix #702 bug: workflow sets DEPLOYER_SECRET_KEY while
# the script reads DEPLOYER_PRIVATE_KEY.
cat > "${TMPDIR_FIXTURES}/deploy.yml" <<'EOF'
      - name: Deploy contracts
        env:
          DEPLOYER_SECRET_KEY: ${{ secrets.DEPLOYER_SECRET_KEY }}
        run: bash scripts/deploy.sh --network testnet
EOF

cat > "${TMPDIR_FIXTURES}/deploy.sh" <<'EOF'
#!/usr/bin/env bash
if [[ -z "${DEPLOYER_PRIVATE_KEY:-}" ]]; then
  echo "DEPLOYER_PRIVATE_KEY is not set"
  exit 1
fi
EOF

assert_exit "mismatched workflow/script fails" 1 \
  bash "$SCRIPT" --workflow "${TMPDIR_FIXTURES}/deploy.yml" --deploy-script "${TMPDIR_FIXTURES}/deploy.sh"

assert_output_contains "reports both names on mismatch" "DEPLOYER_SECRET_KEY" \
  bash "$SCRIPT" --workflow "${TMPDIR_FIXTURES}/deploy.yml" --deploy-script "${TMPDIR_FIXTURES}/deploy.sh"

# A matching pair must pass.
cat > "${TMPDIR_FIXTURES}/deploy-ok.yml" <<'EOF'
      - name: Deploy contracts
        env:
          DEPLOYER_PRIVATE_KEY: ${{ secrets.DEPLOYER_PRIVATE_KEY }}
        run: bash scripts/deploy.sh --network testnet
EOF

assert_exit "matching workflow/script passes" 0 \
  bash "$SCRIPT" --workflow "${TMPDIR_FIXTURES}/deploy-ok.yml" --deploy-script "${TMPDIR_FIXTURES}/deploy.sh"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
