# Mux Protocol Contracts

This directory contains the smart contracts for the Mux Protocol, a comprehensive account management and permission system for Stellar.

## Contracts

### Core Contracts

- **mux-account**: Smart account implementation with session management
- **mux-account-factory**: Factory contract for creating new Mux accounts
- **mux-permissions**: Role-based access control (RBAC) system
- **mux-registry**: Contract registry for protocol management
- **mux-batcher**: Batch transaction processing

### New Contracts

- **mux-recovery**: Account recovery system for compromised or lost accounts
- **mux-delegation**: Delegation system for permissions and voting power

## mux-recovery

The recovery contract provides a secure mechanism for account recovery when accounts are compromised or access is lost.

### Features

- **Recovery Request Struct**: Comprehensive tracking of recovery requests with metadata
- **Admin Approval System**: Only authorized administrators can approve recovery requests
- **Event Emission**: All recovery actions emit events for transparency and auditability
- **Request Management**: Track pending and completed recovery requests

### Key Functions

- `initialize(admin)`: Initialize the contract with an admin
- `request_recovery(old_account, new_account)`: Submit a recovery request
- `approve_recovery(request_id)`: Admin function to approve recovery requests
- `get_recovery_request(request_id)`: Retrieve recovery request details
- `get_pending_requests()`: List all pending recovery requests

### Events

- `init`: Contract initialization
- `req_sub`: Recovery request submitted
- `req_app`: Recovery request approved

## mux-delegation

The delegation contract enables accounts to delegate specific permissions or voting power to other accounts.

### Features

- **Delegation Management**: Grant and revoke delegations with specific permissions
- **Event Emission**: Emits `delegate_granted` events as required
- **Permission Checking**: Verify delegated permissions
- **Delegation Limits**: Prevents storage griefing with reasonable limits
- **Bidirectional Tracking**: Track both delegators and delegates

### Key Functions

- `initialize(admin)`: Initialize the contract with an admin
- `grant_delegation(delegator, delegate, permissions)`: Grant delegation with specific permissions
- `revoke_delegation(delegator, delegate)`: Revoke an existing delegation
- `has_delegation(delegator, delegate)`: Check if delegation exists and is active
- `has_delegated_permission(delegator, delegate, permission)`: Check specific permission
- `get_delegates(delegator)`: Get all delegates for an account
- `get_delegators(delegate)`: Get all delegators for an account

### Events

- `init`: Contract initialization
- `del_grant`: Delegation granted (the required `delegate_granted` event)
- `del_revok`: Delegation revoked

## Security Features

Both contracts implement:

- **Storage TTL Management**: Automatic TTL extension to prevent data loss
- **Access Control**: Proper authorization checks
- **Storage Griefing Protection**: Limits on data structures to prevent abuse
- **Comprehensive Testing**: Full test coverage for all functionality
- **Event Emission**: Transparent logging of all actions

## Testing

Run tests for individual contracts:

```bash
cargo test --package mux-recovery
cargo test --package mux-delegation
```

Or test all contracts:

```bash
cargo test
```

## Integration

These contracts follow the same patterns as existing Mux Protocol contracts:

- Consistent error handling with custom error types
- Soroban SDK best practices
- Storage optimization with TTL management
- Comprehensive event emission for auditability
- Modular design for easy integration