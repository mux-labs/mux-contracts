#!/usr/bin/env bash
# Tests for check-rollback-log.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-rollback-log.sh"

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

echo "Testing check-rollback-log.sh..."

# The real log (no entries yet) must pass with a SKIP.
assert_exit "current ops/rollback-log.md passes (no entries yet)" 0 bash "$SCRIPT"

TMPDIR_RB="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_RB"' EXIT

# An entry missing required fields must fail.
cat > "${TMPDIR_RB}/missing-field.md" <<'EOF'
# Rollback Log

## mux-batcher - 2026-06-01
- Strategy used: 1
- Broken contract ID: CABC
EOF
assert_exit "entry missing required fields fails" 1 \
  bash "$SCRIPT" --log "${TMPDIR_RB}/missing-field.md"

# An entry with an unchecked checklist box must fail.
cat > "${TMPDIR_RB}/unchecked.md" <<'EOF'
# Rollback Log

## mux-batcher - 2026-06-01
- Strategy used: 1
- Broken contract ID: CABC
- Restored/new contract ID: CDEF
- Pre-Rollback Checklist completed: [x]
- Post-Rollback Steps completed: [ ]
- Incident report: https://example.com/incident/1
- Follow-up issue: https://example.com/issue/1
EOF
assert_exit "entry with unchecked post-rollback box fails" 1 \
  bash "$SCRIPT" --log "${TMPDIR_RB}/unchecked.md"

# A fully complete entry must pass.
cat > "${TMPDIR_RB}/complete.md" <<'EOF'
# Rollback Log

## mux-batcher - 2026-06-01
- Strategy used: 1
- Broken contract ID: CABC
- Restored/new contract ID: CDEF
- Pre-Rollback Checklist completed: [x]
- Post-Rollback Steps completed: [x]
- Incident report: https://example.com/incident/1
- Follow-up issue: https://example.com/issue/1
EOF
assert_exit "fully complete entry passes" 0 \
  bash "$SCRIPT" --log "${TMPDIR_RB}/complete.md"

# The template's own header (literal YYYY-MM-DD, not digits) must never be
# mistaken for a real entry, so a doc containing only the template passes.
cat > "${TMPDIR_RB}/template-only.md" <<'EOF'
# Rollback Log

## Entry Template

```
## <contract-or-scope> - YYYY-MM-DD
- Strategy used: <1|2|3>
```
EOF
assert_exit "template-only doc is not mistaken for a real entry" 0 \
  bash "$SCRIPT" --log "${TMPDIR_RB}/template-only.md"

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"
if (( FAIL > 0 )); then
  exit 1
fi
