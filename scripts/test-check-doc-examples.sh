#!/usr/bin/env bash
# Tests for check-doc-examples.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-doc-examples.sh"

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

echo "Testing check-doc-examples.sh..."

# The real CONTRIBUTING.md must pass (all example calls are real entrypoints).
assert_exit "current CONTRIBUTING.md passes" 0 bash "$SCRIPT"

TMPDIR_DOC="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_DOC"' EXIT

# A doc referencing a nonexistent entrypoint must fail.
cat > "${TMPDIR_DOC}/phantom.md" <<'EOF'
```rust
client.register_session_key(&session_key, &expires_at, &scopes);
assert!(client.is_session_key_valid(&owner, &session_key));
```
EOF

assert_exit "doc with phantom method fails" 1 \
  bash "$SCRIPT" --doc "${TMPDIR_DOC}/phantom.md"

assert_output_contains "reports the phantom method by name" "is_session_key_valid" \
  bash "$SCRIPT" --doc "${TMPDIR_DOC}/phantom.md"

# A doc with only real entrypoints must pass.
cat > "${TMPDIR_DOC}/real.md" <<'EOF'
```rust
client.register_session_key(&session_key, &expires_at, &scopes);
let _ = client.execute_with_session(&session_key, &payload);
```
EOF

assert_exit "doc with only real methods passes" 0 \
  bash "$SCRIPT" --doc "${TMPDIR_DOC}/real.md"

# A doc with no client.<method>() calls should SKIP, not fail.
cat > "${TMPDIR_DOC}/empty.md" <<'EOF'
No code examples here.
EOF

assert_exit "doc with no client calls skips cleanly" 0 \
  bash "$SCRIPT" --doc "${TMPDIR_DOC}/empty.md"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
