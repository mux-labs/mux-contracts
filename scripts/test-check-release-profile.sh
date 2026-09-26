#!/usr/bin/env bash
# test-check-release-profile.sh — Issue #780
#
# Behavioral tests for scripts/check-release-profile.sh: the real workspace
# profile passes, and every non-conforming variant fails closed.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK="${REPO_ROOT}/scripts/check-release-profile.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

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
  if echo "$out" | grep -q -- "$pattern"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (pattern '$pattern' not found)"
    FAIL=$((FAIL + 1))
  fi
}

GOOD_PROFILE='[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true
'

write_toml() {
  local path="$1" body="$2"
  printf '[workspace]\nmembers = []\n\n%s' "$body" > "$path"
}

echo "── Workspace Cargo.toml ──"
assert_exit "workspace profile conforms" 0 bash "$CHECK"

echo "── Synthetic profiles ──"
write_toml "$TMP/good.toml" "$GOOD_PROFILE"
assert_exit "conforming profile passes" 0 bash "$CHECK" --cargo-toml "$TMP/good.toml"

write_toml "$TMP/comments.toml" "$(printf '%s' "$GOOD_PROFILE" | sed 's/^lto = true$/lto = true  # full LTO/')"
assert_exit "trailing comments tolerated" 0 bash "$CHECK" --cargo-toml "$TMP/comments.toml"

write_toml "$TMP/unwind.toml" "$(printf '%s' "$GOOD_PROFILE" | sed 's/"abort"/"unwind"/')"
assert_exit "panic=unwind fails" 1 bash "$CHECK" --cargo-toml "$TMP/unwind.toml"
assert_output_contains "panic mismatch is reported" "panic" bash "$CHECK" --cargo-toml "$TMP/unwind.toml"

write_toml "$TMP/opt3.toml" "$(printf '%s' "$GOOD_PROFILE" | sed 's/opt-level = "z"/opt-level = 3/')"
assert_exit "opt-level=3 fails" 1 bash "$CHECK" --cargo-toml "$TMP/opt3.toml"

write_toml "$TMP/nolto.toml" "$(printf '%s' "$GOOD_PROFILE" | grep -v '^lto')"
assert_exit "missing lto fails" 1 bash "$CHECK" --cargo-toml "$TMP/nolto.toml"
assert_output_contains "missing key is reported" "<missing>" bash "$CHECK" --cargo-toml "$TMP/nolto.toml"

write_toml "$TMP/cgu.toml" "$(printf '%s' "$GOOD_PROFILE" | sed 's/codegen-units = 1/codegen-units = 16/')"
assert_exit "codegen-units=16 fails" 1 bash "$CHECK" --cargo-toml "$TMP/cgu.toml"

write_toml "$TMP/overflow.toml" "$(printf '%s' "$GOOD_PROFILE" | sed 's/overflow-checks = true/overflow-checks = false/')"
assert_exit "overflow-checks=false fails" 1 bash "$CHECK" --cargo-toml "$TMP/overflow.toml"

write_toml "$TMP/dup.toml" "${GOOD_PROFILE}
[profile.release]
opt-level = 3
"
assert_exit "duplicate [profile.release] fails" 1 bash "$CHECK" --cargo-toml "$TMP/dup.toml"

write_toml "$TMP/override.toml" "${GOOD_PROFILE}
[profile.release.package.mux-batcher]
opt-level = 3
"
assert_exit "per-package override fails" 1 bash "$CHECK" --cargo-toml "$TMP/override.toml"

write_toml "$TMP/empty.toml" ""
assert_exit "missing [profile.release] fails" 1 bash "$CHECK" --cargo-toml "$TMP/empty.toml"

assert_exit "missing Cargo.toml is a usage error" 2 bash "$CHECK" --cargo-toml "$TMP/nope.toml"

echo "── WASM artifacts ──"
mkdir -p "$TMP/wasm-ok" "$TMP/wasm-debug" "$TMP/wasm-bad" "$TMP/wasm-empty"
printf '\x00asm\x01\x00\x00\x00' > "$TMP/wasm-ok/mux_batcher.wasm"
printf '\x00asm\x01\x00\x00\x00\x00\x0b.debug_info' > "$TMP/wasm-debug/mux_batcher.wasm"
printf 'not wasm' > "$TMP/wasm-bad/mux_batcher.wasm"

assert_exit "stripped wasm passes" 0 bash "$CHECK" --cargo-toml "$TMP/good.toml" --wasm-dir "$TMP/wasm-ok"
assert_exit "wasm with .debug_* fails" 1 bash "$CHECK" --cargo-toml "$TMP/good.toml" --wasm-dir "$TMP/wasm-debug"
assert_exit "non-wasm artifact fails" 1 bash "$CHECK" --cargo-toml "$TMP/good.toml" --wasm-dir "$TMP/wasm-bad"
assert_exit "empty wasm dir fails" 1 bash "$CHECK" --cargo-toml "$TMP/good.toml" --wasm-dir "$TMP/wasm-empty"
assert_exit "missing wasm dir fails" 1 bash "$CHECK" --cargo-toml "$TMP/good.toml" --wasm-dir "$TMP/nope"

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"
[[ "$FAIL" -eq 0 ]]
