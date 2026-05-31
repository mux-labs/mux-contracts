/*!
 * mux-account-factory: Account factory for deploying account abstraction instances.
 *
 * Provides a factory contract that deploys new MuxAccount instances and
 * maintains a registry of deployed accounts per owner.
 */

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Vec,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Map of owner -> Vec<deployed account addresses>
    Accounts(Address),
    /// Counter for total accounts deployed
    AccountCount,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxAccountFactoryError {
    Unauthorized = 1,
    InvalidAccount = 2,
}

impl From<soroban_sdk::Error> for MuxAccountFactoryError {
    fn from(_: soroban_sdk::Error) -> Self {
        MuxAccountFactoryError::Unauthorized
    }
}

impl From<&soroban_sdk::Error> for MuxAccountFactoryError {
    fn from(_: &soroban_sdk::Error) -> Self {
        MuxAccountFactoryError::Unauthorized
    }
}

impl Into<soroban_sdk::Error> for MuxAccountFactoryError {
    fn into(self) -> soroban_sdk::Error {
        soroban_sdk::Error::from((
            soroban_sdk::xdr::ScErrorType::WasmVm,
            soroban_sdk::xdr::ScErrorCode::InvalidInput,
        ))
    }
}

impl Into<soroban_sdk::Error> for &MuxAccountFactoryError {
    fn into(self) -> soroban_sdk::Error {
        soroban_sdk::Error::from((
            soroban_sdk::xdr::ScErrorType::WasmVm,
            soroban_sdk::xdr::ScErrorCode::InvalidInput,
        ))
    }
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxAccountFactory;

#[contractimpl]
impl MuxAccountFactory {
    /// Deploy a new account for the given owner.
    /// In production, this would involve on-chain contract creation.
    /// For testing, this demonstrates the factory pattern with registry updates.
    pub fn deploy_account(
        env: Env,
        owner: Address,
        account_address: Address,
    ) -> Result<Address, MuxAccountFactoryError> {
        // Caller must be authorized
        env.current_contract_address().require_auth();

        // Validate that the account address is not the same as the owner
        if account_address == owner {
            return Err(MuxAccountFactoryError::InvalidAccount);
        }

        // Register the new account in the owner's account list
        let mut accounts: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Accounts(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        accounts.push_back(account_address.clone());
        env.storage()
            .instance()
            .set(&DataKey::Accounts(owner), &accounts);

        // Increment account count
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AccountCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::AccountCount, &(count + 1));

        Ok(account_address)
    }

    /// Get all accounts deployed for a given owner.
    pub fn get_accounts(env: Env, owner: Address) -> Result<Vec<Address>, MuxAccountFactoryError> {
        env.storage()
            .instance()
            .get(&DataKey::Accounts(owner))
            .ok_or(MuxAccountFactoryError::InvalidAccount)
    }

    /// Get the total count of deployed accounts.
    pub fn account_count(env: Env) -> Result<u64, MuxAccountFactoryError> {
        Ok(env
            .storage()
            .instance()
            .get(&DataKey::AccountCount)
            .unwrap_or(0))
    }
}

pub mod wallet_factory_stub;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, MuxAccountFactoryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccountFactory);
        let client = MuxAccountFactoryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    #[test]
    fn test_deploy_account() {
        let (env, client, owner) = setup();
        let account_addr = Address::generate(&env);

        // Deploy a new account
        let deployed = client.deploy_account(&owner, &account_addr);

        // Verify the deployed address is returned
        assert_eq!(deployed, account_addr);
    }

    #[test]
    fn test_deployed_address_distinct_from_owner() {
        let (env, client, owner) = setup();
        let account_addr = Address::generate(&env);

        let deployed = client.deploy_account(&owner, &account_addr);
        // Verify deployed address is distinct from owner
        assert_ne!(deployed, owner);
    }

    #[test]
    fn test_account_registry_updated_after_deployment() {
        let (env, client, owner) = setup();
        let account_addr = Address::generate(&env);

        client.deploy_account(&owner, &account_addr);

        // Verify account registry is updated
        let accounts = client.get_accounts(&owner);
        assert!(accounts.len() == 1);
        assert_eq!(accounts.get(0).unwrap(), account_addr);

        // Verify account count is incremented
        let count = client.account_count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_multiple_account_deployments() {
        let (env, client, owner) = setup();

        let account1 = Address::generate(&env);
        let account2 = Address::generate(&env);

        client.deploy_account(&owner, &account1);
        client.deploy_account(&owner, &account2);

        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts.get(0).unwrap(), account1);
        assert_eq!(accounts.get(1).unwrap(), account2);

        let count = client.account_count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_invalid_account_same_as_owner() {
        let (_env, client, owner) = setup();

        // Try to deploy account with same address as owner
        let result = client.try_deploy_account(&owner, &owner);
        assert!(result.is_err());
    }
}
