# Upgrade Auth Requirements

This document defines the authorization requirements for contract upgrade operations in Mux Protocol. Upgrades are critical security events that require strict authorization, logging, and validation.

---

## Overview

Contract upgrades in Soroban replace the WASM bytecode of a deployed contract instance. This is a privileged operation that can change contract behavior, potentially affecting user funds. Therefore, upgrade operations must be:

- **Authorized** by multiple independent parties
- **Logged** with correlation IDs and audit trails
- **Validated** with pre-upgrade checks
- **Reversible** with documented rollback procedures
- **Rate-limited** to prevent abuse

---

## Authorization Layers

### Layer 1: Contract-Level Authorization

Every upgradeable contract must implement `require_auth()` on the admin address:

```rust
pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin)
        .expect("admin not initialised");
    admin.require_auth();  // ← Critical: auth before upgrade
    env.deployer().update_current_contract_wasm(new_wasm_hash);
}
```

**Invariant**: `require_auth()` must be called BEFORE `update_current_contract_wasm()`.

### Layer 2: Network-Level Authorization

Upgrade operations require network-specific authorization:

| Network | Authz Method | Quorum | Time Window |
|---------|--------------|--------|-------------|
| Mainnet | Multisig (3/5) | 3 of 5 signers | During approved upgrade window |
| Testnet | Delegate or Admin | 1/1 | Any time |
| Localnet | None | N/A | Any time |

### Layer 3: Feature-Flag Authorization

Upgrade operations require the `ENABLE_CONTRACT_UPGRADE` feature flag:

```bash
# Fail-closed: upgrade disabled by default
ENABLE_CONTRACT_UPGRADE=true bash scripts/upgrade.sh ...
```

### Layer 4: Pre-Upgrade Verification

Before executing an upgrade, run verification:

```bash
bash scripts/verify-upgrade.sh \
  --network mainnet \
  --contract mux-account \
  --new-wasm-hash <hash>
```

---

## Required Log Fields

Every upgrade operation MUST log the following fields:

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `correlation_id` | UUID | Unique identifier for the upgrade operation | `550e8400-e29b-41d4-a716-446655440000` |
| `timestamp` | ISO8601 | UTC timestamp of the operation | `2024-01-15T10:30:00Z` |
| `operator` | string | Operator identity (redacted if sensitive) | `operator-123` or `GAB...XYZ` |
| `network` | enum | Target network (testnet/mainnet/localnet) | `mainnet` |
| `contract_name` | string | Contract being upgraded | `mux-account` |
| `contract_id` | string | Contract ID being upgraded | `CABCD...` |
| `old_wasm_hash` | string | Previous WASM hash (for rollback) | `a1b2c3...` |
| `new_wasm_hash` | string | New WASM hash being deployed | `d4e5f6...` |
| `authz_method` | enum | Authorization method (multisig/delegate/admin) | `multisig` |
| `authz_id` | string | Authorization identifier (redacted) | `auth-456` |
| `status` | enum | Operation status (started/completed/failed) | `completed` |
| `error_code` | string (optional) | Error code if failed | `UPGRADE_AUTHZ_DENIED` |
| `git_commit` | string | Git commit SHA of upgrade script | `abc123def456` |

---

## Pre-Upgrade Checklist

Before executing any upgrade:

- [ ] **Authorization verified**: Multisig quorum met (mainnet) or delegate authorized (testnet)
- [ ] **Feature flag enabled**: `ENABLE_CONTRACT_UPGRADE=true` set
- [ ] **WASM hash verified**: New WASM hash matches expected value
- [ ] **Old WASM hash recorded**: Previous hash saved for potential rollback
- [ ] **Tests passed**: All unit and integration tests pass with new WASM
- [ ] **Storage compatibility verified**: Storage layout changes are backward-compatible or migration ready
- [ ] **Testnet deployed**: New version deployed and smoke-tested on testnet
- [ ] **Rollback plan documented**: Procedure to revert to old WASM documented
- [ ] **Correlation ID generated**: Unique ID for logging and tracking
- [ ] **Incident channel notified**: Team alerted to upgrade in progress

---

## Upgrade Strategies

### Strategy 1: Direct Upgrade (preferred for non-breaking changes)

Use when storage layout is backward-compatible:

1. Build new WASM
2. Upload WASM to network
3. Call `upgrade()` on contract
4. Verify upgrade with smoke tests

**When to use**: Storage layout unchanged or only additive changes.

### Strategy 2: Upgrade with Migration

Use when storage layout changes:

1. Build new WASM with migration function
2. Upload WASM to network
3. Call `upgrade()` on contract
4. Call `migrate()` to transform storage
5. Verify migration with tests
6. Keep old WASM hash for rollback

**When to use**: Storage fields removed, renamed, or type-changed.

### Strategy 3: Two-Phase Upgrade

