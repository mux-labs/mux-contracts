# Relayer Integration

This document defines the production-grade relayer integration for Mux Protocol's account abstraction layer. Relayers enable gasless transactions by sponsoring network fees and executing transactions on behalf of users.

---

## Overview

Relayers are trusted entities that:
- **Sponsor gas fees** — Pay network fees on behalf of users
- **Execute transactions** — Submit signed intents to the blockchain
- **Validate signatures** — Verify user session key signatures before execution
- **Batch operations** — Combine multiple transactions for efficiency
- **Enforce policies** — Respect spend limits and authorization rules

The relayer integration ensures fail-closed security, proper authorization, and observability for all relayer operations.

---

## Architecture

### Relayer Flow

```
┌─────────────┐
│   Client    │  1. Submit signed intent
│  (Browser)  │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Relayer Service                               │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  1. Validate API key / JWT                              │  │
│  │  2. Rate limit check                                     │  │
│  │  3. Parse intent (account_id, calls, signature, nonce)   │  │
│  │  4. Verify session key signature                         │  │
│  │  5. Check spend limits                                   │  │
│  │  6. Estimate gas                                         │  │
│  │  7. Build Soroban transaction                            │  │
│  │  8. Sign with relayer funding key                        │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Mux Batcher (Optional)                         │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  1. Batch multiple transactions                          │  │
│  │  2. Optimize gas usage                                    │  │
│  │  3. Submit to network                                     │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Mux Account Contract                           │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  1. Verify relayer authorization                         │  │
│  │  2. Verify session key signature                         │  │
│  │  3. Check spend limits                                   │  │
│  │  4. Execute calls                                        │  │
│  │  5. Emit audit events                                     │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Authorization Layers

### Layer 1: API-Level Authorization

Relayer API endpoints require authentication:

| Method | Auth Type | Required Headers | Scope |
|--------|-----------|------------------|-------|
| `POST /v1/transactions/intent` | API Key or JWT | `Authorization: Bearer <token>` | `relayer:submit` |
| `GET /v1/transactions/:id` | API Key or JWT | `Authorization: Bearer <token>` | `relayer:read` |
| `POST /v1/transactions/batch` | API Key or JWT | `Authorization: Bearer <token>` | `relayer:batch` |

**API Key Format**:
```
mux_relayer_<version>_<key_id>_<signature>
```

**JWT Format**:
- Issuer: Mux Relayer Service
- Subject: Relayer ID
- Claims: `scope`, `exp`, `iat`
- Signed with relayer service private key

### Layer 2: Rate Limiting

Relayer operations are rate-limited to prevent abuse:

| Network | Rate Limit | Burst | Time Window |
|---------|------------|-------|-------------|
| Mainnet | 100 requests per minute | 10 | 1 minute |
| Testnet | 1000 requests per minute | 50 | 1 minute |
| Localnet | No limit | N/A | N/A |

Rate limiting is enforced per API key and per relayer ID.

### Layer 3: Contract-Level Authorization

The `mux-account` contract must authorize the relayer to execute transactions:

```rust
// Relayer must be registered as a delegate
DataKey::Delegates(relayer_address) -> DelegateInfo {
    address: relayer_address,
    expiry_ledger: <never expires>,
    can_spend: true,  // Can execute on behalf of users
}
```

### Layer 4: Session Key Authorization

User intents must be signed with a valid session key:

```rust
// Session key must be valid
DataKey::SessionKey(owner, session_key) -> SessionKeyRecord {
    expires_at: <future timestamp>,
    scopes: [Symbol::short("pay"), Symbol::short("transfer")],
    revoked: false,
}
```

### Layer 5: Feature-Flag Authorization

Relayer operations require the `ENABLE_RELAYER` feature flag:

```bash
# Fail-closed: relayer disabled by default on mainnet
ENABLE_RELAYER=true
```

---

## API Specification

### POST /v1/transactions/intent

Submit a single transaction intent for relaying.

**Request**:
```json
{
  "account_id": "CABCD...",
  "calls": [
    {
      "contract": "CXYZ...",
      "function": "pay",
      "args": ["amount", "recipient"]
    }
  ],
  "signature": "a1b2c3...",
  "nonce": 42,
  "session_key": "CDEF..."
}
```

**Response**:
```json
{
  "transaction_id": "tx_abc123",
  "status": "submitted",
  "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
  "estimated_gas": 150000,
  "fee_estimate": "0.01 XLM"
}
```

**Error Codes**:
- `RELAYER_AUTHZ_DENIED` (401) — API key/JWT invalid or expired
- `RELAYER_RATE_LIMITED` (429) — Rate limit exceeded
- `RELAYER_INVALID_INTENT` (400) — Invalid intent format
- `RELAYER_INVALID_SIGNATURE` (400) — Session key signature invalid
- `RELAYER_SESSION_EXPIRED` (400) — Session key expired
- `RELAYER_SESSION_REVOKED` (400) — Session key revoked
- `RELAYER_SPEND_LIMIT_EXCEEDED` (400) — Spend limit exceeded
- `RELAYER_NONCE_MISMATCH` (400) — Nonce mismatch
- `RELAYER_ACCOUNT_LOCKED` (400) — Account locked
- `RELAYER_DEPENDENCY_UNAVAILABLE` (503) — RPC/DB/Horizon unavailable

### POST /v1/transactions/batch

Submit multiple transaction intents for batched execution.

**Request**:
```json
{
  "intents": [
    {
      "account_id": "CABCD...",
      "calls": [...],
      "signature": "a1b2c3...",
      "nonce": 42,
      "session_key": "CDEF..."
    },
    {
      "account_id": "CXYZ...",
      "calls": [...],
      "signature": "d4e5f6...",
      "nonce": 43,
      "session_key": "CGHI..."
    }
  ]
}
```

**Response**:
```json
{
  "batch_id": "batch_xyz789",
  "status": "submitted",
  "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
  "transaction_ids": ["tx_abc123", "tx_def456"],
  "estimated_gas": 300000,
  "fee_estimate": "0.02 XLM"
}
```

### GET /v1/transactions/:id

Get the status of a submitted transaction.

**Response**:
```json
{
  "transaction_id": "tx_abc123",
  "status": "success",
  "ledger": 12345,
  "result": "..."
}
```

---

## Required Log Fields

Every relayer operation MUST log the following fields:

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `correlation_id` | UUID | Unique identifier for the operation | `550e8400-e29b-41d4-a716-446655440000` |
| `timestamp` | ISO8601 | UTC timestamp of the operation | `2024-01-15T10:30:00Z` |
| `relayer_id` | string | Relayer identity (redacted if sensitive) | `relayer-123` or `RAB...XYZ` |
| `network` | enum | Target network (testnet/mainnet/localnet) | `mainnet` |
| `account_id` | string | Account contract being executed | `CABCD...` |
| `operation_type` | enum | Operation type (intent/batch/read) | `intent` |
| `authz_method` | enum | Authorization method (api_key/jwt) | `api_key` |
| `authz_id` | string | Authorization identifier (redacted) | `key-456` |
| `status` | enum | Operation status (started/completed/failed) | `completed` |
| `error_code` | string (optional) | Error code if failed | `RELAYER_AUTHZ_DENIED` |
| `transaction_id` | string (optional) | Transaction ID if submitted | `tx_abc123` |
| `estimated_gas` | number (optional) | Estimated gas units | `150000` |
| `git_commit` | string | Git commit SHA of relayer service | `abc123def456` |

---

## Pre-Relay Checklist

Before executing any relayer operation:

- [ ] **Authorization verified**: API key/JWT valid and not expired
- [ ] **Rate limit check passed**: Not exceeding rate limits
- [ ] **Feature flag enabled**: `ENABLE_RELAYER=true` set
- [ ] **Relayer registered**: Relayer address registered as delegate
- [ ] **Session key valid**: Session key exists, not expired, not revoked
- [ ] **Signature verified**: User signature over intent is valid
- [ ] **Spend limits checked**: Within allowed spend limits
- [ ] **Nonce valid**: Nonce matches expected value
- [ ] **Account not locked**: Account is not in locked state
- [ ] **RPC available**: Soroban RPC endpoint is reachable
- [ ] **Correlation ID generated**: Unique ID for logging and tracking
- [ ] **Gas estimated**: Gas cost estimated and within limits

---

## Error Codes

| Error Code | Description | Severity | Action |
|------------|-------------|----------|--------|
| `RELAYER_AUTHZ_DENIED` | Authorization denied (API key/JWT invalid) | Critical | Do not proceed |
| `RELAYER_RATE_LIMITED` | Rate limit exceeded | High | Retry after backoff |
| `RELAYER_INVALID_INTENT` | Invalid intent format | High | Validate and retry |
| `RELAYER_INVALID_SIGNATURE` | Session key signature invalid | High | Verify signature |
| `RELAYER_SESSION_EXPIRED` | Session key expired | High | User must refresh session |
| `RELAYER_SESSION_REVOKED` | Session key revoked | High | User must create new session |
| `RELAYER_SPEND_LIMIT_EXCEEDED` | Spend limit exceeded | High | User must increase limit |
| `RELAYER_NONCE_MISMATCH` | Nonce mismatch | High | Fetch latest nonce and retry |
| `RELAYER_ACCOUNT_LOCKED` | Account locked | High | User must unlock account |
| `RELAYER_DEPENDENCY_UNAVAILABLE` | RPC/DB/Horizon unavailable | High | Retry later |
| `RELAYER_BATCH_TOO_LARGE` | Batch exceeds maximum size | High | Reduce batch size |
| `RELAYER_GAS_ESTIMATION_FAILED` | Gas estimation failed | High | Retry with different parameters |
| `RELAYER_TRANSACTION_FAILED` | Transaction execution failed | High | Investigate and retry |

---

## Idempotency and Replay Protection

### Replay Detection

```bash
# Check if this intent was already executed
is_intent_replay() {
  local correlation_id="$1"
  local account_id="$2"
  local nonce="$3"
  local log_file="ops/logs/relayer-*.log"

  if grep -q "correlation_id=${correlation_id}.*account_id=${account_id}.*nonce=${nonce}.*status=completed" "$log_file"; then
    return 0  # Already completed
  fi
  return 1  # Not a replay
}

