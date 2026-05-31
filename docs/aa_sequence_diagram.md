# Account Abstraction (AA) Sequence Diagram

This sequence diagram illustrates the typical lifecycle of an ERC-4337 transaction within the Mux Protocol.

```mermaid
sequenceDiagram
    participant User as User / Application
    participant Bundler as Bundler Network
    participant EP as EntryPoint Contract
    participant Wallet as Smart Wallet Contract
    participant Paymaster as Paymaster Contract

    User->>Bundler: Submit UserOperation
    Bundler->>EP: handleOps([UserOp])
    
    rect rgb(200, 220, 240)
        Note over EP, Paymaster: Validation Phase
        EP->>Wallet: validateUserOp(UserOp, hash, missingFunds)
        Wallet-->>EP: sigTimeRange / validation success
        
        opt If Paymaster specified
            EP->>Paymaster: validatePaymasterUserOp(UserOp, hash, maxCost)
            Paymaster-->>EP: context, sigTimeRange
        end
    end
    
    rect rgb(220, 240, 200)
        Note over EP, Wallet: Execution Phase
        EP->>Wallet: execute(dest, value, callData)
        Wallet-->>EP: Execution result
        
        opt If Paymaster specified
            EP->>Paymaster: postOp(mode, context, actualGasCost)
        end
    end
    
    EP-->>Bundler: Transaction receipt
```

## Overview
1. **Submission**: The user submits a signed `UserOperation` to the bundler network.
2. **Validation**: The EntryPoint contract first asks the Smart Wallet to validate the signature and the nonce. If a Paymaster is involved, it is also queried to ensure it is willing to sponsor the transaction.
3. **Execution**: The EntryPoint executes the specific calldata on the Smart Wallet. Post-operation logic runs on the Paymaster if applicable.
