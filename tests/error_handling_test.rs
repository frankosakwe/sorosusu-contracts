#![cfg(test)]

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};
use sorosusu_contracts::{SoroSusu, SoroSusuClient};

#[test]
fn test_error_handling_join_circle_invalid_shares() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    let token_address = Address::generate(&env);
    
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    // Initialize contract and create a circle
    client.init(&admin, &0);
    let circle_id = client.create_circle(
        &creator,
        &100i128,
        &5u32,
        &token_address,
        &604800u64,
        &0i128,
    );
    
    // Try to join with invalid shares (3) - this should panic with old error handling
    // In the refactored version, this would return a Result
    let result = std::panic::catch_unwind(|| {
        client.join_circle(&user, &circle_id, &3u32);
    });
    
    assert!(result.is_err());
    // The panic message should contain "Shares must be 1 or 2"
    let panic_message = result.unwrap_err();
    let panic_str = panic_message.downcast_ref::<String>().unwrap();
    assert!(panic_str.contains("Shares must be 1 or 2"));
}

#[test]
fn test_error_codes_uniqueness() {
    use sorosusu_contracts::errors::SoroSusuError;
    
    // Test that key error codes are unique and properly defined
    assert_eq!(SoroSusuError::Unauthorized.code(), 1000);
    assert_eq!(SoroSusuError::CircleNotFound.code(), 1002);
    assert_eq!(SoroSusuError::InvalidShares.code(), 4004);
    assert_eq!(SoroSusuError::ZeroRounds.code(), 3003);
    assert_eq!(SoroSusuError::InsuranceFeeExceedsLimit.code(), 5007);
    
    // Test error messages are descriptive
    assert_eq!(SoroSusuError::Unauthorized.message(), "Unauthorized access");
    assert_eq!(SoroSusuError::CircleNotFound.message(), "Circle not found");
    assert_eq!(SoroSusuError::InvalidShares.message(), "Shares must be 1 or 2");
    assert_eq!(SoroSusuError::ZeroRounds.message(), "Number of rounds must be greater than zero");
    assert_eq!(SoroSusuError::InsuranceFeeExceedsLimit.message(), "Insurance fee exceeds 100%");
}

#[test]
fn test_error_categories() {
    use sorosusu_contracts::errors::SoroSusuError;
    
    // Test error categorization for frontend use
    assert_eq!(SoroSusuError::Unauthorized.category(), "authentication");
    assert_eq!(SoroSusuError::CircleNotFound.category(), "not_found");
    assert_eq!(SoroSusuError::InsufficientBalance.category(), "insufficient_funds");
    assert_eq!(SoroSusuError::ContractPaused.category(), "paused");
    assert_eq!(SoroSusuError::Overflow.category(), "arithmetic");
    assert_eq!(SoroSusuError::InvalidShares.category(), "general");
}
fn test_error_handling_deposit_zero_rounds() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    let token_address = Address::generate(&env);
    
    let contract_id = env.register_contract(None, SoroSusuContract);
    let client = SoroSusuContractClient::new(&env, &contract_id);
    
    // Initialize contract and create a circle
    client.init(&admin);
    let circle_id = client.create_circle(
        &creator,
        &100u64,
        &5u32,
        &token_address,
        &604800u64,
        &true,
        &50u32,
        &86400u64,
        &500u32,
    );
    
    // User joins the circle
    client.join_circle(&user, &circle_id, &1u32, &None);
    
    // Try to deposit with zero rounds
    let result = client.try_deposit(&user, &circle_id, &0u32);
    
    assert!(result.is_err());
    let error = result.err().unwrap();
    
    // Verify it's the expected error type
    match error {
        soroban_sdk::Error::Contract(3003) => {
            // ZeroRounds error code
        },
        _ => panic!("Expected ZeroRounds error"),
    }
}

#[test]
fn test_error_handling_circle_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let contract_id = env.register_contract(None, SoroSusuContract);
    let client = SoroSusuContractClient::new(&env, &contract_id);
    
    // Initialize contract
    client.init(&admin);
    
    // Try to join a non-existent circle
    let result = client.try_join_circle(&user, &999u64, &1u32, &None);
    
    assert!(result.is_err());
    let error = result.err().unwrap();
    
    // Verify it's the expected error type
    match error {
        soroban_sdk::Error::Contract(1002) => {
            // CircleNotFound error code
        },
        _ => panic!("Expected CircleNotFound error"),
    }
}

#[test]
fn test_error_codes_are_unique() {
    // This test ensures all error codes are unique
    use sorosusu_contracts::errors::SoroSusuError;
    
    let mut codes = std::collections::HashSet::new();
    
    // Test a few key error codes to ensure they're unique
    codes.insert(SoroSusuError::Unauthorized.code());
    codes.insert(SoroSusuError::CircleNotFound.code());
    codes.insert(SoroSusuError::MemberNotFound.code());
    codes.insert(SoroSusuError::InvalidShares.code());
    codes.insert(SoroSusuError::ZeroRounds.code());
    codes.insert(SoroSusuError::InsuranceFeeExceedsLimit.code());
    
    // Verify we have the expected number of unique codes
    assert_eq!(codes.len(), 6);
    
    // Test specific codes
    assert_eq!(SoroSusuError::Unauthorized.code(), 1000);
    assert_eq!(SoroSusuError::CircleNotFound.code(), 1002);
    assert_eq!(SoroSusuError::InvalidShares.code(), 4004);
    assert_eq!(SoroSusuError::ZeroRounds.code(), 3003);
    assert_eq!(SoroSusuError::InsuranceFeeExceedsLimit.code(), 5007);
}

#[test]
fn test_error_messages() {
    use sorosusu_contracts::errors::SoroSusuError;
    
    // Test that error messages are descriptive
    assert_eq!(SoroSusuError::Unauthorized.message(), "Unauthorized access");
    assert_eq!(SoroSusuError::CircleNotFound.message(), "Circle not found");
    assert_eq!(SoroSusuError::InvalidShares.message(), "Shares must be 1 or 2");
    assert_eq!(SoroSusuError::ZeroRounds.message(), "Number of rounds must be greater than zero");
    assert_eq!(SoroSusuError::InsuranceFeeExceedsLimit.message(), "Insurance fee exceeds 100%");
}

#[test]
fn test_error_categories() {
    use sorosusu_contracts::errors::SoroSusuError;
    
    // Test error categorization
    assert_eq!(SoroSusuError::Unauthorized.category(), "authentication");
    assert_eq!(SoroSusuError::CircleNotFound.category(), "not_found");
    assert_eq!(SoroSusuError::InsufficientBalance.category(), "insufficient_funds");
    assert_eq!(SoroSusuError::ContractPaused.category(), "paused");
    assert_eq!(SoroSusuError::Overflow.category(), "arithmetic");
    assert_eq!(SoroSusuError::InvalidShares.category(), "general");
}