# Before executing intent
if is_intent_replay "$CORRELATION_ID" "$ACCOUNT_ID" "$NONCE"; then
  log_warn "Intent $CORRELATION_ID for account $ACCOUNT_ID with nonce $NONCE already completed - skipping"
  exit 0
fi
```

### Idempotent Operations

Relayer operations must be idempotent where possible:

- **Intent submission**: Check if transaction already submitted before submitting
- **Nonce handling**: Use on-chain nonce to prevent replay
- **Batch submission**: Check if batch already submitted before submitting

---

## Secret Redaction

**NEVER log the following in plaintext:**

- Private keys (relayer funding keys)
- API keys (full key string)
- JWT tokens
- Session key private keys
- User signatures (full signature)

**Redaction format:**

```bash
# Before logging
RELAYER_PRIVATE_KEY="SABC123...XYZ"
API_KEY="mux_relayer_v1_key123_signature456"

# After redaction (log only)
RELAYER_PRIVATE_KEY="S***"  # Show first char, redact rest
API_KEY="m***"  # Show first char, redact rest
# or
RELAYER_PRIVATE_KEY="<REDACTED>"
API_KEY="<REDACTED>"
```

---

## Rate Limiting

Relayer operations must be rate-limited:

| Network | Rate Limit | Burst | Time Window |
|---------|------------|-------|-------------|
| Mainnet | 100 requests per minute | 10 | 1 minute |
| Testnet | 1000 requests per minute | 50 | 1 minute |
| Localnet | No limit | N/A | N/A |

Rate limiting is enforced at the API level and logged.

---

## Metrics and Observability

### Required Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `relayer_operations_total` | counter | operation_type, network, status | Total relayer operations |
| `relayer_duration_seconds` | histogram | operation_type, network | Operation duration |
| `relayer_authz_failures_total` | counter | authz_method, network | Authorization failures |
| `relayer_rate_limit_exceeded_total` | counter | network | Rate limit exceeded |
| `relayer_gas_used_total` | counter | network, operation_type | Total gas used |
| `relayer_batch_size` | histogram | network | Batch size distribution |
| `relayer_active_operations` | gauge | network | Currently active operations |

### Metric Logging Example

```bash
log_metric() {
  local metric_name="$1"
  local metric_value="$2"
  local labels="$3"
  echo "[METRIC] ${metric_name}=${metric_value} ${labels}" >> "$RELAYER_LOG_FILE"
}

