# Account Upgrade Migration Path

This document defines the migration path for account contract upgrades in Mux Protocol. When upgrading account contracts, storage layout changes require careful migration to preserve user data and ensure continuity of service.

---

## Overview

Account contracts store critical user data including:
- Owner addresses
- Delegates with expiration
- Spend limits per asset
- Guardian sets
- Session keys with scopes and expiration
- Nonces for transaction ordering

When upgrading an account contract, the storage layout may change. This document provides a standardized migration path to ensure:
- **Data preservation** — No user data is lost during upgrade
- **Authorization** — Only authorized parties can trigger migration
- **Idempotency** — Migration can be safely retried
- **Observability** — All migrations are logged with correlation IDs
- **Rollback** — Migration can be reversed if needed

---

## Storage Layout Evolution

### Version 1.0 (Initial)

```rust
#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
    SpendLimit(Address),
    GuardianSet,
    Nonce,
    SessionKey(Address, Address),
    SessionKeyIndex(Address),
    Paused,
    Executing,
}
```

### Version 1.1 (Added Recovery Metadata)

```rust
#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
    SpendLimit(Address),
    GuardianSet,
    Nonce,
    SessionKey(Address, Address),
    SessionKeyIndex(Address),
    Paused,
    Executing,
    RecoveryMetadata,  // ← New field
}
```

### Version 2.0 (Refactored Session Keys)

```rust
#[contracttype]
pub enum DataKey {
    Owner,
    Delegates,
    SpendLimit(Address),
    GuardianSet,
    Nonce,
    // Session keys moved to dedicated storage
    SessionKeyV2(Address, Address),  // ← New structure
    SessionKeyIndexV2(Address),      // ← New structure
    Paused,
    Executing,
    RecoveryMetadata,
}
```

---

## Migration Strategies

### Strategy 1: Additive Migration (Safe)

Use when only adding new fields without modifying existing ones.

**When to use**: Adding new optional fields that don't affect existing logic.

**Procedure**:
1. Deploy new contract version with new DataKey variants
2. Call `upgrade()` to switch WASM
3. New code reads old fields, new fields are initialized as needed
4. No migration function required

**Example**:
```rust
// Old code can read Owner, Delegates, etc.
// New code can also read RecoveryMetadata (returns None if not set)
let recovery: Option<RecoveryMetadata> = env.storage()
    .instance()
    .get(&DataKey::RecoveryMetadata);
```

### Strategy 2: Transformative Migration (Requires Migration Function)

Use when renaming, removing, or changing the type of existing fields.

**When to use**: Storage layout changes that break backward compatibility.

**Procedure**:
1. Deploy new contract version with migration function
2. Call `upgrade()` to switch WASM
3. Call `migrate()` to transform storage
4. Verify migration with tests
5. Keep old WASM hash for rollback

**Migration Function Template**:
```rust
pub fn migrate(env: Env) {
    // 1. Authorize — only admin may migrate
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin)
        .expect("admin not initialised");
    admin.require_auth();

    // 2. Transform storage
    // Example: migrate SessionKey → SessionKeyV2
    let owner: Address = env.storage().instance()
        .get(&DataKey::Owner)
        .expect("owner not initialised");

    let session_keys: Vec<Address> = env.storage()
        .instance()
        .get(&DataKey::SessionKeyIndex(owner))
        .unwrap_or_default();

    for session_key in session_keys {
        if let Some(old_record) = env.storage()
            .instance()
            .get::<_, SessionKeyRecord>(&DataKey::SessionKey(owner, session_key))
        {
            // Transform to new structure
            let new_record = SessionKeyRecordV2 {
                expires_at: old_record.expires_at,
                scopes: old_record.scopes,
                revoked: old_record.revoked,
                created_at: env.ledger().timestamp(),
            };

            // Write to new location
            env.storage()
                .instance()
                .set(&DataKey::SessionKeyV2(owner, session_key), &new_record);

            // Remove old entry
            env.storage()
                .instance()
                .remove(&DataKey::SessionKey(owner, session_key));
        }
    }

    // 3. Update index
    env.storage()
        .instance()
        .remove(&DataKey::SessionKeyIndex(owner));
    env.storage()
        .instance()
        .set(&DataKey::SessionKeyIndexV2(owner), &session_keys);

    // 4. Emit migration event
    env.events()
        .publish((symbol_short!("migrate"), symbol_short!("complete")), owner);
}
```

### Strategy 3: Two-Phase Migration (High Risk)

Use for major architectural changes that cannot be done in-place.

**When to use**: Complete contract replacement with incompatible storage.

