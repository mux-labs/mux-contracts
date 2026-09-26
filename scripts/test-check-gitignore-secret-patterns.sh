#!/usr/bin/env bash
# Tests for check-gitignore-secret-patterns.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-gitignore-secret-patterns.sh"

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

echo "Testing check-gitignore-secret-patterns.sh..."

# The real .gitignore must cover every required secret-bearing filename.
assert_exit "current .gitignore covers all required secret filenames" 0 bash "$SCRIPT"

TMPDIR_GI="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_GI"' EXIT

# A gitignore missing deployment.env / *.secret / deployer.json must fail.
cat > "${TMPDIR_GI}/incomplete.gitignore" <<'EOF'
.env
EOF
assert_exit "gitignore missing deployment.env/*.secret/deployer.json fails" 1 \
  bash "$SCRIPT" --gitignore "${TMPDIR_GI}/incomplete.gitignore"

# A gitignore with every required pattern must pass.
cat > "${TMPDIR_GI}/complete.gitignore" <<'EOF'
.env
*.secret
deployment.env
deployer.json
EOF
assert_exit "gitignore with all required patterns passes" 0 \
  bash "$SCRIPT" --gitignore "${TMPDIR_GI}/complete.gitignore"

# A missing gitignore file is a hard error, not a silent pass.
assert_exit "missing gitignore file errors" 1 \
  bash "$SCRIPT" --gitignore "${TMPDIR_GI}/does-not-exist.gitignore"

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"
if (( FAIL > 0 )); then
  exit 1
fi
