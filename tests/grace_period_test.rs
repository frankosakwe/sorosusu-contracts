#![cfg(test)]
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{contract, contractimpl, Address, Env, token, Vec};
use sorosusu_contracts::{SoroSusuClient, SoroSusu};

#[contract]
pub struct MockTokenSusu;

#[contractimpl]
impl MockTokenSusu {
    pub fn init_mock(_env: Env, _admin: Address) {}
    pub fn mint(_env: Env, _to: Address, _amount: i128) {}
    pub fn transfer(_env: Env, _from: Address, _to: Address, _amount: i128) {}
    pub fn balance(_env: Env, _addr: Address) -> i128 { 1_000_000_000 }
}

#[test]
fn test_on_time_deposit_no_penalty() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    // Create circle with 24 hour grace period and 1% late fee
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64, // 24 hour grace period
        &100u32, // 1% late fee
    );
    
    client.join_circle(&user, &circle_id);
    
    // Deposit on time (before deadline)
    client.deposit(&user, &circle_id);
    
    // Verify member has contributed and no missed deadline
    let member_key = sorosusu_contracts::DataKey::Member(user.clone());
    let member = env.storage().instance().get::<sorosusu_contracts::DataKey, sorosusu_contracts::Member>(&member_key).unwrap();
    assert!(member.has_contributed);
    assert_eq!(member.missed_deadline_timestamp, 0);
}

#[test]
fn test_late_deposit_requires_late_contribution() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64,
        &100u32,
    );
    
    client.join_circle(&user, &circle_id);
    
    // Advance time past deadline
    env.ledger().set_timestamp(env.ledger().timestamp() + 100000);
    
    // Regular deposit should fail
    let result = std::panic::catch_unwind(|| {
        client.deposit(&user, &circle_id);
    });
    assert!(result.is_err());
    
    // Verify missed deadline was tracked
    let member_key = sorosusu_contracts::DataKey::Member(user.clone());
    let member = env.storage().instance().get::<sorosusu_contracts::DataKey, sorosusu_contracts::Member>(&member_key).unwrap();
    assert!(member.missed_deadline_timestamp > 0);
}

#[test]
fn test_late_contribution_within_grace_period() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64, // 24 hour grace period
        &100u32, // 1% late fee
    );
    
    client.join_circle(&user, &circle_id);
    
    // Advance time past deadline but within grace period
    env.ledger().set_timestamp(env.ledger().timestamp() + 100000);
    
    // Get initial Group Reserve balance
    let initial_reserve: u64 = env.storage().instance()
        .get(&sorosusu_contracts::DataKey::GroupReserve)
        .unwrap_or(0);
    
    // Late contribution should succeed with fee
    client.late_contribution(&user, &circle_id);
    
    // Verify late fee was added to Group Reserve (1% of 100_000_000 = 1_000_000)
    let final_reserve: u64 = env.storage().instance()
        .get(&sorosusu_contracts::DataKey::GroupReserve)
        .unwrap_or(0);
    assert_eq!(final_reserve - initial_reserve, 1_000_000);
    
    // Verify member was marked as contributed and missed deadline reset
    let member_key = sorosusu_contracts::DataKey::Member(user.clone());
    let member = env.storage().instance().get::<sorosusu_contracts::DataKey, sorosusu_contracts::Member>(&member_key).unwrap();
    assert!(member.has_contributed);
    assert_eq!(member.missed_deadline_timestamp, 0);
}

#[test]
fn test_late_contribution_after_grace_period_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64, // 24 hour grace period
        &100u32,
    );
    
    client.join_circle(&user, &circle_id);
    
    // Advance time past grace period (deadline + 25 hours)
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400 + 90000);
    
    // Late contribution should fail
    let result = std::panic::catch_unwind(|| {
        client.late_contribution(&user, &circle_id);
    });
    assert!(result.is_err());
}

#[test]
fn test_execute_default_after_grace_period() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64,
        &100u32,
    );
    
    client.join_circle(&user, &circle_id);
    
    // Advance time past grace period
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400 + 90000);
    
    // Try to deposit to trigger missed deadline tracking
    let _ = std::panic::catch_unwind(|| {
        client.deposit(&user, &circle_id);
    });
    
    // Execute default should succeed
    let result = client.execute_default(&circle_id, &user);
    assert!(result.is_ok());
    
    // Verify member is marked as defaulted
    let defaulted_key = sorosusu_contracts::DataKey::DefaultedMember(circle_id, user.clone());
    let is_defaulted: bool = env.storage().instance()
        .get(&defaulted_key)
        .unwrap_or(false);
    assert!(is_defaulted);
}

#[test]
fn test_execute_default_before_grace_period_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64,
        &100u32,
    );
    
    client.join_circle(&user, &circle_id);
    
    // Advance time past deadline but within grace period
    env.ledger().set_timestamp(env.ledger().timestamp() + 100000);
    
    // Try to deposit to trigger missed deadline tracking
    let _ = std::panic::catch_unwind(|| {
        client.deposit(&user, &circle_id);
    });
    
    // Execute default should fail (grace period not expired)
    let result = client.execute_default(&circle_id, &user);
    assert!(result.is_err());
}

#[test]
fn test_late_contribution_not_late_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64,
        &100u32,
    );
    
    client.join_circle(&user, &circle_id);
    
    // Try late contribution when payment is not late
    let result = std::panic::catch_unwind(|| {
        client.late_contribution(&user, &circle_id);
    });
    assert!(result.is_err());
}

#[test]
fn test_custom_late_fee_calculation() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    // Create circle with 5% late fee (500 bps)
    let circle_id = client.create_circle(
        &creator, 
        &100_000_000i128, 
        &10u32, 
        &token_contract, 
        &86400u64, 
        &100i128,
        &86400u64,
        &500u32, // 5% late fee
    );
    
    client.join_circle(&user, &circle_id);
    
    // Advance time past deadline
    env.ledger().set_timestamp(env.ledger().timestamp() + 100000);
    
    let initial_reserve: u64 = env.storage().instance()
        .get(&sorosusu_contracts::DataKey::GroupReserve)
        .unwrap_or(0);
    
    client.late_contribution(&user, &circle_id);
    
    // Verify 5% late fee was added (5% of 100_000_000 = 5_000_000)
    let final_reserve: u64 = env.storage().instance()
        .get(&sorosusu_contracts::DataKey::GroupReserve)
        .unwrap_or(0);
    assert_eq!(final_reserve - initial_reserve, 5_000_000);
}