Use for high-risk changes:

1. Deploy new contract at new address
2. Migrate state from old to new contract
3. Update config to point to new address
4. Keep old contract for rollback

**When to use**: Major architectural changes or contract replacement.

---

## Error Codes

| Error Code | Description | Severity | Action |
|------------|-------------|----------|--------|
| `UPGRADE_AUTHZ_DENIED` | Authorization denied | Critical | Do not proceed |
| `UPGRADE_MULTISIG_QUORUM_NOT_MET` | Multisig quorum not met | Critical | Obtain additional signatures |
| `UPGRADE_CONFLICT` | Conflicting upgrade in progress | High | Wait for completion |
| `UPGRADE_LOG_FAILURE` | Failed to write to log | Critical | Block operation |
| `UPGRADE_WASM_HASH_MISMATCH` | WASM hash verification failed | Critical | Verify build |
| `UPGRADE_STORAGE_INCOMPATIBLE` | Storage layout incompatible | High | Implement migration |
| `UPGRADE_FLAG_DISABLED` | Upgrade feature flag disabled | Critical | Enable flag |
| `UPGRADE_DEPENDENCY_UNAVAILABLE` | RPC/DB/Horizon unavailable | High | Retry later |
| `UPGRADE_INVALID_CONTRACT` | Contract not found or not upgradeable | High | Verify contract ID |

---

## Idempotency and Replay Protection

### Replay Detection

```bash
# Check if this upgrade was already executed
is_upgrade_replay() {
  local correlation_id="$1"
  local log_file="ops/logs/upgrade-*.log"

  if grep -q "correlation_id=$correlation_id.*status=completed" "$log_file"; then
    return 0  # Already completed
  fi
  return 1  # Not a replay
}

# Before executing upgrade
if is_upgrade_replay "$CORRELATION_ID"; then
  log_warn "Upgrade $CORRELATION_ID already completed - skipping"
  exit 0
fi
```

### Idempotent Operations

Upgrade operations must be idempotent where possible:

- **Direct upgrade**: Check if current WASM hash matches target before upgrading
- **Migration**: Check if migration already applied before running
- **Two-phase**: Check if new contract already deployed

---

## Secret Redaction

**NEVER log the following in plaintext:**

- Private keys (admin, multisig signers)
- JWT tokens
- API keys
- Mnemonic phrases
- Full Stellar secret keys

**Redaction format:**

```bash
# Before logging
ADMIN_PRIVATE_KEY="SABC123...XYZ"

# After redaction (log only)
ADMIN_PRIVATE_KEY="S***"  # Show first char, redact rest
# or
ADMIN_PRIVATE_KEY="<REDACTED>"
```

---

## Rate Limiting

Upgrade operations must be rate-limited:

| Network | Rate Limit | Burst | Time Window |
|---------|------------|-------|-------------|
| Mainnet | 1 upgrade per 24 hours | 1 | 24 hours |
| Testnet | 10 upgrades per hour | 2 | 1 hour |
| Localnet | No limit | N/A | N/A |

Rate limiting is enforced at the script level and logged.

---

## Storage Migration Rules

### Additive Changes (Safe)

Adding new fields is always safe:

```rust
// Old version
#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
}

// New version (safe)
#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
    NewField,  // ← New field added
}
```

### Removal or Renaming (Requires Migration)

Removing or renaming fields requires migration:

```rust
pub fn migrate(env: Env) {
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin).expect("not initialised");
    admin.require_auth();

    // Migrate OldKey → NewKey
    if let Some(val) = env.storage().persistent().get::<_, OldType>(&DataKey::OldKey) {
        env.storage().persistent().set(&DataKey::NewKey, &val);
        env.storage().persistent().remove(&DataKey::OldKey);
    }
}
```

### Type Changes (Requires Migration)

Changing field types requires migration:

```rust
// Old: u32
// New: u64
pub fn migrate(env: Env) {
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin).expect("not initialised");
    admin.require_auth();

    if let Some(old_val) = env.storage().persistent().get::<_, u32>(&DataKey::Counter) {
        env.storage().persistent().set(&DataKey::Counter, &(old_val as u64));
    }
}
```

---

## Rollback Procedures

### Rollback via Re-upgrade

If the new version has issues:

1. **Identify the old WASM hash** from logs or git history
2. **Re-upload the old WASM** (if not already on ledger)
3. **Call upgrade() with old hash**:
   ```bash
   stellar contract invoke \
     --id $CONTRACT_ID \
     --source $ADMIN_ACCOUNT \
     --network $NETWORK \
     -- upgrade \
     --new_wasm_hash $OLD_WASM_HASH
   ```
4. **If storage was migrated**, run reverse migration
5. **Verify rollback** with smoke tests

### Rollback via Address Re-point

If using two-phase upgrade:

