# Security Policy

 feat/771-upgrade-auth-requirements
This document outlines the security policies, procedures, and guidelines for the Mux Protocol contracts repository.

---

## Reporting Vulnerabilities

**Do NOT open a public issue for security vulnerabilities.**

To report a security vulnerability:

1. Send an email to: security@mux-protocol.com
2. Include detailed information about the vulnerability
3. Provide proof-of-concept if possible
4. Allow us 90 days to address the vulnerability before public disclosure

Our security team will acknowledge receipt within 48 hours and provide regular updates on the remediation timeline.

---

## Supported Versions

| Version | Supported | Security Updates |
|---------|-----------|------------------|
| Latest main branch | ✅ Yes | Yes |
| Release tags (last 6 months) | ✅ Yes | Critical fixes only |
| Older releases | ❌ No | No |

---

## Security Principles

### 1. Fail-Closed by Default

All operations that could affect user funds or contract state must fail-closed:

- If authorization cannot be verified, deny the operation
- If logging fails, block the operation
- If RPC/DB/Horizon is unavailable, block writes
- If feature flags are ambiguous, default to disabled

### 2. Defense in Depth

- Multiple layers of authorization (delegate + guardian + multisig)
- Contract-level and infrastructure-level validation
- Audit logging at every layer
- Rate limiting on all external entrypoints

### 3. Principle of Least Privilege

- Deployer keys have no admin role post-deployment
- Delegates have time-limited, scope-limited permissions
- API keys are scoped to specific operations
- No hardcoded secrets in code or logs

### 4. Auditability

- All privileged operations are logged with correlation IDs
- Logs are immutable and tamper-evident
- Secrets are redacted before logging
- Logs are retained for 90 days minimum

---

## Rollback Security

Rollback operations are critical security events. See [ops/rollback-log.md](ops/rollback-log.md) for detailed procedures.

### Rollback Authorization Requirements

| Network | Authz Method | Quorum | Time Window |
|---------|--------------|--------|-------------|
| Mainnet | Multisig | 3/5 | During incident only |
| Testnet | Delegate | 1/1 | Any time |
| Localnet | None | N/A | Any time |

### Rollback Security Checklist

Before executing any rollback:

- [ ] Correlation ID generated and logged
- [ ] Authorization verified (multisig/delegate/guardian)
- [ ] No conflicting rollback in progress
- [ ] Secrets redacted from all logs
- [ ] Rollback strategy documented
- [ ] Incident channel notified
- [ ] Feature flags checked (if applicable)
- [ ] Post-rollback verification plan ready

### Rollback Fail-Closed Conditions

Rollback operations MUST be blocked if:

- Authorization verification fails
- Logging system is unavailable
- RPC endpoint is unreachable
- Conflicting rollback is in progress
- Feature flag is disabled (for money-path changes)
- WASM hash verification fails

---

## Secret Management

### Prohibited Practices

❌ **Never:**
- Commit secrets to git
- Log secrets in plaintext
- Share secrets via email/chat
- Hardcode secrets in code
- Store secrets in environment files committed to repo

### Required Practices

✅ **Always:**
- Use secrets manager for production secrets
- Rotate secrets quarterly or on compromise
- Use least-privilege IAM roles
- Redact secrets in logs (first char + `***` + last char)
- Audit secret access monthly

### Secret Redaction Example

