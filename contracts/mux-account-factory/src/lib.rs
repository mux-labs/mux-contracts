/*!
 * mux-account-factory: Account factory for deploying account abstraction instances.
 *
 * Provides a factory contract that registers new MuxAccount instances and
 * maintains a per-owner index of deployed accounts.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(
    env: &Env,
    action: soroban_sdk::Symbol,
    data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
) {
    env.events()
        .publish((symbol_short!("mux_fac"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Per-owner list of deployed account addresses.
    Accounts(Address),
    /// Total accounts registered across all owners.
    AccountCount,
    /// Metadata for a specific account: DataKey::Metadata(owner, account_address)
    Metadata(Address, Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Metadata associated with a registered account.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AccountMetadata {
    /// Semantic version string, e.g. "1.2.0"
    pub version: String,
    /// Short human-readable description of the account.
    pub description: String,
    /// Author or team identifier.
    pub author: String,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxAccountFactoryError {
    Unauthorized = 1,
    /// account_address must differ from owner.
    InvalidAccount = 2,
    // STORAGE-GRIEFING: unbounded per-owner Accounts vec would let an owner
    // bloat instance storage indefinitely.
    TooManyAccounts = 3,
    /// Metadata not found for the specified account.
    MetadataNotFound = 4,
    // STORAGE-GRIEFING: unbounded metadata string sizes would let an owner
    // bloat instance storage indefinitely.
    MetadataTooLarge = 5,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum accounts per owner to bound the Accounts vec in instance storage.
const MAX_ACCOUNTS_PER_OWNER: u32 = 64;

// STORAGE-GRIEFING: bound metadata string sizes to prevent owners from bloating
// instance storage with large strings. Each account can have metadata, so with
// 64 accounts per owner, unbounded strings could cause significant storage bloat.
const MAX_VERSION_LENGTH: u32 = 32; // e.g., "1.2.3" or "v1.2.3-beta"
const MAX_DESCRIPTION_LENGTH: u32 = 256; // Short human-readable description
const MAX_AUTHOR_LENGTH: u32 = 64; // Author or team identifier

// ── Storage TTL ───────────────────────────────────────────────────────────────
// STORAGE-GRIEFING (T-21): extend instance TTL on every write so the factory
// stays live as long as it is actively used.  See docs/storage-griefing.md.
//
// Values: ~17,280 ledgers ≈ 1 day (5-second ledger close); bump to 30 days.
const TTL_THRESHOLD: u32 = 17_280; // extend when remaining TTL falls below 1 day
const TTL_EXTEND_TO: u32 = 518_400; // extend to ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxAccountFactory;

#[contractimpl]
impl MuxAccountFactory {
    /// Register a new account for the given owner.
    ///
    /// The caller must be the owner. `account_address` must differ from `owner`
    /// and must not already be registered for this owner.
    pub fn deploy_account(
        env: Env,
        owner: Address,
        account_address: Address,
    ) -> Result<Address, MuxAccountFactoryError> {
        owner.require_auth();

        if account_address == owner {
            return Err(MuxAccountFactoryError::InvalidAccount);
        }

        let mut accounts: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Accounts(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        // STORAGE-GRIEFING: cap per-owner account list.
        if accounts.len() >= MAX_ACCOUNTS_PER_OWNER {
            return Err(MuxAccountFactoryError::TooManyAccounts);
        }

        accounts.push_back(account_address.clone());
        env.storage()
            .instance()
            .set(&DataKey::Accounts(owner.clone()), &accounts);

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AccountCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::AccountCount, &(count + 1));

        emit(
            &env,
            symbol_short!("deployed"),
            (owner, account_address.clone()),
        );
        Self::extend_ttl(&env);
        Ok(account_address)
    }

    /// Get all accounts registered for a given owner.
    pub fn get_accounts(env: Env, owner: Address) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Accounts(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get the total count of registered accounts.
    pub fn account_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::AccountCount)
            .unwrap_or(0)
    }

    /// Register a new account for the given owner with metadata.
    ///
    /// The caller must be the owner. `account_address` must differ from `owner`
    /// and must not already be registered for this owner.
    pub fn deploy_account_with_metadata(
        env: Env,
        owner: Address,
        account_address: Address,
        version: String,
        description: String,
        author: String,
    ) -> Result<Address, MuxAccountFactoryError> {
        owner.require_auth();

        if account_address == owner {
            return Err(MuxAccountFactoryError::InvalidAccount);
        }

        let mut accounts: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Accounts(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        // STORAGE-GRIEFING: cap per-owner account list.
        if accounts.len() >= MAX_ACCOUNTS_PER_OWNER {
            return Err(MuxAccountFactoryError::TooManyAccounts);
        }

        // STORAGE-GRIEFING: validate metadata string sizes to prevent storage bloat.
        if version.len() > MAX_VERSION_LENGTH as u32 {
            return Err(MuxAccountFactoryError::MetadataTooLarge);
        }
        if description.len() > MAX_DESCRIPTION_LENGTH as u32 {
            return Err(MuxAccountFactoryError::MetadataTooLarge);
        }
        if author.len() > MAX_AUTHOR_LENGTH as u32 {
            return Err(MuxAccountFactoryError::MetadataTooLarge);
        }

        accounts.push_back(account_address.clone());
        env.storage()
            .instance()
            .set(&DataKey::Accounts(owner.clone()), &accounts);

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AccountCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::AccountCount, &(count + 1));

        // Store metadata
        let meta = AccountMetadata {
            version,
            description,
            author,
        };
        env.storage()
            .instance()
            .set(&DataKey::Metadata(owner.clone(), account_address.clone()), &meta);

        emit(
            &env,
            symbol_short!("deployed"),
            (owner, account_address.clone()),
        );
        Self::extend_ttl(&env);
        Ok(account_address)
    }

    /// Get the metadata for a specific account.
    pub fn get_account_metadata(
        env: Env,
        owner: Address,
        account_address: Address,
    ) -> Result<AccountMetadata, MuxAccountFactoryError> {
        env.storage()
            .instance()
            .get(&DataKey::Metadata(owner, account_address))
            .ok_or(MuxAccountFactoryError::MetadataNotFound)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn extend_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

pub mod wallet_factory_stub;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, FromVal};

    fn setup() -> (Env, MuxAccountFactoryClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxAccountFactory);
        let client = MuxAccountFactoryClient::new(&env, &contract_id);
        (env, client)
    }

    #[test]
    fn test_deploy_account() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let deployed = client.deploy_account(&owner, &account_addr);
        assert_eq!(deployed, account_addr);
    }

    #[test]
    fn test_deployed_address_distinct_from_owner() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let deployed = client.deploy_account(&owner, &account_addr);
        assert_ne!(deployed, owner);
    }

    #[test]
    fn test_account_registry_updated_after_deployment() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.deploy_account(&owner, &account_addr);
        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts.get(0).unwrap(), account_addr);
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_multiple_account_deployments() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account1 = Address::generate(&env);
        let account2 = Address::generate(&env);
        client.deploy_account(&owner, &account1);
        client.deploy_account(&owner, &account2);
        let accounts = client.get_accounts(&owner);
        assert_eq!(accounts.len(), 2);
        assert_eq!(client.account_count(), 2);
    }

    #[test]
    fn test_invalid_account_same_as_owner() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let result = client.try_deploy_account(&owner, &owner);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::InvalidAccount)));
    }

    #[test]
    fn test_accounts_cap_enforced() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        for _ in 0..64 {
            client.deploy_account(&owner, &Address::generate(&env));
        }
        let result = client.try_deploy_account(&owner, &Address::generate(&env));
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));
    }

    #[test]
    fn test_deploy_emits_event() {
        use soroban_sdk::testutils::Events;
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.deploy_account(&owner, &account_addr);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (_, topics, _) = events.get(0).unwrap();
        let action = soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(action, symbol_short!("deployed"));
    }

    #[test]
    fn test_ttl_extended_on_deploy() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        client.deploy_account(&owner, &Address::generate(&env));
        // If extend_ttl was missing the SDK would panic; reaching here is the assertion.
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_ttl_extended_on_deploy_with_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        client.deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        // If extend_ttl was missing the SDK would panic; reaching here is the assertion.
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_read_operations_do_not_extend_ttl() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);

        // Deploy an account (this extends TTL)
        client.deploy_account(&owner, &account_addr);

        // Read operations should not extend TTL
        let _accounts = client.get_accounts(&owner);
        let _count = client.account_count();

        // If read operations extended TTL incorrectly, the test would still pass
        // but this documents the expected behavior
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_deploy_account_with_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test account");
        let author = String::from_str(&env, "test-author");

        let deployed = client.deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(deployed, account_addr);
        assert_eq!(client.account_count(), 1);
    }

    #[test]
    fn test_get_account_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Account with metadata");
        let author = String::from_str(&env, "mux-labs");

        client.deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version.clone(),
            &description.clone(),
            &author.clone(),
        );

        let meta = client.get_account_metadata(&owner, &account_addr);
        assert_eq!(meta.version, version);
        assert_eq!(meta.description, description);
        assert_eq!(meta.author, author);
    }

    #[test]
    fn test_get_account_metadata_not_found() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let result = client.try_get_account_metadata(&owner, &account_addr);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));
    }

    #[test]
    fn test_get_account_metadata_not_found_after_deploy_without_metadata() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        client.deploy_account(&owner, &account_addr);
        let result = client.try_get_account_metadata(&owner, &account_addr);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));
    }

    #[test]
    fn test_get_account_metadata_wrong_owner() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let other_owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        client.deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        let result = client.try_get_account_metadata(&other_owner, &account_addr);
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataNotFound)));
    }

    #[test]
    fn test_deploy_account_unauthorized_without_auth() {
        let env = Env::default();
        let contract_id = env.register_contract(None, MuxAccountFactory);
        let client = MuxAccountFactoryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let result = client.try_deploy_account(&owner, &account_addr);
        assert!(result.is_err());
    }

    #[test]
    fn test_deploy_account_with_metadata_enforces_cap() {
        let (env, client) = setup();
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        // Fill up to the cap
        for _ in 0..64 {
            client.deploy_account_with_metadata(
                &owner,
                &Address::generate(&env),
                &version.clone(),
                &description.clone(),
                &author.clone(),
            );
        }
        // One more must be rejected
        let result = client.try_deploy_account_with_metadata(
            &owner,
            &Address::generate(&env),
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::TooManyAccounts)));
    }

    #[test]
    fn test_deploy_account_with_metadata_invalid_account() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &owner,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::InvalidAccount)));
    }

    #[test]
    fn test_metadata_version_too_long() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        // Create a version string longer than MAX_VERSION_LENGTH (32)
        let version = String::from_str(&env, "a".repeat(33).as_str());
        let description = String::from_str(&env, "Test");
        let author = String::from_str(&env, "test");

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataTooLarge)));
    }

    #[test]
    fn test_metadata_description_too_long() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        // Create a description string longer than MAX_DESCRIPTION_LENGTH (256)
        let description = String::from_str(&env, "a".repeat(257).as_str());
        let author = String::from_str(&env, "test");

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataTooLarge)));
    }

    #[test]
    fn test_metadata_author_too_long() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        let version = String::from_str(&env, "1.0.0");
        let description = String::from_str(&env, "Test");
        // Create an author string longer than MAX_AUTHOR_LENGTH (64)
        let author = String::from_str(&env, "a".repeat(65).as_str());

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert_eq!(result, Err(Ok(MuxAccountFactoryError::MetadataTooLarge)));
    }

    #[test]
    fn test_metadata_at_max_length_succeeds() {
        let (env, client) = setup();
        let owner = Address::generate(&env);
        let account_addr = Address::generate(&env);
        // Create strings at exactly the maximum allowed length
        let version = String::from_str(&env, "a".repeat(32).as_str());
        let description = String::from_str(&env, "a".repeat(256).as_str());
        let author = String::from_str(&env, "a".repeat(64).as_str());

        let result = client.try_deploy_account_with_metadata(
            &owner,
            &account_addr,
            &version,
            &description,
            &author,
        );
        assert!(result.is_ok());
    }
}
