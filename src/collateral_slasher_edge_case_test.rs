#![cfg(test)]

use soroban_sdk::{Address, Env, token};
use crate::{
    SoroSusu, DataKey, Member, CircleInfo,
};

#[test]
fn test_partial_default_slashing_returns_remainder() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let defaulter = Address::generate(&env);
    let token_address = Address::generate(&env);
    let nft_contract = Address::generate(&env);

    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());

    // Create circle with 1% late fee
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        creator.clone(),
        1000, // contribution amount
        2,     // max members
        token_address.clone(),
        604800, // 1 week cycle duration
        false,  // yield_enabled
        0,      // risk_tolerance
        24 * 60 * 60, // grace period
        100,    // 1% late fee
    );

    // Users join circle
    SoroSusu::join_circle(env.clone(), creator.clone(), circle_id);
    SoroSusu::join_circle(env.clone(), defaulter.clone(), circle_id);

    // Mint tokens
    let token_client = token::Client::new(&env, &token_address);
    token_client.mint(&creator, &2000);
    token_client.mint(&defaulter, &2000);

    // Creator makes full deposit
    SoroSusu::deposit(env.clone(), creator.clone(), circle_id, 1000);

    // Defaulter makes partial deposit of 600 (more than half)
    SoroSusu::deposit(env.clone(), defaulter.clone(), circle_id, 600);

    // Advance past deadline and grace period
    env.ledger().set_timestamp(604800 + 24 * 60 * 60 + 1);

    // Execute default
    SoroSusu::execute_default(env.clone(), circle_id, defaulter.clone()).unwrap();

    // Get initial balance
    let initial_balance = token_client.balance(&defaulter);

    // Slash collateral
    SoroSusu::slash_collateral(env.clone(), circle_id, defaulter.clone()).unwrap();

    // Check final balance
    let final_balance = token_client.balance(&defaulter);

    // Remaining needed = 1000 - 600 = 400
    // Penalty = (400 * 100) / 10000 = 4
    // Slash amount = 400 + 4 = 404
    // Since 404 < 600, slash 404, remainder = 600 - 404 = 196
    // So final_balance = initial_balance + 196
    assert_eq!(final_balance, initial_balance + 196);
}