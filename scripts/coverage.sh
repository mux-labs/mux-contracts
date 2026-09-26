#!/usr/bin/env bash
#
# coverage.sh  (#662)
#
# Generate a test coverage report for Mux Protocol Soroban contracts.
#
# Primary path  — cargo-llvm-cov (recommended, bundles its own llvm tools):
#   cargo install cargo-llvm-cov
#   rustup component add llvm-tools-preview
#
# Fallback path — raw llvm-tools-preview without cargo-llvm-cov (legacy).
# Stub          — printed when no llvm tooling is present at all.
#
# Usage:
#   bash scripts/coverage.sh [--html] [--open] [--lcov] [--stub] [--help]
#
# Flags:
#   --html   Generate an HTML report (saved to coverage/html/)
#   --open   Open the HTML report in the default browser after generation
#   --lcov   Export LCOV data to coverage/lcov.info (use for CI upload)
#   --stub   Print the coverage report stub only (no tests; for script checks)
#   --help   Show this help
#
# Output:
#   coverage/lcov.info    LCOV coverage data (--lcov)
#   coverage/html/        HTML report (--html)
#
# CI quick-start:
#   # Install once in the CI job:
#   cargo install cargo-llvm-cov --locked
#   rustup component add llvm-tools-preview
#
#   # Run coverage and emit LCOV:
#   bash scripts/coverage.sh --lcov
#   # or via make:
#   make coverage-ci
#
# Exit codes:
#   0  Coverage run complete (or stub printed)
#   1  Toolchain error or test failure

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COV_DIR="${REPO_ROOT}/coverage"

# ── Colours ───────────────────────────────────────────────────────────────────
BLUE='\033[0;34m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
log_info()    { echo -e "${BLUE}ℹ️  ${NC}$*"; }
log_success() { echo -e "${GREEN}✓${NC} $*"; }
log_warning() { echo -e "${YELLOW}⚠️  ${NC}$*"; }
log_error()   { echo -e "${RED}✗${NC} $*" >&2; }

# ── Stub (backward-compatible, also used by test-coverage.sh) ─────────────────
print_coverage_stub() {
  echo ""
  log_warning "Coverage summary not available (llvm tools not installed)."
  log_warning "Install cargo-llvm-cov and llvm-tools-preview to get coverage data:"
  log_warning "  cargo install cargo-llvm-cov --locked"
  log_warning "  rustup component add llvm-tools-preview"
  echo ""
  echo "  ┌──────────────────────────────────────────────────────────────┐"
  echo "  │  COVERAGE REPORT STUB — install llvm-tools-preview to view  │"
  echo "  │                                                              │"
  echo "  │  Workspace crates instrumented:                              │"
  echo "  │    • mux-account                                             │"
  echo "  │    • mux-account-factory                                     │"
  echo "  │    • mux-batcher                                             │"
  echo "  │    • mux-delegation                                          │"
  echo "  │    • mux-permissions                                         │"
  echo "  │    • mux-policy                                              │"
  echo "  │    • mux-recovery                                            │"
  echo "  │    • mux-registry                                            │"
  echo "  │    • mux-spending-policy                                     │"
  echo "  │    • mux-wallet-registry                                     │"
  echo "  │                                                              │"
  echo "  │  Run:  bash scripts/coverage.sh --html --open               │"
  echo "  │  CI:   bash scripts/coverage.sh --lcov                      │"
  echo "  │  Make: make coverage                                         │"
  echo "  └──────────────────────────────────────────────────────────────┘"
}

# ── Argument parsing ──────────────────────────────────────────────────────────
WANT_HTML=false
WANT_OPEN=false
WANT_LCOV=false
STUB_ONLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --html)   WANT_HTML=true;  shift ;;
    --open)   WANT_OPEN=true;  shift ;;
    --lcov)   WANT_LCOV=true;  shift ;;
    --stub)   STUB_ONLY=true;  shift ;;
    --help|-h)
      grep '^#' "$0" | sed 's/^# \{0,1\}//' | head -50
      exit 0 ;;
    *) log_error "Unknown argument: $1"; exit 1 ;;
  esac
done

if [[ "$STUB_ONLY" == "true" ]]; then
  print_coverage_stub
  exit 0
fi

mkdir -p "${COV_DIR}"

# ── Primary path: cargo-llvm-cov ──────────────────────────────────────────────
# cargo-llvm-cov bundles its own copy of llvm-cov/llvm-profdata so we don't
# have to locate them manually from the toolchain directory.
if command -v cargo-llvm-cov >/dev/null 2>&1 \
   || cargo llvm-cov --version >/dev/null 2>&1; then

  log_info "Using cargo-llvm-cov (primary path)"

  # Build the argument list for cargo llvm-cov.
  # We exclude test-only crates and external deps from the report.
  LLVM_COV_ARGS=(
    --workspace
    --all-features
    --exclude soroban-test-helpers
    --exclude mux-contract-tests
    --ignore-filename-regex '(/.cargo/|/rustc/|/tests/)'
  )

  if [[ "$WANT_LCOV" == "true" ]]; then
    log_info "Running tests with LCOV output → ${COV_DIR}/lcov.info"
    cargo llvm-cov \
      "${LLVM_COV_ARGS[@]}" \
      --lcov \
      --output-path "${COV_DIR}/lcov.info"
    log_success "LCOV → ${COV_DIR}/lcov.info"
  fi

  if [[ "$WANT_HTML" == "true" ]]; then
    HTML_DIR="${COV_DIR}/html"
    log_info "Running tests with HTML output → ${HTML_DIR}/"
    cargo llvm-cov \
      "${LLVM_COV_ARGS[@]}" \
      --html \
      --output-dir "${HTML_DIR}"
    log_success "HTML report → ${HTML_DIR}/index.html"

    if [[ "$WANT_OPEN" == "true" ]]; then
      if command -v open &>/dev/null; then
        open "${HTML_DIR}/index.html"
      elif command -v xdg-open &>/dev/null; then
        xdg-open "${HTML_DIR}/index.html"
      fi
    fi
  fi

  # Always print a summary to stdout (even if we also emitted LCOV/HTML).
  log_info "Coverage summary:"
  cargo llvm-cov \
    "${LLVM_COV_ARGS[@]}" \
    --summary-only 2>&1 || true

  log_success "Coverage report complete (cargo-llvm-cov)."
  exit 0
