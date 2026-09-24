# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| latest mainnet deployment | Yes |
| previous mainnet deployment | Security fixes only |
| older | No |

## Reporting a Vulnerability

**Do not open public issues or pull requests for security vulnerabilities.**

Instead, report vulnerabilities through one of these channels:

1. **GitHub Security Advisory** (preferred): Go to the [Security tab](https://github.com/mux-labs/mux-contracts/security/advisories/new) and click "Report a vulnerability".
2. **Email**: Send a description to **security@mux-protocol.xyz**

### What to Include

- Description of the vulnerability and its impact
- Steps to reproduce or a proof-of-concept
- Suggested fix (if any)
- Your contact information for follow-up

### Response Timeline & Service Level Agreement (SLA)

We commit to the following Service Level Agreement (SLA) targets for all reports submitted via our private reporting channels:

| Stage | Target SLA | Description |
|-------|------------|-------------|
| **Initial Acknowledgment** | **48 hours** | Initial response confirming receipt of report and assigning a triage coordinator. |
| **Triage & Severity Assessment** | **5 business days** | Confirmation of reproducibility, impact classification, and severity assignment. |
| **Fix for Critical / High Severity** | **14 business days** | Patch developed, tested in isolation, audited, and scheduled for deployment. |
| **Fix for Medium / Low Severity** | **30 business days** | Remediation included in the next scheduled release cycle. |
| **Coordinated Public Disclosure** | **30 business days after fix** | Public advisory published in collaboration with reporter after mainnet deployment. |

#### Severity Classification
- **Critical:** Direct unauthorized theft of funds, account takeover, or complete denial of service across all accounts.
- **High:** Temporary lock of user funds, unauthorized permission escalation, or storage griefing compromising contract availability.
- **Medium:** Partial policy bypass without direct fund loss, non-critical gas griefing, or state inconsistencies.
- **Low:** Minor logic edge cases, client-side binding discrepancies, or documentation ambiguities affecting security assumptions.

This SLA is mirrored in our RFC 9116 security declaration at [`.well-known/security.txt`](.well-known/security.txt). We work closely with researchers throughout the triage and disclosure process.


## Scope

The following are in scope for security reports:

- **Soroban smart contracts** under `contracts/` — logic bugs, authorization bypasses, storage griefing, overflow/underflow, reentrancy
- **TypeScript bindings** under `bindings/` — error handling flaws that could cause loss of funds or unauthorized actions
- **Deployment scripts** under `scripts/` — key management, access control during deployment
- **Configuration** — `config/addresses.json` exposure, network passphrase misconfiguration

The following are **out of scope**:

- Soroban runtime or Stellar network consensus issues (report to [Stellar](https://stellar.org/bug-bounty))
- Denial-of-service against infrastructure (RPC nodes,Horizon servers) unrelated to contract logic
- Social engineering attacks

## Safe Harbor

We support safe harbor for security researchers who:

- Make a good-faith effort to avoid privacy violations, data destruction, or service disruption
- Only interact with accounts you own or have explicit permission to test
- Report vulnerabilities promptly and do not publicly disclose details before a fix is deployed

We will not pursue legal action against researchers who follow these guidelines.

## Threat Model

See [docs/threat-model.md](docs/threat-model.md) for the current threat model, trust boundaries, and known mitigations.

## Partial Crate Rollback

Partial rollback is a privileged, money-path-adjacent surface. It is authorized
server-side (owner/delegate/guardian/API-key/JWT), deny-by-default, idempotent via
correlation ids, and fails closed on RPC/DB/Horizon outages. Rollback logs redact
keys, JWTs, and webhook secrets. See [docs/rollback-guide.md](docs/rollback-guide.md)
for invariants, stable error codes, and the flag/kill-switch strategy.

## Account Abstraction (AA)

The AA roadmap invariants, authorization model (owner/delegate/guardian/API-key/JWT),
stable error codes, idempotency, and fail-closed requirements are defined in
[docs/aa-milestone-roadmap.md](docs/aa-milestone-roadmap.md). That document is the
authoritative exit-criteria reference for AA work; report AA-related issues against
its invariants.

## Audit History

See [docs/audit-prep.md](docs/audit-prep.md) for audit preparation notes and the [docs/access-control-checklist.md](docs/access-control-checklist.md) for the access control review checklist.

## Security Contact

- **Email**: security@mux-protocol.xyz
- **GitHub**: [Security Advisories](https://github.com/mux-labs/mux-contracts/security/advisories)
- **security.txt**: [.well-known/security.txt](.well-known/security.txt)
