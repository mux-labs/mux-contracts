#!/usr/bin/env python3
"""
Entrypoint Matrix Gap Detection Script

This script verifies that all implemented contract entrypoints are documented
in docs/entrypoint-matrix.md.

Entrypoints are *discovered* from the contract sources: every `pub fn` inside
a `#[contractimpl]` block in `contracts/<crate>/src/lib.rs` is collected and
compared against the entrypoints parsed from the matrix. There is no
hard-coded list of missing entrypoints, so a newly added entrypoint is flagged
automatically until it is documented — the matrix cannot silently drift out of
date again. Test-only code (`#[cfg(test)]` modules) is ignored.

Usage: python3 scripts/check_entrypoint_matrix.py
       python3 scripts/check_entrypoint_matrix.py --matrix PATH --contracts-dir DIR

Exit codes:
    0  every discovered entrypoint is documented in the matrix
    1  one or more discovered entrypoints are missing from the matrix

Smoke test (Python standard library only, no network access):
    python3 scripts/check_entrypoint_matrix.py && echo OK

Self-test: scripts/test-check-entrypoint-matrix.sh
"""

import argparse
import sys
import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MATRIX = REPO_ROOT / "docs" / "entrypoint-matrix.md"
DEFAULT_CONTRACTS_DIR = REPO_ROOT / "contracts"

# Optional allowlist of (contract, function) pairs that are intentionally NOT
# documented in docs/entrypoint-matrix.md. Every entry MUST carry a comment
# explaining why it is excluded. The list is empty today: every discovered
# #[contractimpl] entrypoint is documented.
ALLOWLIST = {
    # ("mux-foo", "internal_probe"),  # why: never callable off-chain
}

_CONTRACTIMPL_RE = re.compile(r"#\[contractimpl\]")
_CFG_TEST_RE = re.compile(r"#\[cfg\(\s*test\s*\)\]")
_PUB_FN_RE = re.compile(r"\bpub\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")


def read_entrypoint_matrix(matrix_path=DEFAULT_MATRIX):
    """Parse docs/entrypoint-matrix.md and extract documented entrypoints."""
    matrix_path = Path(matrix_path)
    if not matrix_path.exists():
        print(f"❌ ERROR: {matrix_path} not found")
        return {}
    
    with open(matrix_path, 'r') as f:
        content = f.read()
    
    documented = {}
    current_contract = ""
    
    for line in content.split('\n'):
        # Detect contract section headers (e.g., "## mux-account")
        if line.startswith("## "):
            current_contract = line[3:].strip()
            documented[current_contract] = []
            continue
        
        # Parse entrypoint table rows
        if line.startswith('|') and "Entrypoint" not in line and "---" not in line:
            parts = [p.strip() for p in line.split('|')]
            if len(parts) >= 3 and parts[1]:
                # Extract function name, removing backticks and parameters
                entrypoint_cell = parts[1].strip('`')
                func_name = entrypoint_cell.split('(')[0].strip()
                
                if func_name and func_name not in ["Auth", "Notes", ""]:
                    documented[current_contract].append(func_name)
    
    return documented


def _code_lines(lines):
    """Yield source lines with // and /* */ comments stripped.

    Keeps the scanner from matching `pub fn` examples inside doc comments or
    commented-out code blocks.
    """
    in_block = False
    for line in lines:
        out = []
        i = 0
        n = len(line)
        while i < n:
            if in_block:
                end = line.find("*/", i)
                if end == -1:
                    i = n
                else:
                    in_block = False
                    i = end + 2
                continue
            slash = line.find("//", i)
            block = line.find("/*", i)
            if slash != -1 and (block == -1 or slash < block):
                out.append(line[i:slash])
                i = n
            elif block != -1:
                out.append(line[i:block])
                i = block + 2
                in_block = True
            else:
                out.append(line[i:])
                i = n
        yield "".join(out)