```bash
# Correct
DEPLOYER_SECRET_KEY="S***Z"
AUTH_TOKEN="jwt***xyz"

# Incorrect
DEPLOYER_SECRET_KEY="SABCD1234...XYZ"
AUTH_TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

---

## Access Control

### Contract Roles

| Role | Permissions | Assignment |
|------|-------------|------------|
| Owner | Full control over account | Wallet owner |
| Delegate | Limited spend/approve operations | Owner-granted |
| Guardian | Recovery operations | Owner-granted |
| Admin | Contract upgrade/pause | Multisig |
| Deployer | Contract deployment only | Temporary, revoked post-deploy |

### API Key Scopes

| Scope | Operations | Risk Level |
|-------|-----------|------------|
| `read` | Read-only contract queries | Low |
| `write` | Non-critical writes | Medium |
| `rollback` | Rollback operations | Critical |
| `admin` | Admin operations | Critical |

### Authorization Flow

```
Request → API Key Check → Role Check → Delegate Check → Guardian Check → Multisig Check → Execute
```

Any check failure results in immediate denial with logged error code.

---

## Rate Limiting

### External Entrypoints

All external entrypoints (API, webhooks, RPC) must implement rate limiting:

| Entrypoint | Rate Limit | Burst | Time Window |
|------------|------------|-------|-------------|
| Contract invoke | 100 req/min | 10 | 1 minute |
| Rollback ops | 10 req/hour | 1 | 1 hour |
| Admin ops | 50 req/hour | 5 | 1 hour |
| Read queries | 1000 req/min | 100 | 1 minute |

### Rate Limit Exceeded Response

```json
{
  "error": "RateLimitExceeded",
  "message": "Too many requests. Please retry later.",
  "retry_after": 60
}
```

---

## Input Validation

### Contract-Level Validation

All contract functions must validate:

- Address format (Stellar public key)
- Amount bounds (no overflow/underflow)
- Nonce monotonicity
- Signature validity
- Permission scope

### API-Level Validation

All API endpoints must validate:

- Request size (max 1MB)
- Field types and formats
- Required fields presence
- Enum values
- String length (max 1024 chars)

### Adversarial Input Protection

- Reject oversized batches (> 100 operations)
- Reject suspicious patterns (repeated same operation)
- Rate limit per-user and per-IP
- Sanitize all log outputs
- Validate all JSON schemas

---

## Logging and Monitoring

### Required Log Fields

All privileged operations must log:

- Correlation ID (UUID)
- Timestamp (ISO8601 UTC)
- Operator identity (redacted if sensitive)
- Network (testnet/mainnet/localnet)
- Operation type
- Authorization method
- Status (started/completed/failed)
- Error code (if failed)

### Security Events to Alert

- Failed authorization attempts (> 5/min)
- Rollback operations (any)
- Admin operations (any)
- Contract upgrades (any)
- Unusual spend patterns
- RPC endpoint failures
- Log write failures

### Log Security

- Logs stored in append-only storage if possible
- Log files: `chmod 640`
- Log directory: `chmod 750`
- Daily checksum verification
- Monthly access audit

---

## Network Security

### RPC Endpoints

| Network | RPC URL | TLS | Auth |
|---------|---------|-----|------|
| Mainnet | https://rpc-mainnet.stellar.org | ✅ Yes | None |
| Testnet | https://soroban-testnet.stellar.org | ✅ Yes | None |
| Localnet | http://localhost:8000 | ❌ No | None |

### RPC Security Requirements

- Always use HTTPS for mainnet/testnet
- Validate RPC responses
- Implement RPC timeout (30s)
- Cache RPC responses where safe
- Monitor RPC endpoint health

### Network Isolation

- Mainnet operations require explicit confirmation
- Testnet and mainnet configs are separate
- No cross-network state sharing
- Network-specific feature flags

---

## Feature Flags

### Money-Path Changes

Any change that could affect user funds must be behind a feature flag:

```bash
# Example feature flag check
if [[ "$ENABLE_MONEY_PATH_CHANGE" != "true" ]]; then
  log_error "Money-path change feature flag is disabled"
  exit 1
