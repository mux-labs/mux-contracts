/*!
 * mux-recovery: Account recovery system for Mux Protocol.
 *
 * Implements a secure account recovery mechanism that allows authorized
 * administrators to approve recovery requests for compromised or lost accounts.
 */

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

// ── Audit events ──────────────────────────────────────────────────────────────
fn emit(env: &Env, action: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events()
        .publish((symbol_short!("mux_recv"), action), data);
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    RecoveryRequest(u64),
    NextRequestId,
    PendingRequests,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryRequest {
    pub id: u64,
    pub old_account: Address,
    pub new_account: Address,
    pub requester: Address,
    pub timestamp: u64,
    pub approved: bool,
    pub approver: Option<Address>,
    pub approval_timestamp: Option<u64>,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MuxRecoveryError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    RequestNotFound = 4,
    RequestAlreadyApproved = 5,
    InvalidAccount = 6,
    SelfRecovery = 7,
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
// Extend instance TTL on every write to keep the recovery system active
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MuxRecovery;

#[contractimpl]
impl MuxRecovery {
    /// Initialize the recovery contract with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MuxRecoveryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MuxRecoveryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextRequestId, &1u64);
        env.storage().instance().set(&DataKey::PendingRequests, &Vec::<u64>::new(&env));
        emit(&env, symbol_short!("init"), admin);
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Submit a recovery request for an account.
    pub fn request_recovery(
        env: Env,
        old_account: Address,
        new_account: Address,
    ) -> Result<u64, MuxRecoveryError> {
        let requester = env.current_contract_address();
        requester.require_auth();

        // Prevent self-recovery attempts
        if old_account == new_account {
            return Err(MuxRecoveryError::SelfRecovery);
        }

        let request_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextRequestId)
            .unwrap_or(1u64);

        let recovery_request = RecoveryRequest {
            id: request_id,
            old_account: old_account.clone(),
            new_account: new_account.clone(),
            requester: requester.clone(),
            timestamp: env.ledger().timestamp(),
            approved: false,
            approver: None,
            approval_timestamp: None,
        };

        // Store the recovery request
        env.storage().instance().set(
            &DataKey::RecoveryRequest(request_id),
            &recovery_request,
        );

        // Update pending requests list
        let mut pending: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::PendingRequests)
            .unwrap_or_else(|| Vec::new(&env));
        pending.push_back(request_id);
        env.storage().instance().set(&DataKey::PendingRequests, &pending);

        // Increment next request ID
        env.storage().instance().set(&DataKey::NextRequestId, &(request_id + 1));

        emit(&env, symbol_short!("req_sub"), (request_id, old_account, new_account));
        Self::extend_ttl(&env);
        Ok(request_id)
    }

    /// Approve a recovery request (admin only).
    pub fn approve_recovery(env: Env, request_id: u64) -> Result<(), MuxRecoveryError> {
        Self::require_admin(&env)?;
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxRecoveryError::NotInitialized)?;

        let mut request: RecoveryRequest = env
            .storage()
            .instance()
            .get(&DataKey::RecoveryRequest(request_id))
            .ok_or(MuxRecoveryError::RequestNotFound)?;

        if request.approved {
            return Err(MuxRecoveryError::RequestAlreadyApproved);
        }

        // Update request with approval details
        request.approved = true;
        request.approver = Some(admin.clone());
        request.approval_timestamp = Some(env.ledger().timestamp());

        // Store updated request
        env.storage().instance().set(
            &DataKey::RecoveryRequest(request_id),
            &request,
        );

        // Remove from pending requests
        let mut pending: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::PendingRequests)
            .unwrap_or_else(|| Vec::new(&env));
        if let Some(pos) = pending.iter().position(|id| id == request_id) {
            pending.remove(pos as u32);
            env.storage().instance().set(&DataKey::PendingRequests, &pending);
        }

        emit(&env, symbol_short!("req_app"), (request_id, admin));
        Self::extend_ttl(&env);
        Ok(())
    }

    /// Get a recovery request by ID.
    pub fn get_recovery_request(env: Env, request_id: u64) -> Result<RecoveryRequest, MuxRecoveryError> {
        env.storage()
            .instance()
            .get(&DataKey::RecoveryRequest(request_id))
            .ok_or(MuxRecoveryError::RequestNotFound)
    }

    /// Get all pending recovery request IDs.
    pub fn get_pending_requests(env: Env) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::PendingRequests)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Result<Address, MuxRecoveryError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxRecoveryError::NotInitialized)
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Result<(), MuxRecoveryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MuxRecoveryError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    /// Extend instance-storage TTL on every write to prevent silent data loss.
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
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events},
        Env, FromVal,
    };

    fn topic_action(
        env: &Env,
        events: &soroban_sdk::Vec<(
            soroban_sdk::Address,
            soroban_sdk::Vec<soroban_sdk::Val>,
            soroban_sdk::Val,
        )>,
        idx: u32,
    ) -> soroban_sdk::Symbol {
        let (_, topics, _) = events.get(idx).unwrap();
        soroban_sdk::Symbol::from_val(env, &topics.get(1).unwrap())
    }

    fn setup() -> (Env, MuxRecoveryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
    }

    #[test]
    fn test_initialize_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, MuxRecovery);
        let client = MuxRecoveryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        assert_eq!(topic_action(&env, &events, 0), symbol_short!("init"));
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, _admin) = setup();
        let other = Address::generate(&env);
        assert!(client.try_initialize(&other).is_err());
    }

    #[test]
    fn test_request_recovery() {
        let (env, client, _admin) = setup();
        let old_account = Address::generate(&env);
        let new_account = Address::generate(&env);
        
        let request_id = client.request_recovery(&old_account, &new_account);
        assert_eq!(request_id, 1);

        let request = client.get_recovery_request(&request_id);
        assert_eq!(request.old_account, old_account);
        assert_eq!(request.new_account, new_account);
        assert!(!request.approved);
    }

    #[test]
    fn test_request_recovery_emits_event() {
        let (env, client, _admin) = setup();
        let old_account = Address::generate(&env);
        let new_account = Address::generate(&env);
        
        client.request_recovery(&old_account, &new_account);
        let events = env.events().all();
        // init + req_sub
        assert_eq!(events.len(), 2);
        assert_eq!(topic_action(&env, &events, 1), symbol_short!("req_sub"));
    }

    #[test]
    fn test_self_recovery_fails() {
        let (env, client, _admin) = setup();
        let account = Address::generate(&env);
        
        let result = client.try_request_recovery(&account, &account);
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_recovery() {
        let (env, client, _admin) = setup();
        let old_account = Address::generate(&env);
        let new_account = Address::generate(&env);
        
        let request_id = client.request_recovery(&old_account, &new_account);
        client.approve_recovery(&request_id);

        let request = client.get_recovery_request(&request_id);
        assert!(request.approved);
        assert!(request.approver.is_some());
        assert!(request.approval_timestamp.is_some());
    }

    #[test]
    fn test_approve_recovery_emits_event() {
        let (env, client, _admin) = setup();
        let old_account = Address::generate(&env);
        let new_account = Address::generate(&env);
        
        let request_id = client.request_recovery(&old_account, &new_account);
        client.approve_recovery(&request_id);

        let events = env.events().all();
        // init + req_sub + req_app
        assert_eq!(events.len(), 3);
        assert_eq!(topic_action(&env, &events, 2), symbol_short!("req_app"));
    }

    #[test]
    fn test_approve_nonexistent_request_fails() {
        let (_env, client, _admin) = setup();
        let result = client.try_approve_recovery(&999);
        assert!(result.is_err());
    }

    #[test]
    fn test_double_approve_fails() {
        let (env, client, _admin) = setup();
        let old_account = Address::generate(&env);
        let new_account = Address::generate(&env);
        
        let request_id = client.request_recovery(&old_account, &new_account);
        client.approve_recovery(&request_id);
        
        let result = client.try_approve_recovery(&request_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_pending_requests() {
        let (env, client, _admin) = setup();
        let old_account1 = Address::generate(&env);
        let new_account1 = Address::generate(&env);
        let old_account2 = Address::generate(&env);
        let new_account2 = Address::generate(&env);
        
        let request_id1 = client.request_recovery(&old_account1, &new_account1);
        let request_id2 = client.request_recovery(&old_account2, &new_account2);
        
        let pending = client.get_pending_requests();
        assert_eq!(pending.len(), 2);
        assert!(pending.contains(&request_id1));
        assert!(pending.contains(&request_id2));
        
        // Approve one request
        client.approve_recovery(&request_id1);
        
        let pending_after = client.get_pending_requests();
        assert_eq!(pending_after.len(), 1);
        assert!(pending_after.contains(&request_id2));
        assert!(!pending_after.contains(&request_id1));
    }

    #[test]
    fn test_get_admin() {
        let (_env, client, admin) = setup();
        let retrieved_admin = client.get_admin();
        assert_eq!(retrieved_admin, admin);
    }
}