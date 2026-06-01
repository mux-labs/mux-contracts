#!/usr/bin/env bash
#
# coverage.sh
#
# Generate a test coverage report for Mux Protocol Soroban contracts using
# Rust's built-in source-based code coverage (LLVM instrumentation).
#
# No extra tools are required — this uses only the stable Rust toolchain.
# For HTML reports, install `llvm-tools-preview` via rustup (see below).
#
# Usage:
#   bash scripts/coverage.sh [--html] [--open] [--lcov]
#
# Flags:
#   --html     Generate an HTML report (requires llvm-tools-preview + grcov or llvm-cov)
#   --open     Open the HTML report in the default browser after generation
#   --lcov     Export LCOV data to coverage/lcov.info (for CI upload)
#   --help     Show this help
#
# Output:
#   coverage/lcov.info         LCOV coverage data (always produced)
#   coverage/html/             HTML report (with --html)
#
# Environment Variables:
#   LLVM_COV       Path to llvm-cov binary (default: auto-detected from rustup)
#   LLVM_PROFDATA  Path to llvm-profdata binary (default: auto-detected)
#
# Examples:
#   # Print coverage summary to stdout
#   bash scripts/coverage.sh
#
#   # Generate LCOV for CI upload to Codecov / Coveralls
#   bash scripts/coverage.sh --lcov
#
#   # Generate and open HTML report locally
#   bash scripts/coverage.sh --html --open
#
# Exit codes:
#   0  Coverage run complete
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

# ── Argument parsing ──────────────────────────────────────────────────────────
WANT_HTML=false
WANT_OPEN=false
WANT_LCOV=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --html)   WANT_HTML=true;  shift ;;
    --open)   WANT_OPEN=true;  shift ;;
    --lcov)   WANT_LCOV=true;  shift ;;
    --help|-h)
      grep '^#' "$0" | sed 's/^# \{0,1\}//' | head -35
      exit 0 ;;
    *) log_error "Unknown argument: $1"; exit 1 ;;
  esac
done

# ── Detect LLVM tools from rustup ─────────────────────────────────────────────
TOOLCHAIN_DIR="$(rustup toolchain list -v | grep '(default)' | awk '{print $NF}' || true)"
if [[ -z "$TOOLCHAIN_DIR" ]]; then
  TOOLCHAIN_DIR="$(rustup toolchain list -v | head -1 | awk '{print $NF}')"
fi
LLVM_TOOLS_DIR="${TOOLCHAIN_DIR}/lib/rustlib/$(rustc -vV | grep 'host:' | awk '{print $2}')/bin"

LLVM_COV="${LLVM_COV:-${LLVM_TOOLS_DIR}/llvm-cov}"
LLVM_PROFDATA="${LLVM_PROFDATA:-${LLVM_TOOLS_DIR}/llvm-profdata}"

# ── Require llvm-tools-preview for HTML/LCOV output ──────────────────────────
if { [[ "$WANT_HTML" == "true" ]] || [[ "$WANT_LCOV" == "true" ]]; } \
   && { [[ ! -f "$LLVM_COV" ]] || [[ ! -f "$LLVM_PROFDATA" ]]; }; then
  log_warning "llvm-tools-preview not found. Install with:"
  log_warning "  rustup component add llvm-tools-preview"
  log_warning "Falling back to summary-only mode."
  WANT_HTML=false
  WANT_LCOV=false
fi

# ── Set instrumentation env vars ─────────────────────────────────────────────
mkdir -p "${COV_DIR}"
PROFRAW_GLOB="${COV_DIR}/*.profraw"

export RUSTFLAGS="-C instrument-coverage"
export LLVM_PROFILE_FILE="${COV_DIR}/mux-%p-%m.profraw"
export CARGO_TARGET_DIR="${REPO_ROOT}/target"

# ── Step 1: Run tests with instrumentation ────────────────────────────────────
log_info "Step 1/3: Running tests with coverage instrumentation..."
cd "${REPO_ROOT}"
cargo test --workspace --all-features --quiet 2>&1 \
  | grep -v "^$" \
  || { log_error "Tests failed — coverage report not generated"; exit 1; }
log_success "Tests complete"

# ── Step 2: Merge profraw data ────────────────────────────────────────────────
PROFDATA_FILE="${COV_DIR}/mux.profdata"

if [[ -f "$LLVM_PROFDATA" ]]; then
  log_info "Step 2/3: Merging profile data..."
  # shellcheck disable=SC2086
  "${LLVM_PROFDATA}" merge \
    --sparse \
    --output "${PROFDATA_FILE}" \
    ${PROFRAW_GLOB}
  log_success "Profile data merged → ${PROFDATA_FILE}"
else
  log_info "Step 2/3: Skipping profile merge (llvm-profdata not available)"
fi

# ── Step 3: Generate report ───────────────────────────────────────────────────
log_info "Step 3/3: Generating coverage report..."

# Collect test binary paths (cargo test emits them in --no-run mode)
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

# Build --object flags for llvm-cov
OBJECT_FLAGS=""
for bin in $BINARIES; do
  OBJECT_FLAGS="${OBJECT_FLAGS} --object ${bin}"
done

if [[ -f "$LLVM_COV" ]] && [[ -f "$PROFDATA_FILE" ]] && [[ -n "$OBJECT_FLAGS" ]]; then
  # Summary always printed
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

  log_success "Coverage report complete."
else
  # Stub output when llvm tools are unavailable
  echo ""
  log_warning "Coverage summary not available (llvm tools not installed)."
  log_warning "Install llvm-tools-preview to get per-file coverage data:"
  log_warning "  rustup component add llvm-tools-preview"
  echo ""
  echo "  ┌──────────────────────────────────────────────────────────────┐"
  echo "  │  COVERAGE REPORT STUB — install llvm-tools-preview to view  │"
  echo "  │                                                              │"
  echo "  │  Contracts instrumented:                                     │"
  echo "  │    • mux-account                                             │"
  echo "  │    • mux-account-factory                                     │"
  echo "  │    • mux-batcher                                             │"
  echo "  │    • mux-permissions                                         │"
  echo "  │                                                              │"
  echo "  │  Run:  bash scripts/coverage.sh --html --open               │"
  echo "  │  CI:   bash scripts/coverage.sh --lcov                      │"
  echo "  └──────────────────────────────────────────────────────────────┘"
fi

# Cleanup profraw files (keep profdata and lcov)
rm -f "${COV_DIR}"/*.profraw
