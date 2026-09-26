#!/usr/bin/env bash
# Tests for check-changelog-release-artifacts.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check-changelog-release-artifacts.sh"

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

echo "Testing check-changelog-release-artifacts.sh..."

# The real CHANGELOG.md (only Unreleased so far) must pass.
assert_exit "current CHANGELOG.md passes (no tagged releases yet)" 0 bash "$SCRIPT"

TMPDIR_CL="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_CL"' EXIT

# A tagged release with no Release Artifacts subsection must fail.
cat > "${TMPDIR_CL}/missing.md" <<'EOF'
# Changelog

## [Unreleased]

### Added
- Something new

## [1.0.0] - 2026-05-30

### Added
- Feature 1
EOF

assert_exit "release missing artifacts section fails" 1 \
  bash "$SCRIPT" --changelog "${TMPDIR_CL}/missing.md"

assert_output_contains "reports the offending version" "1.0.0" \
  bash "$SCRIPT" --changelog "${TMPDIR_CL}/missing.md"

# A tagged release with a Release Artifacts subsection must pass.
cat > "${TMPDIR_CL}/present.md" <<'EOF'
# Changelog

## [Unreleased]

### Added
- Something new

## [1.0.0] - 2026-05-30

### Added
- Feature 1

### Release Artifacts
- Bindings package: `@mux-protocol/contracts@1.0.0`
- Contract WASM (SHA-256):
  | Contract | SHA-256 |
  |---|---|
  | `mux-account` | `abc123...` |
EOF

assert_exit "release with artifacts section passes" 0 \
  bash "$SCRIPT" --changelog "${TMPDIR_CL}/present.md"

# Multiple tagged releases: one good, one missing — must fail and name both.
cat > "${TMPDIR_CL}/mixed.md" <<'EOF'
# Changelog

## [Unreleased]

## [2.0.0] - 2026-06-01

### Added
- Feature 2

## [1.0.0] - 2026-05-30

### Added
- Feature 1

### Release Artifacts
- Bindings package: `@mux-protocol/contracts@1.0.0`
EOF

assert_exit "mixed changelog fails on the missing release" 1 \
  bash "$SCRIPT" --changelog "${TMPDIR_CL}/mixed.md"

assert_output_contains "mixed changelog still credits the good release" "OK:   \[1.0.0\]" \
  bash "$SCRIPT" --changelog "${TMPDIR_CL}/mixed.md"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
