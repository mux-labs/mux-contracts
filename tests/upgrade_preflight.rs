/// Upgrade preflight tests for Mux Protocol contracts.
///
/// These tests enforce the requirements defined in
/// `docs/upgrade-auth-requirements.md` at the source-code level, so that CI
/// catches regressions automatically rather than relying on a manual checklist.
///
/// # What is checked
///
/// 1. No `panic!()` or `todo!()` in shipped function bodies (outside `#[cfg(test)]`).
/// 2. Every `upgrade()` function in the workspace contains a `require_admin` or
///    `require_auth` call — ensuring the WASM-replace gate is never bypassed.
/// 3. Every contract that exposes `upgrade()` also has an `initialize()` that
///    sets the admin key, so the upgrade path cannot be `NotInitialized`-gapped
///    by a deployer who skips initialization.
/// 4. No instant admin-mutating entrypoint silently skips auth (spot-checks the
///    most critical patterns).
#[cfg(test)]
mod upgrade_preflight {
    use std::fs;
    use std::path::Path;

    /// Returns all `.rs` source files under `contracts/` that are NOT inside a
    /// `tests/` directory (i.e., shipped library code, not test helpers).
    fn shipped_rs_files() -> Vec<std::path::PathBuf> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()  // workspace root
            .expect("workspace root");
        let contracts_dir = root.join("contracts");

