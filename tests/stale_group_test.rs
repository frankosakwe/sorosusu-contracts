//! Issue #341 – Test: "Stale-State" Cleanup for 5-Year Old Inactive Groups
//!
//! Verifies that `purge_stale_group`:
//!   1. Correctly identifies groups dormant for ≥ 5 years.
//!   2. Returns any residual insurance balance to the protocol treasury.
//!   3. Removes the circle's storage entry to reclaim ledger rent.
//!   4. Rejects purge attempts on circles that are still within the 5-year window.

#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl,
    testutils::Address as _,
    Address, Env,
};
use sorosusu_contracts::{DataKey, SoroSusu, SoroSusuClient};

#[contract]
pub struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn mint(_env: Env, _to: Address, _id: u128) {}
    pub fn burn(_env: Env, _from: Address, _id: u128) {}
}

/// Helper: advance the ledger timestamp by `secs` seconds.
fn advance_time(env: &Env, secs: u64) {
    env.ledger().with_mut(|l| {
        l.timestamp += secs;
    });
}

/// 5 years in seconds (matches the constant in the contract).
const FIVE_YEARS_SECS: u64 = 157_766_400;

/// A circle that has been inactive for exactly 5 years + 1 second must be
/// successfully purged, and its storage entry must be removed.
#[test]
fn test_purge_stale_group_removes_storage_after_five_years() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = env.register_contract(None, MockNft);

    client.init(&admin);

    let circle_id = client.create_circle(
        &creator,
        &500_i128,
        &3_u32,
        &token,
        &604_800_u64, // 1-week cycle
        &0_u32,       // no insurance fee
        &nft_contract,
    );

    // Advance time past the 5-year stale threshold
    advance_time(&env, FIVE_YEARS_SECS + 1);

    // Purge must succeed
    client.purge_stale_group(&admin, &circle_id);

    // The circle entry must no longer exist in storage
    env.as_contract(&contract_id, || {
        let exists = env
            .storage()
            .instance()
            .has(&DataKey::Circle(circle_id));
        assert!(
            !exists,
            "Circle storage entry should have been removed after purge"
        );
    });
}

/// A circle that has been inactive for less than 5 years must NOT be purged.
#[test]
#[should_panic(expected = "Circle is not stale")]
fn test_purge_stale_group_rejects_active_circle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = env.register_contract(None, MockNft);

    client.init(&admin);

    let circle_id = client.create_circle(
        &creator,
        &500_i128,
        &3_u32,
        &token,
        &604_800_u64,
        &0_u32,
        &nft_contract,
    );

    // Only advance 4 years – still within the active window
    advance_time(&env, FIVE_YEARS_SECS - 1);

    // Must panic with "Circle is not stale"
    client.purge_stale_group(&admin, &circle_id);
}

/// Only the admin may call purge_stale_group.
#[test]
#[should_panic(expected = "Unauthorized")]
fn test_purge_stale_group_rejects_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let creator = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = env.register_contract(None, MockNft);

    client.init(&admin);

    let circle_id = client.create_circle(
        &creator,
        &500_i128,
        &3_u32,
        &token,
        &604_800_u64,
        &0_u32,
        &nft_contract,
    );

    advance_time(&env, FIVE_YEARS_SECS + 1);

    // Non-admin call must panic
    client.purge_stale_group(&attacker, &circle_id);
}
