/// Test that verifies all implemented contract entrypoints are documented
/// in the entrypoint-matrix.md file.
///
/// This test prevents entrypoint matrix documentation gaps from being
/// reintroduced by ensuring every #[contractimpl] method is either:
/// 1. Documented in docs/entrypoint-matrix.md, or  
/// 2. Explicitly excluded (like internal helpers)
///
/// FAILURE MODE: If this test fails, it means there are implemented contract
/// entrypoints that are missing from the audit documentation. This is a
/// blocker for mainnet deployment and Soroban audit readiness.
///
/// As of issue #964 the two-way comparison is enforced for real: once the
/// documented and implemented maps are extracted, `diff_entrypoints` computes
/// the set difference in both directions for every contract and the test
/// fails naming the contract plus the sorted offending entrypoints. A
/// helper-level unit test feeds a synthetic documented/implemented pair
/// through the same diff logic so the guard cannot silently regress.
///
/// As of issue #969 the documented auth class is cross-checked against the
/// actual source body: every `A`/`U` entrypoint must contain a recognised
/// auth call, and every `P` entrypoint must not (unless explicitly
/// allowlisted). See `AUTH_CALL_NAMES` and `PUBLIC_AUTH_ALLOWLIST`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// The production contract crates covered by the entrypoint matrix.
const CONTRACTS: [&str; 10] = [
    "mux-account",
    "mux-account-factory",
    "mux-batcher",
    "mux-delegation",
    "mux-permissions",
    "mux-policy",
    "mux-recovery",
    "mux-registry",
    "mux-spending-policy",
    "mux-wallet-registry",
];

/// Recognised auth-call names. A non-public entrypoint must contain at least
/// one of these in its body; a public entrypoint must contain none (unless
/// allowlisted). Keep this list in one place so the check is auditable.
const AUTH_CALL_NAMES: [&str; 3] = ["require_auth", "require_admin", "require_owner"];

/// Documented `P` entrypoints that legitimately call an auth helper for a
/// read. Each entry is `(contract, entrypoint)`. Add here with a comment
/// rather than weakening the rule for every public method.
const PUBLIC_AUTH_ALLOWLIST: [(&str, &str); 0] = [];

/// Absolute path to a file in the workspace root, resolved from this crate's
/// manifest dir so the test does not depend on the CWD cargo uses.
fn workspace_path(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(rel)
}

/// Expected entrypoints that should be documented in entrypoint-matrix.md
/// This is the source of truth for what functions exist vs what's documented.
#[allow(dead_code)]
#[derive(Debug)]
struct ContractEntrypoint {
    name: String,
    contract: String,
    auth_class: AuthClass,
    notes: String,
}

#[derive(Debug, PartialEq)]
enum AuthClass {
    Admin,  // A - Admin/owner requires stored admin authorization
    User,   // U - User/actor requires specific caller authorization  
    Public, // P - Public no authorization required (read-only)
}

impl AuthClass {
    fn from_char(c: char) -> Option<Self> {
        match c {
            'A' => Some(AuthClass::Admin),
            'U' => Some(AuthClass::User), 
            'P' => Some(AuthClass::Public),
            _ => None,
        }
    }

    #[allow(dead_code)]
    fn as_char(&self) -> char {
        match self {
            AuthClass::Admin => 'A',
            AuthClass::User => 'U',
            AuthClass::Public => 'P',
        }
    }
}

/// Parse the entrypoint-matrix.md file and extract documented entrypoints.
///
/// Table rows like `| `initialize(admin)` | A | ... |` are stored by bare
/// function name (`initialize`) so they can be compared against the
/// `pub fn` names extracted from contract sources.
fn parse_documented_entrypoints() -> Result<HashMap<String, Vec<ContractEntrypoint>>, String> {
    let matrix_path = workspace_path("docs/entrypoint-matrix.md");
    let content = fs::read_to_string(&matrix_path)
        .map_err(|e| format!("Failed to read {}: {}", matrix_path.display(), e))?;
    
    let mut documented = HashMap::new();
    let mut current_contract = String::new();
    let lines: Vec<&str> = content.lines().collect();
    
    for line in &lines {
        // Detect contract section headers (e.g., "## mux-account")
        if line.starts_with("## ") {
            current_contract = line[3..].trim().to_string();
            documented.insert(current_contract.clone(), Vec::new());
            continue;
        }
        
        // Parse entrypoint table rows
        if line.starts_with('|') && !line.contains("Entrypoint") && !line.contains("---") {
            let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if parts.len() >= 4 && !parts[1].is_empty() && !parts[2].is_empty() {
                let entrypoint_name = parts[1].trim_matches('`').trim();
                let bare_name = entrypoint_name
                    .split('(')
                    .next()
                    .unwrap_or(entrypoint_name)
                    .trim();
                let auth_str = parts[2].trim();
                let notes = parts[3..].join(" | ").trim().to_string();
                
                if let Some(auth_class) = auth_str.chars().next().and_then(AuthClass::from_char) {
                    if let Some(contract_entries) = documented.get_mut(&current_contract) {
                        contract_entries.push(ContractEntrypoint {
                            name: bare_name.to_string(),
                            contract: current_contract.clone(),
                            auth_class,
                            notes,
                        });
                    }
                }
            }
        }
    }
    
    Ok(documented)
}

