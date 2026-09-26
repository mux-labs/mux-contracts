# Security Acknowledgments

Mux Labs thanks the following researchers for responsibly disclosing
vulnerabilities in Mux Protocol contracts, following the process in
[SECURITY.md](../SECURITY.md).

This page is referenced as the `Acknowledgments` field of
[`.well-known/security.txt`](../.well-known/security.txt) per [RFC
9116](https://www.rfc-editor.org/rfc/rfc9116).

---

## Hall of Fame

_No public disclosures yet._

Once a reported vulnerability has been fixed and the researcher has agreed to
public credit, add a row here:

| Date | Researcher | Severity | Summary | Advisory |
|---|---|---|---|---|
| — | — | — | — | — |

## Inclusion Criteria

A researcher is listed here if all of the following hold:

- The report was submitted through a private channel (GitHub Security
  Advisory or `security@mux-protocol.xyz`) — see
  [SECURITY.md § Reporting a Vulnerability](../SECURITY.md#reporting-a-vulnerability).
- The report was not publicly disclosed before a fix shipped, consistent
  with [SECURITY.md § Safe Harbor](../SECURITY.md#safe-harbor).
- The researcher opted in to being credited. We never publish a
  researcher's name, handle, or report details without their explicit
  consent, and always honor a request to be listed anonymously or removed.

## Acknowledgments Process & Workflow

The lifecycle from initial vulnerability submission to public acknowledgment follows a strict, coordinated workflow:

### 1. Intake and Triage
- Reports are received via private GitHub Security Advisories or encrypted email to `security@mux-protocol.xyz`.
- Acknowledgement of receipt is issued within **24 hours**.
- The security team conducts initial triage and feasibility testing within **72 hours**.

### 2. Validation & Severity Classification
Findings are scored using CVSS v3.1 tailored for smart contract risk profiles:

| Severity | Impact on Mux Protocol | Target SLA for Patch |
|---|---|---|
| **Critical** | Direct theft of user funds, contract lockup, unauthorized admin takeover | < 24 hours |
| **High** | Circumvention of spend limits, temporary denial of service on batches | < 72 hours |
| **Medium** | Griefing attacks, storage exhaustion vectors, fee calculation manipulation | < 7 days |
| **Low** | Informational security improvements, minor event emission discrepancies | < 14 days |

### 3. Remediation & Verification
- Patch is developed in a private security fork.
- Automated regression tests are added to verify the fix fail-closed.
- The reporting researcher is invited to review the fix in the private advisory where appropriate.
- Fix is deployed to mainnet / contract upgrade executed.

### 4. Researcher Credit & Consent Request
Prior to publishing an advisory or updating the Hall of Fame, Mux Labs sends a formal acknowledgment consent request asking the researcher:
1. Preferred display name or pseudonym.
2. Link (GitHub profile, X/Twitter handle, personal website).
3. Public summary wording preference.
4. Option for anonymous attribution or full opt-out.

Researchers have a minimum of **7 days** to review the proposed advisory and their attribution text.

### 5. Publication & Hall of Fame Update
- Security advisory is published with CVE assignment if applicable.
- The Hall of Fame table in this document is updated with date, researcher name/link, severity, summary, and advisory link.
- `.well-known/security.txt` metadata is refreshed.

### 6. Ongoing Maintenance & Privacy
- **Updates**: Researchers may contact `security@mux-protocol.xyz` at any time to update their handle or linked profile.
- **Takedown**: Any researcher may request immediate removal of their name from the Hall of Fame. Requests are processed within 2 business days.

## Not Listed Here

- Reports still under triage or embargo (see the
  [Response Timeline](../SECURITY.md#response-timeline) in SECURITY.md).
- Reports for out-of-scope targets (Soroban runtime, network consensus,
  infrastructure DoS, social engineering) — see
  [SECURITY.md § Scope](../SECURITY.md#scope).
- Reports filed as public GitHub issues instead of through a private
  channel; see SECURITY.md for why this matters for smart-contract
  vulnerabilities specifically.

