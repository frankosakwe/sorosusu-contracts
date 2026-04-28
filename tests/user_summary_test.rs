#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env};
use sorosusu_contracts::{SoroSusuClient, SoroSusu, UserSummary};

#[test]
fn test_get_user_summary_normal_flow() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, soroban_sdk::token::StellarAssetContract);
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
    
    let summary = client.get_user_summary(&user).expect("summary should exist");
    
    assert_eq!(summary.next_payment_amount, 100_000_000);
    assert_eq!(summary.due_date, 86400 + 86400); // initial deadline + 1 cycle
    assert_eq!(summary.current_position, 0);
    assert_eq!(summary.ri_score, 0);
}

#[test]
fn test_get_user_summary_after_contribution() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, soroban_sdk::token::StellarAssetContract);
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
    client.deposit(&user, &circle_id, &100_000_000);
    
    let summary = client.get_user_summary(&user).expect("summary should exist");
    
    assert_eq!(summary.next_payment_amount, 100_000_000);
    assert_eq!(summary.due_date, 86400 + (1 * 86400)); // after 1 contribution
    assert_eq!(summary.current_position, 1);
    assert_eq!(summary.ri_score, 100);
}

#[test]
fn test_get_user_summary_nonexistent_user() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    client.init(&admin, &0);
    
    let summary = client.get_user_summary(&user);
    assert!(summary.is_none());
}

#[test]
fn test_get_user_summary_edge_cases() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    
    let token_contract = env.register_contract(None, soroban_sdk::token::StellarAssetContract);
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
    
    // Simulate multiple contributions
    for i in 0..10 {
        client.deposit(&user, &circle_id, &100_000_000);
        env.ledger().set_timestamp(env.ledger().timestamp() + 86400);
    }
    
    let summary = client.get_user_summary(&user).expect("summary should exist");
    
    assert_eq!(summary.current_position, 10);
    assert_eq!(summary.ri_score, 1000); // 10 * 100
    assert_eq!(summary.due_date, 86400 + (11 * 86400)); // after 10 contributions
}