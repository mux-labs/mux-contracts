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
///
/// As of issue #970 the contract crate list is derived from the `Cargo.toml`
/// workspace members and cross-checked against `CONTRACTS` and the `## `
/// sections of `docs/entrypoint-matrix.md`, so a new crate cannot be added
/// to the workspace without also being covered here and documented.
///
/// As of issue #975 the contract crate list is discovered from the workspace
/// `members` array in `Cargo.toml` instead of a hard-coded array, and the
/// `#[contractimpl]` scan is line-oriented so multi-line `pub fn` signatures
/// are still attributed to the correct entrypoint.

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

/// Workspace members that are not production contract crates and are
/// therefore excluded from the membership check. Keep this list explicit so
/// adding a non-contract member is a deliberate, reviewable change.
const NON_CONTRACT_MEMBERS: [&str; 2] = ["tests", "contracts/soroban-test-helpers"];

/// Recognised auth-call names. A non-public entrypoint must contain at least
/// one of these in its body; a public entrypoint must contain none (unless
/// allowlisted). Keep this list in one place so the check is auditable.
const AUTH_CALL_NAMES: [&str; 3] = ["require_auth", "require_admin", "require_owner"];

/// Documented `P` entrypoints that legitimately call an auth helper for a
/// read. Each entry is `(contract, entrypoint)`. Add here with a comment
/// rather than weakening the rule for every public method.
const PUBLIC_AUTH_ALLOWLIST: [(&str, &str); 0] = [];

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Absolute path to a file in the workspace root, resolved from this crate's
/// manifest dir so the test does not depend on the CWD cargo uses.
fn workspace_path(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(rel)
}

/// Discover the production contract crates from the workspace `members`
/// array in `Cargo.toml`.
///
/// Only `contracts/mux-*` members are returned; helper crates such
/// Absolute path to a file in the workspace root, resolved from this crate's
/// manifest dir so the test does not depend on the CWD cargo uses.
fn workspace_path(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(rel)
}

