# Account Abstraction (AA) Milestone Roadmap

This document outlines the planned milestones for implementing Account Abstraction (AA) in the Mux Protocol.

## Phase 1: Foundational AA Structures (Current)
- [x] Scaffold smart-wallet contract (`mux-account`)
- [x] Add wallet factory contract stub (`mux-account-factory`)
- [x] Spend limit enforcement
- [x] Guardian set storage
- [x] Session key registration and data structures

## Phase 2: Transaction Execution & Relay (Complete)
- [x] Implement `execute_with_session()` transaction logic
- [x] Add relayer sponsorship logic and gas abstraction
- [x] Build basic frontend integration examples for session keys
- [x] Publish documentation on integrating with the relayer network

`execute_with_session(session_key, target, function, args)` now dispatches to
`target` under the account's authorization while the reentrancy guard is held,
and matches `function` against the session key's granted `scopes` fail-closed.
`execute_with_session_sponsored` adds the relayer path, gated by the
owner-managed allowlist (`set_sponsor` / `is_sponsor`). See
[relayer-integration.md](relayer-integration.md) and
[`examples/session-key-usage.ts`](../examples/session-key-usage.ts).

## Phase 3: Advanced Authentication
- [ ] Multi-signature authorization policies (n-of-m)
- [ ] Off-chain signature aggregation support
- [ ] Guardian-based account recovery mechanisms
- [ ] Hardware wallet session key delegation

## Phase 4: Integrations & Scaling
- [ ] PaymentProcessor integration for merchant checkouts
- [ ] Batch transaction execution support
- [ ] Rate-limited sub-accounts for connected devices
- [ ] Mainnet deployment and public audit
