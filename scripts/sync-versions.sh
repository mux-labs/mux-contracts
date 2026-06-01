#!/usr/bin/env bash
#
# sync-versions.sh
#
# Keeps the TypeScript bindings package version in sync with the Rust workspace version.
#
# How versioning works:
#   - The authoritative version lives in [workspace.package] version in Cargo.toml.
#   - This script reads that version and writes it to bindings/package.json.
#   - Run after bumping Cargo.toml, or let the CI check flag drift.
#
# Usage:
#   bash scripts/sync-versions.sh [--check] [--help]
#
# Flags:
#   --check    Exit non-zero if versions are out of sync (used in CI)
#   --help     Show this help
#
# Examples:
#   # Sync bindings/package.json to match Cargo.toml
#   bash scripts/sync-versions.sh
#
#   # Check for drift (CI mode — no writes)
#   bash scripts/sync-versions.sh --check
#
# Exit codes:
#   0  Versions in sync (or successfully synced)
#   1  Versions out of sync (--check mode) or error

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_TOML="${REPO_ROOT}/Cargo.toml"
PKG_JSON="${REPO_ROOT}/bindings/package.json"

# ── Colours ───────────────────────────────────────────────────────────────────
BLUE='\033[0;34m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
log_info()    { echo -e "${BLUE}ℹ️  ${NC}$*"; }
log_success() { echo -e "${GREEN}✓${NC} $*"; }
log_error()   { echo -e "${RED}✗${NC} $*" >&2; }

# ── Parse args ────────────────────────────────────────────────────────────────
CHECK_ONLY=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) CHECK_ONLY=true; shift ;;
    --help|-h)
      grep '^#' "$0" | sed 's/^# \{0,1\}//' | head -25
      exit 0 ;;
    *) log_error "Unknown argument: $1"; exit 1 ;;
  esac
done

# ── Extract Cargo workspace version ──────────────────────────────────────────
# Reads [workspace.package] version = "..." from Cargo.toml
CARGO_VERSION=$(
  python3 - "${CARGO_TOML}" <<'PYEOF'
import sys, re
with open(sys.argv[1]) as f:
    content = f.read()
# Find [workspace.package] section, then the first version = "..." line in it
m = re.search(r'\[workspace\.package\][^\[]*version\s*=\s*"([^"]+)"', content, re.DOTALL)
if not m:
    sys.exit("ERROR: could not find [workspace.package] version in Cargo.toml")
print(m.group(1))
PYEOF
)

# ── Extract bindings package.json version ────────────────────────────────────
PKG_VERSION=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'])" "${PKG_JSON}")

log_info "Cargo workspace version  : ${CARGO_VERSION}"
log_info "bindings package version : ${PKG_VERSION}"

if [[ "${CARGO_VERSION}" == "${PKG_VERSION}" ]]; then
  log_success "Versions are in sync (${CARGO_VERSION})"
  exit 0
fi

# ── Versions differ ───────────────────────────────────────────────────────────
if [[ "${CHECK_ONLY}" == "true" ]]; then
  log_error "Version mismatch: Cargo.toml=${CARGO_VERSION}, bindings/package.json=${PKG_VERSION}"
  echo ""
  echo "Fix: run  bash scripts/sync-versions.sh  and commit the result."
  exit 1
fi

# ── Apply sync ────────────────────────────────────────────────────────────────
log_info "Updating bindings/package.json: ${PKG_VERSION} → ${CARGO_VERSION}"

python3 - "${PKG_JSON}" "${CARGO_VERSION}" <<'PYEOF'
import json, sys
path, new_version = sys.argv[1], sys.argv[2]
with open(path) as f:
    pkg = json.load(f)
pkg["version"] = new_version
with open(path, "w") as f:
    json.dump(pkg, f, indent=2)
    f.write("\n")
PYEOF

log_success "bindings/package.json updated to ${CARGO_VERSION}"
echo ""
echo "Next steps:"
echo "  1. git add bindings/package.json"
echo "  2. git commit -m \"chore: sync bindings version to ${CARGO_VERSION}\""
echo "  3. Open a PR"