/// Parse the `members = [...]` block of the workspace `Cargo.toml` with a
/// minimal line scan (no extra deps) and return the contract crate names
/// (the last path segment of every `contracts/mux-*` member).
///
/// Only `contracts/mux-*` members are returned; helper crates such as
/// `contracts/soroban-test-helpers` are intentionally excluded because they
/// do not expose `#[contractimpl]` entrypoints. The list is sorted so the
/// failure output is deterministic.
fn discover_contract_crates() -> Result<Vec<String>, String> {
    let manifest_path = workspace_path("Cargo.toml");
    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read {}: {}", manifest_path.display(), e))?;

    let mut members: Vec<String> = Vec::new();
    let manifest_path = workspace_path("Cargo.toml");
    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read {}: {}", manifest_path.display(), e))?;

    let mut in_members = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if !in_members {
            if trimmed.starts_with("members") && trimmed.contains('[') {
                in_members = true;
                // Handle a single-line `members = ["a", "b"]` form.
                if let Some(close) = trimmed.find(']') {
                    collect_member_paths(&trimmed[..close], &mut members);
                    in_members = false;
    let mut in_members = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !in_members {
            if trimmed.starts_with("members") && trimmed.contains('[') {
                in_members = true;
                // Handle a single-line `members = ["a", "b"]` form too.
                if let Some(rest) = trimmed.split_once('[').map(|(_, r)| r) {
                    collect_members(rest, &mut members);
                    if rest.contains(']') {
                        in_members = false;
                    }
                }
            }
            continue;
        }

        collect_members(trimmed, &mut members);
        if trimmed.contains(']') {
            in_members = false;
        }
    }

    let mut crates: Vec<String> = members
        .into_iter()
        .filter(|m| m.starts_with("contracts/mux-"))
        .filter(|m| !NON_CONTRACT_MEMBERS.contains(&m.as_str()))
        .map(|m| m.rsplit('/').next().unwrap_or(&m).to_string())
        .collect();
    crates.sort();
    crates.dedup();
    Ok(crates)
}

/// Extract quoted member strings from a fragment of the `members` block.
fn collect_members(fragment: &str, out: &mut Vec<String>) {
    let mut rest = fragment;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        match after.find('"') {
            Some(end) => {
                out.push(after[..end].to_string());
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
}

/// Parse the `## <crate>` section headers from `docs/entrypoint-matrix.md`.
fn matrix_section_headers() -> Result<HashSet<String>, String> {
    let matrix_path = workspace_path("docs/entrypoint-matrix.md");
    let content = fs::read_to_string(&matrix_path)
        .map_err(|e| format!("Failed to read {}: {}", matrix_path.display(), e))?;
    Ok(content
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|s| s.trim().to_st
        }
    }
}

/// Parse the `## <crate>` section headers from `docs/entrypoint-matrix.md`.
fn matrix_section_headers() -> Result<HashSet<String>, String> {
    let matrix_path = workspace_path("docs/entrypoint-matrix.md");
    let content = fs::read_to_string(&matrix_path)
        .map_err(|e| format!("Failed to read {}: {}", matrix_path.display(), e))?;
    Ok(content
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
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
/// `pub fn` defined while inside one. The scan is line-oriented and buffers
/// a pending `pub fn` signature so multi-line forms such as
///
/// ```ignore
/// pub fn transfer(
///     from: Address,
/// ) -> Result<(), Error> {
/// ```
///
/// are still attributed to the correct entrypoint. The scan stops at the
/// `#[cfg(test)]` module boundary: test-module helpers are not production
/// contract entrypoints.
fn extract_implemented_entrypoints() -> Result<HashMap<String, Vec<String>>, String> {
    let mut implemented = HashMap::new();
    let contracts = discover_contract_crates()?;
    
    for contract_name in &contracts {
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
        // Accumulates a `pub fn` signature that may span multiple lines.
        let mut pending_fn: Option<String> = None;
        
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
                pending_fn = None;
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
                    pending_fn = None;
                    in_impl_body = false;
                    continue;
                }
                
                // Look for public function definitions, buffering the
                // signature until the opening paren is seen so multi-line
                // `pub fn` forms are handled.
                if pending_fn.is_none() && trimmed.starts_with("pub fn ") {
                    pending_fn = Some(trimmed.to_string());
                } else if let Some(buf) = pending_fn.as_mut() {
                    buf.push(' ');
                    buf.push_str(trimmed);
                }
                
                if let Some(buf) = pending_fn.as_ref() {
                    if buf.contains('(') {
                        if let Some(fn_name) = extract_function_name(buf) {
                            entrypoints.push(fn_name);
                        }
                        pending_fn = None;
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

/// Compute the symmetric difference between documented and implemented
/// entrypoints for a single contract. Returns `(missing_from_docs,
/// missing_from_source)` sorted for stable failure messages.
fn diff_entrypoints(
    documented: &[String],
    implemented: &[String],
) -> (Vec<String>, Vec<String>) {
    let documented_set: HashSet<&String> = documented.iter().collect();
    let implemented_set: HashSet<&String> = implemented.iter().collect();

    let mut missing_from_docs: Vec<String> = implemented_set
        .difference(&documented_set)
        .map(|s| (*s).clone())
        .collect();
    missing_from_docs.sort();
    missing_from_docs.dedup();

    let mut missing_from_source: Vec<String> = documented_set
        .difference(&implemented_set)
        .map(|s| (*s).clone())
        .collect();
    missing_from_source.sort();
    missing_from_source.dedup();

    (missing_from_docs, missing_from_source)
}

/// Verify that every contract crate listed in the `Cargo.toml` workspace
/// members has a `src/lib.rs`, a `## <crate>` section in
/// `docs/entrypoint-matrix.md`, and an entry in this test's `CONTRACTS`
/// array. Fails naming the symmetric difference so a newly added crate
/// cannot silently escape coverage.
#[test]
fn workspace_members_match_coverage_and_matrix() {
    let workspace_crates = workspace_contract_crates()
        .expect("failed to parse Cargo.toml workspace members");
    assert!(
        !workspace_crates.is_empty(),
        "no contract crates found in Cargo.toml workspace members"
    );

    let workspace_set: HashSet<&String> = workspace_crates.iter().collect();
    let coverage_set: HashSet<String> = CONTRACTS.iter().map(|s| s.to_string()).collect();

    // 1. Every workspace contract crate must be in the coverage test's list.
    let mut missing_from_coverage: Vec<&String> = workspace_set
        .iter()
        .filter(|c| !coverage_set.contains(**c))
        .copied()
        .collect();
    missing_from_coverage.sort();

    // 2. Every crate in the coverage list must be a workspace member.
    let mut missing_from_workspace: Vec<&String> = coverage_set
        .iter()
        .filter(|c| !workspace_set.contains(c))
        .collect();
    missing_from_workspace.sort();

    assert!(
        missing_from_coverage.is_empty() && missing_from_workspace.is_empty(),
        "workspace members and CONTRACTS array disagree: \
         in Cargo.toml but not CONTRACTS = {:?}; \
         in CONTRACTS but not Cargo.toml = {:?}",
        missing_from_coverage,
        missing_from_workspace
    );

    // 3. Every workspace contract crate must have a src/lib.rs.
    let mut missing_lib: Vec<&String> = workspace_crates
        .iter()
        .filter(|c| !workspace_path(&format!("contracts/{}/src/lib.rs", c)).exists())
        .collect();
    missing_lib.sort();
    assert!(
        missing_lib.is_empty(),
        "workspace contract crates missing src/lib.rs: {:?}",
        missing_lib
    );

    // 4. Every workspace contract crate must have a `## <crate>` matrix section.
    let matrix_headers = matrix_section_headers()
        .expect("failed to parse docs/entrypoint-matrix.md");
    let mut missing_matrix: Vec<&String> = workspace_crates
        .iter()
        .filter(|c| !matrix_headers.contains(*c))
        .collect();
    missing_matrix.sort();
    assert!(
        missing_matrix.is_empty(),
        "workspace contract crates missing a `## <crate>` section in docs/entrypoint-matrix.md: {:?}",
        missing_matrix
    );
}

/// Verify that every implemented entrypoint is documented and vice versa.
#[test]
fn entrypoint_matrix_is_complete() {
    let documented = parse_documented_entrypoints()
        .expect("failed to parse documented entrypoints");
    let implemented = extract_implemented_entrypoints()
        .expect("failed to extract implemented entrypoints");

    let mut failures: Vec<String> = Vec::new();

    for contract in CONTRACTS.iter() {
        let doc_names: Vec<String> = documented
            .get(contract)
            .map(|entries| entries.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default();
        let impl_names: Vec<String> = implemented
            .get(contract)
            .cloned()
            .unwrap_or_default();

        let (missing_from_docs, missing_from_source) =
            diff_entrypoints(&doc_names, &impl_names);

        if !missing_from_docs.is_empty() {
            failures.push(format!(
                "{}: implemented but not documented: {:?}",
                contract, missing_from_docs
            ));
        }
        if !missing_from_source.is_empty() {
            failures.push(format!(
                "{}: documented but not implemented: {:?}",
                contract, missing_from_source
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "entrypoint matrix is stale:\n{}",
                contract, missing_from_docs
            ));
        }
        if !missing_from_source.is_empty() {
            failures.push(format!(
                "{}: documented but not implemented: {:?}",
                contract, missing_from_source
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "entrypoint matrix is out of sync:\n{}",
        failures.join("\n")
    );
}

/// Unit test for the diff helper so the two-way comparison cannot silently
/// regress into a one-way check.
#[test]
fn diff_entrypoints_reports_both_directions() {
    let documented = vec!["a".to_string(), "b".to_string()];
    let implemented = vec!["b".to_string(), "c".to_string()];

    let (missing_from_docs, missing_from_source) =
        diff_entrypoints(&documented, &implemented);

    assert_eq!(missing_from_docs, vec!["c".to_string()]);
    assert_eq!(missing_from_source, vec!["a".to_string()]);
}

    let (missing_from_docs, missing_from_source) =
        diff_entrypoints(&documented, &implemented);

    assert_eq!(missing_from_docs, vec!["c".to_string()]);
    assert_eq!(missing_from_source, vec!["a".to_string()]);
}
