# npm Publish Flow — @mux-protocol/contracts

This document describes the process for releasing a new version of the
`@mux-protocol/contracts` TypeScript package to npm.

---

## Current state: automated publish pipeline

**The publish pipeline is now automated via `.github/workflows/bindings.yml`.** This workflow:
1. Regenerates TypeScript bindings from the built WASM on every push to `main` when contract or binding files change
2. Fails fast if generated bindings differ from the committed state (binding drift detection)
3. Runs lint, type check, and tests
4. Publishes to npm automatically with SLSA provenance attestation

The pipeline requires an `NPM_TOKEN` secret configured in GitHub repo settings with publish access to the `@mux-protocol` scope.

Manual release is no longer required — simply merge a version bump to `main` and the publish will trigger automatically.

---

## Pipeline prerequisites

An `NPM_TOKEN` secret must be configured in this repository's GitHub settings
with publish access to the `@mux-protocol` scope:

1. In your npm account, generate a new automation token with publish access to `@mux-protocol`
2. In the GitHub repository settings, add the secret as `NPM_TOKEN`

The token is used **only** by the publish job in `.github/workflows/bindings.yml`.

---

## Release process

To release a new version:

### 1. Bump the version

`bindings/package.json`'s `version` must equal the root Cargo workspace
`[workspace.package].version` — this is enforced by
`bindings/__tests__/version-sync.test.ts` (run via `npm test`, calling
`scripts/sync-versions.sh --check`). Bump both together, following
[semver](https://semver.org/) — see
[`docs/BREAKING_CHANGES.md`](BREAKING_CHANGES.md) for what counts as a
MAJOR change, including the error-enum-specific rules there.

```bash
# Update the workspace version in the root Cargo.toml, then:
bash scripts/sync-versions.sh
```

### 2. Verify locally

```bash
cd bindings
npm ci
npm run lint
npx tsc --noEmit
npm test
npm run build
```

All of these must pass — they mirror the pipeline's lint/test job.

### 3. Commit and merge

```bash
git add Cargo.toml bindings/package.json bindings/src/generated/
git commit -m "chore: release v<new-version>"
git push origin main
```

The pipeline will automatically regenerate bindings, verify no drift, run tests,
and publish to npm.

### 4. Verify the release

```bash
npm view @mux-protocol/contracts version
npm install @mux-protocol/contracts@<new-version>
```

### 5. Tag the release

```bash
git tag v<new-version>
git push origin v<new-version>
```

`scripts/check-changelog-release-artifacts.sh` (run in CI) checks that
tagged releases have WASM hashes and a matching binding version recorded in
`CHANGELOG.md` — update `CHANGELOG.md` before or alongside tagging.

---

## npm Provenance Attestation

Published versions include a [SLSA provenance attestation](https://slsa.dev/)
generated during the automated publish workflow. This provides cryptographic
proof of the package's origin and build process.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `403 Forbidden` from npm | Not logged in, or account lacks publish rights to `@mux-protocol` | `npm login`; confirm scope access on npmjs.com |
| `402 Payment Required` | Package is private (scoped default) | Pass `--access public` |
| `version-sync.test.ts` fails | `bindings/package.json` version doesn't match Cargo workspace version | Run `bash scripts/sync-versions.sh` |
| Bindings differ from WASM after `generate-bindings.sh` | Contract changed since bindings were last regenerated | Commit the regenerated files under `bindings/src/generated/` |
| `tsc --noEmit` fails | Type errors in generated or hand-authored TS | Fix type errors before publishing |
| OTP prompt during `npm publish` | 2FA enabled on your npm account | Enter the OTP from your authenticator |
