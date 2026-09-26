# Rollback Log Discipline

This document defines the operational logging discipline for rollback operations in Mux Protocol. It ensures that all rollback actions are auditable, traceable, and safe without exposing sensitive secrets.

---

## Core Principles

1. **Auditability**: Every rollback action must be logged with correlation IDs
2. **Secret Safety**: Never log raw keys, JWTs, or webhook secrets
3. **Fail-Closed**: On logging failure, block the operation
4. **Idempotency**: Logs must support replay detection
5. **Authz Enforcement**: All rollback ops require explicit authorization

---

## Required Log Fields

Every rollback operation MUST log the following fields:

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `correlation_id` | UUID | Unique identifier for the rollback operation | `550e8400-e29b-41d4-a716-446655440000` |
| `timestamp` | ISO8601 | UTC timestamp of the operation | `2024-01-15T10:30:00Z` |
| `operator` | string | Operator identity (redacted if sensitive) | `operator-123` or `GAB...XYZ` |
| `network` | enum | Target network (testnet/mainnet/localnet) | `mainnet` |
| `contract_name` | string | Contract being rolled back | `mux-account` |
| `from_contract_id` | string | Contract ID being rolled back from | `CABCD...` |
| `to_contract_id` | string | Contract ID being rolled back to | `CXYZ...` |
| `rollback_strategy` | enum | Strategy used (address-repoint/deploy-wasm/admin-pause) | `address-repoint` |
| `wasm_hash` | string (optional) | WASM hash if deploying previous version | `a1b2c3...` |
| `authz_method` | enum | Authorization method (delegate/guardian/multisig) | `multisig` |
| `authz_id` | string | Authorization identifier (redacted) | `auth-456` |
| `status` | enum | Operation status (started/completed/failed) | `completed` |
| `error_code` | string (optional) | Error code if failed | `ROLLBACK_AUTHZ_DENIED` |
| `git_commit` | string | Git commit SHA of rollback script | `abc123def456` |

---

## Secret Redaction Rules

**NEVER log the following in plaintext:**

- Private keys (deployer, admin, delegate)
- JWT tokens
- Webhook secrets
- API keys
- Mnemonic phrases
- Full Stellar secret keys

**Redaction format:**

```bash
# Before logging
DEPLOYER_SECRET_KEY="SABC123...XYZ"

# After redaction (log only)
DEPLOYER_SECRET_KEY="S***"  # Show first char, redact rest
# or
DEPLOYER_SECRET_KEY="<REDACTED>"
```

**Implementation example:**

```bash
redact_secret() {
  local secret="$1"
  if [[ -n "$secret" && ${#secret} -gt 4 ]]; then
    echo "${secret:0:1}***${secret: -1}"
  else
    echo "<REDACTED>"
  fi
}

log_safe() {
  local key="$1"
  local value="$2"
  case "$key" in
    *SECRET*|*KEY*|*TOKEN*)
      echo "$key=$(redact_secret "$value")"
      ;;
    *)
      echo "$key=$value"
      ;;
  esac
}
```

---

## Correlation ID Generation

Every rollback operation must generate a unique correlation ID before execution:

```bash
generate_correlation_id() {
  # Use UUID v4 for uniqueness
  if command -v uuidgen &>/dev/null; then
    uuidgen | tr '[:upper:]' '[:lower:]'
  elif command -v python3 &>/dev/null; then
    python3 -c "import uuid; print(str(uuid.uuid4()))"
  else
    # Fallback: timestamp + random
    echo "$(date +%s)-$RANDOM"
  fi
}

CORRELATION_ID=$(generate_correlation_id)
export CORRELATION_ID
```

The correlation ID must be:
- Generated at the start of the operation
- Included in all log entries for that operation
- Written to a persistent log file
- Included in any incident reports

---

## Authorization Requirements

### Pre-Rollback Authz Check

Before executing any rollback, verify:

