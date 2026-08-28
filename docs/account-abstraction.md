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
       │    session_key, target, function, args)
       ▼
┌──────────────────────────┐
│   Account Contract       │  5. Validate:
│ (execute_with_session)   │    - Session key exists
│                          │    - Not expired
│                          │    - Not revoked
│                          │    - Caller authorized
│                          │    - function is in scopes
└──────┬───────────────────┘
       │
       │ 6. Invoke target.function(args)
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
   - `scopes` must be non-empty and must name the invoked method
   - `nonce` must equal the account's current `nonce()`, which then advances
   - A relayer may submit and pay instead via `execute_with_session_sponsored()`

3. **Revocation** — Account owner calls `revoke_session_key(session_key)`
   - Sets `revoked = true`
   - Removes the key from the owner's `SessionKeyIndex` so cap accounting
     and indexers stay in sync
   - The `SessionKey` record itself remains in storage but can no longer be used

4. **Expiration** — Checked in `execute_with_session` and via the
   `is_session_key_valid()` query
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

### In Scope (Phase 2)

- [x] `execute_with_session()` transaction execution (dispatches to a target contract)
- [x] Per-method scope enforcement, fail-closed on empty and unlisted scopes
- [x] Relayer sponsorship and gas abstraction (`set_sponsor`, `execute_with_session_sponsored`)
- [x] Frontend integration example ([`examples/session-key-usage.ts`](../examples/session-key-usage.ts))
- [x] Relayer network documentation ([relayer-integration.md](relayer-integration.md))

### Deferred (Phase 3+)

- [ ] Guardian-based recovery mechanism
- [ ] Batch transaction execution via session keys
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

Integration is planned for Phase 3 now that `execute_with_session()` dispatches.

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
DataKey::Nonce                              // Transaction counter; checked and advanced by every execution entrypoint
DataKey::SessionKey(owner, session_key)    // SessionKeyRecord
DataKey::SessionKeyIndex(owner)             // Vec<session key addresses>
DataKey::Metadata                           // Optional RegistryMeta for this account instance
DataKey::Sponsor(relayer: Address)          // Relayer gas-sponsorship allowlist entry
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
  expires_at: u64,
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

struct SessionExecutedEvent {
  session_key: Address,
  target: Address,
  function: Symbol,
  sponsor: Option<Address>,  // Some(relayer) when the call was sponsored
}

struct RegistryMeta {
  name: String,         // Human-readable instance name
  version: String,      // Semantic version string, e.g. "1.2.0"
  description: String,  // Optional free-form notes
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
- `version` — Semantic version string (e.g., "1.0.0"), max 32 characters
- `description` — Human-readable description of the account, max 256 characters
- `author` — Author or team identifier, max 64 characters

**Returns:** Ok with account address if successful, Err if unauthorized or invalid

**Errors:**
- `Unauthorized` — Caller is not the owner
- `InvalidAccount` — account_address equals owner
- `TooManyAccounts` — Owner has reached the 64 account cap
- `MetadataTooLarge` — Any metadata string exceeds its maximum length

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

#### `set_metadata(meta) -> Result<(), Error>`

Store registry-level metadata for this account instance. Owner only.

**Parameters:**
- `meta` — `RegistryMeta` with name, version, and description

**Returns:** Ok if successful, Err if not initialized or unauthorized

#### `get_metadata() -> Option<RegistryMeta>`

Return the stored registry metadata, or `None` if not set.

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

#### `nonce() -> Result<u64, Error>`

Return the account's current transaction nonce. Every execution entrypoint
requires the caller to pass exactly this value, and advances it by one on
success.

#### `execute_with_session(session_key, target, function, args, nonce) -> Result<Val, Error>`

Dispatch a call to `target` under the account's authority, authorized by a
session key instead of the owner.

**Parameters:**
- `session_key` — Address of the session key (must be authenticated)
- `target` — Contract to invoke
- `function` — Method on `target`; must be named in the key's `scopes`
- `args` — Arguments forwarded verbatim to `target`
- `nonce` — Must equal the account's current `nonce()`

**Returns:** Ok with the target's return value

**Errors:**
- `Unauthorized` — Key is unknown, revoked, expired, or was granted no scopes
- `ScopeNotGranted` — `function` is not named in the key's `scopes`
- `ReentrancyDetected` — A call is already in flight on this account
- `InvalidNonce` — `nonce` is not the account's current nonce

#### `execute_with_session_sponsored(session_key, sponsor, target, function, args, nonce) -> Result<Val, Error>`

Same as `execute_with_session`, but submitted and paid for by an allowlisted
relayer. Both `sponsor` and `session_key` must authenticate. See
[relayer-integration.md](relayer-integration.md).

**Errors:** as above, plus `SponsorNotAuthorized` when the relayer is not on the
account's sponsor allowlist.

#### `set_sponsor(sponsor, allowed) -> Result<(), Error>`

Add (`allowed = true`) or remove (`allowed = false`) a relayer from the
gas-sponsorship allowlist. Owner only.

#### `is_sponsor(sponsor) -> bool`

Return whether `sponsor` may currently relay session calls for this account.

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

## Usage Examples

See [`examples/account-factory-usage.ts`](../examples/account-factory-usage.ts) for a complete TypeScript example demonstrating:

- Deploying accounts with and without metadata
- Retrieving accounts for an owner
- Fetching account metadata
- Getting total account count

The example uses the TypeScript bindings from `@mux-protocol/contracts` and supports localnet, testnet, and mainnet environments.

## Future Enhancements

1. **Batch Operations** — Multiple transactions in one session key use
2. **Conditional Authorization** — Time-based or threshold-based spending approval
3. **Key Rotation** — Automatic or explicit key retirement and replacement
4. **Audit Trail** — Immutable record of all session key operations
5. **Recovery Flows** — Guardian-based account recovery mechanisms
6. **Target-scoped Sessions** — Scopes currently match method names only, not target addresses

## References

- [Soroban Documentation](https://developers.stellar.org/soroban)
- [Mux Protocol Whitepaper](https://mux.cash)
- [ERC-4337 (Ethereum Account Abstraction)](https://eips.ethereum.org/EIPS/eip-4337)
