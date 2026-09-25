> **Non-canonical scratch note.** This file is a historical issue-tracking note and
> does not reflect the current contract workspace. For architecture, see
> [`docs/architecture-overview.md`](docs/architecture-overview.md) and
> [`contracts/README.md`](contracts/README.md).

# Somzilla.md — status

**Status: archived (historical).** This document is retained only as a record of
past issue-tracking notes. It is **not** a source of truth for the current
contract workspace and must not be used to infer current behavior, interfaces,
or security policy.

## Canonical documentation

For up-to-date, authoritative information, use the following instead:

- Architecture: [`docs/architecture-overview.md`](docs/architecture-overview.md)
- Contracts workspace: [`contracts/README.md`](contracts/README.md)
- Project overview: [`README.md`](README.md)
- Security policy: [`SECURITY.md`](SECURITY.md)
- Contributing guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Deployed contract IDs: [`CONTRACT_IDS.md`](CONTRACT_IDS.md)

If any statement below conflicts with the canonical docs above, the canonical
docs win. Do not treat the notes below as current requirements.

## Historical notes (superseded)

Issue:#396 Define recovery request storage struct

Context
Soroban contracts should harden recovery struct. Issue 'Define recovery request storage struct' tracks a concrete improvement so Mux on-chain behavior stays auditable, bounded in storage, and easy to bind from TypeScript.

Tasks
Implement or document recovery struct in the relevant contract crate or script
Keep changes no_std-safe and aligned with existing error enums
Add or update Rust unit tests or script checks where behavior changes
Update contracts docs or bindings notes if the public interface changes
Acceptance Criteria
Recovery struct is implemented or documented as specified
cargo test and clippy remain green for touched crates
Storage growth stays bounded where collections are involved
Public entrypoints and errors stay consistent with existing patterns
