#!/usr/bin/env bash
# Tests for check-security-policy.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-security-policy.sh"

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

echo "Testing check-security-policy.sh..."

# The real SECURITY.md / security.txt / issue config must pass.
assert_exit "current security policy passes" 0 bash "$SCRIPT"

TMPDIR_SEC="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_SEC"' EXIT

# --- Missing files entirely ---------------------------------------------
assert_exit "missing SECURITY.md fails" 1 \
  bash "$SCRIPT" --security-md "${TMPDIR_SEC}/nope.md"

assert_exit "missing security.txt fails" 1 \
  bash "$SCRIPT" --security-txt "${TMPDIR_SEC}/nope.txt"

assert_exit "missing issue config fails" 1 \
  bash "$SCRIPT" --issue-config "${TMPDIR_SEC}/nope.yml"

# --- SECURITY.md missing required content --------------------------------
cat > "${TMPDIR_SEC}/no-private-contact.md" <<'EOF'
# Security Policy

## Scope

Everything under contracts/.

## Response Timeline

We try our best.
EOF

assert_exit "SECURITY.md without private contact fails" 1 \
  bash "$SCRIPT" --security-md "${TMPDIR_SEC}/no-private-contact.md"
assert_output_contains "reports missing private contact" "no private contact" \
  bash "$SCRIPT" --security-md "${TMPDIR_SEC}/no-private-contact.md"

cat > "${TMPDIR_SEC}/no-scope.md" <<'EOF'
# Security Policy

Do not open public issues for vulnerabilities. Email security@example.com.

## Response Timeline

48 hours to acknowledge.
EOF

assert_exit "SECURITY.md without Scope section fails" 1 \
  bash "$SCRIPT" --security-md "${TMPDIR_SEC}/no-scope.md"
assert_output_contains "reports missing Scope section" "Scope section" \
  bash "$SCRIPT" --security-md "${TMPDIR_SEC}/no-scope.md"

cat > "${TMPDIR_SEC}/no-sla.md" <<'EOF'
# Security Policy

Do not open public issues for vulnerabilities. Email security@example.com.

## Scope

contracts/ and bindings/.
EOF

assert_exit "SECURITY.md without SLA section fails" 1 \
  bash "$SCRIPT" --security-md "${TMPDIR_SEC}/no-sla.md"
assert_output_contains "reports missing SLA section" "SLA section" \
  bash "$SCRIPT" --security-md "${TMPDIR_SEC}/no-sla.md"

# --- security.txt with a dangling link -----------------------------------
cat > "${TMPDIR_SEC}/dangling.txt" <<EOF
Contact: mailto:security@example.com
Acknowledgments: https://github.com/mux-labs/mux-contracts/blob/main/docs/does-not-exist.md
EOF

assert_exit "security.txt with dangling link fails" 1 \
  bash "$SCRIPT" --security-txt "${TMPDIR_SEC}/dangling.txt"
assert_output_contains "reports the missing linked doc" "docs/does-not-exist.md" \
  bash "$SCRIPT" --security-txt "${TMPDIR_SEC}/dangling.txt"

# --- issue template config without a security contact link ---------------
cat > "${TMPDIR_SEC}/generic-config.yml" <<'EOF'
blank_issues_enabled: true
contact_links:
  - name: Ask a question
    url: https://example.com/discuss
    about: General questions.
EOF

assert_exit "issue config without security link fails" 1 \
  bash "$SCRIPT" --issue-config "${TMPDIR_SEC}/generic-config.yml"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
