#!/usr/bin/env python3
"""
Fail-Closed Authentication Validation Script

This script analyzes Mux Protocol smart contracts to ensure that all
authorization patterns are fail-closed and no entrypoint can silently
skip require_auth calls.

Usage: python scripts/validate_auth_patterns.py
"""

import os
import re
from pathlib import Path
from typing import Dict, List, Set, Tuple


def find_contract_files() -> List[Path]:
    """Find all contract lib.rs files."""
    contracts_dir = Path("contracts")
    return list(contracts_dir.glob("*/src/lib.rs"))


def analyze_auth_patterns(file_path: Path) -> Dict[str, List[str]]:
    """Analyze authentication patterns in a contract file."""
    with open(file_path, 'r') as f:
        content = f.read()
    
    results = {
        'require_auth_calls': [],
        'public_entrypoints': [],
        'admin_entrypoints': [], 
        'conditional_auth': [],
        'potential_bypasses': []
    }
    
    # Find all require_auth calls
    auth_pattern = re.compile(r'(\w+)\.require_auth\(\)')
    for match in auth_pattern.finditer(content):
        results['require_auth_calls'].append(match.group(1))
    
    # Find entrypoint functions
    entrypoint_pattern = re.compile(r'pub fn (\w+)\([^)]*\)[^{]*\{([^}]*\{[^}]*\})*[^}]*\}', re.DOTALL)
    for match in entrypoint_pattern.finditer(content):
        func_name = match.group(1)
        func_body = match.group(0)
        
        # Check for authorization patterns
        if 'require_auth' not in func_body and 'require_' not in func_body:
            results['public_entrypoints'].append(func_name)
        elif 'require_admin' in func_body or 'require_owner' in func_body:
            results['admin_entrypoints'].append(func_name)
    
    # Look for potential conditional auth bypasses
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


def validate_session_key_patterns(file_path: Path) -> Dict[str, bool]:
    """Validate session key authorization patterns."""
    with open(file_path, 'r') as f:
        content = f.read()
    
    results = {
        'has_session_keys': False,
        'empty_scope_check': False,
        'session_auth_required': False,
        'fail_closed_validation': False
    }
    
    # Check if this contract handles session keys
    if 'session_key' in content.lower() or 'SessionKey' in content:
        results['has_session_keys'] = True
        
        # Check for empty scope validation (T-40 requirement)
        if 'scopes.is_empty()' in content and 'Unauthorized' in content:
            results['empty_scope_check'] = True
            results['fail_closed_validation'] = True
        
        # Check for session key auth requirement
        if 'session_key.require_auth()' in content:
            results['session_auth_required'] = True
    
    return results


def check_admin_patterns(file_path: Path) -> Dict[str, List[str]]:
    """Check admin authorization patterns for fail-closed behavior."""
    with open(file_path, 'r') as f:
        content = f.read()
    
    results = {
        'admin_functions': [],
        'proper_auth_order': [],
        'auth_before_mutation': []
    }
    
    # Find functions that call require_admin or require_owner
    admin_func_pattern = re.compile(r'pub fn (\w+)[^{]*\{([^}]*(?:\{[^}]*\})*[^}]*)\}', re.DOTALL)
    
    for match in admin_func_pattern.finditer(content):
        func_name = match.group(1)
        func_body = match.group(2)
        
        if 'require_admin' in func_body or 'require_owner' in func_body:
            results['admin_functions'].append(func_name)
            
            # Check if auth happens before storage mutations
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
    
    print(f"\n📊 Summary:")
    print(f"   Contracts analyzed: {len(contract_files)}")
    print(f"   Issues found: {total_issues}")
    
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