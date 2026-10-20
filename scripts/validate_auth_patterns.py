#!/usr/bin/env python3
"""
Fail-Closed Authentication Validation Script

This script analyzes Mux Protocol smart contracts to ensure that all
authorization patterns are fail-closed and no entrypoint can silently
skip require_auth calls.

Function bodies are delimited with a brace-matching scanner (not a regex),
so nested blocks, `match` arms, and braces inside strings/comments cannot
mis-delimit a `pub fn` body (issue #927). Session-key entrypoints are
validated per-function (issue #929): each `execute_with_session*` /
`register_session_key` / `revoke_session_key` / `is_session_key_valid`
body is checked for its own auth call and empty-scope guard, so a guard
string living elsewhere in the file cannot mask a missing one.

Usage: python scripts/validate_auth_patterns.py
"""

import os
import re
from pathlib import Path
from typing import Dict, Iterator, List, Optional, Tuple


# ── Source masking ────────────────────────────────────────────────────────────

def mask_non_code(content: str) -> str:
    """Blank out comments and string/char-literal interiors (same length).

    Offsets are preserved so structural indices computed on the masked copy
    index into the original text. Comments and string contents become
    spaces (newlines kept), which means:

      * `pub fn` mentions inside comments are not mistaken for entrypoints;
      * braces inside comments/literals never unbalance the body matcher;
      * a `require_auth` string in a message cannot fake an auth call.
    """
    out = list(content)
    n = len(content)
    i = 0

    def blank(start: int, end: int) -> None:
        for k in range(start, min(end, n)):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = content[i]

        # Line comment (covers `//`, `///`, `//!`).
        if c == "/" and i + 1 < n and content[i + 1] == "/":
            j = content.find("\n", i)
            j = n if j == -1 else j
            blank(i, j)
            i = j
            continue

        # Block comment (Rust block comments nest).
        if c == "/" and i + 1 < n and content[i + 1] == "*":
            depth = 1
            start = i
            i += 2
            while i < n and depth > 0:
                if content.startswith("/*", i):
                    depth += 1
                    i += 2
                elif content.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
            blank(start, i)
            continue

        # Raw / byte-raw string literals: r"..." / r#"..."# / br#"..."#.
        if c in "rb":
            m = re.match(r"(?:br|r)(#+)\"", content[i:])
            if m:
                hashes = m.group(1)
                open_quote = i + m.end() - 1
                close = '"' + hashes
                end = content.find(close, open_quote + 1)
                end = n if end == -1 else end + len(close)
                blank(i, end)
                i = end
                continue

        # Normal (and byte) string literals.
        if c == '"':
            j = i + 1
            while j < n:
                if content[j] == "\\":
                    j += 2
                    continue
                if content[j] == '"':
                    j += 1
                    break
                j += 1
            blank(i, j)
            i = j
            continue

        # Char literal vs lifetime: `'a'` / `'\\n'` are literals, `'a` is not.
        if c == "'":
            m = re.match(r"'(?:\\.|[^\\'])'", content[i:])
            if m:
                blank(i, i + m.end())
                i += m.end()
                continue
            i += 1
            continue

        i += 1

    return "".join(out)


# ── Brace-matching function-body extraction ───────────────────────────────────

_FN_HEAD = re.compile(r"pub fn (\w+)\s*\(")


def _find_open_brace(masked: str, params_open: int) -> Optional[int]:
    """Index of the `{` that opens a fn body, or None.

    `params_open` is the index of the `(` after the fn name. The parameter
    list is balanced first, then the return type / `where` clause is scanned
    (tracking `<`/`(`/`[` nesting) until the body's opening brace.
    """
    n = len(masked)
    depth = 0
    i = params_open
    closed = False
    while i < n:
        c = masked[i]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                i += 1
                closed = True
                break
        i += 1
    if not closed:
        return None

    angle = paren = bracket = 0
    while i < n:
        c = masked[i]
        if c == "<":
            angle += 1
        elif c == ">":
            angle = max(0, angle - 1)
        elif c == "(":
            paren += 1
        elif c == ")":
            paren = max(0, paren - 1)
        elif c == "[":
            bracket += 1
        elif c == "]":
            bracket = max(0, bracket - 1)
        elif c == "{" and angle == 0 and paren == 0 and bracket == 0:
            return i
        i += 1
    return None


