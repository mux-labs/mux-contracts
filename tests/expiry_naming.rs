//! Expiry field naming guard (#586, extended by #920).
//!
//! The contract parameter, struct field, and `dlg_set` event payload all use
//! `expires_at: u64` (a Unix timestamp), but the interface documentation, the
//! shared JSON test vectors, and the TypeScript bindings still called it
//! `expiry_ledger` / `expiryLedger` — a name that is wrong in both spelling and
//! unit, and that encoded the value as a `u32` ledger sequence. These tests
//! fail if either spelling comes back.
//!
//! Issue #920 replaced the fixed `SURFACES` list with a recursive directory
//! walk over `bindings/src/` and `contracts/*/src/`, so a regression in any
//! generated client or hand-written bindings source — including files added
//! after this guard was written — is caught. A companion test pins that the
//! walk actually visits the known generated clients so an empty walk cannot
//! make the guard pass vacuously.
//!
//! Run with: cargo test -p mux-contract-tests --test expiry_naming

#[cfg(test)]
mod expiry_naming {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Spellings that must never appear again. `expiry_ledger` also implies the
    /// wrong unit: the field is a timestamp, not a ledger sequence.
    const FORBIDDEN: [&str; 2] = ["expiry_ledger", "expiryLedger"];

    /// Doc surfaces kept as an explicit list (outside `bindings/src` and
    /// `contracts/*/src`, which the directory walk covers). Previously part of
    /// the fixed `SURFACES` array — retained so doc drift stays guarded.
    const DOC_SURFACES: [&str; 3] = [
        "docs/mux-account-interface.md",
        "docs/abi_reference.md",
        "docs/audit-events.md",
    ];

    /// Generated clients the walk must visit (issue #920: an empty walk must
    /// not make the forbidden-spelling guard pass vacuously).
    const MUST_VISIT: [&str; 2] = [
        "bindings/src/generated/mux-account.ts",
        "bindings/src/generated/mux-delegation.ts",
    ];

    /// A doc line that only *quotes* the forbidden spelling while banning it
    /// (the naming admonition in `docs/mux-account-interface.md`). Doc-only:
    /// bindings and contract sources are scanned strictly.
    fn line_quotes_the_ban(line: &str) -> bool {
        line.contains("must not be")
            || line.contains("must not reappear")
            || line.contains("spelling was wrong")
            || line.contains("enforces this")
    }

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

