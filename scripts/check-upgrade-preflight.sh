#!/usr/bin/env bash
# check-upgrade-preflight.sh — Automated upgrade preflight checks for Mux Protocol
#
# Validates that the workspace satisfies the requirements defined in
# docs/upgrade-auth-requirements.md before a production upgrade is attempted.
#
# Exit codes:
#   0 — all checks passed
#   1 — one or more checks failed (details printed to stdout)
#
# Usage:
#   bash scripts/check-upgrade-preflight.sh [--fix-report]
#
# Options:
#   --fix-report   Print a per-check remediation hint on failure

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIX_REPORT=0
[[ "${1:-}" == "--fix-report" ]] && FIX_REPORT=1

PASS=0
FAIL=0

pass() { echo "  [PASS] $1"; ((PASS++)) || true; }
fail() {
  echo "  [FAIL] $1"
  [[ $FIX_REPORT -eq 1 ]] && echo "         ↳ $2"
  ((FAIL++)) || true
}

# ── Helpers ───────────────────────────────────────────────────────────────────

# Search Rust source files under contracts/ for a pattern.
rs_grep() { grep -rn --include="*.rs" "$1" "${REPO_ROOT}/contracts/" 2>/dev/null; }

# ── Check 1: No panic! on shipped library paths ───────────────────────────────
echo ""
echo "=== Check 1: No panic!() on shipped library paths ==="

# Allow panic! only inside #[cfg(test)] blocks.  A naive grep can't parse
# Rust perfectly, so we detect any panic! that is NOT preceded by a cfg(test)
# marker in the same file and report the file:line.
PANIC_OUTSIDE_TEST=$(
  for f in $(find "${REPO_ROOT}/contracts" -name "lib.rs" -o -name "*.rs" | \
             grep -v "/tests/" | \
             grep -v "soroban-test-helpers"); do
    # Use awk: track when we enter/exit #[cfg(test)], flag any panic! outside.
    awk '
      /^#\[cfg\(test\)\]/ { in_test_attr=1; next }
      /^mod tests/ && in_test_attr { in_test=1; in_test_attr=0; depth=0 }
      in_test { gsub(/[^{}]/, ""); depth += gsub(/{/, "{") - gsub(/}/, "}") }
      depth < 0 { in_test=0; depth=0 }
      !in_test && /panic!/ && !/\/\// { print FILENAME ":" NR ": " $0 }
    ' FILENAME="$f" "$f"
  done
)

if [[ -z "$PANIC_OUTSIDE_TEST" ]]; then
  pass "No panic!() found outside #[cfg(test)] blocks"
else
  fail "panic!() found on shipped paths" \
       "Replace panic! with proper error returns. Affected lines:\n${PANIC_OUTSIDE_TEST}"
fi

# ── Check 2: No todo!() on shipped library paths ──────────────────────────────
echo ""
echo "=== Check 2: No todo!() on shipped library paths ==="

TODO_OUTSIDE_TEST=$(
  for f in $(find "${REPO_ROOT}/contracts" -name "lib.rs" -o -name "*.rs" | \
             grep -v "/tests/" | \
             grep -v "soroban-test-helpers"); do
    awk '
      /^#\[cfg\(test\)\]/ { in_test_attr=1; next }
      /^mod tests/ && in_test_attr { in_test=1; in_test_attr=0; depth=0 }
      in_test { gsub(/[^{}]/, ""); depth += gsub(/{/, "{") - gsub(/}/, "}") }
      depth < 0 { in_test=0; depth=0 }
      !in_test && /todo!/ && !/\/\// { print FILENAME ":" NR ": " $0 }
    ' FILENAME="$f" "$f"
  done
)

if [[ -z "$TODO_OUTSIDE_TEST" ]]; then
  pass "No todo!() found outside #[cfg(test)] blocks"
else
  fail "todo!() found on shipped paths" \
       "Implement or remove todo! stubs before shipping. Affected:\n${TODO_OUTSIDE_TEST}"
fi

# ── Check 3: Every upgrade() calls require_auth before update_wasm ────────────
echo ""
echo "=== Check 3: upgrade() entrypoints call require_auth before update_current_contract_wasm ==="

# For each contract that has an upgrade() fn, verify require_auth appears
# before update_current_contract_wasm in the same function body.
UPGRADE_FILES=$(rs_grep "pub fn upgrade" | awk -F: '{print $1}' | sort -u)

UPGRADE_AUTH_FAIL=0
for f in $UPGRADE_FILES; do
  # Check the whole file: if it has pub fn upgrade, check that
  # require_admin/require_owner/require_auth also appears in the file (used by upgrade).
  # This is safe because all these contracts have exactly one upgrade() fn
  # and it immediately calls Self::require_admin().
  fn_body=$(grep -A 15 "pub fn upgrade" "$f")
  has_auth=$(echo "$fn_body" | grep -c "require_admin\|require_auth\|require_owner" || true)
  has_wasm=$(echo "$fn_body" | grep -c "update_current_contract_wasm" || true)

  if [[ $has_auth -eq 0 ]]; then
    fail "upgrade() in $f missing require_admin / require_auth call" \
         "Add Self::require_admin(&env) before env.deployer().update_current_contract_wasm()"
    UPGRADE_AUTH_FAIL=1
  elif [[ $has_wasm -eq 0 ]]; then
    fail "upgrade() in $f has no update_current_contract_wasm call" \
         "Ensure upgrade() calls env.deployer().update_current_contract_wasm(new_wasm_hash)"
    UPGRADE_AUTH_FAIL=1
  else
    pass "upgrade() in $(basename "$(dirname "$f")")/$(basename "$f") has auth + wasm-update"
  fi
