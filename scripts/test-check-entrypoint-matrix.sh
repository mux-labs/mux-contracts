#!/usr/bin/env bash
# Tests for check_entrypoint_matrix.py (issue #954)
#
# Verifies that the script derives missing entrypoints from the contract
# sources instead of a hard-coded list:
#   - the real repo tree passes (exit 0)
#   - a temporary undocumented #[contractimpl] entrypoint fails (exit 1) and
#     is named in the output
#   - documenting the entrypoint restores exit 0
#   - #[cfg(test)]-only entrypoints are ignored

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/check_entrypoint_matrix.py"

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

echo "Testing check_entrypoint_matrix.py..."

# The real repo state must pass — every discovered entrypoint is documented.
assert_exit "real repo tree passes" 0 python3 "$SCRIPT"

# Build a hermetic fixture: a probe contract + the real matrix as a baseline.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "${WORK}/contracts/zz-probe/src" "${WORK}/docs"
cp "${REPO_ROOT}/docs/entrypoint-matrix.md" "${WORK}/docs/entrypoint-matrix.md"

RUN_FIXTURE=(
  python3 "$SCRIPT"
  --contracts-dir "${WORK}/contracts"
  --matrix "${WORK}/docs/entrypoint-matrix.md"
)

# An undocumented #[contractimpl] entrypoint must fail and be named.
cat > "${WORK}/contracts/zz-probe/src/lib.rs" <<'EOF'
#[contractimpl]
impl ZzProbe {
    pub fn probe_entrypoint(_env: Env) -> u32 {
        7
    }
}
EOF
assert_exit "undocumented entrypoint fails" 1 "${RUN_FIXTURE[@]}"
assert_output_contains "missing entrypoint is named" "zz-probe :: probe_entrypoint" "${RUN_FIXTURE[@]}"

# Documenting the entrypoint must restore exit 0.
cat >> "${WORK}/docs/entrypoint-matrix.md" <<'EOF'

## zz-probe

| Entrypoint | Auth | Notes |
|---|---|---|
| `probe_entrypoint()` | P | Fixture entrypoint |
EOF
assert_exit "documented entrypoint passes" 0 "${RUN_FIXTURE[@]}"

# Test-only contracts (inside #[cfg(test)]) must be ignored.
cat > "${WORK}/contracts/zz-probe/src/lib.rs" <<'EOF'
#[cfg(test)]
mod tests {
    #[contractimpl]
    impl TestOnly {
        pub fn test_only_entrypoint(_env: Env) -> u32 {
            0
        }
    }
}
EOF
assert_exit "cfg(test)-only entrypoints are ignored" 0 "${RUN_FIXTURE[@]}"

# Commented-out pub fn examples must not count as entrypoints.
cat > "${WORK}/contracts/zz-probe/src/lib.rs" <<'EOF'
// pub fn commented_out_example(_env: Env) -> u32 { 0 }
/* pub fn block_commented_example(_env: Env) -> u32 { 0 } */
#[contractimpl]
impl ZzProbe {
    pub fn real_entrypoint(_env: Env) -> u32 {
        1
    }
}
EOF
cat >> "${WORK}/docs/entrypoint-matrix.md" <<'EOF'
| `real_entrypoint()` | P | Fixture entrypoint |
EOF
assert_exit "commented-out pub fn examples are ignored" 0 "${RUN_FIXTURE[@]}"

# A contracts directory with no contract sources at all must fail loudly —
# a renamed/moved contracts/ directory can never be satisfied silently.
mkdir -p "${WORK}/empty/nowhere"
assert_exit "missing contract sources fail" 1 \
  python3 "$SCRIPT" --contracts-dir "${WORK}/empty/nowhere" --matrix "${WORK}/docs/entrypoint-matrix.md"

echo ""
echo "${PASS} passed, ${FAIL} failed"
[[ "$FAIL" -eq 0 ]]
