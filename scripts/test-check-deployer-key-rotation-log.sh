#!/usr/bin/env bash
# Tests for check-deployer-key-rotation-log.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-deployer-key-rotation-log.sh"

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

echo "Testing check-deployer-key-rotation-log.sh..."

# The real log (no entries yet) must pass with a SKIP.
assert_exit "current ops/deployer-key-rotation-log.md passes (no entries yet)" 0 bash "$SCRIPT"

TMPDIR_RL="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_RL"' EXIT

# An entry missing required fields must fail.
cat > "${TMPDIR_RL}/missing-field.md" <<'EOF'
# Deployer Key Rotation Log

## mainnet - 2026-06-01
- Deploy run: https://example.com/run/1
- Deployer public key: GABC
EOF
assert_exit "entry missing required fields fails" 1 \
  bash "$SCRIPT" --log "${TMPDIR_RL}/missing-field.md"

# An entry with an unchecked confirmation box must fail.
cat > "${TMPDIR_RL}/unchecked.md" <<'EOF'
# Deployer Key Rotation Log

## mainnet - 2026-06-01
- Deploy run: https://example.com/run/1
- Deployer public key: GABC
- Drained to treasury: [ ] pending
- Old key archived/revoked in secrets manager: [x] yes
- Verified by: alice
EOF
assert_exit "entry with unchecked drain box fails" 1 \
  bash "$SCRIPT" --log "${TMPDIR_RL}/unchecked.md"

# A fully complete entry must pass.
cat > "${TMPDIR_RL}/complete.md" <<'EOF'
# Deployer Key Rotation Log

## mainnet - 2026-06-01
- Deploy run: https://example.com/run/1
- Deployer public key: GABC
- Drained to treasury: [x] yes — tx hash: abc123
- Old key archived/revoked in secrets manager: [x] yes
- Verified by: alice
EOF
assert_exit "fully complete entry passes" 0 \
  bash "$SCRIPT" --log "${TMPDIR_RL}/complete.md"

# The template's own header (literal <YYYY-MM-DD>, not digits) must never be
# mistaken for a real entry, so a doc containing only the template passes.
cat > "${TMPDIR_RL}/template-only.md" <<'EOF'
# Deployer Key Rotation Log

## Entry Template

```
## mainnet - YYYY-MM-DD
- Deploy run: <GitHub Actions run URL or commit SHA>
```
EOF
assert_exit "template-only doc is not mistaken for a real entry" 0 \
  bash "$SCRIPT" --log "${TMPDIR_RL}/template-only.md"

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"
if (( FAIL > 0 )); then
  exit 1
fi