**Procedure**:
1. Deploy new contract at new address
2. Call `migrate_from_old()` on new contract to copy data from old contract
3. Update factory registry to point to new contract
4. Keep old contract for rollback
5. Deprecate old contract after migration period

---

## Authorization Requirements

### Layer 1: Contract-Level Authorization

Migration functions must enforce admin authorization:

```rust
pub fn migrate(env: Env) {
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin)
        .expect("admin not initialised");
    admin.require_auth();  // ← Critical: auth before migration
    // ... migration logic
}
```

**Invariant**: `require_auth()` must be called BEFORE any storage modification.

### Layer 2: Network-Level Authorization

Migration operations require network-specific authorization:

| Network | Authz Method | Quorum | Time Window |
|---------|--------------|--------|-------------|
| Mainnet | Multisig (3/5) | 3 of 5 signers | During approved migration window |
| Testnet | Delegate or Admin | 1/1 | Any time |
| Localnet | None | N/A | Any time |

### Layer 3: Feature-Flag Authorization

Migration operations require the `ENABLE_ACCOUNT_MIGRATION` feature flag:

```bash
# Fail-closed: migration disabled by default
ENABLE_ACCOUNT_MIGRATION=true bash scripts/migrate-account.sh ...
```

### Layer 4: Pre-Migration Verification

Before executing migration, run verification:

```bash
bash scripts/verify-migration.sh \
  --network mainnet \
  --account-id <account_id> \
  --from-version 1.0 \
  --to-version 1.1
```

---

## Required Log Fields

Every migration operation MUST log the following fields:

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `correlation_id` | UUID | Unique identifier for the migration operation | `550e8400-e29b-41d4-a716-446655440000` |
| `timestamp` | ISO8601 | UTC timestamp of the operation | `2024-01-15T10:30:00Z` |
| `operator` | string | Operator identity (redacted if sensitive) | `operator-123` or `GAB...XYZ` |
| `network` | enum | Target network (testnet/mainnet/localnet) | `mainnet` |
| `account_id` | string | Account contract being migrated | `CABCD...` |
| `from_version` | string | Source version | `1.0` |
| `to_version` | string | Target version | `1.1` |
| `migration_strategy` | enum | Strategy used (additive/transformative/two-phase) | `transformative` |
| `authz_method` | enum | Authorization method (multisig/delegate/admin) | `multisig` |
| `authz_id` | string | Authorization identifier (redacted) | `auth-456` |
| `status` | enum | Operation status (started/completed/failed) | `completed` |
| `error_code` | string (optional) | Error code if failed | `MIGRATION_AUTHZ_DENIED` |
| `migrated_keys_count` | number | Number of storage keys migrated | `42` |
| `git_commit` | string | Git commit SHA of migration script | `abc123def456` |

---

## Pre-Migration Checklist

Before executing any migration:

- [ ] **Authorization verified**: Multisig quorum met (mainnet) or delegate authorized (testnet)
- [ ] **Feature flag enabled**: `ENABLE_ACCOUNT_MIGRATION=true` set
- [ ] **Account contract verified**: Account ID exists and is upgradeable
- [ ] **Backup snapshot taken**: Current storage state recorded
- [ ] **Migration function tested**: Migration logic tested on testnet
- [ ] **Rollback plan documented**: Procedure to revert to old version
- [ ] **Correlation ID generated**: Unique ID for logging and tracking
- [ ] **Incident channel notified**: Team alerted to migration in progress
- [ ] **User communication sent**: Users notified of potential downtime (if applicable)

---

## Error Codes

| Error Code | Description | Severity | Action |
|------------|-------------|----------|--------|
| `MIGRATION_AUTHZ_DENIED` | Authorization denied | Critical | Do not proceed |
| `MIGRATION_MULTISIG_QUORUM_NOT_MET` | Multisig quorum not met | Critical | Obtain additional signatures |
| `MIGRATION_CONFLICT` | Conflicting migration in progress | High | Wait for completion |
| `MIGRATION_LOG_FAILURE` | Failed to write to log | Critical | Block operation |
| `MIGRATION_ACCOUNT_NOT_FOUND` | Account contract not found | High | Verify account ID |
| `MIGRATION_VERSION_MISMATCH` | Version mismatch (already migrated) | High | Verify current version |
| `MIGRATION_FLAG_DISABLED` | Migration feature flag disabled | Critical | Enable flag |
| `MIGRATION_DEPENDENCY_UNAVAILABLE` | RPC/DB/Horizon unavailable | High | Retry later |
| `MIGRATION_STORAGE_CORRUPTION` | Storage data corrupted or invalid | Critical | Investigate and restore |
| `MIGRATION_KEY_NOT_FOUND` | Expected storage key not found | Medium | Verify storage layout |

