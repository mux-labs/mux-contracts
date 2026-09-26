#!/usr/bin/env bash
# check-release-profile.sh — Issue #780
#
# Fail-closed verification of the workspace release profile documented in
# docs/release-profile-verification.md.
#
#   1. Profile invariants — every required key in `[profile.release]` of the
#      workspace Cargo.toml must be present with exactly the expected value.
#      A missing key, a different value, or a duplicate `[profile.release]`
#      table fails the check.
#   2. Override guard — `[profile.release.package.*]` and
#      `[profile.release.build-override]` tables can silently weaken the
#      invariants for individual crates, so their presence fails the check.
#   3. Artifact checks (optional, with --wasm-dir) — every release WASM must
#      exist, start with the WASM magic header, and carry no DWARF debug
#      sections (`.debug_*`), which would indicate `debug`/`strip` were not
#      honoured.
#
# Usage:
#   bash scripts/check-release-profile.sh [--cargo-toml <path>] [--wasm-dir <path>]
#
# Exit codes:
#   0  all checks passed
#   1  one or more invariants violated (details printed as ::error lines)
#   2  usage error / input file missing

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_TOML="${REPO_ROOT}/Cargo.toml"
WASM_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --cargo-toml) CARGO_TOML="${2:?'--cargo-toml requires a value'}"; shift 2 ;;
    --wasm-dir)   WASM_DIR="${2:?'--wasm-dir requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ ! -f "$CARGO_TOML" ]]; then
  echo "::error::Cargo.toml not found at ${CARGO_TOML}" >&2
  exit 2
fi

# Required `[profile.release]` settings. Keep in sync with the verification
# checklist in docs/release-profile-verification.md.
declare -A REQUIRED=(
  ["opt-level"]='"z"'
  ["overflow-checks"]='true'
  ["debug"]='0'
  ["strip"]='"symbols"'
  ["debug-assertions"]='false'
  ["panic"]='"abort"'
  ["codegen-units"]='1'
  ["lto"]='true'
)

failed=0
fail() {
  echo "::error file=${CARGO_TOML}::$*"
  failed=1
}

# ── 1. Extract [profile.release] ──────────────────────────────────────────────
# Prints "key=value" pairs (comments and whitespace stripped) for the table,
# and "__TABLES__ <n>" with the number of [profile.release] headers seen.
profile_dump=$(awk '
  {
    line = $0
    sub(/#.*/, "", line)
    gsub(/^[ \t]+|[ \t]+$/, "", line)
  }
  line ~ /^\[/ {
    header = line
    gsub(/[ \t]/, "", header)
    in_release = (header == "[profile.release]")
    if (in_release) tables++
    next
  }
  in_release && line ~ /=/ {
    key = line; sub(/=.*/, "", key); gsub(/[ \t]/, "", key)
    val = line; sub(/^[^=]*=/, "", val); gsub(/^[ \t]+|[ \t]+$/, "", val)
    print key "=" val
  }
  END { print "__TABLES__ " (tables + 0) }
' "$CARGO_TOML")

table_count=$(printf '%s\n' "$profile_dump" | awk '/^__TABLES__/ { print $2 }')
if [[ "$table_count" -eq 0 ]]; then
  fail "[profile.release] table is missing"
elif [[ "$table_count" -gt 1 ]]; then
  fail "[profile.release] is declared ${table_count} times; expected exactly one"
fi

declare -A ACTUAL=()
declare -A SEEN=()
while IFS= read -r pair; do
  [[ -z "$pair" || "$pair" == __TABLES__* ]] && continue
  key="${pair%%=*}"
  val="${pair#*=}"
  if [[ -n "${SEEN[$key]:-}" ]]; then
    fail "[profile.release] sets '${key}' more than once"
  fi
  SEEN[$key]=1
  ACTUAL[$key]="$val"
done <<< "$profile_dump"

echo "Release profile: ${CARGO_TOML}"
for key in opt-level overflow-checks debug strip debug-assertions panic codegen-units lto; do
  expected="${REQUIRED[$key]}"
  actual="${ACTUAL[$key]:-<missing>}"
  if [[ "$actual" == "$expected" ]]; then
    printf '  OK    %-18s = %s\n' "$key" "$actual"
  else
    printf '  FAIL  %-18s = %s (expected %s)\n' "$key" "$actual" "$expected"
    fail "[profile.release] ${key} = ${actual}; expected ${expected} (see docs/release-profile-verification.md)"
  fi
done

# ── 2. Override guard ─────────────────────────────────────────────────────────
overrides=$(grep -nE '^[[:space:]]*\[profile\.release\.(package|build-override)' "$CARGO_TOML" || true)
if [[ -n "$overrides" ]]; then
  while IFS= read -r line; do
    fail "per-package release override is not allowed: ${line}"
  done <<< "$overrides"
fi

# ── 3. Artifact checks ────────────────────────────────────────────────────────
if [[ -n "$WASM_DIR" ]]; then
  echo "Release artifacts: ${WASM_DIR}"
  if [[ ! -d "$WASM_DIR" ]]; then
    echo "::error::wasm directory not found: ${WASM_DIR}"
    failed=1
  else
    found=0
    while IFS= read -r wasm; do
      found=1
      magic=$(head -c 4 "$wasm" | od -An -tx1 | tr -d ' \n')
      if [[ "$magic" != "0061736d" ]]; then
        echo "::error file=${wasm}::not a WASM module (magic ${magic:-<empty>})"
        failed=1
        continue
      fi
      if LC_ALL=C grep -aq '\.debug_' "$wasm"; then
        echo "::error file=${wasm}::contains DWARF .debug_* sections; release profile debug/strip settings were not applied"
        failed=1
        continue
      fi
      echo "  OK    $(basename "$wasm")"
    done < <(find "$WASM_DIR" -maxdepth 1 -name '*.wasm' -type f | sort)
    if [[ "$found" -eq 0 ]]; then
      echo "::error::no wasm artifacts found in ${WASM_DIR}; expected compiled contracts"
      failed=1
    fi
  fi
fi

if [[ "$failed" -ne 0 ]]; then
  echo "Release profile verification FAILED."
  exit 1
fi
echo "Release profile verification passed."