1. Update `config/addresses.json` to point back to old contract
2. Regenerate bindings
3. Publish new bindings version
4. Notify dependent services

---

## Metrics and Observability

### Required Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `upgrade_operations_total` | counter | strategy, network, status | Total upgrade operations |
| `upgrade_duration_seconds` | histogram | strategy, network | Upgrade operation duration |
| `upgrade_authz_failures_total` | counter | authz_method, network | Authorization failures |
| `upgrade_replay_attempts_total` | counter | network | Replay detection triggers |
| `upgrade_active_operations` | gauge | network | Currently active upgrades |
| `upgrade_storage_migrations_total` | counter | network, status | Storage migration operations |

### Metric Logging Example

```bash
log_metric() {
  local metric_name="$1"
  local metric_value="$2"
  local labels="$3"
  echo "[METRIC] ${metric_name}=${metric_value} ${labels}" >> "$UPGRADE_LOG_FILE"
}

# After upgrade completion
log_metric "upgrade_duration_seconds" "30.5" \
  "strategy=direct network=mainnet"
log_metric "upgrade_operations_total" "1" \
  "strategy=direct network=mainnet status=success"
```

---

## Testing Requirements

### Unit Tests

Every upgradeable contract must have unit tests for:

- **Authorization**: `require_auth()` is called before upgrade
- **WASM hash validation**: Invalid hashes are rejected
- **Storage compatibility**: Old storage is readable after upgrade
- **Migration**: Migration function transforms data correctly
- **Error handling**: All error codes are tested

### Integration Tests

Integration tests must verify:

- **Full upgrade flow**: Deploy v1, upgrade to v2, verify state
- **Migration flow**: Deploy v1, upgrade with migration, verify transformed state
- **Rollback flow**: Upgrade to v2, rollback to v1, verify state
- **Authz enforcement**: Unauthorized upgrade attempts fail
- **Rate limiting**: Excessive upgrades are blocked

### E2E Tests

E2E tests must verify:

- **Testnet upgrade**: Full upgrade on testnet with real RPC
- **Smoke tests**: Post-upgrade functionality works end-to-end
- **Monitoring**: Metrics are emitted correctly
- **Logging**: Logs are written with correlation IDs

---

## Security Considerations

### Fail-Closed Behavior

- Upgrade disabled by default on mainnet (feature flag)
- Authorization failure blocks operation
- Logging failure blocks operation
- WASM hash mismatch blocks operation
- RPC unavailability blocks operation

### Secret Protection

- Secrets redacted before logging
- No secrets in git history
- No secrets in environment files
- Log files have restricted permissions (640)

### Authorization

- Multisig required for mainnet upgrades
- Admin authorization verified at contract level
- Delegate authorization supported for testnet
- API key scoping for automated upgrades

### Dependency Outage

- RPC unavailability blocks writes
- DB unavailability blocks writes
- Horizon unavailability blocks writes
- Fail-closed on all dependencies

---

## Related Documents

- [Contract Upgrade Pattern](contract-upgrade-pattern.md) — Technical upgrade implementation
- [Rollback Deploy Notes](rollback-deploy.md) — Rollback strategies and procedures
- [Rollback Log Discipline](../ops/rollback-log.md) — Operational logging discipline
- [Security Policy](../SECURITY.md) — Overall security guidelines
- [Mainnet Deploy Checklist](MAINNET_DEPLOY_CHECKLIST.md) — Pre-deployment requirements
- [scripts/verify-upgrade.sh](../scripts/verify-upgrade.sh) — Upgrade verification script
- [scripts/upgrade.sh](../scripts/upgrade.sh) — Upgrade execution script

---

## Appendix: Upgrade Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                     Upgrade Request                             │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              Pre-Upgrade Verification                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • Check feature flag (ENABLE_CONTRACT_UPGRADE)          │   │
│  │ • Verify authorization (multisig/delegate)            │   │
│  │ • Verify WASM hash                                     │   │
│  │ • Check for conflicting upgrade                        │   │
│  │ • Generate correlation ID                              │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Log Upgrade Started                           │
│  • Write to ops/logs/upgrade-*.log with correlation ID          │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Upload New WASM                                │
│  • stellar contract upload --wasm <new.wasm>                    │
│  • Record new_wasm_hash                                         │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Call upgrade()                                │
│  • stellar contract invoke --upgrade --new_wasm_hash <hash>    │
\  • Contract verifies admin.require_auth()                       │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              Run Migration (if required)                         │
│  • stellar contract invoke --migrate                            │
│  • Verify storage transformation                                │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Post-Upgrade Verification                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • Run smoke tests                                        │   │
│  │ • Verify storage compatibility                           │   │
│  │ • Verify new functionality                               │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Log Upgrade Completed                          │
│  • Write status=completed to log                                │
│  • Emit metrics                                                │
└─────────────────────────────────────────────────────────────────┘
```
