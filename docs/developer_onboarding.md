# Developer Onboarding (< 30 minutes)

This guide gets a new contributor from a fresh clone to a passing local test
run and a first local contract invocation in under 30 minutes. It is the
canonical onboarding path referenced by issue #836.

If you only read one section, read [Invariants & security](#invariants--security)
and [Edge cases & failure modes](#edge-cases--failure-modes) before touching any
money-path or mainnet-affecting code.

## Prerequisites

- Rust toolchain (stable) with the `wasm32-unknown-unknown` target.
- `stellar` / Soroban CLI (see the repo `README.md` for the pinned version).
- `make` and a POSIX shell.
- Docker (optional) if you use the localnet container targets.

## 1. Clone

```sh
git clone <repo-url> mux-contracts
cd mux-contracts
```

## 2. Environment setup

Copy the localnet example env file and fill in only local values. Never commit
real secrets.

```sh
cp .env.localnet.example .env.localnet
```

`.env.localnet` is git-ignored. It must contain **localnet-only** values
(local RPC/Horizon endpoints, local test keys). Do not paste mainnet keys,
JWTs, or webhook secrets into it.

## 3. Build

```sh
make build
```

This compiles the workspace and produces the contract WASM artifacts used by
the test and invoke targets below.

## 4. Test

```sh
make test
```

Run this before opening a PR. CI runs the same suite; keep it green.

## 5. Local invoke path

With localnet running and `.env.localnet` sourced, use the existing Makefile
invoke targets and the bindings examples to exercise an entrypoint end to end:

```sh
make localnet-up        # start local RPC/Horizon (if using the container targets)
make invoke             # invoke the example entrypoint via the generated bindings
make localnet-down      # tear down when finished
```

Use the bindings examples under the repo's examples/bindings directory as the
template for calling typed entrypoints. Entrypoints return stable error codes;
surface them verbatim in your client rather than string-matching messages.

## Invariants & security

These hold for every change and are enforced by tests and review:

- **Server/contract is the source of truth** for spends, recovery, and admin
  actions. Clients may request; they never decide. Never trust client-supplied
  balances, roles, or signatures without on-chain/on-server verification.
- **Deny-by-default** for every new privileged surface. New entrypoints that
  can move funds, change roles, or alter recovery must be explicitly authorized
  (owner / delegate / guardian / API-key / JWT as applicable) and must fail
  closed when authorization cannot be established.
- **No secrets in the repo or logs.** Redact keys, JWTs, and webhook secrets.
  Log correlation ids and stable error codes, never raw key material or tokens.
- **Fail closed on writes.** If a dependency (RPC, DB, Horizon) is unavailable,
  reject the write rather than proceeding with stale or partial state.
- **Idempotency.** Money-path and externally-triggered entrypoints must be safe
  against replayed or concurrent requests (idempotency keys / nonces).

## Edge cases & failure modes

- **Testnet vs mainnet misconfig.** Confirm the network passphrase and RPC/
  Horizon endpoints in your env match the network you intend. A mainnet
  passphrase with testnet endpoints (or vice versa) must be treated as a hard
  error, not a warning.
- **Dependency outage (RPC / DB / Horizon).** Reads may degrade; writes must
  fail closed. Do not retry a write blindly without an idempotency key.
- **Auth expiry / wrong role / revoked delegate.** Expired credentials, an
  insufficient role, or a revoked delegate must be rejected with a stable error
  code. Never fall back to a more permissive path.
- **Replayed / concurrent requests.** Assume at-least-once delivery. Use
  idempotency keys and reject duplicates deterministically.
- **Adversarial input.** Bound batch sizes, reject oversized payloads, and
  validate/spoof-check webhooks before acting on them.

## Before you open a PR

- `make build` and `make test` pass locally.
- New privileged surfaces are deny-by-default and covered by auth-negative
  tests.
- No secrets, keys, JWTs, or webhook secrets in code, logs, or fixtures.
- Money-path or mainnet-affecting changes are behind a feature flag / kill
  switch, with a rollback note in the PR description.
- Docs and runbooks are updated; contradictory copy is removed.

## Where to go next

- `SECURITY.md` — disclosure policy and security expectations.
- `docs/aa-milestone-roadmap.md` — AA milestone exit criteria.
- `README.md` — build, test, and toolchain details.
