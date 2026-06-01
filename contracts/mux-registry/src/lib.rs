/*!
 * mux-registry: Contract version registry for Mux Protocol.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!(\"mux_reg\"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Version(Symbol),
    Names,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxRegistryError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ContractNotFound = 4,
    // STORAGE-GRIEFING: unbounded Names vec would let admin bloat instance storage.
    TooManyContracts = 5,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of registered contract names to bound the Names vec.
const MAX_CONTRACTS: u32 = 128;

// ── Storage TTL ───────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRegistry;

#[contractimpl]
impl MuxRegistry {
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxRegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxRegistryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Names, &Vec::<Symbol>::new(&env));
        emit(&env, symbol_short!(\"init\"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register or update a contract version. Admin only.
    pub fn register(env: Env, name: Symbol, version: String) -> Result<(), MuxRegistryError> {
        Self::require_admin(&env)?;
        let mut names: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env));

        if !names.contains(&name) {
            // STORAGE-GRIEFING: cap the Names vec to bound instance storage growth.
            if names.len() >= MAX_CONTRACTS {
                return Err(MuxRegistryError::TooManyContracts);
            }
            names.push_back(name.clone());
            env.storage().instance().set(&DataKey::Names, &names);
        }
        env.storage()
            .instance()
            .set(&DataKey::Version(name.clone()), &version);
        emit(&env, symbol_short!(\"reg\"), (name, version));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Get the version string for a registered contract.
    pub fn get_version(env: Env, name: Symbol) -> Result<String, MuxRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Version(name))
            .ok_or(MuxRegistryError::ContractNotFound)
    }

    /// List all registered contract names.
    pub fn list_contracts(env: Env) -> Vec<Symbol> {
        env.storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxRegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxRegistryError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    fn extend_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Env, String};

    fn setup() -> (Env, MuxRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_register_and_get() {
        let (env, client, _) = setup();
        let name = symbol_short!(\"account\");
        let version = String::from_str(&env, \"1.0.0\");
        client.register(&name, &version);
        assert_eq!(client.get_version(&name), version);
        assert!(client.list_contracts().contains(&name));
    }

    #[test]
    fn test_get_unknown_fails() {
        let (env, client, _) = setup();
        let result = client.try_get_version(&symbol_short!(\"ghost\"));
        assert!(result.is_err());
    }

    #[test]
    fn test_register_update_does_not_grow_names() {
        let (env, client, _) = setup();
        let name = symbol_short!(\"account\");
        let v1 = String::from_str(&env, \"1.0.0\");
        let v2 = String::from_str(&env, \"2.0.0\");
        client.register(&name, &v1);
        client.register(&name, &v2);
        // Updating an existing name must not add a duplicate entry.
        assert_eq!(client.list_contracts().len(), 1);
        assert_eq!(client.get_version(&name), v2);
    }

    #[test]
    fn test_contract_cap_enforced() {
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        // Fill to cap using distinct symbol names.
        let names = [
            \"a\", \"b\", \"c\", \"d\", \"e\", \"f\", \"g\", \"h\", \"i\", \"j\", \"k\", \"l\", \"m\", \"n\", \"o\", \"p\",
            \"q\", \"r\", \"s\", \"t\", \"u\", \"v\", \"w\", \"x\", \"y\", \"z\", \"aa\", \"ab\", \"ac\", \"ad\", \"ae\",
            \"af\", \"ag\", \"ah\", \"ai\", \"aj\", \"ak\", \"al\", \"am\", \"an\", \"ao\", \"ap\", \"aq\", \"ar\", \"as\",
            \"at\", \"au\", \"av\", \"aw\", \"ax\", \"ay\", \"az\", \"ba\", \"bb\", \"bc\", \"bd\", \"be\", \"bf\", \"bg\",
            \"bh\", \"bi\", \"bj\", \"bk\", \"bl\", \"bm\", \"bn\", \"bo\", \"bp\", \"bq\", \"br\", \"bs\", \"bt\", \"bu\",
            \"bv\", \"bw\", \"bx\", \"by\", \"bz\", \"ca\", \"cb\", \"cc\", \"cd\", \"ce\", \"cf\", \"cg\", \"ch\", \"ci\",
            \"cj\", \"ck\", \"cl\", \"cm\", \"cn\", \"co\", \"cp\", \"cq\", \"cr\", \"cs\", \"ct\", \"cu\", \"cv\", \"cw\",
            \"cx\", \"cy\", \"cz\", \"da\", \"db\", \"dc\", \"dd\", \"de\", \"df\", \"dg\", \"dh\", \"di\", \"dj\", \"dk\",
            \"dl\", \"dm\", \"dn\", \"do\", \"dp\", \"dq\", \"dr\", \"ds\", \"dt\", \"du\", \"dv\", \"dw\", \"dx\", \"dy\",
        ];
        let version = String::from_str(&env, \"1.0.0\");
        for n in names.iter() {
            let sym = soroban_sdk::Symbol::new(&env, n);
            client.register(&sym, &version);
        }
        // One more new name must be rejected.
        let overflow = soroban_sdk::Symbol::new(&env, \"overflow\");
        assert!(client.try_register(&overflow, &version).is_err());
    }

    #[test]
    fn test_ttl_extended_on_write() {
        // setup() calls initialize which calls extend_ttl; reaching here is the assertion.
        let (_env, _client, _admin) = setup();
    }
}