fi

# ── Fallback path: raw llvm-tools-preview ─────────────────────────────────────
log_warning "cargo-llvm-cov not found. Falling back to raw llvm-tools-preview."
log_warning "For a better experience install cargo-llvm-cov:"
log_warning "  cargo install cargo-llvm-cov --locked"

TOOLCHAIN_DIR="$(rustup toolchain list -v 2>/dev/null | grep '(default)' | awk '{print $NF}' || true)"
if [[ -z "$TOOLCHAIN_DIR" ]]; then
  TOOLCHAIN_DIR="$(rustup toolchain list -v 2>/dev/null | head -1 | awk '{print $NF}' || true)"
fi

HOST_TRIPLE="$(rustc -vV 2>/dev/null | grep 'host:' | awk '{print $2}' || true)"
LLVM_TOOLS_DIR="${TOOLCHAIN_DIR}/lib/rustlib/${HOST_TRIPLE}/bin"

LLVM_COV="${LLVM_COV:-${LLVM_TOOLS_DIR}/llvm-cov}"
LLVM_PROFDATA="${LLVM_PROFDATA:-${LLVM_TOOLS_DIR}/llvm-profdata}"

if [[ ! -f "$LLVM_COV" ]] || [[ ! -f "$LLVM_PROFDATA" ]]; then
  # Neither cargo-llvm-cov nor raw llvm tools available — print stub and exit 0
  # so that local dev machines without the tooling don't break `make coverage`.
  print_coverage_stub
  exit 0
fi

PROFRAW_GLOB="${COV_DIR}/*.profraw"

export RUSTFLAGS="-C instrument-coverage"
export LLVM_PROFILE_FILE="${COV_DIR}/mux-%p-%m.profraw"
export CARGO_TARGET_DIR="${REPO_ROOT}/target"

log_info "Step 1/3: Running tests with coverage instrumentation..."
cd "${REPO_ROOT}"
cargo test --workspace --all-features --quiet 2>&1 \
  | grep -v "^$" \
  || { log_error "Tests failed — coverage report not generated"; exit 1; }
log_success "Tests complete"

PROFDATA_FILE="${COV_DIR}/mux.profdata"
log_info "Step 2/3: Merging profile data..."
# shellcheck disable=SC2086
"${LLVM_PROFDATA}" merge --sparse --output "${PROFDATA_FILE}" ${PROFRAW_GLOB}
log_success "Profile data merged → ${PROFDATA_FILE}"

log_info "Step 3/3: Generating coverage report..."
BINARIES=$(
  cargo test --workspace --all-features --no-run --message-format=json 2>/dev/null \
    | python3 -c "
import sys, json
for line in sys.stdin:
    try:
        m = json.loads(line)
        if m.get('reason') == 'compiler-artifact' and m.get('executable'):
            print(m['executable'])
    except Exception:
        pass
" 2>/dev/null || true
)

OBJECT_FLAGS=""
for bin in $BINARIES; do
  OBJECT_FLAGS="${OBJECT_FLAGS} --object ${bin}"
done

if [[ -n "$OBJECT_FLAGS" ]]; then
  log_info "Coverage summary:"
  # shellcheck disable=SC2086
  "${LLVM_COV}" report \
    --use-color \
    --instr-profile="${PROFDATA_FILE}" \
    ${OBJECT_FLAGS} \
    --ignore-filename-regex='/.cargo/|/rustc/' \
    | grep -v "^Filename" | head -40 || true

  if [[ "$WANT_LCOV" == "true" ]]; then
    log_info "Exporting LCOV data → ${COV_DIR}/lcov.info"
    # shellcheck disable=SC2086
    "${LLVM_COV}" export \
      --format=lcov \
      --instr-profile="${PROFDATA_FILE}" \
      ${OBJECT_FLAGS} \
      --ignore-filename-regex='/.cargo/|/rustc/' \
      > "${COV_DIR}/lcov.info"
    log_success "LCOV → ${COV_DIR}/lcov.info"
  fi

  if [[ "$WANT_HTML" == "true" ]]; then
    HTML_DIR="${COV_DIR}/html"
    log_info "Generating HTML report → ${HTML_DIR}"
    mkdir -p "${HTML_DIR}"
    # shellcheck disable=SC2086
    "${LLVM_COV}" show \
      --use-color \
      --format=html \
      --instr-profile="${PROFDATA_FILE}" \
      ${OBJECT_FLAGS} \
      --ignore-filename-regex='/.cargo/|/rustc/' \
      --output-dir="${HTML_DIR}"
    log_success "HTML report → ${HTML_DIR}/index.html"

    if [[ "$WANT_OPEN" == "true" ]]; then
      if command -v open &>/dev/null; then
        open "${HTML_DIR}/index.html"
      elif command -v xdg-open &>/dev/null; then
        xdg-open "${HTML_DIR}/index.html"
      fi
    fi
  fi

  log_success "Coverage report complete (raw llvm-tools-preview)."
else
  print_coverage_stub
fi

# Cleanup profraw files (keep profdata and lcov)
rm -f "${COV_DIR}"/*.profraw
