#![cfg(test)]

use soroban_sdk::{contract, contractimpl, Address, Env, symbol_short, token, Symbol};
use soroban_sdk::testutils::{Address as _, Ledger};
use sorosusu_contracts::{SoroSusu, SoroSusuClient};

#[contract]
pub struct MockSanctionsOracle;

#[contractimpl]
impl MockSanctionsOracle {
    pub fn is_sanctioned(env: Env, address: Address) -> bool {
        env.storage().instance().has(&address)
    }
    
    pub fn set_sanctioned(env: Env, address: Address, status: bool) {
        if status {
            env.storage().instance().set(&address, &true);
        } else {
            env.storage().instance().remove(&address);
        }
    }
}

#[contract]
pub struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn mint(_env: Env, _to: Address, _id: u128) {}
    pub fn burn(_env: Env, _from: Address, _id: u128) {}
}

#[test]
fn test_aml_sanctions_payout_gating() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let winner = Address::generate(&env);
    let other_member = Address::generate(&env);
    let nft_contract = env.register_contract(None, MockNft);
    let oracle_id = env.register_contract(None, MockSanctionsOracle);
    let oracle_client = MockSanctionsOracleClient::new(&env, &oracle_id);

    // Register a token contract
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract(token_admin.clone());
    let token_client = token::Client::new(&env, &token);
    let token_admin_client = token::StellarAssetClient::new(&env, &token);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    
    // Initialize
    client.init(&admin);
    client.set_sanctions_oracle(&admin, &oracle_id);
    
    // Create circle
    let amount = 1000i128;
    let max_members = 2u32;
    let circle_id = client.create_circle(
        &creator,
        &amount,
        &max_members,
        &token,
        &86400,
        &0,
        &nft_contract,
    );
    
    // Mint tokens
    token_admin_client.mint(&winner, &10000);
    token_admin_client.mint(&other_member, &10000);
    
    // Join members
    client.join_circle(&winner, &circle_id, &1, &None);
    client.join_circle(&other_member, &circle_id, &1, &None);
    
    // Deposit
    client.deposit(&winner, &circle_id);
    client.deposit(&other_member, &circle_id);
    
    // Finalize round
    client.finalize_round(&creator, &circle_id);
    
    // Advance time for payout
    env.ledger().set_timestamp(env.ledger().timestamp() + 86401);
    
    // Reveal winner (it should be 'winner' if it's round-robin and they joined first)
    let revealed_winner = client.reveal_next_winner(&circle_id).unwrap();
    assert_eq!(revealed_winner, winner);
    
    // CASE 1: Winner is sanctioned
    oracle_client.set_sanctioned(&winner, &true);
    
    // Claim pot should freeze the payout
    client.claim_pot(&winner, &circle_id);
    
    // Verify payout is frozen
    let (frozen_amount, frozen_winner) = client.get_frozen_payout(&circle_id);
    assert_eq!(frozen_amount, amount * (max_members as i128));
    assert_eq!(frozen_winner, Some(winner));
    
    // CASE 2: Review and release
    client.review_frozen_payout(&admin, &circle_id, &true);
    
    // Verify payout is no longer frozen
    let (cleared_amount, cleared_winner) = client.get_frozen_payout(&circle_id);
    assert_eq!(cleared_amount, 0);
    assert_eq!(cleared_winner, None);
}
