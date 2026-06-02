#!/usr/bin/env bash
# ==============================================================================
# scripts/verify-wasm-hash.sh
#
# Verifies the SHA-256 hash of a compiled Soroban WASM file against an
# expected hash. Prevents deploying tampered or mismatched binaries.
#
# Usage:
#   ./scripts/verify-wasm-hash.sh <wasm_file> <expected_sha256_hex>
#
# Exit codes:
#   0 — hash matches (safe to deploy)
#   1 — hash mismatch (do NOT deploy)
#   2 — usage error or file not found
#
# Examples:
#   ./scripts/verify-wasm-hash.sh \
#     target/wasm32-unknown-unknown/release/mux_account.wasm \
#     a1b2c3d4...
#
#   # In CI — fail fast if hash doesn't match
#   ./scripts/verify-wasm-hash.sh "$WASM" "$EXPECTED_HASH" || exit 1
# ==============================================================================

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'

usage() {
  echo "Usage: $0 <wasm_file> <expected_sha256_hex>"
  echo ""
  echo "Arguments:"
  echo "  wasm_file           Path to the compiled .wasm file"
  echo "  expected_sha256_hex Expected SHA-256 hash (64 lowercase hex chars)"
  echo ""
  echo "Options:"
  echo "  --compute-only      Print the hash without comparing (for first-time setup)"
  echo "  --help              Show this help"
  exit 2
}

COMPUTE_ONLY=false
POSITIONAL=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --compute-only) COMPUTE_ONLY=true; shift ;;
    --help|-h) usage ;;
    -*) echo "Unknown option: $1" >&2; usage ;;
    *) POSITIONAL+=("$1"); shift ;;
  esac
done

_compute_hash() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

if [ "$COMPUTE_ONLY" = true ]; then
  WASM_FILE="${POSITIONAL[0]:-}"
  [ -z "$WASM_FILE" ] && { echo "Error: wasm_file required" >&2; usage; }
  [ -f "$WASM_FILE" ] || { echo "Error: file not found: $WASM_FILE" >&2; exit 2; }
  echo "SHA-256 of $WASM_FILE:"
  _compute_hash "$WASM_FILE"
  exit 0
fi

[ ${#POSITIONAL[@]} -lt 2 ] && usage

WASM_FILE="${POSITIONAL[0]}"
EXPECTED_HASH="${POSITIONAL[1]}"

[ -f "$WASM_FILE" ] || {
  echo -e "${RED}[ERROR]${NC} WASM file not found: $WASM_FILE" >&2
  exit 2
}

if ! echo "$EXPECTED_HASH" | grep -qE '^[0-9a-f]{64}$'; then
  echo -e "${RED}[ERROR]${NC} Expected hash is not valid SHA-256 (64 lowercase hex chars): $EXPECTED_HASH" >&2
  exit 2
fi

ACTUAL_HASH=$(_compute_hash "$WASM_FILE")

echo "File:     $WASM_FILE"
echo "Expected: $EXPECTED_HASH"
echo "Actual:   $ACTUAL_HASH"
echo ""

if [ "$ACTUAL_HASH" = "$EXPECTED_HASH" ]; then
  echo -e "${GREEN}✓ Hash verified — safe to deploy${NC}"
  exit 0
else
  echo -e "${RED}✗ Hash MISMATCH — do NOT deploy this binary${NC}"
  echo ""
  echo "This could indicate:"
  echo "  - A different Rust toolchain version was used to build"
  echo "  - Source files were modified after the expected hash was recorded"
  echo "  - Build reproducibility issue (check Cargo.lock and RUSTFLAGS)"
  echo ""
  echo "To update the expected hash, run with --compute-only:"
  echo "  $0 --compute-only $WASM_FILE"
  exit 1
fi
