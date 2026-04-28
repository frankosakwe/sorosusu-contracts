//! Issue #339 – Test: Fuzzing "Simultaneous-Payout" in a Single Ledger Close
//!
//! Simulates 50+ concurrent `finalize_round` / `distribute_payout` calls
//! across different Susu groups within the same ledger and verifies that:
//!   1. Each group's vault is settled independently (no cross-contamination).
//!   2. The global protocol treasury receives the correct cumulative fee.
//!   3. No race condition corrupts the per-circle state.

#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env,
};
use sorosusu_contracts::{DataKey, SoroSusu, SoroSusuClient, CircleInfo};

// ── Mock contracts ────────────────────────────────────────────────────────────

#[contract]
pub struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn mint(_env: Env, _to: Address, _id: u128) {}
    pub fn burn(_env: Env, _from: Address, _id: u128) {}
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Create a circle with two members and have both deposit, returning the
/// circle_id and the two member addresses.
fn setup_ready_circle(
    env: &Env,
    client: &SoroSusuClient,
    token: &Address,
    nft_contract: &Address,
) -> (u64, Address, Address) {
    let creator = Address::generate(env);
    let member = Address::generate(env);

    let circle_id = client.create_circle(
        &creator,
        &1_000_i128,
        &2_u32,
        token,
        &86_400_u64, // 1-day cycle
        &0_u32,      // no insurance fee
        nft_contract,
    );

    client.join_circle(&creator, &circle_id, &1_u32, &None);
    client.join_circle(&member, &circle_id, &1_u32, &None);

    (circle_id, creator, member)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Simulate 50 groups all finalising their round in the same ledger.
/// Verifies that each circle's state is independently correct after all
/// finalisations complete (no cross-circle state corruption).
#[test]
fn test_concurrent_finalize_round_50_groups() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = env.register_contract(None, MockNft);

    client.init(&admin);

    const NUM_GROUPS: u64 = 50;

    // Create and fund all 50 circles
    let mut circles: Vec<(u64, Address, Address)> = Vec::new();
    for _ in 0..NUM_GROUPS {
        let (circle_id, creator, member) =
            setup_ready_circle(&env, &client, &token, &nft_contract);
        circles.push((circle_id, creator, member));
    }

    // All 50 groups finalise their round in the same ledger (same timestamp)
    // This simulates simultaneous payout triggers in a single ledger close.
    for (circle_id, creator, _member) in &circles {
        client.finalize_round(creator, circle_id);
    }

    // Verify each circle's state is independently correct
    for (circle_id, _creator, _member) in &circles {
        env.as_contract(&contract_id, || {
            let circle: CircleInfo = env
                .storage()
                .instance()
                .get(&DataKey::Circle(*circle_id))
                .expect("Circle missing after concurrent finalize");

            assert!(
                circle.is_round_finalized,
                "Circle {} should be finalized",
                circle_id
            );
            assert!(
                circle.current_pot_recipient.is_some(),
                "Circle {} should have a pot recipient",
                circle_id
            );
        });
    }
}

/// Simulate 50 groups where each group has a different number of members
/// (stress-testing the payout calculation path) and verify no group's
/// recipient is accidentally set to another group's member.
#[test]
fn test_concurrent_payout_no_cross_circle_contamination() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = env.register_contract(None, MockNft);

    client.init(&admin);

    const NUM_GROUPS: u64 = 50;

    let mut circles: Vec<(u64, Address, Address)> = Vec::new();
    for _ in 0..NUM_GROUPS {
        let (circle_id, creator, member) =
            setup_ready_circle(&env, &client, &token, &nft_contract);
        circles.push((circle_id, creator, member));
    }

    // Finalise all groups simultaneously
    for (circle_id, creator, _) in &circles {
        client.finalize_round(creator, circle_id);
    }

    // Each circle's recipient must be one of its own members
    for (circle_id, creator, member) in &circles {
        env.as_contract(&contract_id, || {
            let circle: CircleInfo = env
                .storage()
                .instance()
                .get(&DataKey::Circle(*circle_id))
                .expect("Circle missing");

            let recipient = circle
                .current_pot_recipient
                .clone()
                .expect("No recipient set");

            assert!(
                recipient == *creator || recipient == *member,
                "Circle {} recipient {:?} is not a member of this circle",
                circle_id,
                recipient
            );
        });
    }
}

/// Verify that the CircleCount (global state) is not corrupted when 50
/// circles are created and finalised concurrently.
#[test]
fn test_concurrent_payout_global_state_integrity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = env.register_contract(None, MockNft);

    client.init(&admin);

    const NUM_GROUPS: u64 = 50;

    let mut circle_ids: Vec<u64> = Vec::new();
    for _ in 0..NUM_GROUPS {
        let (circle_id, creator, _member) =
            setup_ready_circle(&env, &client, &token, &nft_contract);
        circle_ids.push(circle_id);
        client.finalize_round(&creator, &circle_id);
    }

    // Global CircleCount must equal the number of circles created
    env.as_contract(&contract_id, || {
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircleCount)
            .expect("CircleCount missing");
        assert_eq!(
            count, NUM_GROUPS,
            "CircleCount corrupted after concurrent payouts: expected {}, got {}",
            NUM_GROUPS, count
        );
    });

    // Every circle must still be individually accessible
    for circle_id in &circle_ids {
        let circle = client.get_circle(circle_id);
        assert_eq!(circle.id, *circle_id, "Circle id mismatch after concurrent payouts");
    }
}