---

## Idempotency and Replay Protection

### Replay Detection

```bash
# Check if this migration was already executed
is_migration_replay() {
  local correlation_id="$1"
  local account_id="$2"
  local log_file="ops/logs/migration-*.log"

  if grep -q "correlation_id=${correlation_id}.*account_id=${account_id}.*status=completed" "$log_file"; then
    return 0  # Already completed
  fi
  return 1  # Not a replay
}

# Before executing migration
if is_migration_replay "$CORRELATION_ID" "$ACCOUNT_ID"; then
  log_warn "Migration $CORRELATION_ID for account $ACCOUNT_ID already completed - skipping"
  exit 0
fi
```

### Idempotent Operations

Migration functions must be idempotent where possible:

- **Additive migration**: Check if new field already has value before setting
- **Transformative migration**: Check if target key already exists before writing
- **Two-phase migration**: Check if new contract already has data before copying

```rust
// Idempotent migration example
pub fn migrate(env: Env) {
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin)
        .expect("admin not initialised");
    admin.require_auth();

    // Check if already migrated
    if env.storage().instance().has(&DataKey::MigrationComplete) {
        return;  // Already migrated, idempotent
    }

    // ... migration logic

    // Mark as complete
    env.storage().instance().set(&DataKey::MigrationComplete, &true);
}
```

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

Migration operations must be rate-limited:

| Network | Rate Limit | Burst | Time Window |
|---------|------------|-------|-------------|
| Mainnet | 10 migrations per hour | 2 | 1 hour |
| Testnet | 100 migrations per hour | 10 | 1 hour |
| Localnet | No limit | N/A | N/A |

Rate limiting is enforced at the script level and logged.

---

## Rollback Procedures

### Rollback via Re-migration

If the new version has issues:

1. **Identify the old version** from logs or git history
2. **Re-deploy old WASM** (if not already on ledger)
3. **Call upgrade() with old WASM hash**
4. **Call reverse_migration()** to transform storage back
5. **Verify rollback** with tests

**Reverse Migration Template**:
```rust
pub fn reverse_migration(env: Env) {
    let admin: Address = env.storage().instance()
        .get(&DataKey::Admin)
        .expect("admin not initialised");
    admin.require_auth();

    // Reverse the transformation
    // Example: migrate SessionKeyV2 → SessionKey
    let owner: Address = env.storage().instance()
        .get(&DataKey::Owner)
        .expect("owner not initialised");

    let session_keys: Vec<Address> = env.storage()
        .instance()
        .get(&DataKey::SessionKeyIndexV2(owner))
        .unwrap_or_default();

    for session_key in session_keys {
        if let Some(new_record) = env.storage()
            .instance()
            .get::<_, SessionKeyRecordV2>(&DataKey::SessionKeyV2(owner, session_key))
        {
            // Transform back to old structure
            let old_record = SessionKeyRecord {
                expires_at: new_record.expires_at,
                scopes: new_record.scopes,
                revoked: new_record.revoked,
            };

            // Write to old location
            env.storage()
                .instance()
                .set(&DataKey::SessionKey(owner, session_key), &old_record);

            // Remove new entry
            env.storage()
                .instance()
                .remove(&DataKey::SessionKeyV2(owner, session_key));
        }
    }

    // Remove migration complete flag
    env.storage().instance().remove(&DataKey::MigrationComplete);

    // Emit rollback event
    env.events()
        .publish((symbol_short!("migrate"), symbol_short!("rollback")), owner);
}
```

### Rollback via Account Re-point

If using two-phase migration:

1. Update factory registry to point back to old account
2. Deprecate new account
3. Notify dependent services

---

## Metrics and Observability

### Required Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `migration_operations_total` | counter | strategy, network, status | Total migration operations |
| `migration_duration_seconds` | histogram | strategy, network, account_version | Migration operation duration |
| `migration_authz_failures_total` | counter | authz_method, network | Authorization failures |
| `migration_replay_attempts_total` | counter | network | Replay detection triggers |
| `migration_active_operations` | gauge | network | Currently active migrations |
| `migration_keys_migrated_total` | counter | network, from_version, to_version | Total keys migrated |

### Metric Logging Example

```bash
log_metric() {
  local metric_name="$1"
  local metric_value="$2"
  local labels="$3"
  echo "[METRIC] ${metric_name}=${metric_value} ${labels}" >> "$MIGRATION_LOG_FILE"
}

# After migration completion
log_metric "migration_duration_seconds" "15.3" \
  "strategy=transformative network=mainnet"
log_metric "migration_operations_total" "1" \
  "strategy=transformative network=mainnet status=success"
log_metric "migration_keys_migrated_total" "42" \
  "network=mainnet from_version=1.0 to_version=1.1"
```