```bash
check_rollback_authz() {
  local correlation_id="$1"
  local network="$2"
  local operator="$3"

  # 1. Verify operator has rollback permission
  if ! has_rollback_permission "$operator" "$network"; then
    log_rollback "$correlation_id" "failed" "ROLLBACK_AUTHZ_DENIED" \
      "operator=$operator network=$network"
    exit 1
  fi

  # 2. Verify multisig quorum if required
  if requires_multisig "$network"; then
    if ! verify_multisig_quorum "$correlation_id"; then
      log_rollback "$correlation_id" "failed" "ROLLBACK_MULTISIG_QUORUM_NOT_MET"
      exit 1
    fi
  fi

  # 3. Verify no conflicting rollback in progress
  if has_active_rollback "$network"; then
    log_rollback "$correlation_id" "failed" "ROLLBACK_CONFLICT" \
      "conflicting_operation=$(get_active_rollback_id)"
    exit 1
  fi
}
```

### Authz Methods

| Method | Description | When Required |
|--------|-------------|--------------|
| `delegate` | Delegate approval from contract owner | Testnet, non-critical |
| `guardian` | Guardian approval for recovery rollbacks | Recovery operations |
| `multisig` | Multisig quorum approval | Mainnet, critical |
| `api_key` | API key with rollback scope | Automated rollbacks |

---

## Idempotency and Replay Protection

### Replay Detection

```bash
# Check if this rollback was already executed
is_rollback_replay() {
  local correlation_id="$1"
  local log_file="ops/rollback-history.log"

  if grep -q "correlation_id=$correlation_id.*status=completed" "$log_file"; then
    return 0  # Already completed
  fi
  return 1  # Not a replay
}

# Before executing rollback
if is_rollback_replay "$CORRELATION_ID"; then
  log_warn "Rollback $CORRELATION_ID already completed - skipping"
  exit 0
fi
```

### Idempotent Operations

Rollback strategies must be idempotent:

- **Address re-point**: Safe to re-apply (idempotent)
- **Deploy previous WASM**: Check if target contract ID already exists
- **Admin pause**: Check if already paused before calling

---

## Log Storage and Rotation

### Log File Location

```bash
ROLLBACK_LOG_DIR="ops/logs"
ROLLBACK_LOG_FILE="${ROLLBACK_LOG_DIR}/rollback-$(date +%Y-%m-%d).log"
```

### Log Format

```log
[2024-01-15T10:30:00Z] [INFO] correlation_id=550e8400-e29b-41d4-a716-446655440000 timestamp=2024-01-15T10:30:00Z operator=operator-123 network=mainnet contract_name=mux-account from_contract_id=CABCD... to_contract_id=CXYZ... rollback_strategy=address-repoint authz_method=multisig authz_id=auth-456 status=started git_commit=abc123def456

[2024-01-15T10:30:15Z] [INFO] correlation_id=550e8400-e29b-41d4-a716-446655440000 status=completed

[2024-01-15T10:30:15Z] [METRIC] rollback_duration_ms=15000 rollback_strategy=address-repoint network=mainnet success=true
```

### Log Rotation

- Keep logs for 90 days
- Compress logs older than 30 days
- Archive to cold storage after 90 days
- Index correlation IDs for quick lookup

---

## Metrics and Observability

### Required Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rollback_operations_total` | counter | strategy, network, status | Total rollback operations |
| `rollback_duration_seconds` | histogram | strategy, network | Rollback operation duration |
| `rollback_authz_failures_total` | counter | authz_method, network | Authorization failures |
| `rollback_replay_attempts_total` | counter | network | Replay detection triggers |
| `rollback_active_operations` | gauge | network | Currently active rollbacks |

### Metric Logging Example

```bash
log_metric() {
  local metric_name="$1"
  local metric_value="$2"
  local labels="$3"
  echo "[METRIC] ${metric_name}=${metric_value} ${labels}" >> "$ROLLBACK_LOG_FILE"
}

# After rollback completion
log_metric "rollback_duration_seconds" "15.5" \
  "strategy=address-repoint network=mainnet"
log_metric "rollback_operations_total" "1" \
  "strategy=address-repoint network=mainnet status=success"
```

---

## Failure Modes and Handling

### Logging Failure

If logging fails, the operation must be blocked:

