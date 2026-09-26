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

const REPO_ROOT = path.resolve(__dirname, '../..');

// ---------------------------------------------------------------------------
// Gap 1: generate-bindings.sh must fail on missing WASM, not silently skip
// ---------------------------------------------------------------------------

describe('generate-bindings.sh — fail-closed WASM check (#630 #627)', () => {
  it('generate-bindings.sh should exit non-zero when a WASM is missing', () => {
    const script = fs.readFileSync(
      path.join(REPO_ROOT, 'scripts/generate-bindings.sh'),
      'utf8'
    );

    // The current script has: echo "[WARN] WASM not found... skipping"
    // It should instead: echo "[ERROR] WASM not found..." >&2 && exit 1
    // This FAILS because the script currently continues on missing WASM
    const hasFailClosedMissingWasm =
      script.includes('exit 1') &&
      !script.includes('[WARN] WASM not found') &&
      script.includes('[ERROR]');

    expect(hasFailClosedMissingWasm).toBe(true);
  });

  it('generate-bindings.sh should not use silent continue on missing WASM', () => {
    const script = fs.readFileSync(
      path.join(REPO_ROOT, 'scripts/generate-bindings.sh'),
      'utf8'
    );

    // Currently the script has a WARN + continue pattern — this should be removed
    // This FAILS because the WARN+continue pattern currently exists
    expect(script).not.toContain('[WARN] WASM not found');
  });
});

// ---------------------------------------------------------------------------
// Gap 2: ci.yml bindings job should assert all expected contracts are generated
// ---------------------------------------------------------------------------

describe('ci.yml bindings job — contract count assertion (#628)', () => {
  it('ci.yml bindings job should include a step that asserts all expected contracts are generated', () => {
    const ciYaml = fs.readFileSync(
      path.join(REPO_ROOT, '.github/workflows/ci.yml'),
      'utf8'
    );

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
    const checklist = fs.readFileSync(
      path.join(REPO_ROOT, 'docs/access-control-checklist.md'),
      'utf8'
    );

    // The checklist should have a passing mark [x] for batcher upgrade fail-closed
    // This checks that the checklist item is marked Pass (not Fail/open)
    // Currently the checkboxes are [ ] (open), so this FAILS
    const batcherUpgradePassClosed =
      checklist.match(/\[x\].*upgrade.*NotInitialized|\[x\].*fail-closed.*batcher/i) !== null ||
      checklist.match(/upgrade.*\[x\].*require_admin/i) !== null;

    expect(batcherUpgradePassClosed).toBe(true);
  });

  it('access-control-checklist.md documents execute_batch as fail-closed with auth', () => {
    const checklist = fs.readFileSync(
      path.join(REPO_ROOT, 'docs/access-control-checklist.md'),
      'utf8'
    );

    // execute_batch should be marked [x] Pass in the checklist
    // Currently all checkboxes are [ ] (open), so this FAILS
    const executeBatchPass =
      checklist.match(/\[x\].*execute_batch.*require_auth|\[x\].*bat_start/i) !== null;

    expect(executeBatchPass).toBe(true);
  });
});
