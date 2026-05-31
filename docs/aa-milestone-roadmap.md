# Account Abstraction (AA) Milestone Roadmap

This document outlines the planned milestones for implementing Account Abstraction (AA) in the Mux Protocol.

## Phase 1: Foundational AA Structures (Current)
- [x] Scaffold smart-wallet contract (`mux-account`)
- [x] Add wallet factory contract stub (`mux-account-factory`)
- [x] Spend limit enforcement
- [x] Guardian set storage
- [x] Session key registration and data structures

## Phase 2: Transaction Execution & Relay
- [ ] Implement `execute_with_session()` transaction logic
- [ ] Add relayer sponsorship logic and gas abstraction
- [ ] Build basic frontend integration examples for session keys
- [ ] Publish documentation on integrating with the relayer network

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
