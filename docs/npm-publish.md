# npm Publish Flow — @mux-protocol/contracts

This document describes the end-to-end process for releasing a new version of the
`@mux-protocol/contracts` TypeScript package to npm.

---

## Overview

Publishing is **fully automated** via GitHub Actions
(`.github/workflows/bindings.yml`). A human only needs to:

1. Bump the version in `bindings/package.json`.
2. Commit with the magic prefix `chore: release`.
3. Push / merge to `main`.

CI does the rest.

---

## Prerequisites

### npm Access Token

A scoped npm publish token must be stored as the GitHub repository secret
`NPM_TOKEN`.

1. Log in at <https://www.npmjs.com> with an account that has **publish** rights
   to the `@mux-protocol` scope.
2. Generate an **Automation** token (type: *Automation* — bypasses MFA, safe for
   CI):  
   *Profile → Access Tokens → Generate New Token → Automation*
3. Add it to the repository:  
   *GitHub repo → Settings → Secrets and variables → Actions → New repository secret*  
   Name: `NPM_TOKEN`, Value: the token from step 2.

### Package Scope

The package is published as `@mux-protocol/contracts` (scoped, public). Scoped
packages are private by default on npm; the CI job passes `--access public` to
override this.

---

## Step-by-Step Release Process

### 1. Update the version

In `bindings/package.json`, bump `version` following [semver](https://semver.org/):

```bash
cd bindings
# patch bump (e.g. 0.1.0 → 0.1.1)
npm version patch --no-git-tag-version

# minor bump (e.g. 0.1.0 → 0.2.0)
npm version minor --no-git-tag-version

# major bump (e.g. 0.1.0 → 1.0.0)
npm version major --no-git-tag-version
```

The `--no-git-tag-version` flag updates only the file without creating a git tag
(CI handles publishing, not git tags).

### 2. Commit with the release trigger

The publish job is gated on the commit message **starting with** `chore: release`:

```bash
git add bindings/package.json
git commit -m "chore: release v0.2.0"
git push origin main
```

Any commit that does **not** start with `chore: release` will run all CI checks
but skip the publish step.

### 3. CI pipeline

On every push to `main` the `bindings.yml` workflow runs these jobs in order:

| Job | What it does |
|-----|-------------|
| `build-contracts` | Compiles Rust contracts to WASM (release profile) |
| `test-contracts` | Runs `cargo test --workspace --all-features` |
| `generate-bindings` | Calls `stellar contract bindings typescript` to regenerate TS clients |
| `test-bindings` | `npm ci` → lint → `tsc --noEmit` → `npm test` → `npm run build` |
| `check-binding-drift` | (PRs only) Fails if generated bindings differ from committed state |
| **`publish`** | Downloads built dist/, runs `npm publish --provenance --access public` |

The `publish` job runs **only** when the commit message starts with
`chore: release`. All other jobs run on every push and PR.

### 4. Verify the release

After the pipeline completes:

```bash
# Confirm the new version is live
npm view @mux-protocol/contracts version

# Install and smoke-test
npm install @mux-protocol/contracts@<new-version>
```

---

## npm Provenance Attestation

The publish job uses the `--provenance` flag, which generates a
[SLSA provenance attestation](https://slsa.dev/) and attaches it to the npm
package. This lets consumers verify:

- Which GitHub repository and commit produced the package.
- That the package was built by the official CI pipeline.

No additional setup is required; GitHub Actions provides the OIDC token that
`npm publish --provenance` uses automatically when run in a GitHub Actions
environment.

Provenance records are visible on the npm package page under
*Provenance → View attestation*.

---

## Manual Publish (break-glass)

If CI is broken and you must publish manually:

```bash
cd bindings

# 1. Ensure bindings are up-to-date
bash ../scripts/generate-bindings.sh

# 2. Build the package
npm ci && npm run build

# 3. Publish (you will be prompted for OTP if 2FA is enabled)
npm publish --access public
# Note: --provenance is unavailable outside GitHub Actions
```

Manual publishes do **not** include provenance attestation. Prefer the automated
flow whenever possible.

---

## Binding Drift Check

The `check-binding-drift` CI job (PRs only) regenerates the TypeScript bindings
from the WASM artifact and diffs them against what is committed in the PR. If
they differ, the check fails with a message like:

```
Error: Generated bindings differ from committed state.
Re-run `bash scripts/generate-bindings.sh` and commit the result.
```

This prevents accidental stale bindings from being merged. If you see this
failure, run:

```bash
bash scripts/generate-bindings.sh
git add bindings/src/generated/
git commit -m "chore: regenerate bindings"
```

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Publish job skipped | Commit message does not start with `chore: release` | Amend or add a new commit with the correct prefix |
| `403 Forbidden` from npm | `NPM_TOKEN` secret is missing or expired | Regenerate token and update the GitHub secret |
| `402 Payment Required` | Package is private (scoped default) | Ensure `--access public` is in the publish command (it already is in CI) |
| Binding drift check fails on PR | Bindings not regenerated after contract change | Run `bash scripts/generate-bindings.sh` and commit |
| `tsc --noEmit` fails | Type errors in generated or hand-authored TS | Fix type errors before pushing |