/// Extract entrypoint names from contract source code.
///
/// Tracks `#[contractimpl]` impl blocks by brace depth and records every
/// `pub fn` defined while inside one. The scan stops at the `#[cfg(test)]`
/// module boundary implicitly: test modules do not declare `pub fn`.
fn extract_implemented_entrypoints() -> Result<HashMap<String, Vec<String>>, String> {
    let mut implemented = HashMap::new();
    
    for contract_name in &CONTRACTS {
        let lib_path = workspace_path(&format!("contracts/{}/src/lib.rs", contract_name));
        if !lib_path.exists() {
            continue;
        }
        
        let content = fs::read_to_string(&lib_path)
            .map_err(|e| format!("Failed to read {}: {}", lib_path.display(), e))?;
            
        let mut entrypoints = Vec::new();
        let mut in_contractimpl = false;
        let mut brace_depth = 0i32;
        let mut in_impl_body = false;
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Stop scanning at the #[cfg(test)] boundary: test-module helpers
            // are not production contract entrypoints.
            if trimmed == "#[cfg(test)]" {
                break;
            }
            // Track when we enter/exit #[contractimpl] blocks
            if trimmed == "#[contractimpl]" {
                in_contractimpl = true;
                brace_depth = 0;
                in_impl_body = false;
                continue;
            }
            
            if in_contractimpl {
                brace_depth += line.matches('{').count() as i32;
                brace_depth -= line.matches('}').count() as i32;
                
                if brace_depth > 0 {
                    in_impl_body = true;
                }
                
                // The impl block closes once its body has been entered and
                // the depth returns to zero — stop scanning so later items
                // (other impls, test modules) are not mis-attributed.
                if in_impl_body && brace_depth <= 0 {
                    in_contractimpl = false;
                    continue;
                }
                
                // Look for public function definitions
                if trimmed.starts_with("pub fn ") {
                    if let Some(fn_name) = extract_function_name(trimmed) {
                        entrypoints.push(fn_name);
                    }
                }
            }
        }
        
        implemented.insert(contract_name.to_string(), entrypoints);
    }
    
    Ok(implemented)
}

/// Extract function name from a "pub fn name(...)" line
fn extract_function_name(line: &str) -> Option<String> {
    if let Some(start) = line.find("pub fn ") {
        let after_fn = &line[start + 7..];
        if let Some(paren_pos) = after_fn.find('(') {
            let name = after_fn[..paren_pos].trim();
            return Some(name.to_string());
        }
    }
    None
}