    /// Recursively collect every file under `dir` (read_dir, no extra deps).
    fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            // A missing directory contributes no files rather than panicking:
            // `contracts/*/src` is matched per-crate below.
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, out);
            } else if path.is_file() {
                out.push(path);
            }
        }
    }

    /// Every file under `bindings/src/` (recursively) and under each
    /// `contracts/<crate>/src/` tree, as paths relative to the repo root.
    fn walked_surfaces() -> Vec<String> {
        let root = repo_root();
        let mut files: Vec<PathBuf> = Vec::new();

        collect_files(&root.join("bindings/src"), &mut files);

        let contracts_dir = root.join("contracts");
        let mut contract_srcs: Vec<PathBuf> = fs::read_dir(&contracts_dir)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", contracts_dir.display()))
            .flatten()
            .map(|entry| entry.path().join("src"))
            .filter(|src| src.is_dir())
            .collect();
        contract_srcs.sort();
        for src in &contract_srcs {
            collect_files(src, &mut files);
        }

        files.sort();
        files
            .iter()
            .filter_map(|path| {
                path.strip_prefix(&root)
                    .ok()
                    .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            })
            .collect()
    }

    #[test]
    fn no_surface_uses_the_old_expiry_spelling() {
        let mut offenders: Vec<String> = Vec::new();

        // Full coverage: every file under bindings/src and contracts/*/src
        // (strict, whole-file substring match).
        for surface in walked_surfaces() {
            let text = read(&surface);
            for spelling in FORBIDDEN {
                if text.contains(spelling) {
                    offenders.push(format!("{surface} uses `{spelling}`"));
                }
            }
        }

        // Explicit doc surfaces (outside the walked trees). Docs are scanned
        // line-by-line so the naming admonitions that *quote* the banned
        // spelling while forbidding it (e.g. the interface doc's
        // "…`expiry_ledger` spelling was wrong… must not be reintroduced")
        // do not trip the guard — while any real usage still fails.
        for surface in DOC_SURFACES {
            let text = read(surface);
            for (idx, line) in text.lines().enumerate() {
                if line_quotes_the_ban(line) {
                    continue;
                }
                for spelling in FORBIDDEN {
                    if line.contains(spelling) {
                        offenders.push(format!("{surface}:{} uses `{spelling}`", idx + 1));
                    }
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "the expiry field is named `expires_at`: {offenders:?}"
        );
    }

    /// Issue #920: the walk must actually visit the known generated clients —
    /// an empty or truncated walk would let the guard above pass vacuously.
    #[test]
    fn expiry_walk_visits_known_generated_clients() {
        let visited = walked_surfaces();

        assert!(
            !visited.is_empty(),
            "directory walk visited no files — guard would pass vacuously"
        );
        for expected in MUST_VISIT {
            assert!(
                visited.iter().any(|path| path == expected),
                "walk must visit {expected}; visited: {visited:?}"
            );
        }
        // Sanity: the walk also reaches hand-written bindings sources.
        assert!(
            visited.iter().any(|path| path.starts_with("bindings/src/")),
            "walk must include bindings/src files"
        );
        assert!(
            visited.iter().any(|path| path.starts_with("contracts/")),
            "walk must include contracts/*/src files"
        );
    }

    /// The contract is the source of truth for the name and the type.
    #[test]
    fn contract_declares_expires_at_as_u64() {
        let source = read("contracts/mux-account/src/lib.rs");
        assert!(
            source.contains("pub expires_at: u64,"),
            "DelegateInfo / SessionKeyRecord must declare `expires_at: u64`"
        );
        assert!(
            source.contains("expires_at: u64,\n        can_spend: bool,"),
            "set_delegate must take `expires_at: u64`"
        );
    }

    /// The interface doc must name the field the contract actually exposes.
    #[test]
    fn interface_doc_documents_expires_at() {
        let doc = read("docs/mux-account-interface.md");
        assert!(
            doc.contains("set_delegate(delegate, expires_at, can_spend)"),
            "the interface doc must show the real `set_delegate` signature"
        );
        assert!(
            doc.contains("`expires_at` (Unix timestamp, `u64`)"),
            "the interface doc must state the field's name and unit"
        );
    }

    /// The shared JSON vectors are read by both the Rust and TypeScript suites,
    /// so a stale key name there silently desynchronises the two.
    #[test]
    fn fixtures_use_expires_at() {
        for fixture in [
            "tests/fixtures/test_vectors.json",
            "tests/fixtures/account_limit_vectors.json",
        ] {
            let text = read(fixture);
            assert!(
                text.contains("\"expires_at\""),
                "{fixture} must key delegate expiry as `expires_at`"
            );
        }
    }

    /// The generated client must send a `u64` timestamp, not a `u32` ledger
    /// sequence — the old name carried the old encoding with it.
    #[test]
    fn bindings_encode_expires_at_as_u64() {
        let client = read("bindings/src/generated/mux-account.ts");
        assert!(
            client.contains("expiresAt: bigint")
                && client.contains("nativeToScVal(expiresAt, { type: \"u64\" })"),
            "setDelegate must take `expiresAt: bigint` and encode it as u64"
        );

        let types = read("bindings/src/types.ts");
        assert!(
            types.contains("expiresAt: bigint;"),
            "DelegateInfo must expose `expiresAt: bigint`"
        );
    }
}
