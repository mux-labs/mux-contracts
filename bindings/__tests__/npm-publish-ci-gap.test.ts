/**
 * Failing tests capturing documented gaps in the npm-publish pipeline and
 * access-control checklist — issues #625 #629 #624 #623.
 *
 * Gap 1 (npm-publish.md / bindings.yml): The publish job reads NPM_TOKEN from
 * GitHub secrets but ci.yml has no job that validates the secret name matches
 * the name referenced in bindings.yml. A rename of the secret silently breaks
 * the publish pipeline with no pre-merge signal.
 *
 * Gap 2 (access-control-checklist.md T-40): execute_with_session stores
 * SessionKeyRecord.scopes but does NOT match them against the payload's target
 * method. Any non-empty scope list passes. Only empty scopes are rejected.
 *
 * These tests are intentionally failing until the gaps are closed:
 *   - Gap 1: add a CI step that parses bindings.yml and asserts the secret
 *     name equals NPM_TOKEN.
 *   - Gap 2: implement scope-to-payload dispatch matching in execute_with_session.
 */

import * as fs from 'fs';
import * as path from 'path';
import * as yaml from 'js-yaml';

const REPO_ROOT = path.resolve(__dirname, '../..');

// ---------------------------------------------------------------------------
// Gap 1: NPM_TOKEN secret name consistency between bindings.yml and ci.yml
// ---------------------------------------------------------------------------

describe('npm-publish pipeline gap — secret name consistency (#625 #629)', () => {
  it('ci.yml should have a job that validates the NPM_TOKEN secret name', () => {
    const ciYaml = fs.readFileSync(
      path.join(REPO_ROOT, '.github/workflows/ci.yml'),
      'utf8'
    );
    const ci = yaml.load(ciYaml) as Record<string, unknown>;
    const jobs = (ci as { jobs: Record<string, unknown> }).jobs;

    // Expect a dedicated job (or step) that verifies NPM_TOKEN is consistent
    // between ci.yml and bindings.yml.
    const hasSecretValidationJob =
      'check-npm-token-secret-name' in jobs ||
      Object.values(jobs).some((job: unknown) => {
        const j = job as { steps?: Array<{ name?: string; run?: string }> };
        return (j.steps ?? []).some(
          (s) =>
            (s.name ?? '').toLowerCase().includes('npm_token') ||
            (s.run ?? '').includes('NPM_TOKEN') ||
            (s.run ?? '').includes('check-npm-token')
        );
      });

    // This assertion FAILS until a secret-name validation step is added to ci.yml
    expect(hasSecretValidationJob).toBe(true);
  });

  it('bindings.yml publish job should reference NPM_TOKEN and ci.yml should cross-check it', () => {
    const bindingsYaml = fs.readFileSync(
      path.join(REPO_ROOT, '.github/workflows/bindings.yml'),
      'utf8'
    );
    const ciYaml = fs.readFileSync(
      path.join(REPO_ROOT, '.github/workflows/ci.yml'),
      'utf8'
    );

    // bindings.yml references NPM_TOKEN
    expect(bindingsYaml).toContain('NPM_TOKEN');

    // ci.yml should also reference or validate NPM_TOKEN to prevent silent drift
    // This FAILS until ci.yml includes a cross-check step
    expect(ciYaml).toMatch(/NPM_TOKEN.*secret|check.*npm.*token|npm.*token.*check/i);
  });
});

// ---------------------------------------------------------------------------
// Gap 2: execute_with_session scope enforcement (T-40)
// ---------------------------------------------------------------------------

describe('access-control-checklist gap — execute_with_session scope enforcement (#624 #623)', () => {
  it('access-control-checklist.md should document scope enforcement as CLOSED (not just tracked)', () => {
    const checklist = fs.readFileSync(
      path.join(REPO_ROOT, 'docs/access-control-checklist.md'),
      'utf8'
    );

    // The checklist currently notes the gap as open: "Remaining limitation:
    // payload is not decoded...non-empty scope list is not matched against the
    // payload's target method".
    // Once the gap is closed, the checklist should reflect enforcement is complete.
    // This FAILS until scope enforcement is implemented and the doc is updated.
    const hasOpenScopeGap =
      checklist.includes('non-empty scope list is not matched') ||
      checklist.includes('payload is not decoded or dispatched') ||
      checklist.includes('Remaining limitation: `payload` is not decoded');

    // We expect NO open scope gap statement — currently fails because the gap exists
    expect(hasOpenScopeGap).toBe(false);
  });

  it('entrypoint-matrix.md should document execute_with_session as fully scope-enforced', () => {
    const matrix = fs.readFileSync(
      path.join(REPO_ROOT, 'docs/entrypoint-matrix.md'),
      'utf8'
    );

    // The matrix currently has two rows for execute_with_session — one updated (with
    // T-40 enforcement) and one legacy (without enforcement). Once the duplicate
    // is removed and enforcement is complete, only the enforced description should
    // remain.
    const duplicateRow = (
      matrix.match(/execute_with_session/g) ?? []
    ).length;

    // Expect exactly ONE execute_with_session row in the matrix (no duplicates).
    // This FAILS because the matrix currently has duplicate rows.
    expect(duplicateRow).toBe(1);
  });
});
