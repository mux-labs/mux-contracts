#!/usr/bin/env bash
# test-check-contract-sizes.sh
#
# 1. Coverage check — every contract directory under contracts/ must have a
#    matching entry in check-contract-sizes.sh so new contracts cannot
#    silently skip the WASM size gate in CI.
#
# 2. Behavioral tests — exercises the size-check script logic for
#    mux-account-factory: SKIP on missing WASM, output content, exit codes.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIZE_SCRIPT="${REPO_ROOT}/scripts/check-contract-sizes.sh"
CONTRACTS_DIR="${REPO_ROOT}/contracts"

PASS=0
FAIL=0

# ── Helpers ───────────────────────────────────────────────────────────────────

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

assert_output_not_contains() {
  local label="$1" pattern="$2"; shift 2
  local out
  out=$("$@" 2>&1) || true
  if ! echo "$out" | grep -q "$pattern"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (unexpected pattern '$pattern' found)"
    FAIL=$((FAIL + 1))
  fi
}

# ── Part 1: Coverage check ────────────────────────────────────────────────────

echo "Part 1: Coverage — every mux-* contract must be in check-contract-sizes.sh"

COVERAGE_FAILED=0
for dir in "${CONTRACTS_DIR}"/mux-*/; do
  name="$(basename "$dir")"
  wasm_name="${name//-/_}.wasm"

  if ! grep -q "\"${wasm_name}\"" "${SIZE_SCRIPT}"; then
    echo "  FAIL: MISSING: ${wasm_name} not found in check-contract-sizes.sh"
    COVERAGE_FAILED=1
    FAIL=$((FAIL + 1))
  else
    echo "  PASS: ${wasm_name} has a size-check entry"
    PASS=$((PASS + 1))
  fi
done

if (( COVERAGE_FAILED )); then
  echo "ERROR: Some contracts are missing from the size check script." >&2
fi

# ── Part 2: Behavioral tests for mux-account-factory ─────────────────────────

echo ""
echo "Part 2: Behavioral tests for mux-account-factory"

TMPDIR_WASM="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_WASM"' EXIT

FACTORY_WASM="${TMPDIR_WASM}/mux_account_factory.wasm"
LIMIT=65536   # must match check-contract-sizes.sh

# 2a. Missing WASM → script exits 0 and prints SKIP (missing artifacts are not fatal)
assert_exit "missing WASM exits 0" 0 \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

assert_output_contains "missing WASM prints SKIP for mux_account_factory" "SKIP" \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

# 2b. WASM within budget → exits 0, prints OK
python3 -c "open('${FACTORY_WASM}','wb').write(b'\\x00'*1024)" # 1 KiB — well under 64 KiB limit
assert_exit "under-budget WASM exits 0" 0 \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

assert_output_contains "under-budget WASM shows mux_account_factory" "mux_account_factory" \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

assert_output_contains "under-budget WASM prints OK" "OK" \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

# 2c. WASM at 85% of budget → exits 0, prints WARN
WARN_SIZE=$(( LIMIT * 85 / 100 ))  # ~55 KiB
python3 -c "open('${FACTORY_WASM}','wb').write(b'\\x00'*${WARN_SIZE})"
assert_exit "warn-threshold WASM exits 0" 0 \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

assert_output_contains "warn-threshold WASM prints WARN" "WARN" \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

# 2d. WASM over budget → exits non-zero, prints FAIL
OVER_SIZE=$(( LIMIT + 1 ))
python3 -c "open('${FACTORY_WASM}','wb').write(b'\\x00'*${OVER_SIZE})"
assert_exit "over-budget WASM exits non-zero" 1 \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

assert_output_contains "over-budget WASM prints FAIL" "FAIL" \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

# 2e. Output always includes mux_account_factory in the report
python3 -c "open('${FACTORY_WASM}','wb').write(b'\\x00'*1024)"
assert_output_contains "report includes mux_account_factory.wasm entry" "mux_account_factory" \
  bash "$SIZE_SCRIPT" --wasm-dir "$TMPDIR_WASM"

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]] || exit 1
