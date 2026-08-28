//! AA Phase 2 milestone guard (#583).
//!
//! `docs/aa-milestone-roadmap.md` listed `execute_with_session()` transaction
//! logic, relayer sponsorship, and the frontend/relayer integration material as
//! unchecked Phase 2 work while the contract shipped a validation-only stub.
//! These tests fail if that gap is reintroduced — either by un-checking the
//! roadmap, by deleting the integration material, or by regressing the
//! entrypoint back to a non-dispatching stub.
//!
//! Run with: cargo test -p mux-contract-tests --test aa_phase2_milestone

#[cfg(test)]
mod aa_phase2_milestone {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf()
    }

    fn read(relative: &str) -> String {
        let path = repo_root().join(relative);
        fs::read_to_string(&path).unwrap_or_else(|_| panic!("{relative} must exist"))
    }

    /// Extract the lines of a `## `-delimited section, excluding its heading.
    fn section(doc: &str, heading_prefix: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let mut inside = false;
        for line in doc.lines() {
            if line.starts_with("## ") {
                inside = line.starts_with(heading_prefix);
                continue;
            }
            if inside {
                lines.push(line.to_string());
            }
        }
        assert!(!lines.is_empty(), "section `{heading_prefix}` not found");
        lines
    }

    /// Every Phase 2 roadmap item must be checked off.
    #[test]
    fn roadmap_phase_two_has_no_open_items() {
        let doc = read("docs/aa-milestone-roadmap.md");
        let open: Vec<String> = section(&doc, "## Phase 2")
            .into_iter()
            .filter(|line| line.trim_start().starts_with("- [ ]"))
            .collect();
        assert!(
            open.is_empty(),
            "AA Phase 2 must be closed, but these items are still open: {open:?}"
        );
    }

    /// The milestone's deliverables must actually be present in the tree — a
    /// checked box with nothing behind it is the same gap in a new form.
    #[test]
    fn phase_two_deliverables_exist() {
        let relayer_doc = read("docs/relayer-integration.md");
        assert!(
            relayer_doc.contains("execute_with_session_sponsored"),
            "relayer documentation must describe the sponsored entrypoint"
        );

        let example = read("examples/session-key-usage.ts");
        for expected in [
            "register_session_key",
            "execute_with_session",
            "execute_with_session_sponsored",
            "set_sponsor",
        ] {
            assert!(
                example.contains(expected),
                "the frontend example must demonstrate `{expected}`"
            );
        }
    }

    /// `execute_with_session` must dispatch rather than validate-and-return.
    /// The stub took a `payload: Bytes` it never decoded; the shipped
    /// entrypoint takes an explicit target and function and invokes them.
    #[test]
    fn execute_with_session_is_not_a_stub() {
        let source = read("contracts/mux-account/src/lib.rs");
        assert!(
            source.contains("pub fn execute_with_session(\n        env: Env,\n        session_key: Address,\n        target: Address,\n        function: Symbol,\n        args: Vec<Val>,\n        nonce: u64,\n    ) -> Result<Val, MuxAccountError> {"),
            "execute_with_session must take (session_key, target, function, args) and return the target's Val"
        );
        assert!(
            source.contains("fn dispatch(") && source.contains("env.invoke_contract::<Val>(target, function, args)"),
            "execute_with_session must invoke the target contract"
        );
        assert!(
            source.contains("pub fn execute_with_session_sponsored("),
            "the sponsored relayer path must exist"
        );
    }

    /// The docs the audit reads must not still describe the stub.
    #[test]
    fn docs_do_not_describe_a_non_dispatching_session_path() {
        for doc in [
            "docs/aa_sequence_diagram.md",
            "docs/mux-account-interface.md",
            "docs/entrypoint-matrix.md",
            "docs/account-abstraction.md",
        ] {
            let text = read(doc);
            assert!(
                !text.contains("does not execute `payload`")
                    && !text.contains("payload dispatch itself is not yet implemented"),
                "{doc} still describes execute_with_session as a non-dispatching stub"
            );
        }
    }
}
