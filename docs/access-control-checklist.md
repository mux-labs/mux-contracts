# Access-Control Checklist for Mutating Entrypoints

This checklist enumerates every mutating entrypoint exposed by the Mux
contracts and the authorization required to invoke it. It is enforced by an
automated test (`bindings/__tests__/access-control-checklist.test.ts`) that
scans the generated bindings and fails closed: any mutating entrypoint that is
not mapped to a documented authorization requirement fails the suite.

## Invariants

1. **Deny by default.** A mutating entrypoint with no documented authorization
   mapping is treated as unguarded and fails CI.
2. **Server/contract is the source of truth.** Authorization is enforced
   on-chain; the bindings only describe the required role so clients cannot
   bypass policy.
3. **No secrets.** Authorization is expressed as roles/keys, never as raw key
   material, JWTs, or webhook secrets.
4. **Fail closed on writes.** If an authorization check cannot be evaluated
   (missing role, expired delegate, revoked guardian), the write is rejected.
5. **Session scopes are enforced.** `execute_with_session` decodes the
   payload's target contract/method and rejects the call unless the session
   key's `scopes` explicitly authorize that target. Unauthorized targets fail
   closed with `ScopeNotAllowed`; an empty scope list authorizes nothing.

## Mutating entrypoints and required authorization

| Entrypoint | Contract | Required authorization |
| --- | --- | --- |
| `register` | mux-registry | owner |
| `update` | mux-registry | owner |
| `deregister` | mux-registry | owner |
| `set_version` | mux-registry | owner |
| `register_wallet` | mux-wallet-registry | owner |
| `update_wallet` | mux-wallet-registry | owner |
| `deregister_wallet` | mux-wallet-registry | owner |
| `grant_role` | mux-permissions | owner |
| `revoke_role` | mux-permissions | owner |
| `set_delegate` | mux-permissions | owner |
| `revoke_delegate` | mux-permissions | owner |
| `set_guardian` | mux-permissions | owner |
| `revoke_guardian` | mux-permissions | owner |
| `initialize` | mux-account | public (one-time init) |
| `set_delegate` | mux-account | owner |
| `remove_delegate` | mux-account | owner |
| `set_spend_limit` | mux-account | owner |
| `execute_with_session` | mux-account | session key with matching scope |
| `set_registry_metadata` | mux-batcher | owner |
| `initialize` | mux-permissions | public (one-time init) |
| `set_admin_threshold` | mux-permissions | owner |
| `initialize` | mux-registry | public (one-time init) |
| `register_with_metadata` | mux-registry | owner |
| `initialize` | mux-spending-policy | public (one-time init) |
| `set_policy` | mux-spending-policy | owner |
| `remove_allowlisted_asset` | mux-spending-policy | owner or delegate |
| `set_relayer_sponsorship_limit` | mux-spending-policy | owner or delegate |
| `initialize` | mux-wallet-registry | public (one-time init) |
| `register_wallet_with_metadata` | mux-wallet-registry | owner |

The mux-account rows are mirrored by the machine-readable
`MUX_ACCOUNT_MUTATING_AUTHZ` map in `bindings/src/generated/mux-account.ts`;
the checklist test enumerates `MuxAccountClient.prototype` and fails if the
map and the client drift apart in either direction. Read-only methods (such
as `owner`, a simulation-only view function) are excluded by an explicit
allowlist in the test, not by omission.

## Adding a new mutating entrypoint

1. Add the entrypoint to the table above with its required authorization.
2. Regenerate the bindings.
3. Run `bindings/__tests__/access-control-checklist.test.ts`; it will fail
   until the new entrypoint is mapped.

## References

- `SECURITY.md`
- `bindings/src/generated/mux-permissions.ts`
