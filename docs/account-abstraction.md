# Account Abstraction for Mux Protocol

## Overview

Account abstraction (AA) enables Mux users to interact with smart contracts without directly managing cryptographic keys, private keys, or gas fees. Mux's AA implementation provides:

- **Gasless Transactions** — Transactions can be sponsored by relayers
- **Session Keys** — Delegate temporary signing authority to applications or devices
- **Smart Account Recovery** — Guardian-based recovery mechanisms for account access
- **Spend Limits** — Per-asset and per-period spending constraints
- **Flexible Authorization** — Multi-signature and delegated authority patterns

## Problem Statement

Traditional blockchain accounts are limited by:

1. **Key Management Burden** — Users must protect and manage private keys
2. **Gas Fee Overhead** — Every transaction requires native network tokens for fees
3. **Limited Flexibility** — Fixed signing authority with no granular delegation options
4. **Poor UX** — Mobile and web apps struggle with key management and confirmation flows

Mux Account Abstraction solves these problems by:

- Moving signing authority to smart contracts that enforce custom rules
- Enabling relayers to pay for transaction gas on behalf of users
- Allowing session keys to be issued for specific applications or time periods
- Separating authorization from account ownership for better UX

## Architecture

### Components

#### Account Factory Contract (`mux-account-factory`)

The factory contract manages the lifecycle of account instances:

- **Deploys Account Contracts** — Creates a new contract instance for each user
- **Maintains Registry** — Tracks all deployed accounts and their owners
- **Enables Discovery** — Allows applications to locate a user's account
- **Links Delegation State to Registry** — Accounts are registered on-chain so delegate-enabled contracts can discover account metadata through the shared registry
- **Stores Account Metadata** — Associates version, description, and author information with each registered account

#### Account Contract (`mux-account`)

Each user has a dedicated smart contract account that:

- **Holds Owner Address** — The original account owner (can be a Stellar account or contract)
- **Manages Delegates** — Tracks temporary signing authorities with expiration
- **Enforces Spend Limits** — Per-asset spending constraints with time windows
- **Stores Guardians** — Recovery mechanism through guardian approval
- **Manages Session Keys** — Stores session key records with scope and expiration

#### Session Key Registry

Session keys are stored with metadata:

```
SessionKey(owner: Address, session_key: Address) -> SessionKeyRecord {
  expires_at: u64,
  scopes: Vec<Scope>,  // Method names the key can call
  revoked: bool,
}

SessionKeyIndex(owner: Address) -> Vec<Address>  // Quick lookup of all keys
```

### Flow: Session Key Signed Transaction

A typical session-key-signed transaction flow:

```
┌─────────────┐
│   Client    │  1. Requests action via app
│  (Browser)  │
└──────┬──────┘
       │
       │ 2. Sign with session key
       ▼
┌─────────────┐
│    App      │  3. Create transaction payload
│ (Relayer)   │     targeting account contract
└──────┬──────┘
       │
       │ 4. Call execute_with_session(
       │    session_key, payload)
       ▼
┌──────────────────────────┐
│   Account Contract       │  5. Validate:
│ (execute_with_session)   │    - Session key exists
│                          │    - Not expired
│                          │    - Not revoked
│                          │    - Caller authorized
└──────┬───────────────────┘
       │
       │ 6. Execute payload
       │    (e.g., call PaymentProcessor.pay)
       ▼
┌──────────────────────────┐
│  PaymentProcessor or      │
│  Other Target Contract    │
└──────────────────────────┘
```

### Session Key Lifecycle

1. **Registration** — Account owner calls `register_session_key(session_key, expires_at, scopes)`
   - Stores the key record
   - Adds key to owner's index
   - Initialized with `revoked = false`

2. **Usage** — App uses key to sign and submit transactions via `execute_with_session()`
   - Key must exist and be in the SessionKeyIndex
   - Current timestamp must be < expires_at
   - revoked flag must be false

3. **Revocation** — Account owner calls `revoke_session_key(session_key)`
   - Sets `revoked = true`
   - Key remains in storage but can no longer be used
   - Index is not updated (for audit trail)

4. **Expiration** — Automatically handled by `is_session_key_valid()` check
   - Old keys remain in storage (can be pruned later)
   - No revocation action needed

## Current Implementation Status

### In Scope (Phase 1)

- [x] Account factory contract for deployment
- [x] Account contract with owner and delegate management
- [x] Spend limit enforcement
- [x] Guardian set storage
- [x] Session key storage data structures
- [x] Session key registration, revocation, and validation functions
- [x] Unit tests for session key functionality
- [x] Storage design documentation

### Deferred (Phase 2+)

- [ ] `execute_with_session()` function implementation (transaction execution)
- [ ] Guardian-based recovery mechanism
- [ ] Batch transaction execution via session keys
- [ ] Relayer sponsorship and gas abstraction
- [ ] Off-chain signature aggregation
- [ ] Multi-signature authorization policies
- [ ] Interaction with PaymentProcessor integration
- [ ] Interaction with MerchantRegistry integration

## Integration with Existing Contracts

### PaymentProcessor

The account abstraction layer sits between users and the PaymentProcessor:

```
User Account → Session Key Auth → Account Contract → PaymentProcessor
```

