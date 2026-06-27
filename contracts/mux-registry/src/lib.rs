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
        .publish((symbol_short!("mux_reg"), action), data);
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Version(Symbol),
    Names,
    Metadata(Symbol),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Metadata associated with a registered contract.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ContractMetadata {
    /// Semantic version string, e.g. "1.2.0"
    pub version: String,
    /// Short human-readable description of the contract.
    pub description: String,
    /// Author or team identifier.
    pub author: String,
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
        emit(&env, symbol_short!("init"), admin);
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
        emit(&env, symbol_short!("reg"), (name, version));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Register or update a contract with full metadata. Admin only.
    pub fn register_with_metadata(
        env: Env,
        name: Symbol,
        version: String,
        description: String,
        author: String,
    ) -> Result<(), MuxRegistryError> {
        Self::require_admin(&env)?;
        let mut names: Vec<Symbol> = env
            .storage()
            .instance()
            .get(&DataKey::Names)
            .unwrap_or_else(|| Vec::new(&env));
        if !names.contains(&name) {
            if names.len() >= MAX_CONTRACTS {
                return Err(MuxRegistryError::TooManyContracts);
            }
            names.push_back(name.clone());
            env.storage().instance().set(&DataKey::Names, &names);
        }
        env.storage()
            .instance()
            .set(&DataKey::Version(name.clone()), &version.clone());
        let meta = ContractMetadata {
            version: version.clone(),
            description,
            author,
        };
        env.storage()
            .instance()
            .set(&DataKey::Metadata(name.clone()), &meta);
        emit(&env, symbol_short!("reg"), (name, version));
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

    /// Get the full metadata for a registered contract.
    pub fn get_metadata(env: Env, name: Symbol) -> Result<ContractMetadata, MuxRegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Metadata(name))
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
        let name = symbol_short!("account");
        let version = String::from_str(&env, "1.0.0");
        client.register(&name, &version);
        assert_eq!(client.get_version(&name), version);
        assert!(client.list_contracts().contains(&name));
    }

    #[test]
    fn test_get_unknown_fails() {
        let (_env, client, _) = setup();
        let result = client.try_get_version(&symbol_short!("ghost"));
        assert!(result.is_err());
    }

    #[test]
    fn test_register_with_metadata() {
        let (env, client, _) = setup();
        let name = symbol_short!("account");
        let version = String::from_str(&env, "2.0.0");
        let description = String::from_str(&env, "Account abstraction contract");
        let author = String::from_str(&env, "mux-labs");

        client.register_with_metadata(&name, &version, &description, &author);

        let meta = client.get_metadata(&name);
        assert_eq!(meta.version, version);
        assert_eq!(meta.description, description);
        assert_eq!(meta.author, author);
        // version key also updated
        assert_eq!(client.get_version(&name), version);
        assert!(client.list_contracts().contains(&name));
    }

    #[test]
    fn test_get_metadata_unknown_fails() {
        let (_env, client, _) = setup();
        let result = client.try_get_metadata(&symbol_short!("ghost"));
        assert!(result.is_err());
    }

    #[test]
    fn test_metadata_update() {
        let (env, client, _) = setup();
        let name = symbol_short!("batcher");
        let v1 = String::from_str(&env, "1.0.0");
        let v2 = String::from_str(&env, "1.1.0");
        let desc = String::from_str(&env, "Batcher contract");
        let author = String::from_str(&env, "mux-labs");

        client.register_with_metadata(&name, &v1, &desc, &author);
        client.register_with_metadata(&name, &v2, &desc, &author);

        let meta = client.get_metadata(&name);
        assert_eq!(meta.version, v2);
        // name appears only once in list
        let names = client.list_contracts();
        let count = names.iter().filter(|n| *n == name).count();
        assert_eq!(count, 1);
    }

    // ── #294: negative path tests ─────────────────────────────────────────────

    #[test]
    fn test_double_initialize_fails() {
        let (_, client, admin) = setup();
        assert_eq!(
            client.try_initialize(&admin),
            Err(Ok(MuxRegistryError::AlreadyInitialized))
        );
    }

    #[test]
    fn test_get_version_unknown_contract_fails() {
        let (_env, client, _) = setup();
        assert_eq!(
            client.try_get_version(&symbol_short!("ghost")),
            Err(Ok(MuxRegistryError::ContractNotFound))
        );
    }

    #[test]
    fn test_get_metadata_unknown_contract_fails() {
        let (_env, client, _) = setup();
        assert_eq!(
            client.try_get_metadata(&symbol_short!("ghost")),
            Err(Ok(MuxRegistryError::ContractNotFound))
        );
    }

    #[test]
    fn test_register_without_init_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRegistry);
        let client = MuxRegistryClient::new(&env, &contract_id);
        let name = symbol_short!("x");
        let version = String::from_str(&env, "1.0.0");
        assert_eq!(
            client.try_register(&name, &version),
            Err(Ok(MuxRegistryError::NotInitialized))
        );
    }

    #[test]
    fn test_register_cap_enforced() {
        // Fill the registry to MAX_CONTRACTS and verify the next registration fails.
        let (env, client, _) = setup();
        env.budget().reset_unlimited();
        let version = String::from_str(&env, "1.0.0");
        // Register MAX_CONTRACTS unique names (128).
        // We use a small helper: symbol_short! requires a literal, so we use
        // a pre-built list of 128 distinct symbols via a loop over u8 values
        // encoded as symbols using the SDK's String→Symbol path isn't available
        // in no_std; instead we register 128 entries by re-using register_with_metadata
        // which shares the same cap check.
        for i in 0..MAX_CONTRACTS {
            // Build unique Symbol values. Symbol can hold up to 9 alphanumeric
            // chars; we encode i as a fixed-width decimal string via a tiny
            // no_std-compatible formatter.
            let name = symbol_from_u32(&env, i);
            client.register(&name, &version);
        }
        let extra = symbol_from_u32(&env, MAX_CONTRACTS);
        assert_eq!(
            client.try_register(&extra, &version),
            Err(Ok(MuxRegistryError::TooManyContracts))
        );
    }

    /// Build a `Symbol` from a small integer for test uniqueness.
    fn symbol_from_u32(env: &Env, n: u32) -> soroban_sdk::Symbol {
        // Symbols support up to 9 chars; encode n as decimal digits.
        // We construct via the Symbol::new convenience if available, or fall
        // back to a pre-built table for 0..=128.
        let s = u32_to_sym_str(n);
        soroban_sdk::Symbol::new(env, s)
    }

    /// Return a string slice for values 0..=200. Sufficient for MAX_CONTRACTS+1.
    fn u32_to_sym_str(n: u32) -> &'static str {
        // Generated table covers 0..=200.
        const TABLE: [&str; 201] = [
            "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12", "n13",
            "n14", "n15", "n16", "n17", "n18", "n19", "n20", "n21", "n22", "n23", "n24", "n25",
            "n26", "n27", "n28", "n29", "n30", "n31", "n32", "n33", "n34", "n35", "n36", "n37",
            "n38", "n39", "n40", "n41", "n42", "n43", "n44", "n45", "n46", "n47", "n48", "n49",
            "n50", "n51", "n52", "n53", "n54", "n55", "n56", "n57", "n58", "n59", "n60", "n61",
            "n62", "n63", "n64", "n65", "n66", "n67", "n68", "n69", "n70", "n71", "n72", "n73",
            "n74", "n75", "n76", "n77", "n78", "n79", "n80", "n81", "n82", "n83", "n84", "n85",
            "n86", "n87", "n88", "n89", "n90", "n91", "n92", "n93", "n94", "n95", "n96", "n97",
            "n98", "n99", "n100", "n101", "n102", "n103", "n104", "n105", "n106", "n107", "n108",
            "n109", "n110", "n111", "n112", "n113", "n114", "n115", "n116", "n117", "n118", "n119",
            "n120", "n121", "n122", "n123", "n124", "n125", "n126", "n127", "n128", "n129", "n130",
            "n131", "n132", "n133", "n134", "n135", "n136", "n137", "n138", "n139", "n140", "n141",
            "n142", "n143", "n144", "n145", "n146", "n147", "n148", "n149", "n150", "n151", "n152",
            "n153", "n154", "n155", "n156", "n157", "n158", "n159", "n160", "n161", "n162", "n163",
            "n164", "n165", "n166", "n167", "n168", "n169", "n170", "n171", "n172", "n173", "n174",
            "n175", "n176", "n177", "n178", "n179", "n180", "n181", "n182", "n183", "n184", "n185",
            "n186", "n187", "n188", "n189", "n190", "n191", "n192", "n193", "n194", "n195", "n196",
            "n197", "n198", "n199", "n200",
        ];
        TABLE[n as usize]
    }
}
