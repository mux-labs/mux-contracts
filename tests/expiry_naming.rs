//! Expiry field naming guard (#586).
//!
//! The contract parameter, struct field, and `dlg_set` event payload all use
//! `expires_at: u64` (a Unix timestamp), but the interface documentation, the
//! shared JSON test vectors, and the TypeScript bindings still called it
//! `expiry_ledger` / `expiryLedger` — a name that is wrong in both spelling and
//! unit, and that encoded the value as a `u32` ledger sequence. These tests
//! fail if either spelling comes back.
//!
//! Run with: cargo test -p mux-contract-tests --test expiry_naming

#[cfg(test)]
mod expiry_naming {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Spellings that must never appear again. `expiry_ledger` also implies the
    /// wrong unit: the field is a timestamp, not a ledger sequence.
    const FORBIDDEN: [&str; 2] = ["expiry_ledger", "expiryLedger"];

    /// Files that define or consume the expiry field across the three surfaces
    /// that drifted: the contract, the docs, the fixtures, and the bindings.
    const SURFACES: [&str; 8] = [
        "contracts/mux-account/src/lib.rs",
        "docs/mux-account-interface.md",
        "docs/abi_reference.md",
        "docs/audit-events.md",
        "tests/fixtures/test_vectors.json",
        "tests/fixtures/account_limit_vectors.json",
        "bindings/src/types.ts",
        "bindings/src/generated/mux-account.ts",
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

    #[test]
    fn no_surface_uses_the_old_expiry_spelling() {
        let mut offenders: Vec<String> = Vec::new();
        for surface in SURFACES {
            let text = read(surface);
            for spelling in FORBIDDEN {
                if text.contains(spelling) {
                    offenders.push(format!("{surface} uses `{spelling}`"));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "the expiry field is named `expires_at`: {offenders:?}"
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