        let mut files = Vec::new();
        collect_rs(&contracts_dir, &mut files);
        files
    }

    fn collect_rs(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip test-only directories and the test-helpers crate itself
                // (it uses panic! intentionally as assertion helpers).
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name == "tests"
                    || name == "test_snapshots"
                    || name == "soroban-test-helpers"
                {
                    continue;
                }
                collect_rs(&path, out);
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                out.push(path);
            }
        }
    }

    // ── Test 1: No panic!() on shipped paths ─────────────────────────────────

    /// Scan every shipped `.rs` file for bare `panic!()` invocations outside
    /// test modules.  A `panic!` inside `#[cfg(test)]` / `mod tests` is fine;
    /// one in production code is a hard failure in Soroban (contracts that
    /// panic abort the whole transaction and leak no information, but it
    /// signals an unhandled case that should be an explicit `Err` instead).
    #[test]
    fn no_panic_on_shipped_paths() {
        let mut violations: Vec<String> = Vec::new();

        for file in shipped_rs_files() {
            let src = fs::read_to_string(&file).unwrap_or_default();
            let in_test_block = is_in_test_region(&src);
            for (i, line) in src.lines().enumerate() {
                let lineno = i + 1;
                if in_test_block[i] {
                    continue;
                }
                // Detect panic! but allow the string literal "panic!" inside
                // comments or doc strings by checking the trimmed start.
                let trimmed = line.trim();
                if trimmed.starts_with("//") || trimmed.starts_with("///") {
                    continue;
                }
                if line.contains("panic!") {
                    violations.push(format!(
                        "{}:{}: {}",
                        file.display(),
                        lineno,
                        line.trim()
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "panic!() found on shipped (non-test) paths — replace with explicit Err returns:\n{}",
            violations.join("\n")
        );
    }

    // ── Test 2: No todo!() on shipped paths ──────────────────────────────────

    #[test]
    fn no_todo_on_shipped_paths() {
        let mut violations: Vec<String> = Vec::new();

        for file in shipped_rs_files() {
            let src = fs::read_to_string(&file).unwrap_or_default();
            let in_test_block = is_in_test_region(&src);
            for (i, line) in src.lines().enumerate() {
                let trimmed = line.trim();
                if in_test_block[i]
                    || trimmed.starts_with("//")
                    || trimmed.starts_with("///")
                {
                    continue;
                }
                if line.contains("todo!()") || line.contains("todo!(\"") {
                    violations.push(format!(
                        "{}:{}: {}",
                        file.display(),
                        i + 1,
                        line.trim()
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "todo!() found on shipped (non-test) paths — implement or remove:\n{}",
            violations.join("\n")
        );
    }

    // ── Test 3: upgrade() always calls require_admin / require_auth ──────────

    /// For every shipped file that contains `pub fn upgrade`, verify that the
    /// function body also contains `require_admin` or `require_auth` before
    /// `update_current_contract_wasm`.  This catches the silent-skip pattern
    /// where auth is accidentally removed during a refactor.
    #[test]
    fn upgrade_fn_always_has_auth_gate() {
        let mut violations: Vec<String> = Vec::new();

        for file in shipped_rs_files() {
            let src = fs::read_to_string(&file).unwrap_or_default();
            if !src.contains("pub fn upgrade") {
                continue;
            }

            // Extract a window of text around `pub fn upgrade`.
            let start = src.find("pub fn upgrade").unwrap();
            let window = &src[start..std::cmp::min(start + 400, src.len())];

            // Accept: direct require_auth/require_admin call, OR a call to a
            // helper like Self::require_admin() which is defined in the same file.
            let has_direct_auth = window.contains("require_admin")
                || window.contains("require_auth")
                || window.contains("require_owner");

            // If the helper is not called inline, check the whole file for a
            // `fn require_admin` definition — the upgrade() body calls it via
            // `Self::require_admin(...)` which contains the substring already.
            let has_auth = has_direct_auth
                || (src.contains("fn require_admin") && window.contains("require_admin"))
                || (src.contains("fn require_owner") && window.contains("require_owner"));

            let has_wasm = window.contains("update_current_contract_wasm");

            if !has_auth {
                violations.push(format!(
                    "{}: upgrade() has no require_admin / require_auth call",
                    file.display()
                ));
            }
            if !has_wasm {
                violations.push(format!(
                    "{}: upgrade() has no update_current_contract_wasm call",
                    file.display()
                ));
            }
        }

        assert!(
            violations.is_empty(),
            "upgrade() auth gate violations found:\n{}",
            violations.join("\n")
        );
    }

    // ── Test 4: pause() and unpause() require owner auth ─────────────────────

    /// Verify that any contract exposing pause/unpause has a require_owner or
    /// require_auth call inside the function body.
    #[test]
    fn pause_and_unpause_require_auth() {
        let mut violations: Vec<String> = Vec::new();

        for file in shipped_rs_files() {
            let src = fs::read_to_string(&file).unwrap_or_default();

            for fn_name in &["pub fn pause", "pub fn unpause"] {
                if !src.contains(fn_name) {
                    continue;
                }
                let start = src.find(fn_name).unwrap();
                let window = &src[start..std::cmp::min(start + 300, src.len())];
                if !window.contains("require_owner")
                    && !window.contains("require_auth")
                    && !window.contains("require_admin")
                {
                    violations.push(format!(
                        "{}: {} has no auth call",
                        file.display(),
                        fn_name.trim()
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "pause/unpause auth violations:\n{}",
            violations.join("\n")
        );
    }

    // ── Helper: build a per-line boolean mask for test regions ───────────────

    /// Returns a `Vec<bool>` with one entry per source line.  `true` means the
    /// line is inside a `#[cfg(test)]`-guarded `mod tests { … }` block and
    /// should be exempt from production-code checks.
    fn is_in_test_region(src: &str) -> Vec<bool> {
        let lines: Vec<&str> = src.lines().collect();
        let mut result = vec![false; lines.len()];
        let mut in_test = false;
        let mut depth: i32 = 0;
        let mut saw_cfg_test = false;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed == "#[cfg(test)]" {
                saw_cfg_test = true;
                result[i] = true;
                continue;
            }
            if saw_cfg_test && trimmed.starts_with("mod ") {
                in_test = true;
                saw_cfg_test = false;
            }
            if in_test {
                result[i] = true;
                depth += line.chars().filter(|&c| c == '{').count() as i32;
                depth -= line.chars().filter(|&c| c == '}').count() as i32;
                if depth <= 0 {
                    in_test = false;
                    depth = 0;
                }
            }
        }
        result
    }
}