# After intent submission
log_metric "relayer_duration_seconds" "0.5" \
  "operation_type=intent network=mainnet"
log_metric "relayer_operations_total" "1" \
  "operation_type=intent network=mainnet status=success"
log_metric "relayer_gas_used_total" "150000" \
  "network=mainnet operation_type=intent"
```

---

## Testing Requirements

### Unit Tests

Every relayer function must have unit tests for:

- **Authorization**: API key/JWT validation
- **Rate limiting**: Rate limit enforcement
- **Intent validation**: Intent format validation
- **Signature verification**: Session key signature verification
- **Spend limit checking**: Spend limit enforcement
- **Nonce handling**: Nonce validation and increment
- **Error handling**: All error codes are tested
- **Idempotency**: Replay detection and idempotent operations

### Integration Tests

Integration tests must verify:

- **Full intent flow**: Submit intent, verify, execute, confirm
- **Batch flow**: Submit batch, verify, execute, confirm
- **Authz enforcement**: Unauthorized relayer attempts fail
- **Rate limiting**: Excessive requests are blocked
- **Conflict detection**: Concurrent operations are handled
- **RPC failure**: RPC unavailability is handled gracefully

### E2E Tests

E2E tests must verify:

- **Testnet relaying**: Full relaying on testnet with real RPC
- **Smoke tests**: Post-relay functionality works end-to-end
- **Monitoring**: Metrics are emitted correctly
- **Logging**: Logs are written with correlation IDs

---

## Security Considerations

### Fail-Closed Behavior

- Relayer disabled by default on mainnet (feature flag)
- Authorization failure blocks operation
- Logging failure blocks operation
- Invalid signature blocks operation
- RPC unavailability blocks operation

### Secret Protection

- Secrets redacted before logging
- No secrets in git history
- No secrets in environment files
- Log files have restricted permissions (640)

### Authorization

- API key or JWT required for all operations
- Rate limiting enforced per API key
- Relayer must be registered as delegate
- Session key must be valid and not revoked
- Spend limits enforced at contract level

### Dependency Outage

- RPC unavailability blocks writes
- DB unavailability blocks writes
- Horizon unavailability blocks writes
- Fail-closed on all dependencies

### Adversarial Input

- Intent size limited (max 1MB)
- Batch size limited (max 100 intents)
- Signature validation prevents spoofing
- Nonce validation prevents replay
- Gas estimation prevents griefing

---

## Related Documents

- [Account Abstraction Design](account-abstraction.md) — Account contract architecture
- [AA Backend Orchestrator](aa-backend-orchestrator.md) — Backend orchestrator integration
- [Account Upgrade Migration Path](account-upgrade-migration.md) — Storage migration procedures
- [Contract Upgrade Pattern](contract-upgrade-pattern.md) — Technical upgrade implementation
- [Upgrade Auth Requirements](upgrade-auth-requirements.md) — Authorization requirements for upgrades
- [Rollback Deploy Notes](rollback-deploy.md) — Rollback strategies and procedures
- [Rollback Log Discipline](../ops/rollback-log.md) — Operational logging discipline
- [Security Policy](../SECURITY.md) — Overall security guidelines
- [scripts/verify-relayer.sh](../scripts/verify-relayer.sh) — Relayer verification script
- [scripts/relayer-submit.sh](../scripts/relayer-submit.sh) — Relayer submission script

---

## Appendix: Relayer Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    Intent Submission                            │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              Pre-Relay Verification                             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • Check feature flag (ENABLE_RELAYER)                   │   │
│  │ • Verify API key / JWT                                  │   │
│  │ • Check rate limits                                     │   │
│  │ • Validate intent format                                │   │
│  │ • Verify session key signature                          │   │
│  │ • Check spend limits                                     │   │
│  │ • Validate nonce                                         │   │
│  │ • Check account not locked                               │   │
│  │ • Generate correlation ID                               │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Log Intent Started                           │
│  • Write to ops/logs/relayer-*.log with correlation ID         │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Estimate Gas                                  │
│  • Simulate transaction execution                              │
│  • Calculate gas cost                                          │
│  • Check within limits                                          │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Build Transaction                             │
│  • Create Soroban transaction                                  │
│  • Add calls                                                   │
│  • Sign with relayer funding key                               │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
                         │
                    ┌────┴────┐
                    │ Batch?  │
                    └────┬────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
         ▼               ▼               ▼
    ┌─────────┐    ┌─────────┐    ┌──────────┐
    │ Single  │    │ Batch   │    │ Error    │
    │ Submit  │    │ Submit  │    │ Return   │
    └────┬────┘    └────┬────┘    └──────────┘
         │              │
         │              │
         └──────────────┼──────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Contract Execution                             │
│  • Verify relayer authorization                                 │
│  • Verify session key signature                                 │
│  • Check spend limits                                           │
│  • Execute calls                                                │
│  • Emit audit events                                            │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Post-Execution Verification                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • Verify transaction succeeded                            │   │
│  │ • Update nonce                                            │   │
│  │ • Update spend limits                                     │   │
│  │ • Emit metrics                                            │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Log Intent Completed                           │
│  • Write status=completed to log                                │
│  • Emit metrics                                                │
└─────────────────────────────────────────────────────────────────┘
```