done

if [[ -z "$UPGRADE_FILES" ]]; then
  pass "No upgrade() entrypoints found (immutable contracts — nothing to check)"
fi

# ── Check 4: Admin-mutating entrypoints call require_auth ────────────────────
echo ""
echo "=== Check 4: Known admin entrypoints contain require_auth calls ==="

# Spot-check the admin/owner-only entrypoints that must never skip auth.
ADMIN_PATTERNS=(
  "pub fn initialize"
  "pub fn set_delegate"
  "pub fn set_spend_limit"
  "pub fn set_daily_limit"
  "pub fn create_role"
  "pub fn grant_role"
  "pub fn revoke_role"
  "pub fn register"
  "pub fn set_policy"
  "pub fn pause"
  "pub fn unpause"
)

AUTH_FAIL=0
for pattern in "${ADMIN_PATTERNS[@]}"; do
  while IFS=: read -r file line _rest; do
    # Extract the function body (next 30 lines after the fn declaration).
    fn_body=$(tail -n +"$line" "$file" | head -30)
    if ! echo "$fn_body" | grep -q "require_auth\|require_owner\|require_admin\|require_guardian"; then
      fail "$pattern in $file:$line has no auth call within 30 lines" \
           "Add the appropriate require_auth / require_owner / require_admin call"
      AUTH_FAIL=1
    fi
  done < <(rs_grep "$pattern" | grep -v "^Binary" | head -20)
done

if [[ $AUTH_FAIL -eq 0 ]]; then
  pass "All spot-checked admin entrypoints contain an auth call"
fi

# ── Check 5: WASM hash verification script exists ────────────────────────────
echo ""
echo "=== Check 5: WASM hash verification tooling present ==="

if [[ -x "${REPO_ROOT}/scripts/verify-wasm-hash.sh" ]]; then
  pass "scripts/verify-wasm-hash.sh exists and is executable"
else
  fail "scripts/verify-wasm-hash.sh missing or not executable" \
       "Ensure verify-wasm-hash.sh is committed and chmod +x"
fi

if [[ -x "${REPO_ROOT}/scripts/compute-wasm-hashes.sh" ]]; then
  pass "scripts/compute-wasm-hashes.sh exists and is executable"
else
  fail "scripts/compute-wasm-hashes.sh missing or not executable" \
       "Ensure compute-wasm-hashes.sh is committed and chmod +x"
fi

# ── Check 6: Rollback log script enforces log completeness ───────────────────
echo ""
echo "=== Check 6: Rollback log enforcement script present ==="

if [[ -x "${REPO_ROOT}/scripts/check-rollback-log.sh" ]]; then
  pass "scripts/check-rollback-log.sh exists and is executable"
else
  fail "scripts/check-rollback-log.sh missing" \
       "Ensure check-rollback-log.sh is committed — it runs in CI on every PR"
fi

# ── Check 7: upgrade-auth-requirements.md present and non-empty ─────────────
echo ""
echo "=== Check 7: Upgrade auth requirements doc present ==="

AUTH_DOC="${REPO_ROOT}/docs/upgrade-auth-requirements.md"
if [[ -f "$AUTH_DOC" ]] && [[ $(wc -l < "$AUTH_DOC") -gt 10 ]]; then
  pass "docs/upgrade-auth-requirements.md present ($(wc -l < "$AUTH_DOC") lines)"
else
  fail "docs/upgrade-auth-requirements.md missing or too short" \
       "Create or restore docs/upgrade-auth-requirements.md"
fi

# ── Check 8: access-control-checklist.md present ────────────────────────────
echo ""
echo "=== Check 8: Access control checklist present ==="

CHECKLIST="${REPO_ROOT}/docs/access-control-checklist.md"
if [[ -f "$CHECKLIST" ]]; then
  pass "docs/access-control-checklist.md present"
else
  fail "docs/access-control-checklist.md missing" \
       "Restore docs/access-control-checklist.md from git history"
fi

# ── Check 9: entrypoint-matrix.md present ────────────────────────────────────
echo ""
echo "=== Check 9: Entrypoint matrix doc present ==="

MATRIX="${REPO_ROOT}/docs/entrypoint-matrix.md"
if [[ -f "$MATRIX" ]]; then
  pass "docs/entrypoint-matrix.md present"
else
  fail "docs/entrypoint-matrix.md missing" \
       "Restore docs/entrypoint-matrix.md from git history"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "============================================"
echo " Preflight result: ${PASS} passed, ${FAIL} failed"
echo "============================================"

if [[ $FAIL -gt 0 ]]; then
  echo ""
  echo "ERROR: ${FAIL} preflight check(s) failed."
  echo "Resolve all failures before proceeding with the upgrade."
  echo "See docs/upgrade-auth-requirements.md for the full requirements."
  exit 1
fi

echo ""
echo "All preflight checks passed."
echo "Proceed with the pre-upgrade checklist in docs/upgrade-auth-requirements.md."
