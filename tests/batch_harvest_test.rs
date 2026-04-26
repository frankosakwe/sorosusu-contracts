#![cfg(test)]
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{contract, contractimpl, token, Address, Env, Symbol, Vec};
use sorosusu_contracts::{BatchHarvestProgress, SoroSusu, SoroSusuClient};

#[contract]
pub struct MockTokenSusu;

#[contractimpl]
impl MockTokenSusu {
    pub fn init_mock(_env: Env, _admin: Address) {}
    pub fn mint(_env: Env, _to: Address, _amount: i128) {}
    pub fn transfer(_env: Env, _from: Address, _to: Address, _amount: i128) {}
    pub fn balance(_env: Env, _addr: Address) -> i128 {
        1_000_000_000
    }
}

#[test]
fn test_batch_harvest_single_chunk() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

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

    // Add 5 members (less than chunk size of 10)
    let mut members = Vec::new(&env);
    for _ in 0..5 {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id);
        members.push(user.clone());
    }

    // Execute batch harvest with 1000 yield
    let total_yield = 1000i128;
    let progress = client
        .batch_harvest(&circle_id, &total_yield, &members)
        .unwrap();

    // Verify progress
    assert_eq!(progress.members_processed, 5);
    assert_eq!(progress.total_members, 5);
    assert!(progress.is_complete);

    // Verify each member received correct yield (1000 / 5 = 200 each)
    let expected_per_member = 200i128;
    for member in members.iter() {
        let yield_balance = env
            .storage()
            .instance()
            .get(&sorosusu_contracts::DataKey::YieldBalance(
                circle_id, member,
            ))
            .unwrap();
        assert_eq!(yield_balance, expected_per_member);
    }
}

#[test]
fn test_batch_harvest_multiple_chunks() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin, &0);

    let circle_id = client.create_circle(
        &creator,
        &100_000_000i128,
        &5u32,
        &token_contract,
        &86400u64,
        &100i128,
        &86400u64,
        &100u32,
    );

    let mut members = Vec::new(&env);
    for _ in 0..5 {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id);
        members.push(user.clone());
    }

    let total_yield = 2500i128;

    // First call - processes first 5 members
    let progress1 = client
        .batch_harvest(&circle_id, &total_yield, &members)
        .unwrap();
    assert_eq!(progress1.members_processed, 5);
    assert_eq!(progress1.total_members, 5);
    assert!(progress1.is_complete);

    // Verify each member received correct yield (2500 / 5 = 500 each)
    let expected_per_member = 500i128;
    for member in members.iter() {
        let yield_balance = env
            .storage()
            .instance()
            .get(&sorosusu_contracts::DataKey::YieldBalance(
                circle_id, member,
            ))
            .unwrap();
        assert_eq!(yield_balance, expected_per_member);
    }
}

#[test]
fn test_batch_harvest_already_complete() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    let token_contract = env.register_contract(None, MockTokenSusu);
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin, &0);

    let circle_id = client.create_circle(
        &creator,
        &100_000_000i128,
        &5u32,
        &token_contract,
        &86400u64,
        &100i128,
        &86400u64,
        &100u32,
    );

    let mut members = Vec::new(&env);
    for _ in 0..5 {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id);
        members.push(user.clone());
    }

    let total_yield = 500i128;

    // Complete the batch harvest
    let progress1 = client
        .batch_harvest(&circle_id, &total_yield, &members)
        .unwrap();
    assert!(progress1.is_complete);

    // Call again - should return completed progress without reprocessing
    let progress2 = client
        .batch_harvest(&circle_id, &total_yield, &members)
        .unwrap();
    assert_eq!(progress2.members_processed, 5);
    assert!(progress2.is_complete);
}

#[test]
fn test_batch_harvest_pro_rata_distribution() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

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

    let mut members = Vec::new(&env);
    for _ in 0..10 {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id);
        members.push(user.clone());
    }

    // Test with odd yield amount to verify precise division
    let total_yield = 9999i128;
    let progress = client
        .batch_harvest(&circle_id, &total_yield, &members)
        .unwrap();

    assert!(progress.is_complete);

    // Each member should get 999 (9999 / 10 = 999 with remainder)
    let expected_per_member = 999i128;
    for member in members.iter() {
        let yield_balance = env
            .storage()
            .instance()
            .get(&sorosusu_contracts::DataKey::YieldBalance(
                circle_id, member,
            ))
            .unwrap();
        assert_eq!(yield_balance, expected_per_member);
    }
}

#[test]
fn test_batch_harvest_empty_member_list() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

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

    let members = Vec::new(&env);
    let total_yield = 1000i128;

    // Should handle empty member list gracefully
    let progress = client
        .batch_harvest(&circle_id, &total_yield, &members)
        .unwrap();
    assert_eq!(progress.members_processed, 0);
    assert_eq!(progress.total_members, 0);
    assert!(progress.is_complete);
}

#[test]
fn test_batch_harvest_non_member_excluded() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

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

    // Add 5 actual members
    let mut members = Vec::new(&env);
    for _ in 0..5 {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id);
        members.push(user.clone());
    }

    // Add a non-member to the list
    let non_member = Address::generate(&env);
    members.push(non_member.clone());

    let total_yield = 500i128;
    let progress = client
        .batch_harvest(&circle_id, &total_yield, &members)
        .unwrap();

    // Only 5 actual members should be processed
    assert_eq!(progress.members_processed, 5);
    assert_eq!(progress.total_members, 6); // Total in list including non-member
    assert!(progress.is_complete);

    // Non-member should not receive yield
    let non_member_yield = env
        .storage()
        .instance()
        .get::<sorosusu_contracts::DataKey, i128>(&sorosusu_contracts::DataKey::YieldBalance(
            circle_id, non_member,
        ));
    assert!(non_member_yield.is_none());
}
