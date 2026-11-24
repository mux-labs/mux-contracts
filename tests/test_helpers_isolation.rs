//! Guards the invariants documented in the header comment of
//! `contracts/soroban-test-helpers/src/lib.rs`:
//!
//! * `soroban-test-helpers` must never be a production dependency of any
//!   `mux-*` contract crate — it may only appear under `[dev-dependencies]`.
//! * `soroban-test-helpers` itself must not be built as a `cdylib`.
//!
//! These mirror the rules enforced by `scripts/check-no-testutils.sh` so that
//! `cargo test` fails early, before the shell gate runs. The WASM-string scan
//! is intentionally not duplicated here: it needs a built artifact and stays
//! in the shell script.

use std::fs;
use std::path::{Path, PathBuf};

/// Sections of a `Cargo.toml` in which `soroban-test-helpers` is allowed.
const ALLOWED_SECTIONS: &[&str] = &["dev-dependencies"];

const HELPER_CRATE: &str = "soroban-test-helpers";

fn workspace_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` points at the `tests` crate; the workspace root is
    // its parent.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate should live inside the workspace root")
        .to_path_buf()
}

/// Returns the TOML section header (e.g. `dependencies`) that a line belongs
/// to, or `None` for lines outside any section.
fn section_of(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        Some(trimmed.trim_matches(|c| c == '[' || c == ']').trim().to_string())
    } else {
        None
    }
}

/// Returns the dependency key declared on a line, if any.
fn dependency_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (key, _) = trimmed.split_once('=')?;
    Some(key.trim().trim_matches('"').to_string())
}

fn contract_manifests() -> Vec<PathBuf> {
    let contracts_dir = workspace_root().join("contracts");
    let mut manifests: Vec<PathBuf> = fs::read_dir(&contracts_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", contracts_dir.display()))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("mux-")
        })
        .map(|entry| entry.path().join("Cargo.toml"))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();
    manifests
}

#[test]
fn test_helpers_only_in_dev_dependencies() {
    let manifests = contract_manifests();
    assert!(
        !manifests.is_empty(),
        "expected to find at least one contracts/mux-*/Cargo.toml"
    );

    let mut violations = Vec::new();
    for manifest in &manifests {
        let contents = fs::read_to_string(manifest)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", manifest.display()));
        let crate_name = manifest
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| manifest.display().to_string());

        let mut current_section: Option<String> = None;
        for line in contents.lines() {
            if let Some(section) = section_of(line) {
                current_section = Some(section);
                continue;
            }
            if dependency_key(line).as_deref() == Some(HELPER_CRATE) {
                let section = current_section.as_deref().unwrap_or("<root>");
                if !ALLOWED_SECTIONS.contains(&section) {
                    violations.push(format!(
                        "{crate_name}: `{HELPER_CRATE}` declared under [{section}] (only [dev-dependencies] is allowed)"
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "soroban-test-helpers must only be a dev-dependency of mux-* contracts:\n{}",
        violations.join("\n")
    );
}

#[test]
fn test_helpers_crate_is_not_cdylib() {
    let manifest = workspace_root()
        .join("contracts")
        .join(HELPER_CRATE)
        .join("Cargo.toml");
    let contents = fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", manifest.display()));

    let mut in_lib = false;
    for line in contents.lines() {
        if let Some(section) = section_of(line) {
            in_lib = section == "lib";
            continue;
        }
        if !in_lib {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            if key.trim() == "crate-type" {
                assert!(
                    !value.contains("cdylib"),
                    "{HELPER_CRATE}: [lib] crate-type must not declare `cdylib` (found `{}`)",
                    value.trim()
                );
            }
        }
    }
}
