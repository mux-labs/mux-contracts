/// Bindings coverage tests.
///
/// These tests enforce a 1:1 relationship between the production contract
/// crates (`contracts/mux-*`) and the generated TypeScript clients
/// (`bindings/src/generated/*.ts`). Adding a new contract crate without
/// regenerating its client — or deleting a generated client — is drift that
/// would otherwise only be caught by a human noticing, never by CI.
///
/// This is the same drift class that `tests/threat_model_coverage.rs`
/// (docs coverage) and `scripts/check-entrypoint-matrix` (ABI docs) guard
/// against; see issue #963.
///
/// Run with: cargo test -p mux-contract-tests --test bindings_coverage
#[cfg(test)]
mod bindings_coverage {
    use std::fs;
    use std::path::Path;

    /// Returns the crate names of every `contracts/mux-*` directory (the
    /// production contract crates). `soroban-test-helpers` is excluded — it
    /// is a test-utility crate that does not ship WASM.
    ///
    /// Both sets are derived from the filesystem (no hard-coded crate list)
    /// so the guard stays correct when a new contract crate is added.
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

    /// Returns the base names (`.ts` stripped) of every generated TypeScript
    /// client in `bindings/src/generated/`.
    fn generated_binding_clients() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent() // workspace root
            .expect("workspace root");
        let generated_dir = root.join("bindings/src/generated");

        let mut clients = Vec::new();
        let Ok(entries) = fs::read_dir(&generated_dir) else {
            return clients;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(base) = name.strip_suffix(".ts") {
                clients.push(base.to_string());
            }
        }
        clients.sort();
        clients
    }

    /// Every production contract crate must have a matching generated
    /// TypeScript client, and every generated client must belong to a
    /// production contract crate. Both directions are checked so that a
    /// crate added without regenerating bindings — and a leftover generated
    /// client for a removed contract — both fail loudly.
    #[test]
    fn every_contract_crate_has_a_generated_client_and_vice_versa() {
        let crates = production_contract_crates();
        assert!(
            !crates.is_empty(),
            "expected at least one contracts/mux-* crate; found none — \
             is the test resolving paths from the workspace root?"
        );

        let clients = generated_binding_clients();

        let missing_clients: Vec<&String> = crates.iter().filter(|c| !clients.contains(c)).collect();
        let orphan_clients: Vec<&String> = clients.iter().filter(|c| !crates.contains(c)).collect();

        assert!(
            missing_clients.is_empty() && orphan_clients.is_empty(),
            "contracts/mux-* crates and bindings/src/generated/*.ts clients are out of sync.\n\
             Contract crates without a generated client: {missing_clients:?}\n\
             Generated clients without a contract crate: {orphan_clients:?}\n\
             Contract crates: {crates:?}\n\
             Generated clients: {clients:?}\n\
             Run `npm run generate` (from bindings/) or `bash scripts/generate-bindings.sh` \
             to regenerate bindings, and keep this set 1:1 with the crates that ship WASM."
        );
    }

    /// Sanity-check the enumeration helpers themselves so a silent
    /// regression to "always empty" cannot make the guard pass vacuously.
    #[test]
    fn coverage_enumeration_is_not_vacuous() {
        let crates = production_contract_crates();
        let clients = generated_binding_clients();
        assert!(
            crates.len() >= 3,
            "expected at least the three original contract crates, got: {crates:?}"
        );
        assert!(
            clients.len() >= 3,
            "expected at least the three original generated clients, got: {clients:?}"
        );
    }
}