Session-key-authenticated transactions can:
- Call `PaymentProcessor.pay()` on behalf of the user
- Enforce per-payment spend limits
- Require guardian approval for large payments (future)

### MerchantRegistry

Merchant accounts can use AA for:
- Delegating payment collection to relayers
- Creating session keys for point-of-sale systems
- Enforcing merchant-specific spend limits

Integration is planned for Phase 2 pending completion of `execute_with_session()`.

## Storage Layout

### Account Factory DataKey Variants

```rust
DataKey::Accounts(owner)                    // Vec<Address> of deployed accounts per owner
DataKey::AccountCount                       // Total accounts registered across all owners
DataKey::Metadata(owner, account_address)   // AccountMetadata for a specific account
```

### Account Contract DataKey Variants

```rust
DataKey::Owner                              // Account owner address
DataKey::Delegates                          // Map<Address, DelegateInfo>
DataKey::SpendLimit(asset: Address)        // SpendLimit record per asset
DataKey::GuardianSet                        // Vec<Guardian addresses>
DataKey::Nonce                              // Transaction counter
DataKey::SessionKey(owner, session_key)    // SessionKeyRecord
DataKey::SessionKeyIndex(owner)             // Vec<session key addresses>
```

### Record Structures

#### Account Factory

```rust
struct AccountMetadata {
  version: String,      // Semantic version string, e.g. "1.2.0"
  description: String,  // Short human-readable description
  author: String,       // Author or team identifier
}
```

#### Account Contract

```rust
struct SpendLimit {
  asset: Address,
  amount: i128,
  period_ledgers: u32,
  spent: i128,
  reset_ledger: u32,
}

struct DelegateInfo {
  address: Address,
  expiry_ledger: u32,
  can_spend: bool,
}

struct SessionKeyRecord {
  expires_at: u64,
  scopes: Vec<Scope>,
  revoked: bool,
}

struct Scope {
  method: Symbol,  // e.g., "pay", "transfer"
}
```

## API Reference

### Account Factory Public Functions

#### `deploy_account(owner, account_address) -> Result<Address, Error>`

Register a new account for the given owner.

**Parameters:**
- `owner` — Account owner (must be authenticated)
- `account_address` — Address of the deployed account contract

**Returns:** Ok with account address if successful, Err if unauthorized or invalid

#### `deploy_account_with_metadata(owner, account_address, version, description, author) -> Result<Address, Error>`

Register a new account for the given owner with associated metadata.

**Parameters:**
- `owner` — Account owner (must be authenticated)
- `account_address` — Address of the deployed account contract
- `version` — Semantic version string (e.g., "1.0.0")
- `description` — Human-readable description of the account
- `author` — Author or team identifier

**Returns:** Ok with account address if successful, Err if unauthorized or invalid

#### `get_account_metadata(owner, account_address) -> Result<AccountMetadata, Error>`

Retrieve metadata for a specific registered account.

**Parameters:**
- `owner` — Account owner
- `account_address` — Address of the account contract

**Returns:** Ok with AccountMetadata if found, Err(MetadataNotFound) if not found

#### `get_accounts(owner) -> Vec<Address>`

Get all accounts registered for a given owner.

**Parameters:**
- `owner` — Account owner

**Returns:** Vector of account addresses

#### `account_count() -> u64`

Get the total count of registered accounts across all owners.

**Returns:** Total number of registered accounts

### Account Contract Public Functions

#### `register_session_key(owner, session_key, expires_at, scopes) -> Result<(), Error>`

Register a new session key for the account.

**Parameters:**
- `owner` — Account owner (must be authenticated)
- `session_key` — Address of the session key
- `expires_at` — Ledger timestamp at which the key expires
- `scopes` — Vec of allowed method names

**Returns:** Ok if successful, Err if unauthorized or invalid

#### `revoke_session_key(owner, session_key) -> Result<(), Error>`

Revoke an existing session key.

**Parameters:**
- `owner` — Account owner (must be authenticated)
- `session_key` — Address of the session key to revoke

**Returns:** Ok if successful, Err if not found or unauthorized

#### `is_session_key_valid(owner, session_key) -> Result<bool, Error>`

Check if a session key is valid and usable.

**Parameters:**
- `owner` — Account owner
- `session_key` — Address of the session key

**Returns:** Ok(true) if valid, Ok(false) if expired/revoked/not found, Err on access errors

## Testing

Unit tests cover:

- Session key registration and index updates
- Expiration validation (comparing current timestamp vs expires_at)
- Revocation state (revoked flag prevents use)
- Key lookup failures for non-existent keys
- Multiple keys per owner

Run tests with:
```bash
cargo test --package mux-account
```

## Future Enhancements

1. **Transaction Execution** — Implement `execute_with_session()` to actually run authorized transactions
2. **Batch Operations** — Multiple transactions in one session key use
3. **Conditional Authorization** — Time-based or threshold-based spending approval
4. **Key Rotation** — Automatic or explicit key retirement and replacement
5. **Audit Trail** — Immutable record of all session key operations
6. **Recovery Flows** — Guardian-based account recovery mechanisms

## References

- [Soroban Documentation](https://developers.stellar.org/soroban)
- [Mux Protocol Whitepaper](https://mux.cash)
- [ERC-4337 (Ethereum Account Abstraction)](https://eips.ethereum.org/EIPS/eip-4337)
