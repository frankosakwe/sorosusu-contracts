#![cfg(test)]
use soroban_sdk::{testutils::Address as _, Address, Env, token, contract, contractimpl};
use sorosusu_contracts::{SoroSusu, SoroSusuClient, SoroSusuTrait};

#[contract]
pub struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn mint(_env: Env, _to: Address, _id: u128) {}
    pub fn burn(_env: Env, _from: Address, _id: u128) {}
}

#[test]
fn test_reliability_score_oracle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let other_user = Address::generate(&env);
    
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    client.init(&admin);
    
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    let nft_id = env.register_contract(None, MockNft);
    
    token_client.mint(&user, &1_000_000_000);
    token_client.mint(&other_user, &1_000_000_000);
    
    let amount: i128 = 1000_000_000; // 100 units
    let circle_id = client.create_circle(&user, &amount, &2, &token_id, &86400, &0, &nft_id);
    
    client.join_circle(&user, &circle_id, &1, &None);
    client.join_circle(&other_user, &circle_id, &1, &None);

    // Initial score should be 0
    assert_eq!(client.get_reliability_score(&user), 0);
    
    // --- ROUND 1 ---
    client.deposit(&user, &circle_id);
    client.deposit(&other_user, &circle_id);
    
    // On-time ratio = 1/1 = 100% -> 400 points
    // Volume = 100 -> (100 * 300) / 10000 = 3 points
    let score = client.get_reliability_score(&user);
    assert_eq!(score, 403); 

    client.finalize_round(&user, &circle_id);
    client.claim_pot(&user, &circle_id);
    
    // User 1 has 1 contribution, which is < max_members (2). So cycles_completed should still be 0.
    // Wait, let's check.
    
    // --- ROUND 2 ---
    env.ledger().set_timestamp(env.ledger().timestamp() + 100000); // Past deadline
    client.deposit(&user, &circle_id);
    client.deposit(&other_user, &circle_id);
    
    // On-time ratio = 1/2 = 50% -> 200 points
    // Volume = 200 -> (200 * 300) / 10000 = 6 points
    let score2 = client.get_reliability_score(&user);
    assert_eq!(score2, 206);
    
    client.finalize_round(&user, &circle_id);
    client.claim_pot(&other_user, &circle_id);
    
    // Now both have finished contributing 2 times.
    // other_user just claimed pot, so other_user should have 1 cycle completed.
    // user 1 has not claimed pot in this round.
    // Wait, user 1 should have claimed their pot in round 1.
    // In round 1, user 1 had only 1 contribution. 1 < 2. So no cycle increment.
    // In round 2, other_user had 2 contributions. 2 >= 2. So 1 cycle increment.
    
    let score_other = client.get_reliability_score(&other_user);
    // other_user on-time ratio: 1/2 (since ROUND 1 was on-time and ROUND 2 was late) -> 200
    // other_user volume: 200 -> 6
    // other_user cycles: 1 -> 1 * 30 = 30
    // total = 236
    assert_eq!(score_other, 236);
    
    println!("Oracle Reliability Score Test Passed!");
}