---

## Testing Requirements

### Unit Tests

Every migration function must have unit tests for:

- **Authorization**: `require_auth()` is called before migration
- **Idempotency**: Migration can be called multiple times safely
- **Data transformation**: Old data correctly transforms to new format
- **Error handling**: All error codes are tested
- **Rollback**: Reverse migration restores original state

### Integration Tests

Integration tests must verify:

- **Full migration flow**: Deploy v1, upgrade to v2, migrate, verify state
- **Rollback flow**: Migrate to v2, rollback to v1, verify state
- **Authz enforcement**: Unauthorized migration attempts fail
- **Rate limiting**: Excessive migrations are blocked
- **Conflict detection**: Concurrent migrations are blocked

### E2E Tests

E2E tests must verify:

- **Testnet migration**: Full migration on testnet with real RPC
- **Smoke tests**: Post-migration functionality works end-to-end
- **Monitoring**: Metrics are emitted correctly
- **Logging**: Logs are written with correlation IDs

---

## Security Considerations

### Fail-Closed Behavior

- Migration disabled by default on mainnet (feature flag)
- Authorization failure blocks operation
- Logging failure blocks operation
- Account not found blocks operation
- RPC unavailability blocks operation

### Secret Protection

- Secrets redacted before logging
- No secrets in git history
- No secrets in environment files
- Log files have restricted permissions (640)

### Authorization

- Multisig required for mainnet migrations
- Admin authorization verified at contract level
- Delegate authorization supported for testnet
- API key scoping for automated migrations

### Dependency Outage

- RPC unavailability blocks writes
- DB unavailability blocks writes
- Horizon unavailability blocks writes
- Fail-closed on all dependencies

### Data Integrity

- Backup snapshot before migration
- Verification after migration
- Rollback capability always available
- Corruption detection and alerting

---

## Related Documents

- [Account Abstraction Design](account-abstraction.md) — Account contract architecture
- [Contract Upgrade Pattern](contract-upgrade-pattern.md) — Technical upgrade implementation
- [Upgrade Auth Requirements](upgrade-auth-requirements.md) — Authorization requirements for upgrades
- [Rollback Deploy Notes](rollback-deploy.md) — Rollback strategies and procedures
- [Rollback Log Discipline](../ops/rollback-log.md) — Operational logging discipline
- [Security Policy](../SECURITY.md) — Overall security guidelines
- [scripts/verify-migration.sh](../scripts/verify-migration.sh) — Migration verification script
- [scripts/migrate-account.sh](../scripts/migrate-account.sh) — Migration execution script

---

## Appendix: Migration Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                   Migration Request                              │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              Pre-Migration Verification                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • Check feature flag (ENABLE_ACCOUNT_MIGRATION)        │   │
│  │ • Verify authorization (multisig/delegate)             │   │
│  │ • Verify account contract exists                       │   │
│  │ • Check for conflicting migration                      │   │
│  │ • Take backup snapshot                                  │   │
│  │ • Generate correlation ID                               │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Log Migration Started                         │
│  • Write to ops/logs/migration-*.log with correlation ID         │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Upgrade Contract WASM                         │
│  • stellar contract invoke --upgrade --new_wasm_hash <hash>    │
│  • Contract verifies admin.require_auth()                       │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
                         │
                    ┌────┴────┐
                    │ Strategy │
                    └────┬────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
         ▼               ▼               ▼
    ┌─────────┐    ┌─────────┐    ┌──────────┐
    │Additive │    │Trans-   │    │Two-Phase │
    │         │    │formative│    │          │
    └────┬────┘    └────┬────┘    └────┬─────┘
         │              │               │
         │              │               │
         │              ▼               ▼
         │      ┌─────────────┐  ┌─────────────┐
         │      │ Call migrate()│  │ Deploy new  │
         │      │              │  │ contract    │
         │      └──────┬──────┘  └──────┬──────┘
         │         No migration    │ Copy data  │
         │             │           │            │
         └─────────────┼───────────┴────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│              Post-Migration Verification                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • Verify storage transformation                         │   │
│  │ • Run smoke tests                                        │   │
│  │ • Verify new functionality                               │   │
│  │ • Compare with backup snapshot                           │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Log Migration Completed                         │
│  • Write status=completed to log                                │
│  • Emit metrics                                                │
└─────────────────────────────────────────────────────────────────┘
```
