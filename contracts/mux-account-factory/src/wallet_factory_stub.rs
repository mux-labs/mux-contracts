#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

#[contract]
pub struct WalletFactoryStub;

#[contractimpl]
impl WalletFactoryStub {
    /// Stub entry point — real deployment is handled by the Stellar CLI / host.
    /// Returns the provided `owner` address as a placeholder until on-chain
    /// contract-creation primitives are available in the Soroban SDK.
    pub fn deploy_wallet(_env: Env, owner: Address, _salt: BytesN<32>) -> Address {
        owner.require_auth();
        owner
    }
}