/// Locate the body of `pub fn <name>` in `source` by brace matching and
/// return the text between the outermost braces (exclusive). Returns `None`
/// when the function cannot be found.
fn extract_function_body(source: &str, name: &str) -> Option<String> {
    let needle = format!("pub fn {}", name);
    let mut search_from = 0usize;
    while let Some(rel) = source[search_from..].find(&needle) {
        let start = search_from + rel;
        // Ensure the match is a whole identifier (next char is not ident-ish).
        let after = &source[start + needle.len()..];
        let next_char = after.chars().next();
        let is_ident = matches!(next_char, Some(c) if c.is_alphanumeric() || c == '_');
        if !is_ident {
            // Find the opening brace of the body.
            if let Some(open_rel) = source[start..].find('{') {
                let open = start + open_rel;
                let mut depth = 0i32;
                let bytes = source.as_bytes();
                let mut i = open;
                while i < bytes.len() {
                    match bytes[i] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(source[open + 1..i].to_string());
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
        }
        search_from = start + needle.len();
    }
    None
}

/// Return true when `body` contains any recognised auth call.
fn body_has_auth_call(body: &str) -> bool {
    AUTH_CALL_NAMES.iter().any(|call| body.contains(call))
}

/// Cross-check documented auth classes against the actual source bodies.
///
/// Returns a list of human-readable mismatch descriptions. Empty means the
/// matrix and the code agree.
fn check_auth_classes(
    documented: &HashMap<String, Vec<ContractEntrypoint>>,
) -> Result<Vec<String>, String> {
    let mut mismatches = Vec::new();

    for (contract, entries) in documented {
        let lib_path = workspace_path(&format!("contracts/{}/src/lib.rs", contract));
        if !lib_path.exists() {
            continue;
        }
        let source = fs::read_to_string(&lib_path)
            .map_err(|e| format!("Failed to read {}: {}", lib_path.display(), e))?;

        for entry in entries {
            let body = match extract_function_body(&source, &entry.name) {
                Some(b) => b,
                None => continue,
            };
            let has_auth = body_has_auth_call(&body);
            let allowlisted = PUBLIC_AUTH_ALLOWLIST
                .iter()
                .any(|(c, n)| *c == contract.as_str() && *n == entry.name.as_str());

            match entry.auth_class {
                AuthClass::Admin | AuthClass::User => {
                    if !has_auth {
                        mismatches.push(format!(
                            "{}::{} documented as {} but body has no auth call (expected one of {:?})",
                            contract,
                            entry.name,
                            entry.auth_class.as_char(),
                            AUTH_CALL_NAMES
                        ));
                    }
                }
                AuthClass::Public => {
                    if has_auth && !allowlisted {
                        mismatches.push(format!(
                            "{}::{} documented as P but body contains an auth call (add to PUBLIC_AUTH_ALLOWLIST if intentional)",
                            contract, entry.name
                        ));
                    }
                }
            }
        }
    }

    Ok(mismatches)
}

/// Compute the two-way set difference between documented and implemented
/// entrypoints for a single contract. Returns `(missing_from_docs,
/// missing_from_code)` as sorted vectors.
fn diff_entrypoints(
    documented: &[String],
    implemented: &[String],
) -> (Vec<String>, Vec<String>) {
    let doc_set: HashSet<&String> = documented.iter().collect();
    let impl_set: HashSet<&String> = implemented.iter().collect();

    let mut missing_from_docs: Vec<String> = impl_set
        .iter()
        .filter(|n| !doc_set.contains(*n))
        .map(|n| (*n).clone())
        .collect();
    let mut missing_from_code: Vec<String> = doc_set
        .iter()
        .filter(|n| !impl_set.contains(*n))
        .map(|n| (*n).clone())
        .collect();

    missing_from_docs.sort();
    missing_from_code.sort();
    (missing_from_docs, missing_from_code)
}

#[test]
fn entrypoint_matrix_is_complete() {
    let documented = parse_documented_entrypoints().expect("parse documented entrypoints");
    let implemented = extract_implemented_entrypoints().expect("extract implemented entrypoints");

    let mut failures = Vec::new();

    for contract in &CONTRACTS {
        let doc_names: Vec<String> = documented
            .get(*contract)
            .map(|v| v.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default();
        let impl_names: Vec<String> = implemented.get(*contract).cloned().unwrap_or_default();

        let (missing_from_docs, missing_from_code) = diff_entrypoints(&doc_names, &impl_names);

        if !missing_from_docs.is_empty() {
            failures.push(format!(
                "{}: implemented but not documented: {:?}",
                contract, missing_from_docs
            ));
        }
        if !missing_from_code.is_empty() {
            failures.push(format!(
                "{}: documented but not implemented: {:?}",
                contract, missing_from_code
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "entrypoint matrix is out of sync:\n{}",
        failures.join("\n")
    );
}

#[test]
fn entrypoint_auth_classes_match_source() {
    let documented = parse_documented_entrypoints().expect("parse documented entrypoints");
    let mismatches = check_auth_classes(&documented).expect("check auth classes");

    assert!(
        mismatches.is_empty(),
        "entrypoint auth class mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn diff_entrypoints_detects_both_directions() {
    let documented = vec!["a".to_string(), "b".to_string()];
    let implemented = vec!["b".to_string(), "c".to_string()];

    let (missing_from_docs, missing_from_code) = diff_entrypoints(&documented, &implemented);

    assert_eq!(missing_from_docs, vec!["c".to_string()]);
    assert_eq!(missing_from_code, vec!["a".to_string()]);
}