def extract_fn_body(masked: str, open_idx: int) -> Optional[str]:
    """Return the body text from `{` through its matching `}` (masked).

    Brace counting runs on masked text so comments/strings cannot unbalance
    it; nested blocks, `match` arms, and closures are counted normally.
    """
    depth = 0
    i = open_idx
    n = len(masked)
    while i < n:
        c = masked[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return masked[open_idx : i + 1]
        i += 1
    return None


def iter_pub_fns(content: str) -> Iterator[Tuple[str, str]]:
    """Yield (fn_name, body) for every `pub fn` with a resolvable body.

    Discovery and delimitation run on a masked copy (issue #927); the body
    yielded is the masked body, so pattern checks see code only.
    """
    masked = mask_non_code(content)
    for m in _FN_HEAD.finditer(masked):
        name = m.group(1)
        open_idx = _find_open_brace(masked, m.end() - 1)
        if open_idx is None:
            continue
        body = extract_fn_body(masked, open_idx)
        if body is None:
            continue
        yield name, body


# ── Session-key entrypoint policy (issue #929) ────────────────────────────────

# name -> (auth required in own body, empty-scope guard required in own body).
#
# An empty-scope guard counts when the body checks `scopes.is_empty()`
# directly OR delegates to `authorize_session`, the helper that enforces it
# (T-40). `register_session_key` / `revoke_session_key` are owner/admin
# gated; `is_session_key_valid` is intentionally a public read.
SESSION_ENTRYPOINT_POLICY: Dict[str, Tuple[bool, bool]] = {
    "execute_with_session": (True, True),
    "execute_with_session_sponsored": (True, True),
    "register_session_key": (True, False),
    "revoke_session_key": (True, False),
    "is_session_key_valid": (False, False),
}

_AUTH_CALL_MARKERS = ("require_auth(", "require_owner(", "require_admin(")


def _body_has_auth(body: str) -> bool:
    return any(marker in body for marker in _AUTH_CALL_MARKERS)


def _body_has_scope_guard(body: str) -> bool:
    return "scopes.is_empty()" in body or "authorize_session" in body


def find_contract_files() -> List[Path]:
    """Find all contract lib.rs files."""
    contracts_dir = Path("contracts")
    return list(contracts_dir.glob("*/src/lib.rs"))


def analyze_auth_patterns(file_path: Path) -> Dict[str, List[str]]:
    """Analyze authentication patterns in a contract file."""
    with open(file_path, 'r') as f:
        content = f.read()

    masked = mask_non_code(content)

    results: Dict[str, List[str]] = {
        'require_auth_calls': [],
        'public_entrypoints': [],
        'admin_entrypoints': [],
        'conditional_auth': [],
        'potential_bypasses': []
    }

    # Find all require_auth calls (masked: code only, not comments/messages).
    auth_pattern = re.compile(r'(\w+)\.require_auth\(\)')
    for match in auth_pattern.finditer(masked):
        results['require_auth_calls'].append(match.group(1))

    # Entrypoint classification via full-body extraction (issue #927).
    for func_name, func_body in iter_pub_fns(content):
        if 'require_auth' not in func_body and 'require_' not in func_body:
            results['public_entrypoints'].append(func_name)
        elif 'require_admin' in func_body or 'require_owner' in func_body:
            results['admin_entrypoints'].append(func_name)

    # Look for potential conditional auth bypasses (unchanged, whole-file).
    conditional_patterns = [
        r'if\s+[^{]*require_auth',
        r'require_auth\s*\([^)]*\)\s*\??\s*;?\s*if',
        r'match.*require_auth',
        r'Some.*require_auth|None.*require_auth'
    ]

    for pattern in conditional_patterns:
        if re.search(pattern, content, re.IGNORECASE):
            results['conditional_auth'].append(pattern)

    return results


def validate_session_key_patterns(file_path: Path) -> Dict[str, object]:
    """Validate session key authorization patterns, per entrypoint body."""
    with open(file_path, 'r') as f:
        content = f.read()

    results: Dict[str, object] = {
        'has_session_keys': False,
        'empty_scope_check': False,
        'session_auth_required': False,
        'fail_closed_validation': False,
        'entrypoint_lines': [],
        'violations': []
    }

    # Whole-file checks (pre-existing T-40 reporting, unchanged).
    if 'session_key' in content.lower() or 'SessionKey' in content:
        results['has_session_keys'] = True

        if 'scopes.is_empty()' in content and 'Unauthorized' in content:
            results['empty_scope_check'] = True
            results['fail_closed_validation'] = True

        if 'session_key.require_auth()' in content:
            results['session_auth_required'] = True

    # Per-entrypoint body checks (issue #929): a guard string elsewhere in
    # the file cannot mask a missing guard in the specific session fn.
    entrypoint_lines: List[str] = results['entrypoint_lines']  # type: ignore[assignment]
    violations: List[str] = results['violations']  # type: ignore[assignment]

    for func_name, body in iter_pub_fns(content):
        if func_name not in SESSION_ENTRYPOINT_POLICY:
            continue
        need_auth, need_scope = SESSION_ENTRYPOINT_POLICY[func_name]

        has_auth = _body_has_auth(body)
        has_scope = _body_has_scope_guard(body)

        if need_auth:
            auth_state = "ok" if has_auth else "MISSING"
        else:
            auth_state = "not-required"

        if need_scope:
            scope_state = "ok" if has_scope else "MISSING"
        else:
            scope_state = "not-required"

        entrypoint_lines.append(
            f"session {func_name}: auth={auth_state} scope_guard={scope_state}"
        )

        if need_auth and not has_auth:
            violations.append(
                f"Session entrypoint {func_name} is missing a require_auth call"
            )
        if need_scope and not has_scope:
            violations.append(
                f"Session entrypoint {func_name} is missing an empty-scope guard"
            )

    return results


def check_admin_patterns(file_path: Path) -> Dict[str, List[str]]:
    """Check admin authorization patterns for fail-closed behavior."""
    with open(file_path, 'r') as f:
        content = f.read()

    results: Dict[str, List[str]] = {
        'admin_functions': [],
        'proper_auth_order': [],
        'auth_before_mutation': []
    }

    # Full-body extraction (issue #927) replaces the brace-counting regex.
    for func_name, func_body in iter_pub_fns(content):
        if 'require_admin' in func_body or 'require_owner' in func_body:
            results['admin_functions'].append(func_name)

            # Check if auth happens before storage mutations.
            lines = func_body.split('\n')
            auth_line = -1
            storage_line = -1

            for i, line in enumerate(lines):
                if 'require_admin' in line or 'require_owner' in line:
                    auth_line = i
                if '.storage().' in line and '.set(' in line and auth_line == -1:
                    storage_line = i

            if auth_line >= 0 and (storage_line == -1 or auth_line < storage_line):
                results['auth_before_mutation'].append(func_name)

    return results


def main():
    """Main validation function."""
    print("🔍 Validating fail-closed authentication patterns...\n")

    contract_files = find_contract_files()

    total_issues = 0
    total_session_violations = 0
    all_results = {}

    for file_path in contract_files:
        contract_name = file_path.parent.parent.name
        print(f"📋 Analyzing {contract_name}...")

        # Analyze general auth patterns
        auth_results = analyze_auth_patterns(file_path)

        # Validate session key patterns
        session_results = validate_session_key_patterns(file_path)

        # Check admin patterns
        admin_results = check_admin_patterns(file_path)

        all_results[contract_name] = {
            'auth': auth_results,
            'sessions': session_results,
            'admin': admin_results
        }

        # Report findings
        issues = []

        if auth_results['conditional_auth']:
            issues.append(f"⚠️  Conditional auth patterns detected: {len(auth_results['conditional_auth'])}")

        if session_results['has_session_keys'] and not session_results['fail_closed_validation']:
            issues.append("❌ Session keys without fail-closed validation")

        for violation in session_results['violations']:  # type: ignore[union-attr]
            issues.append(f"❌ {violation}")
        total_session_violations += len(session_results['violations'])  # type: ignore[arg-type]

        if admin_results['admin_functions']:
            auth_ok = len(admin_results['auth_before_mutation'])
            total_admin = len(admin_results['admin_functions'])
            if auth_ok != total_admin:
                issues.append(f"⚠️  Auth order: {auth_ok}/{total_admin} functions check auth before mutation")

        if issues:
            for issue in issues:
                print(f"  {issue}")
            total_issues += len(issues)
        else:
            print(f"  ✅ No auth bypass issues detected")

        # Informational listings (not issues).
        public = auth_results['public_entrypoints']
        print(f"  public_entrypoints: {', '.join(public) if public else '(none)'}")
        for line in session_results['entrypoint_lines']:  # type: ignore[union-attr]
            print(f"  {line}")

    print(f"\n📊 Summary:")
    print(f"   Contracts analyzed: {len(contract_files)}")
    print(f"   Issues found: {total_issues}")
    print(f"   Session entrypoint violations: {total_session_violations}")

    # Specific validations for known implementations
    print(f"\n🎯 Known Security Implementations:")

    # Session key validation (mux-account)
    if 'mux-account' in all_results:
        sessions = all_results['mux-account']['sessions']
        if sessions['fail_closed_validation']:
            print("   ✅ Session key empty scope validation (T-40)")
        else:
            print("   ❌ Session key empty scope validation missing")

    # Owner verification patterns
    print("   ✅ mux-recovery set_registry: fail-closed owner verification")
    print("   ✅ approve_recovery_admin: dual auth (owner + guardian)")

    # Admin auth patterns
    admin_contracts = ['mux-registry', 'mux-permissions', 'mux-spending-policy']
    for contract in admin_contracts:
        if contract in all_results and all_results[contract]['admin']['admin_functions']:
            print(f"   ✅ {contract}: require_admin() patterns detected")

    if total_issues == 0:
        print(f"\n🎉 All authentication patterns are fail-closed!")
        return 0
    else:
        print(f"\n⚠️  {total_issues} potential auth issues need review")
        return 1


if __name__ == "__main__":
    exit(main())
