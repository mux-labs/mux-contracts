//! Error-code documentation coverage guard (#924).
//!
//! Every `pub enum *Error` declared in a contract crate must be catalogued in
//! `docs/error_codes.md`, and the doc must not list variants that no contract
//! declares. Same cross-surface pattern as `expiry_naming.rs`: the test reads
//! files from the repo root via `env!("CARGO_MANIFEST_DIR")` and asserts
//! consistency between the contract sources and the documentation.
//!
//! Doc sections are keyed `## <crate>::<EnumName>`; variants are the second
//! column (in backticks) of each section's markdown table.
//!
//! Run with: cargo test -p mux-contract-tests --test error_codes_coverage

#[cfg(test)]
mod error_codes_coverage {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf()
    }

    fn read(relative: &str) -> String {
        fs::read_to_string(repo_root().join(relative))
            .unwrap_or_else(|_| panic!("{relative} must exist"))
    }

    /// Every contract crate directory that ships a `src/lib.rs`.
    fn contract_sources() -> Vec<(String, String)> {
        let contracts = repo_root().join("contracts");
        let mut crates: Vec<(String, String)> = Vec::new();
        let entries = fs::read_dir(&contracts).expect("contracts/ must exist");
        for entry in entries.flatten() {
            let lib = entry.path().join("src").join("lib.rs");
            if !lib.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let source = fs::read_to_string(&lib)
                .unwrap_or_else(|_| panic!("{} must be readable", lib.display()));
            crates.push((name, source));
        }
        crates.sort();
        crates
    }

    /// Parse `pub enum <Name> { .. }` blocks whose name ends in `Error`.
    /// Variant lines look like `Name = 123,`; doc comments, block-internal
    /// `//` comments, and blank lines are ignored.
    fn parse_error_enums(source: &str) -> BTreeMap<String, BTreeSet<String>> {
        let mut enums: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut current: Option<String> = None;

        for raw in source.lines() {
            let line = raw.split("//").next().unwrap_or("").trim();

            if let Some(name) = current.clone() {
                if line == "}" {
                    current = None;
                    continue;
                }
                // Variant line: `Variant = N,` (possibly with a trailing comment).
                if let Some(stripped) = line.strip_suffix(',') {
                    if let Some((variant, _code)) = stripped.rsplit_once('=') {
                        let variant = variant.trim();
                        if !variant.is_empty()
                            && variant.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            enums
                                .get_mut(&name)
                                .expect("enum open")
                                .insert(variant.to_string());
                        }
                    }
                }
                continue;
            }

            if line.starts_with("pub enum ") && line.ends_with('{') {
                let name = line["pub enum ".len()..line.len() - 1].trim();
                if name.ends_with("Error") {
                    enums.insert(name.to_string(), BTreeSet::new());
                    current = Some(name.to_string());
                }
            }
        }

        enums
    }

    /// Parse `## <crate>::<EnumName>` sections; collect the backticked second
    /// column of each markdown table row as a documented variant.
    fn parse_doc(doc: &str) -> BTreeMap<(String, String), BTreeSet<String>> {
        let mut map: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        let mut current: Option<(String, String)> = None;

        for raw in doc.lines() {
            let line = raw.trim();

            if let Some(heading) = line.strip_prefix("## ") {
                current = heading.split_once("::").and_then(|(cr, en)| {
                    let cr = cr.trim();
                    let en = en.trim();
                    if !cr.is_empty()
                        && !en.is_empty()
                        && en.chars().all(|c| c.is_alphanumeric() || c == '_')
                    {
                        Some((cr.to_string(), en.to_string()))
                    } else {
                        None
                    }
                });
                if let Some(key) = current.clone() {
                    map.entry(key).or_default();
                }
                continue;
            }

            let Some(key) = current.clone() else { continue };
            if !line.starts_with('|') {
                continue;
            }
            // "| 3 | `Unauthorized` | description |"
            let cells: Vec<&str> = line.split('|').collect();
            if cells.len() < 4 {
                continue;
            }
            let cell = cells[2].trim();
            if let Some(variant) = cell.strip_prefix('`').and_then(|s| s.strip_suffix('`')) {
                // Skip the header row (`Variant` has no backticks, but be safe).
                if !variant.is_empty() && variant != "Variant" {
                    map.entry(key.clone())
                        .or_default()
                        .insert(variant.to_string());
                }
            }
        }

        map
    }

    /// Union of all error enums across all contract crates, keyed by enum name.
    /// (Enum names are unique across the workspace.)
    fn all_source_enums() -> BTreeMap<String, (String, BTreeSet<String>)> {
        let mut union: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
        for (crate_name, source) in contract_sources() {
            for (enum_name, variants) in parse_error_enums(&source) {
                let prev = union
                    .entry(enum_name.clone())
                    .or_insert_with(|| (crate_name.clone(), BTreeSet::new()));
                assert_eq!(
                    prev.0, crate_name,
                    "error enum {enum_name} is declared in more than one crate"
                );
                prev.1.extend(variants);
            }
        }
        union
    }

    #[test]
    fn every_contract_error_enum_is_documented() {
        let source = all_source_enums();
        let doc = parse_doc(&read("docs/error_codes.md"));

        assert!(
            !source.is_empty(),
            "parser found no `pub enum *Error` blocks — parser or layout changed"
        );

        let mut problems: Vec<String> = Vec::new();
        for (enum_name, (crate_name, variants)) in &source {
            let key = (crate_name.clone(), enum_name.clone());
            let Some(documented) = doc.get(&key) else {
                problems.push(format!(
                    "{crate_name}::{enum_name} has no `## {crate_name}::{enum_name}` section in docs/error_codes.md"
                ));
                continue;
            };
            let missing: Vec<_> = variants.difference(documented).collect();
            let extra: Vec<_> = documented.difference(variants).collect();
            if !missing.is_empty() || !extra.is_empty() {
                problems.push(format!(
                    "{crate_name}::{enum_name}: missing from doc: {missing:?}; not declared in contract: {extra:?}"
                ));
            }
        }
        assert!(
            problems.is_empty(),
            "docs/error_codes.md drifted from the contract error enums:\n{}",
            problems.join("\n")
        );
    }

    #[test]
    fn doc_has_no_sections_for_unknown_enums() {
        let source = all_source_enums();
        let doc = parse_doc(&read("docs/error_codes.md"));

        let mut problems: Vec<String> = Vec::new();
        for ((crate_name, enum_name), variants) in &doc {
            match source.get(enum_name) {
                None => problems.push(format!(
                    "docs/error_codes.md documents {crate_name}::{enum_name}, which no contract declares"
                )),
                Some((declared_crate, declared)) => {
                    if declared_crate != crate_name {
                        problems.push(format!(
                            "docs/error_codes.md files {enum_name} under {crate_name}, but it is declared in {declared_crate}"
                        ));
                    }
                    let extra: Vec<_> = variants.difference(declared).collect();
                    if !extra.is_empty() {
                        problems.push(format!(
                            "docs/error_codes.md lists {crate_name}::{enum_name} variants not declared in the contract: {extra:?}"
                        ));
                    }
                }
            }
        }
        assert!(
            problems.is_empty(),
            "docs/error_codes.md contains stale entries:\n{}",
            problems.join("\n")
        );
    }

    /// Guard against the parser silently matching nothing on the crates the
    /// issue calls out explicitly.
    #[test]
    fn coverage_includes_key_contracts() {
        let source = all_source_enums();
        for enum_name in [
            "MuxAccountError",
            "MuxBatcherError",
            "RecoveryError",
            "RegistryError",
            "MuxRegistryError",
        ] {
            let variants = source
                .get(enum_name)
                .unwrap_or_else(|| panic!("{enum_name} not parsed from contract sources"));
            assert!(
                !variants.1.is_empty(),
                "{enum_name} parsed with zero variants"
            );
        }
        assert_eq!(source["MuxAccountError"].1.len(), 15);
        assert_eq!(source["MuxBatcherError"].1.len(), 8);
        assert_eq!(source["RecoveryError"].1.len(), 15);
    }
}
