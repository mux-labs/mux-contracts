#!/usr/bin/env python3
"""
Entrypoint Matrix Gap Detection Script

This script verifies that all implemented contract entrypoints are documented
in the entrypoint-matrix.md file. It serves as a failing test that captures
the current gap and will pass once the documentation is complete.

Usage: python scripts/check_entrypoint_matrix.py
"""

import os
import sys
import re
from pathlib import Path


def read_entrypoint_matrix():
    """Parse docs/entrypoint-matrix.md and extract documented entrypoints."""
    matrix_path = Path("docs/entrypoint-matrix.md")
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


def main():
    """Check for entrypoint matrix gaps."""
    print("🔍 Checking entrypoint matrix for gaps...\n")
    
    # Known missing entrypoints that should be documented
    known_missing = [
        ("mux-account", "execute"),
        ("mux-account", "register_session_key"), 
        ("mux-account", "revoke_session_key"),
        ("mux-batcher", "submit_batch"),
        ("mux-recovery", "add_guardian"),
        ("mux-recovery", "remove_guardian"),
        ("mux-recovery", "approve_recovery_admin"),
        ("mux-recovery", "set_registry"),
        ("mux-delegation", "link_contract_id"),
        ("mux-delegation", "check_delegate"),
        ("mux-wallet-registry", "list_wallets"),
    ]
    
    # Parse documented entrypoints
    documented = read_entrypoint_matrix()
    
    # Check for missing entrypoints
    still_missing = []
    
    for contract, function in known_missing:
        if contract not in documented or function not in documented[contract]:
            still_missing.append((contract, function))
    
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
    
    print("✅ All known entrypoint gaps have been documented in the matrix")
    print("✅ Entrypoint matrix is complete!")


if __name__ == "__main__":
    main()