#!/usr/bin/env bash
# Tests for validate_auth_patterns.py (issues #927 and #929).
#
# #927: a `pub fn` whose body nests a `match` inside an `if` and calls
#       `require_auth` must not be reported as a public entrypoint (the
#       old brace-counting regex mis-delimited such bodies).
# #929: session entrypoints are checked per-function body; a guard string
#       elsewhere in the file must not mask a missing empty-scope guard.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${REPO_ROOT}/scripts/validate_auth_patterns.py"

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
  if echo "$out" | grep -qF -- "$pattern"; then
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
  if echo "$out" | grep -qE -- "$pattern"; then
    echo "  FAIL: $label (pattern '$pattern' unexpectedly found)"
    FAIL=$((FAIL + 1))
  else
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  fi
}

echo "Testing validate_auth_patterns.py..."

# ── 1. Real repo: must run without a traceback (issue #927) ──────────────────
REAL_OUT="$(cd "$REPO_ROOT" && python3 "$SCRIPT" 2>&1)" || true
if echo "$REAL_OUT" | grep -q "Traceback"; then
  echo "  FAIL: real-repo run raised a Python traceback"
  FAIL=$((FAIL + 1))
else
  echo "  PASS: real-repo run has no traceback"
  PASS=$((PASS + 1))
fi
if echo "$REAL_OUT" | grep -q "Contracts analyzed:"; then
  echo "  PASS: real-repo run completed with a summary"
  PASS=$((PASS + 1))
else
  echo "  FAIL: real-repo run produced no summary"
  FAIL=$((FAIL + 1))
fi

# ── 2. Good fixture: nested-body auth fn + valid session entrypoint ─────────
GOOD_DIR="$(mktemp -d)"
trap 'rm -rf "$GOOD_DIR" "$BAD_DIR"' EXIT
mkdir -p "$GOOD_DIR/contracts/fixture-auth/src"
cat > "$GOOD_DIR/contracts/fixture-auth/src/lib.rs" <<'RUST'
//! Fixture (issue #927/#929): nested-block auth must be recognised, and a
//! session entrypoint with its own guards must pass.
pub struct FixtureAuth;

impl FixtureAuth {
    /// Nested blocks: a `match` arm inside an `if`.
    /// The arm calls require_auth, and the full body must be delimited,
    /// so this is NOT a public entrypoint.
    pub fn guarded_nested(owner: Address) -> Result<(), ()> {
        let confirmed = true;
        if confirmed {
            match confirmed {
                true => {
                    owner.require_auth();
                }
                false => {}
            }
        }
        Ok(())
    }

    /// No auth call anywhere — must be listed as a public entrypoint.
    pub fn unguarded() -> Result<(), ()> {
        Ok(())
    }

    /// Session entrypoint: own require_auth + authorize_session (the
    /// empty-scope enforcement helper) — must pass per-fn checks.
    pub fn execute_with_session(env: Env, session_key: Address) -> Result<(), ()> {
        session_key.require_auth();
        Self::authorize_session(&env, &session_key)?;
        Ok(())
    }

    fn authorize_session(_env: &Env, _session_key: &Address) -> Result<(), ()> {
        let scopes: Vec<u32> = Vec::new(_env);
        if scopes.is_empty() {
            // Unauthorized callers get nothing.
            return Err(());
        }
        Ok(())
    }
}
RUST

assert_exit "good fixture exits 0" 0 \
  bash -c "cd '$GOOD_DIR' && python3 '$SCRIPT'"
assert_output_contains "good fixture lists the unguarded fn as public" \
  "public_entrypoints: unguarded" \
  bash -c "cd '$GOOD_DIR' && python3 '$SCRIPT'"
assert_output_not_contains "good fixture does not list the nested auth fn as public" \
  "public_entrypoints:.*guarded_nested" \
  bash -c "cd '$GOOD_DIR' && python3 '$SCRIPT'"
assert_output_contains "good fixture session entrypoint passes" \
  "session execute_with_session: auth=ok scope_guard=ok" \
  bash -c "cd '$GOOD_DIR' && python3 '$SCRIPT'"
assert_output_contains "good fixture reports zero session violations" \
  "Session entrypoint violations: 0" \
  bash -c "cd '$GOOD_DIR' && python3 '$SCRIPT'"

# ── 3. Bad fixture: scope guard lives elsewhere in the file (issue #929) ────
BAD_DIR="$(mktemp -d)"
mkdir -p "$BAD_DIR/contracts/fixture-auth/src"
cat > "$BAD_DIR/contracts/fixture-auth/src/lib.rs" <<'RUST'
//! Fixture (issue #929): `scopes.is_empty()` exists elsewhere in the file,
//! but the session entrypoint itself omits the guard — must be flagged.
pub struct FixtureAuth;

impl FixtureAuth {
    /// require_auth is present, but no authorize_session call and no
    /// scopes.is_empty() check in this body.
    pub fn execute_with_session(session_key: Address) -> Result<(), ()> {
        session_key.require_auth();
        Ok(())
    }

    /// Decoy: the guard string lives in a private helper the entrypoint
    /// never calls. Whole-file substring checks would be fooled; the
    /// per-function check must not be.
    fn decoy_helper(_env: &Env) -> Result<(), ()> {
        let scopes: Vec<u32> = Vec::new(_env);
        if scopes.is_empty() {
            // Unauthorized callers get nothing.
            return Err(());
        }
        Ok(())
    }
}
RUST

assert_exit "bad fixture exits non-zero" 1 \
  bash -c "cd '$BAD_DIR' && python3 '$SCRIPT'"
assert_output_contains "bad fixture flags the missing empty-scope guard" \
  "Session entrypoint execute_with_session is missing an empty-scope guard" \
  bash -c "cd '$BAD_DIR' && python3 '$SCRIPT'"
assert_output_contains "bad fixture shows the violation on the result line" \
  "session execute_with_session: auth=ok scope_guard=MISSING" \
  bash -c "cd '$BAD_DIR' && python3 '$SCRIPT'"
assert_output_contains "bad fixture counts one session violation" \
  "Session entrypoint violations: 1" \
  bash -c "cd '$BAD_DIR' && python3 '$SCRIPT'"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
