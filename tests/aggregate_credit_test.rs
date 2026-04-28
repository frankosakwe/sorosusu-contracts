//! Stress tests for Issue #380 — Hierarchical Susu-Aggregation.
//!
//! Builds a 100-group hierarchy (each with up to 50 members) and verifies that
//! the aggregation logic stays within Soroban compute limits.

use soroban_sdk::{testutils::Address as _, Address, Env};
use sorosusu_contracts::aggregate_credit::{
    AggregateCredit, AggregateCreditClient, CollectiveLoanStatus,
    MIN_MEMBER_RI, MIN_AGGREGATE_RI, GROUP_DEFAULT_RI_PENALTY,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Register `count` members with sequential identity hashes starting at
/// `hash_offset`, each with `ri_score`.  Returns the member addresses.
fn setup_members(
    env: &Env,
    client: &AggregateCreditClient,
    count: u32,
    hash_offset: u64,
    ri_score: u32,
) -> soroban_sdk::Vec<Address> {
    let mut members = soroban_sdk::Vec::new(env);
    for i in 0..count {
        let member = Address::generate(env);
        client.register_identity(&member, &(hash_offset + i as u64));
        client.set_member_ri(&member, &ri_score);
        members.push_back(member);
    }
    members
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[test]
fn test_collective_loan_happy_path() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    // 25 members, each RI = 500 → aggregate = 12_500 ≥ MIN_AGGREGATE_RI
    let members = setup_members(&env, &client, 25, 1_000, 500);
    let loan = client.request_collective_loan(&1u64, &members, &1_000_000i128);

    assert_eq!(loan.aggregate_ri, 25 * 500);
    assert_eq!(loan.status, CollectiveLoanStatus::Active);
    assert!(client.is_vault_locked(&1u64));
}

#[test]
fn test_repayment_unlocks_vault() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    let members = setup_members(&env, &client, 25, 2_000, 500);
    client.request_collective_loan(&2u64, &members, &500_000i128);

    let loan = client.repay_collective_loan(&2u64, &500_000i128);
    assert_eq!(loan.status, CollectiveLoanStatus::Repaid);
    assert!(!client.is_vault_locked(&2u64));
}

#[test]
fn test_partial_repayment_keeps_vault_locked() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    let members = setup_members(&env, &client, 25, 3_000, 500);
    client.request_collective_loan(&3u64, &members, &500_000i128);

    let loan = client.repay_collective_loan(&3u64, &100_000i128);
    assert_eq!(loan.status, CollectiveLoanStatus::Active);
    assert!(client.is_vault_locked(&3u64));
}

#[test]
fn test_group_default_penalises_all_members() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    let members = setup_members(&env, &client, 10, 4_000, 600);
    client.request_collective_loan(&4u64, &members, &200_000i128);

    let loan = client.trigger_group_default(&4u64);
    assert_eq!(loan.status, CollectiveLoanStatus::Defaulted);

    // Every member's RI should be reduced by GROUP_DEFAULT_RI_PENALTY
    for member in members.iter() {
        let ri = client.get_member_ri(&member);
        assert_eq!(ri, 600 - GROUP_DEFAULT_RI_PENALTY);
    }
}

#[test]
#[should_panic(expected = "aggregate ri too low")]
fn test_rejects_low_aggregate_ri() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    // 5 members × 500 = 2_500 < MIN_AGGREGATE_RI (10_000)
    let members = setup_members(&env, &client, 5, 5_000, 500);
    client.request_collective_loan(&5u64, &members, &100_000i128);
}

#[test]
#[should_panic(expected = "member ri too low")]
fn test_rejects_member_below_min_ri() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    // 25 members but one has RI = 300 < MIN_MEMBER_RI (400)
    let mut members = setup_members(&env, &client, 24, 6_000, 500);
    let weak = Address::generate(&env);
    client.register_identity(&weak, &99_999u64);
    client.set_member_ri(&weak, &(MIN_MEMBER_RI - 1));
    members.push_back(weak);

    client.request_collective_loan(&6u64, &members, &100_000i128);
}

#[test]
#[should_panic(expected = "member not sep12 verified")]
fn test_rejects_unverified_member() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    let mut members = setup_members(&env, &client, 24, 7_000, 500);
    // Add a member with RI but no SEP-12 identity
    let unverified = Address::generate(&env);
    client.set_member_ri(&unverified, &500u32);
    members.push_back(unverified);

    client.request_collective_loan(&7u64, &members, &100_000i128);
}