fi
```

### Required Feature Flags

| Flag | Description | Default |
|------|-------------|---------|
| `ENABLE_ROLLBACK` | Allow rollback operations | `false` (mainnet) |
| `ENABLE_CONTRACT_UPGRADE` | Allow contract upgrades | `false` (mainnet) |
| `ENABLE_NEW_BATCH_SIZE` | Allow increased batch size | `false` |
| `ENABLE_NEW_POLICY` | Enable new spending policy | `false` |

### Feature Flag Rollout

1. Deploy with flag = `false`
2. Monitor for 24 hours
3. Enable flag on testnet
4. Run integration tests
5. Enable flag on mainnet with monitoring
6. Disable flag if issues detected

---

## Testing Security

### Required Test Coverage

- Unit tests for all authorization logic
- Integration tests for rollback flows
- E2E tests for critical paths
- Fuzz testing for input validation
- Penetration testing before mainnet deploy

### Security Test Checklist

- [ ] Authz denial tested for all roles
- [ ] Secret redaction verified in logs
- [ ] Rate limiting tested and verified
- [ ] Input validation edge cases covered
- [ ] Replay protection tested
- [ ] Fail-closed behavior verified
- [ ] Feature flag behavior tested

---

## Incident Response

### Severity Levels

| Severity | Description | Response Time |
|----------|-------------|---------------|
| Critical | User funds at risk | 15 minutes |
| High | Service unavailable | 1 hour |
| Medium | Degraded performance | 4 hours |
| Low | Minor issue | 24 hours |

### Incident Response Steps

1. **Detection**: Alert received
2. **Assessment**: Determine severity and impact
3. **Containment**: If critical, pause affected services
4. **Eradication**: Apply fix or rollback
5. **Recovery**: Restore normal operations
6. **Post-Mortem**: Document and learn

### Rollback During Incident

If a critical issue is discovered post-deploy:

1. Pause user-facing access immediately
2. Assess if rollback is needed
3. Follow [ops/rollback-log.md](ops/rollback-log.md) procedures
4. Document in incident report
5. Open follow-up issue for root cause fix

---

## Dependency Security

### Dependency Management

- Pin all dependency versions in `Cargo.lock`
- Review dependencies for known CVEs (`cargo deny check`)
- Update dependencies monthly
- Audit dependencies before mainnet deploy

### Supply Chain Security

- Verify WASM hashes before deploy
- Use reproducible builds
- Sign release artifacts
- Verify signatures before use

---

## Compliance and Audits

### Audit Requirements

- External audit before mainnet launch
- Annual security audit thereafter
- Audit findings tracked and remediated
- Audit report available on request

### Regulatory Considerations

- Data retention: 90 days minimum for logs
- Data privacy: Redact PII from logs
- Incident reporting: Within 72 hours for critical issues
- Access control: Role-based, audited

---

## Related Documents

- [Rollback Log Discipline](ops/rollback-log.md) — Rollback operations and logging
- [Rollback Deploy Notes](docs/rollback-deploy.md) — Rollback strategies
- [Threat Model](docs/threat-model.md) — Security threat analysis
- [Access Control Checklist](docs/access-control-checklist.md) — Pre-deployment security checklist
- [Audit Prep](docs/audit-prep.md) — Pre-audit requirements

---

## Contact

- **Security Team**: security@mux-protocol.com
- **GitHub Security**: https://github.com/mux-labs/mux-contracts/security/advisories
- **Incident Channel**: #incidents on Slack (internal)

---

## License

This security policy is part of the Mux Protocol project and is licensed under the MIT License.

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

### Verified Security Contacts

The following private contacts are the canonical, verified reporting channels
for this repository:

- **Email:** [security@mux-protocol.xyz](mailto:security@mux-protocol.xyz)
- **GitHub Security Advisory:** [Report a private vulnerability](https://github.com/mux-labs/mux-contracts/security/advisories/new)

Do not send secrets, private keys, or exploit material through public issues,
pull requests, or chat channels.

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

## Architecture Overview

The current crate set, inter-crate relationships, and the invariants that all
crates must uphold (contract as source of truth for spends/recovery/admin,
deny-by-default for privileged surfaces, fail-closed on RPC/DB/Horizon outages
for writes) are documented in [docs/architecture-overview.md](docs/architecture-overview.md).
That document is the authoritative reference for the crate set and the
authorization model (owner/delegate/guardian/API-key/JWT); report issues that
violate its invariants against it.

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

## Somzilla Status

`Somzilla.md` is a historical status document and is **not** a canonical source of
truth. Its content has been reconciled with the current repo state and archived; it
now points to the canonical documentation below. Do not rely on `Somzilla.md` for
security assumptions, invariants, or exit criteria.

Canonical references:

- [README.md](README.md) — project overview, build/test instructions, and contributor entry points
- [CONTRIBUTING.md](CONTRIBUTING.md) — contribution workflow and review expectations
- [CONTRACT_IDS.md](CONTRACT_IDS.md) — deployed contract ids per network
- [docs/threat-model.md](docs/threat-model.md) — threat model and trust boundaries
- [docs/aa-milestone-roadmap.md](docs/aa-milestone-roadmap.md) — AA invariants and exit criteria
- [docs/rollback-guide.md](docs/rollback-guide.md) — rollback invariants, error codes, and kill-switch strategy

## Audit History

See [docs/audit-prep.md](docs/audit-prep.md) for audit preparation notes and the [docs/access-control-checklist.md](docs/access-control-checklist.md) for the access control review checklist.

## Security Contact

- **Email**: security@mux-protocol.xyz
- **GitHub**: [Security Advisories](https://github.com/mux-labs/mux-contracts/security/advisories)
- **security.txt**: [.well-known/security.txt](.well-known/security.txt)
 main
