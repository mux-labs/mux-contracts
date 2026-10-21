/**
 * Failing tests capturing gaps in scripts/generate-bindings.sh and ci.yml
 * related to silent WASM skipping and missing fail-closed auth enforcement.
 * Issues: #630 #627 #628 #626
 *
 * Gap 1 (generate-bindings.sh): The script uses `continue` (silent skip) when
 * a WASM is missing instead of `exit 1`. This means a contract can silently
 * disappear from the generated bindings with no CI signal.
 *
 * Gap 2 (ci.yml bindings job): No step asserts that all expected contracts are
 * present in bindings/src/generated/ after generation. The binding count is
 * never checked.
 *
 * Gap 3 (fail-closed auth): mux-batcher upgrade() and execute_batch() must
 * return NotInitialized (not silently pass) when initialize() was never called.
 * Current docs confirm the behaviour but no unit-level TS test asserts it.
 *
 * Tests are intentionally failing until gaps are closed.
 */

import * as fs from 'fs';
import * as path from 'path';
import { spawnSync } from 'child_process';

const REPO_ROOT = path.resolve(__dirname, '../..');
const GENERATE_BINDINGS_SCRIPT = path.join(REPO_ROOT, 'scripts/generate-bindings.sh');

/**
 * Read a file relative to the repo root, failing with a clear, actionable
 * error message rather than letting a bare ENOENT from fs.readFileSync
 * bubble up when the file has moved or been renamed (#1006).
 */
function readRepoFile(relPath: string): string {
  const fullPath = path.join(REPO_ROOT, relPath);
  if (!fs.existsSync(fullPath)) {
    throw new Error(
      `generate-bindings-gap.test.ts: expected file not found at "${relPath}" ` +
        `(resolved to ${fullPath}). The file may have moved or been renamed — ` +
        `update the path in this test.`
    );
  }
  return fs.readFileSync(fullPath, 'utf8');
}

// ---------------------------------------------------------------------------
// Gap 1: generate-bindings.sh must fail on missing WASM, not silently skip
// ---------------------------------------------------------------------------

describe('generate-bindings.sh — fail-closed WASM check (#630 #627)', () => {
  /**
   * Actually run the generator with --skip-build against the real repo
   * layout. No cargo build has produced any WASM artifacts in this test
   * environment, so the very first contract lookup is guaranteed to miss —
   * exercising the real fail-closed path end to end instead of grepping the
   * script's source text for expected strings (#1005).
   */
  function runGenerator() {
    return spawnSync('bash', [GENERATE_BINDINGS_SCRIPT, '--skip-build'], {
      cwd: REPO_ROOT,
      encoding: 'utf8',
    });
  }

  it('exits non-zero when a WASM is missing', () => {
    const result = runGenerator();
    expect(result.status).not.toBe(0);
  });

  it('reports the failure on stderr with an [ERROR] marker, not a silent [WARN]+continue', () => {
    const result = runGenerator();
    expect(result.stderr).toContain('[ERROR]');
    expect(result.stderr).not.toContain('[WARN] WASM not found');
    expect(result.stdout).not.toContain('[WARN] WASM not found');
  });

  it('names the missing contract and the expected WASM path in its error output', () => {
    const result = runGenerator();
    // The script iterates contracts in declared order, so the first missing
    // WASM reported should be the first entry in its CONTRACTS array.
    expect(result.stderr).toMatch(/WASM not found for mux-account at/);
    expect(result.stderr).toContain('wasm32-unknown-unknown/release/mux_account.wasm');
  });
});

// ---------------------------------------------------------------------------
// Gap 2: ci.yml bindings job should assert all expected contracts are generated
// ---------------------------------------------------------------------------

describe('ci.yml bindings job — contract count assertion (#628)', () => {
  it('ci.yml bindings job should include a step that asserts all expected contracts are generated', () => {
    const ciYaml = readRepoFile('.github/workflows/ci.yml');

    // Expect a step like: "Assert all expected contracts generated"
    // or a command that checks the generated directory for all contract names
    // This FAILS until such a step is added
    const hasContractCountCheck =
      ciYaml.includes('assert') ||
      ciYaml.includes('check-generated-contracts') ||
      ciYaml.match(/generated.*contracts|contracts.*generated|bindings.*count|count.*bindings/i) !== null;

    expect(hasContractCountCheck).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Gap 3: fail-closed auth — mux-batcher upgrade must reject uninitialised state
// ---------------------------------------------------------------------------

describe('fail-closed auth — mux-batcher require_auth not silently skipped (#626)', () => {
  it('access-control-checklist.md documents batcher upgrade as fail-closed (NotInitialized)', () => {
    const checklist = readRepoFile('docs/access-control-checklist.md');

    // The checklist should have a passing mark [x] for batcher upgrade fail-closed
    // This checks that the checklist item is marked Pass (not Fail/open)
    // Currently the checkboxes are [ ] (open), so this FAILS
    const batcherUpgradePassClosed =
      checklist.match(/\[x\].*upgrade.*NotInitialized|\[x\].*fail-closed.*batcher/i) !== null ||
      checklist.match(/upgrade.*\[x\].*require_admin/i) !== null;

    expect(batcherUpgradePassClosed).toBe(true);
  });

  it('access-control-checklist.md documents execute_batch as fail-closed with auth', () => {
    const checklist = readRepoFile('docs/access-control-checklist.md');

    // execute_batch should be marked [x] Pass in the checklist
    // Currently all checkboxes are [ ] (open), so this FAILS
    const executeBatchPass =
      checklist.match(/\[x\].*execute_batch.*require_auth|\[x\].*bat_start/i) !== null;

    expect(executeBatchPass).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Missing-path guard (#1006): a moved/renamed repo file must fail with a
// clear, actionable error message instead of a bare ENOENT throw.
// ---------------------------------------------------------------------------

describe('readRepoFile — missing-path guard (#1006)', () => {
  it('throws a clear, actionable error when the file does not exist', () => {
    expect(() => readRepoFile('scripts/does-not-exist.sh')).toThrow(
      /expected file not found at "scripts\/does-not-exist\.sh"/
    );
  });

  it('reads the file normally when the path exists', () => {
    expect(() => readRepoFile('scripts/generate-bindings.sh')).not.toThrow();
  });
});