def _entrypoints_in_file(lib_path):
    """Collect `pub fn` names inside `#[contractimpl]` blocks of one file.

    Tracks brace depth to know when a `#[contractimpl]` impl block opens and
    closes, and skips `#[cfg(test)]` modules entirely — test-only contracts
    defined for harnesses are not part of the deployed ABI.
    """
    names = []
    armed = False        # saw #[contractimpl]; waiting for the impl block
    entered = False      # impl block brace has been opened at least once
    depth = 0            # brace depth inside the active impl block
    skipping = False     # inside a #[cfg(test)] module
    scan = 0             # brace depth while skipping
    scan_entered = False # saw an opening brace while skipping

    for line in _code_lines(lib_path.read_text().splitlines()):
        if skipping:
            scan += line.count("{") - line.count("}")
            if scan > 0:
                scan_entered = True
            if scan_entered and scan <= 0:
                skipping = False
                scan_entered = False
            continue
        if _CFG_TEST_RE.search(line):
            skipping = True
            scan = 0
            scan_entered = False
            continue
        if _CONTRACTIMPL_RE.search(line):
            armed = True
            entered = False
            depth = 0
        if armed:
            depth += line.count("{") - line.count("}")
            if depth > 0:
                entered = True
                names.extend(m.group(1) for m in _PUB_FN_RE.finditer(line))
            elif entered:
                # The impl block closed; wait for the next #[contractimpl].
                armed = False
                entered = False
    return names


def discover_entrypoints(contracts_dir=DEFAULT_CONTRACTS_DIR):
    """Walk contracts/*/src/lib.rs and collect #[contractimpl] entrypoints.

    Returns {contract_name: [function_name, ...]} for every contract crate
    that defines at least one entrypoint.
    """
    contracts_dir = Path(contracts_dir)
    discovered = {}
    for lib_path in sorted(contracts_dir.glob("*/src/lib.rs")):
        contract = lib_path.parent.parent.name
        fns = _entrypoints_in_file(lib_path)
        if fns:
            # De-duplicate while preserving order (multiple impl blocks).
            seen = []
            for fn in fns:
                if fn not in seen:
                    seen.append(fn)
            discovered[contract] = seen
    return discovered


def main(argv=None):
    """Check for entrypoint matrix gaps."""
    parser = argparse.ArgumentParser(
        description="Verify docs/entrypoint-matrix.md covers every "
                    "#[contractimpl] entrypoint discovered in the sources.",
    )
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX,
                        help="path to entrypoint-matrix.md")
    parser.add_argument("--contracts-dir", type=Path, default=DEFAULT_CONTRACTS_DIR,
                        help="path to the contracts/ directory")
    args = parser.parse_args(argv)

    print("🔍 Checking entrypoint matrix for gaps...\n")

    contracts_dir = Path(args.contracts_dir)
    if not list(contracts_dir.glob("*/src/lib.rs")):
        print(f"❌ ERROR: no contract sources found under {contracts_dir} "
              "(expected contracts/<crate>/src/lib.rs)")
        print("   If the contracts directory moved, update "
              "scripts/check_entrypoint_matrix.py accordingly.")
        sys.exit(1)

    # Parse documented entrypoints from the matrix.
    documented = read_entrypoint_matrix(args.matrix)

    # Discover implemented entrypoints from the contract sources.
    discovered = discover_entrypoints(args.contracts_dir)

    # missing = discovered - documented, per contract.
    still_missing = [
        (contract, function)
        for contract in sorted(discovered)
        for function in discovered[contract]
        if (contract, function) not in ALLOWLIST
        and function not in documented.get(contract, [])
    ]
    
    if still_missing:
        print("❌ ENTRYPOINT MATRIX GAP DETECTED:")
        print("The following implemented entrypoints are missing from docs/entrypoint-matrix.md:\n")
        
        for contract, function in still_missing:
            print(f"  {contract} :: {function}")
        
        print("\n🔧 REQUIRED ACTION:")
        print("Add these entrypoints to docs/entrypoint-matrix.md with proper auth classification (A/U/P)")
        print("This is required for Soroban audit readiness and mainnet deployment.")
        print("\n📍 Each entrypoint needs a table row like:")
        print("| `entrypoint_name(params)` | A/U/P | Description and auth requirements |")
        print("\nLegend: A=Admin, U=User/Actor auth required, P=Public read-only")
        
        print(f"\n💥 AUDIT BLOCKER: {len(still_missing)} entrypoints missing from matrix")
        sys.exit(1)
    
    print("✅ All discovered entrypoints are documented in the matrix")
    print("✅ Entrypoint matrix is complete!")


if __name__ == "__main__":
    main()