#[test]
#[should_panic(expected = "duplicate identity in group")]
fn test_rejects_sybil_duplicate_identity() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    // Two different addresses share the same identity hash → Sybil
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.register_identity(&alice, &42u64);
    // Bob tries to register with the same hash — register_identity allows it
    // (the SEP-12 anchor is responsible for uniqueness at registration time),
    // but the intra-group duplicate check in request_collective_loan fires.
    client.register_identity(&bob, &42u64);
    client.set_member_ri(&alice, &500u32);
    client.set_member_ri(&bob, &500u32);

    // Build a group large enough to pass aggregate RI with just these two
    // (we need aggregate ≥ 10_000, so pad with legitimate members)
    let mut members = setup_members(&env, &client, 23, 8_000, 500);
    members.push_back(alice);
    members.push_back(bob); // same hash as alice → panic

    client.request_collective_loan(&8u64, &members, &100_000i128);
}

#[test]
#[should_panic(expected = "loan already active")]
fn test_rejects_second_loan_while_active() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    let members = setup_members(&env, &client, 25, 9_000, 500);
    client.request_collective_loan(&9u64, &members, &100_000i128);
    client.request_collective_loan(&9u64, &members, &100_000i128); // should panic
}

// ---------------------------------------------------------------------------
// Stress test — 100-group hierarchy
// ---------------------------------------------------------------------------

/// Build 100 independent groups (each with 25 members) and request a
/// collective loan for every group.  This verifies that the aggregation
/// logic scales within Soroban's compute budget.
#[test]
fn stress_test_100_group_hierarchy() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    const NUM_GROUPS: u64 = 100;
    const MEMBERS_PER_GROUP: u32 = 25;
    const RI_PER_MEMBER: u32 = 500; // 25 × 500 = 12_500 ≥ MIN_AGGREGATE_RI

    for group_id in 0..NUM_GROUPS {
        // Each group gets a unique identity-hash range to avoid cross-group
        // Sybil collisions.
        let hash_offset = group_id * MEMBERS_PER_GROUP as u64 * 10;
        let members =
            setup_members(&env, &client, MEMBERS_PER_GROUP, hash_offset, RI_PER_MEMBER);

        let loan =
            client.request_collective_loan(&group_id, &members, &1_000_000i128);

        assert_eq!(loan.status, CollectiveLoanStatus::Active);
        assert_eq!(loan.aggregate_ri, MEMBERS_PER_GROUP * RI_PER_MEMBER);
        assert!(client.is_vault_locked(&group_id));
    }

    // Verify all 100 loans are independently stored and active
    for group_id in 0..NUM_GROUPS {
        let loan = client.get_loan(&group_id).expect("loan should exist");
        assert_eq!(loan.status, CollectiveLoanStatus::Active);
        assert_eq!(loan.group_id, group_id);
    }
}

/// Repay all 100 group loans and confirm vaults are unlocked.
#[test]
fn stress_test_100_group_full_repayment() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    const NUM_GROUPS: u64 = 100;
    const MEMBERS_PER_GROUP: u32 = 25;

    for group_id in 0..NUM_GROUPS {
        let hash_offset = (NUM_GROUPS + group_id) * MEMBERS_PER_GROUP as u64 * 10;
        let members = setup_members(&env, &client, MEMBERS_PER_GROUP, hash_offset, 500);
        client.request_collective_loan(&group_id, &members, &1_000_000i128);
    }

    for group_id in 0..NUM_GROUPS {
        let loan = client.repay_collective_loan(&group_id, &1_000_000i128);
        assert_eq!(loan.status, CollectiveLoanStatus::Repaid);
        assert!(!client.is_vault_locked(&group_id));
    }
}

/// Trigger default on all 100 groups and verify RI penalties are applied.
#[test]
fn stress_test_100_group_mass_default() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AggregateCredit);
    let client = AggregateCreditClient::new(&env, &contract_id);

    const NUM_GROUPS: u64 = 100;
    const MEMBERS_PER_GROUP: u32 = 25;
    const RI: u32 = 600;

    let mut all_members: soroban_sdk::Vec<soroban_sdk::Vec<Address>> =
        soroban_sdk::Vec::new(&env);

    for group_id in 0..NUM_GROUPS {
        let hash_offset = (2 * NUM_GROUPS + group_id) * MEMBERS_PER_GROUP as u64 * 10;
        let members = setup_members(&env, &client, MEMBERS_PER_GROUP, hash_offset, RI);
        client.request_collective_loan(&group_id, &members, &500_000i128);
        all_members.push_back(members);
    }

    for group_id in 0..NUM_GROUPS {
        let loan = client.trigger_group_default(&group_id);
        assert_eq!(loan.status, CollectiveLoanStatus::Defaulted);
    }

    // Spot-check: first group's members should all have RI = RI - penalty
    let first_group = all_members.get(0).unwrap();
    for member in first_group.iter() {
        assert_eq!(client.get_member_ri(&member), RI - GROUP_DEFAULT_RI_PENALTY);
    }
}
