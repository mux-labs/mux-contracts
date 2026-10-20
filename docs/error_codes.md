# Error Codes

This document lists the stable error codes returned by the mux-account contract
(`contracts/mux-account/src/lib.rs`, `MuxAccountError`). The bindings expose the
same table in `bindings/src/types.ts` (`muxAccountErrorMessage`), and
`bindings/__tests__/types.test.ts` asserts the two stay in sync.

| Code | Variant | Description |
|------|---------|-------------|
| 1 | `NotInitialized` | The contract has not been initialized. |
| 2 | `AlreadyInitialized` | The contract has already been initialized. |
| 3 | `Unauthorized` | The caller is not authorized to perform the requested action. |
| 4 | `DelegateNotFound` | No delegate is registered for the supplied address. |
| 5 | `DelegateExpired` | The delegate's authorization has expired. |
| 6 | `SpendLimitExceeded` | The spend would exceed the configured limit for the current period. |
| 7 | `InvalidAmount` | The supplied amount is zero, negative, or otherwise invalid. |
| 8 | `InvalidPeriod` | The supplied period is zero or otherwise out of range. |
| 9 | `TooManyDelegates` | The delegate cap has been reached. |
| 10 | `ReentrancyDetected` | A reentrant call was rejected while the reentrancy guard was held. |
| 11 | `ArithmeticOverflow` | An arithmetic operation overflowed. |
| 12 | `TooManySessionKeys` | The session-key cap has been reached. |
| 13 | `ScopeNotGranted` | The session key's scopes do not grant the requested method. `execute_with_session` fails closed with this variant when the target method is not explicitly listed in the session key's scopes (an empty scope list authorizes nothing). |
| 14 | `SponsorNotAuthorized` | The sponsor is not on the owner-managed relayer allowlist. |
| 15 | `InvalidNonce` | The supplied nonce does not match the account's current nonce. |