```bash
log_rollback() {
  local correlation_id="$1"
  local status="$2"
  local error_code="${3:-}"
  local extra_fields="${4:-}"

  local log_entry="[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] correlation_id=${correlation_id} status=${status} ${error_code:+error_code=${error_code}} ${extra_fields}"

  if ! echo "$log_entry" >> "$ROLLBACK_LOG_FILE"; then
    echo "CRITICAL: Failed to write to rollback log. Operation blocked."
    exit 1
  fi
}
```

### Dependency Outage

If RPC/DB/Horizon is unavailable:

1. Log the outage with correlation ID
2. Fail-closed - do not proceed with rollback
3. Alert on-call team
4. Record in incident log

### Auth Expiry

If authorization expires during rollback:

1. Log auth expiry with correlation ID
2. Abort the operation
3. Require fresh authorization
4. Do auto-retry only if safe (idempotent)

---

## Rollback History Query

### Query by Correlation ID

```bash
query_rollback_by_id() {
  local correlation_id="$1"
  grep "correlation_id=${correlation_id}" ops/logs/rollback-*.log
}
```

### Query by Network

```bash
query_rollback_by_network() {
  local network="$1"
  grep "network=${network}" ops/logs/rollback-*.log
}
```

### Query by Date Range

```bash
query_rollback_by_date() {
  local start_date="$1"
  local end_date="$2"
  find ops/logs -name "rollback-*.log" -newermt "$start_date" ! -newermt "$end_date" -exec cat {} \;
}
```

---

## Integration with Deploy Script

The deploy script (`scripts/deploy.sh`) must:

1. Generate correlation ID at start
2. Log all deployment actions with correlation ID
3. Redact secrets before logging
4. Write to ops/rollback log on rollback operations
5. Include correlation ID in error messages
6. Support rollback-specific logging mode

Example integration:

```bash
# In deploy.sh
if [[ "$ROLLBACK_MODE" == "true" ]]; then
  CORRELATION_ID=$(generate_correlation_id)
  export CORRELATION_ID
  log_rollback "$CORRELATION_ID" "started" "" \
    "network=$NETWORK contract=$TARGET_CONTRACT rollback_strategy=$STRATEGY"
fi
```

---

## Security Considerations

### Log Access Control

- Log files must be readable only by authorized operators
- Use file permissions: `chmod 640 ops/logs/*.log`
- Log directory: `chmod 750 ops/logs`
- Audit log access monthly

### Log Integrity

- Use append-only file system if available
- Calculate log checksums daily
- Store checksums in separate, tamper-evident location
- Alert on log modification

### Log Retention and Deletion

- Never delete logs before retention period
- Secure deletion after retention period (shred)
- Maintain index of deleted correlation IDs
- Archive to immutable storage before deletion

---

## Related Documents

- [Rollback Deploy Notes](../docs/rollback-deploy.md) — Rollback strategies and procedures
- [Mainnet Deploy Checklist](../docs/mainnet-deploy-checklist.md) — Pre-deployment requirements
- [Security Policy](../SECURITY.md) — Overall security guidelines
- [Threat Model](../docs/threat-model.md) — Security threat analysis

---

## Appendix: Error Codes

| Error Code | Description | Severity |
|------------|-------------|----------|
| `ROLLBACK_AUTHZ_DENIED` | Authorization denied | Critical |
| `ROLLBACK_MULTISIG_QUORUM_NOT_MET` | Multisig quorum not met | Critical |
| `ROLLBACK_CONFLICT` | Conflicting rollback in progress | High |
| `ROLLBACK_LOG_FAILURE` | Failed to write to log | Critical |
| `ROLLBACK_REPLAY_DETECTED` | Replay attempt detected | Medium |
| `ROLLBACK_DEPENDENCY_UNAVAILABLE` | RPC/DB/Horizon unavailable | High |
| `ROLLBACK_INVALID_STRATEGY` | Invalid rollback strategy | High |
| `ROLLBACK_CONTRACT_NOT_FOUND` | Target contract not found | High |
| `ROLLBACK_WASM_HASH_MISMATCH` | WASM hash verification failed | Critical |
