/// Threat-model coverage tests.
///
/// These tests enforce that `docs/threat-model.md` documents **every**
/// production contract crate in the workspace, not just a subset. This is
/// required for the Mux Soroban audit and mainnet readiness: an undocumented
/// contract is an un-reviewed attack surface.
///
/// # What is checked
///
/// 1. Every `contracts/mux-*` crate (i.e. every crate that ships WASM; the
///    non-WASM `soroban-test-helpers` crate is excluded) must be named in
///    `docs/threat-model.md`.
/// 2. The doc must still contain the structural anchors (scope section) that
///    the rest of the repo links to.
///
/// Mirrors `scripts/check-architecture-docs.sh`, which does the same
/// coverage check for `docs/architecture-overview.md`.
#[cfg(test)]
mod threat_model_coverage {
    use std::fs;
    use std::path::Path;

    /// Returns the crate names of every `contracts/mux-*` directory (the
    /// production contract crates). `soroban-test-helpers` is excluded — it
    /// is a test-utility crate that does not ship WASM.
    fn production_contract_crates() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent() // workspace root
            .expect("workspace root");
        let contracts_dir = root.join("contracts");

        let mut crates = Vec::new();
        let Ok(entries) = fs::read_dir(&contracts_dir) else {
            return crates;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("mux-") {
                crates.push(name);
            }
        }
        crates.sort();
        crates
    }

    /// Every production contract crate must be named in the threat model.
    /// A crate that is never mentioned has no documented attack surface,
    /// which is the exact gap this test was added to close.
    #[test]
    fn every_production_contract_is_covered_in_threat_model() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let doc = fs::read_to_string(root.join("docs/threat-model.md"))
            .expect("docs/threat-model.md must exist");

        let crates = production_contract_crates();
        assert!(
            crates.len() >= 3,
            "expected at least the three originally documented contracts, got: {crates:?}"
        );

        let mut missing: Vec<String> = Vec::new();
        for name in &crates {
            if !doc.contains(name.as_str()) {
                missing.push(name.clone());
            }
        }

        assert!(
            missing.is_empty(),
            "docs/threat-model.md does not cover these production contract crates: {missing:?}. \
             Every contract that ships WASM must be documented (scope table, trust boundaries, \
             and a per-contract threat section) before the Mux Soroban audit / mainnet readiness \
             review. See docs/threat-model.md §1 Scope."
        );
    }

    /// The threat model must keep the structural anchors other docs link to
    /// (e.g. `docs/storage-griefing.md` links to `#45-storage-griefing`).
    #[test]
    fn threat_model_keeps_structural_anchors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let doc = fs::read_to_string(root.join("docs/threat-model.md"))
            .expect("docs/threat-model.md must exist");

        for anchor in [
            "## 1. Scope",
            "## 2. Assets",
            "## 3. Trust Boundaries",
            "## 4. Threats and Mitigations",
            "## 5. Security Controls",
            "## 6. Out-of-Scope / Residual Risks",
            "## 7. Revision History",
        ] {
            assert!(
                doc.contains(anchor),
                "docs/threat-model.md is missing structural anchor {anchor:?}"
            );
        }
    }
}
