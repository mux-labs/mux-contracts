#!/usr/bin/env bash
# check-security-policy.sh
#
# Guards against SECURITY.md drift (#673): smart-contract vulnerabilities
# must never be filed as public issues, and the private reporting channel,
# scope, and SLA must stay documented and internally consistent. Checks:
#
#   1. SECURITY.md tells reporters not to open public issues/PRs.
#   2. SECURITY.md documents a private contact (email or GitHub Security
#      Advisory link).
#   3. SECURITY.md has a Scope section.
#   4. SECURITY.md has a response-time / SLA section.
#   5. .well-known/security.txt exists and its Contact field is present.
#   6. Every relative doc path referenced by security.txt's Policy and
#      Acknowledgments fields resolves to a file that actually exists (a
#      stale link here would 404 for a researcher trying to reach the
#      private channel).
#   7. SECURITY.md and security.txt publish the same verified security email.
#   8. security.txt has canonical and policy metadata for this repository.
#   9. .github/ISSUE_TEMPLATE/config.yml exists and offers a contact link
#      that routes security reports off the public issue tracker.
#
# Usage:
#   bash scripts/check-security-policy.sh
#
# Exit 0 on success, 1 if any check fails.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECURITY_MD="${REPO_ROOT}/SECURITY.md"
SECURITY_TXT="${REPO_ROOT}/.well-known/security.txt"
ISSUE_CONFIG="${REPO_ROOT}/.github/ISSUE_TEMPLATE/config.yml"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --security-md) SECURITY_MD="${2:?'--security-md requires a value'}"; shift 2 ;;
    --security-txt) SECURITY_TXT="${2:?'--security-txt requires a value'}"; shift 2 ;;
    --issue-config) ISSUE_CONFIG="${2:?'--issue-config requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

FAILED=0

fail() {
  echo "  FAIL: $1"
  FAILED=1
}

ok() {
  echo "  OK:   $1"
}

echo "==> Checking SECURITY.md"

if [[ ! -f "$SECURITY_MD" ]]; then
  fail "$(basename "$SECURITY_MD") not found at $SECURITY_MD"
else
  if grep -qiE "do not open (a )?public issues?" "$SECURITY_MD"; then
    ok "tells reporters not to open public issues"
  else
    fail "missing an explicit 'do not open public issues' instruction"
  fi

  if grep -qE '[[:alnum:].+_-]+@[[:alnum:].-]+\.[[:alpha:]]{2,}' "$SECURITY_MD" \
    || grep -qiE "security/advisories" "$SECURITY_MD"; then
    ok "documents a private contact (email or Security Advisory link)"
  else
    fail "no private contact (email or Security Advisory link) found"
  fi

  if grep -qiE '^##+ .*scope' "$SECURITY_MD"; then
    ok "has a Scope section"
  else
    fail "missing a Scope section"
  fi

  if grep -qiE '^##+ .*(response time|timeline|sla)' "$SECURITY_MD"; then
    ok "has a response-time / SLA section"
  else
    fail "missing a response-time / SLA section"
  fi
fi

echo ""
echo "==> Checking .well-known/security.txt"

if [[ ! -f "$SECURITY_TXT" ]]; then
  fail "security.txt not found at $SECURITY_TXT"
else
  if grep -qE '^Contact:' "$SECURITY_TXT"; then
    ok "has a Contact field"
  else
    fail "missing a Contact field"
  fi

  # Any field pointing at a path under this repo's GitHub blob URL must
  # resolve to a real file, so a researcher following the link doesn't 404.
  while IFS= read -r url; do
    rel="${url#*/blob/main/}"
    target="${REPO_ROOT}/${rel}"
    if [[ -f "$target" ]]; then
      ok "linked doc exists: ${rel}"
    else
      fail "linked doc missing: ${rel} (referenced in security.txt)"
    fi
  done < <(grep -oE 'https://github\.com/[^[:space:]]+/blob/main/[^[:space:]]+' "$SECURITY_TXT" || true)
fi

echo ""
echo "==> Checking .github/ISSUE_TEMPLATE/config.yml"

if [[ ! -f "$ISSUE_CONFIG" ]]; then
  fail "config.yml not found at $ISSUE_CONFIG"
else
  if grep -qiE "security" "$ISSUE_CONFIG" && grep -qiE "advisories|security@" "$ISSUE_CONFIG"; then
    ok "offers a security-reporting contact link"
  else
    fail "no security-reporting contact link found"
  fi

  contact_email=$(grep -oE 'mailto:[^[:space:]]+' "$SECURITY_TXT" | head -1 | sed 's/^mailto://' || true)
  policy_email=$(grep -oE '[[:alnum:].+_-]+@[[:alnum:].-]+\.[[:alpha:]]{2,}' "$SECURITY_MD" | head -1 || true)
  if [[ -n "$contact_email" && "$contact_email" == "$policy_email" ]]; then
    ok "Contact email matches SECURITY.md verified contact"
  else
    fail "security.txt Contact does not match SECURITY.md verified contact"
  fi

  if grep -qE '^Canonical: https://github\.com/mux-labs/mux-contracts/blob/main/\.well-known/security\.txt$' "$SECURITY_TXT"; then
    ok "has the repository canonical security.txt URL"
  else
    fail "security.txt Canonical URL is missing or points at another repository"
  fi

  if grep -qE '^Policy: https://github\.com/mux-labs/mux-contracts/blob/main/SECURITY\.md$' "$SECURITY_TXT"; then
    ok "has the repository SECURITY.md policy URL"
  else
    fail "security.txt Policy URL is missing or points at another repository"
  fi
fi

echo ""
if (( FAILED )); then
  echo "ERROR: SECURITY.md / security.txt / issue template config drifted."
  echo "       See docs/security-acknowledgments.md and SECURITY.md for the"
  echo "       expected private-contact, scope, and SLA content."
  exit 1
fi

echo "All checks passed: security policy is hardened and internally consistent."
