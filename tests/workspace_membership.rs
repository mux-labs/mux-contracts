//! Workspace membership consistency checks.
//!
//! Verifies that every contract crate listed in `Cargo.toml` workspace members
//! has a corresponding `src/lib.rs`, a `## <crate>` section in
//! `docs/entrypoint-matrix.md`, and is present in the coverage test's own
//! `contracts` array. This prevents a new contract crate from silently
//! escaping the entrypoint-matrix coverage check.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Workspace members that are not contract crates and are therefore excluded
/// from the membership check.
const NON_CONTRACT_MEMBERS: &[&str] = &["tests", "contracts/soroban-test-helpers"];

/// Prefix that identifies a contract crate workspace member.
const CONTRACT_PREFIX: &str = "contracts/mux-";

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Minimal line scan of the `members = [...]` block in `Cargo.toml`.
///
/// The workspace deliberately avoids extra dependencies, so this parses the
/// members array by hand rather than pulling in a TOML crate.
fn workspace_members() -> Vec<String> {
    let manifest = fs::read_to_string(repo_root().join("Cargo.toml"))
        .expect("failed to read Cargo.toml");

    let mut members = Vec::new();
    let mut in_members = false;

    for line in manifest.lines() {
        let trimmed = line.trim();

        if !in_members {
            if trimmed.starts_with("members") && trimmed.contains('[') {
                in_members = true;
                // Handle a single-line `members = ["a", "b"]` form.
                if let Some(rest) = trimmed.split_once('[').map(|(_, r)| r) {
                    collect_quoted(rest, &mut members);
                    if rest.contains(']') {
                        break;
                    }
                }
            }
            continue;
        }

        collect_quoted(trimmed, &mut members);
        if trimmed.contains(']') {
            break;
        }
    }

    members
}

/// Extract double-quoted strings from a line fragment.
fn collect_quoted(fragment: &str, out: &mut Vec<String>) {
    let mut chars = fragment.chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            let mut value = String::new();
            for inner in chars.by_ref() {
                if inner == '"' {
                    break;
                }
                value.push(inner);
            }
            out.push(value);
        }
    }
}

/// Contract crate names (e.g. `mux-account`) derived from workspace members.
fn contract_crates() -> BTreeSet<String> {
    workspace_members()
        .into_iter()
        .filter(|m| m.starts_with(CONTRACT_PREFIX))
        .filter(|m| !NON_CONTRACT_MEMBERS.contains(&m.as_str()))
        .map(|m| m.trim_start_matches("contracts/").to_string())
        .collect()
}

/// `## <crate>` section headers parsed from `docs/entrypoint-matrix.md`.
fn matrix_sections() -> BTreeSet<String> {
    let doc = fs::read_to_string(repo_root().join("docs/entrypoint-matrix.md"))
        .expect("failed to read docs/entrypoint-matrix.md");

    doc.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("## ")
                .map(|rest| rest.trim().to_string())
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// Crate names listed in the coverage test's own `contracts` array.
fn coverage_test_crates() -> BTreeSet<String> {
    let source = fs::read_to_string(repo_root().join("tests/entrypoint_matrix_coverage.rs"))
        .expect("failed to read tests/entrypoint_matrix_coverage.rs");

    let mut crates = BTreeSet::new();
    let mut in_array = false;

    for line in source.lines() {
        let trimmed = line.trim();

        if !in_array {
            if trimmed.starts_with("const CONTRACTS") || trimmed.starts_with("let contracts") {
                if trimmed.contains('[') {
                    in_array = true;
                    collect_quoted(trimmed, &mut crates);
                    if trimmed.contains(']') {
                        break;
                    }
                }
            }
            continue;
        }

        collect_quoted(trimmed, &mut crates);
        if trimmed.contains(']') {
            break;
        }
    }

    crates
}

fn symmetric_difference(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    a.symmetric_difference(b).cloned().collect()
}

#[test]
fn every_contract_crate_has_source_file() {
    let mut missing = Vec::new();
    for crate_name in contract_crates() {
        let lib = repo_root()
            .join("contracts")
            .join(&crate_name)
            .join("src/lib.rs");
        if !lib.is_file() {
            missing.push(crate_name);
        }
    }

    assert!(
        missing.is_empty(),
        "contract crates missing src/lib.rs: {missing:?}"
    );
}

#[test]
fn every_contract_crate_has_matrix_section() {
    let crates = contract_crates();
    let sections = matrix_sections();
    let diff = symmetric_difference(&crates, &sections);

    assert!(
        diff.is_empty(),
        "contract crates and docs/entrypoint-matrix.md sections disagree; symmetric difference: {diff:?}"
    );
}

#[test]
fn every_contract_crate_is_in_coverage_test() {
    let crates = contract_crates();
    let covered = coverage_test_crates();
    let diff = symmetric_difference(&crates, &covered);

    assert!(
        diff.is_empty(),
        "contract crates and tests/entrypoint_matrix_coverage.rs contracts array disagree; symmetric difference: {diff:?}"
    );
}
