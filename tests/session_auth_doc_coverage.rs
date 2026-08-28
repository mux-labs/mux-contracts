//! Session-key authorization documentation guard (#584).
//!
//! `docs/entrypoint-matrix.md` and `docs/authorize-flow-example.md` labelled
//! session-key auth as a TODO while `mux-account` already enforced it, and the
//! matrix carried a stale duplicate `execute_with_session` row describing the
//! older behaviour. An auditor reading either document would have concluded the
//! path was unauthenticated. These tests fail if that drift returns.
//!
//! Run with: cargo test -p mux-contract-tests --test session_auth_doc_coverage

#[cfg(test)]
mod session_auth_doc_coverage {
    use std::fs;
    use std::path::{Path, PathBuf};

    const AUTH_DOCS: [&str; 2] = [
        "docs/entrypoint-matrix.md",
        "docs/authorize-flow-example.md",
    ];

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

    /// The `## mux-account` table of the entrypoint matrix, as table rows.
    fn mux_account_rows() -> Vec<String> {
        let matrix = read("docs/entrypoint-matrix.md");
        let mut rows = Vec::new();
        let mut inside = false;
        for line in matrix.lines() {
            if line.starts_with("## ") {
                inside = line.trim() == "## mux-account";
                continue;
            }
            if inside && line.starts_with("| `") {
                rows.push(line.to_string());
            }
        }
        assert!(!rows.is_empty(), "the `## mux-account` table must exist");
        rows
    }

    /// First column (the entrypoint signature) of a table row.
    fn entrypoint_cell(row: &str) -> String {
        row.split('|')
            .nth(1)
            .expect("table row has a first column")
            .trim()
            .to_string()
    }

    /// The authorization docs are what an auditor reads to decide whether a
    /// path is gated. An unresolved TODO there is a documented gap.
    #[test]
    fn auth_docs_carry_no_unresolved_todo() {
        for doc in AUTH_DOCS {
            let doc_text = read(doc);
            let offenders: Vec<&str> = doc_text
                .lines()
                .filter(|line| line.contains("TODO"))
                .collect();
            assert!(
                offenders.is_empty(),
                "{doc} still carries unresolved TODOs: {offenders:?}"
            );
        }
    }

    /// Both docs must state the actual session-key auth rule, not merely name
    /// the entrypoint.
    #[test]
    fn auth_docs_describe_session_key_authorization() {
        let flow = read("docs/authorize-flow-example.md");
        assert!(
            flow.contains("session_key.require_auth()"),
            "authorize-flow-example.md must show the session-key auth call"
        );
        assert!(
            flow.contains("ScopeNotGranted"),
            "authorize-flow-example.md must show the fail-closed scope outcome"
        );

        let matrix = read("docs/entrypoint-matrix.md");
        assert!(
            matrix.contains("Session key auth"),
            "entrypoint-matrix.md must classify execute_with_session as session-key authorized"
        );
    }

    /// A single entrypoint must have a single row. The duplicate row this test
    /// guards against described the opposite behaviour from the row above it.
    #[test]
    fn mux_account_matrix_has_no_duplicate_rows() {
        let mut seen: Vec<String> = Vec::new();
        let mut duplicates: Vec<String> = Vec::new();
        for row in mux_account_rows() {
            let entry = entrypoint_cell(&row);
            if seen.contains(&entry) {
                duplicates.push(entry);
            } else {
                seen.push(entry);
            }
        }
        assert!(
            duplicates.is_empty(),
            "the mux-account matrix lists the same entrypoint twice: {duplicates:?}"
        );
    }

    /// Every public `mux-account` entrypoint must appear in the matrix, so a
    /// newly added auth-bearing path cannot ship undocumented.
    #[test]
    fn every_mux_account_entrypoint_is_in_the_matrix() {
        let source = read("contracts/mux-account/src/lib.rs");
        // Stop at the test module: its helper contracts also declare `pub fn`.
        let production = source
            .split_once("#[cfg(test)]")
            .map(|(head, _)| head.to_string())
            .unwrap_or(source);

        let documented: Vec<String> = mux_account_rows()
            .iter()
            .map(|row| entrypoint_cell(row))
            .collect();

        let mut missing: Vec<String> = Vec::new();
        for line in production.lines() {
            let Some(rest) = line.trim().strip_prefix("pub fn ") else {
                continue;
            };
            let name = rest.split(['(', '<']).next().unwrap_or("").trim();
            if name.is_empty() {
                continue;
            }
            let signature = format!("`{name}(");
            if !documented.iter().any(|row| row.starts_with(&signature)) {
                missing.push(name.to_string());
            }
        }
        assert!(
            missing.is_empty(),
            "entrypoint-matrix.md does not document these mux-account entrypoints: {missing:?}"
        );
    }
}
