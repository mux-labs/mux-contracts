# ABI Reference

This page provides references to the Application Binary Interfaces (ABIs) of the key Mux Protocol contracts.

## Smart Wallet
The smart wallet implementation follows the ERC-4337 standard.

### Key Methods
- `execute(address dest, uint256 value, bytes calldata func)`: Executes a transaction.
- `executeBatch(address[] calldata dest, uint256[] calldata value, bytes[] calldata func)`: Executes a batch of transactions.
- `validateUserOp(UserOperation calldata userOp, bytes32 userOpHash, uint256 missingAccountFunds)`: Validates a user operation signature and nonce.

## Wallet Factory
The factory contract handles deterministic deployments.

### Key Methods
- `createAccount(address owner, uint256 salt)`: Deploys a new Smart Wallet instance if one does not already exist at the computed address.
- `getAddress(address owner, uint256 salt)`: Computes the address of a smart wallet prior to deployment.

## Extensibility
Please refer to the source code under the `contracts/` directory for the full JSON ABIs, which are generated during compilation.
