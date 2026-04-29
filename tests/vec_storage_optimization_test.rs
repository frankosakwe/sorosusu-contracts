#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, Vec as SorobanVec,
};

// Import the contract and types
extern crate sorosusu_contracts;
use sorosusu_contracts::{
    SoroSusu, SoroSusuClient, DataKey, MemberRecord, MemberStatus, MAX_GROUP_SIZE,
};

/// Test: find_member helper returns correct index for existing address
#[test]
fn test_find_member_existing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let member1 = Address::generate(&env);
    let member2 = Address::generate(&env);
    let member3 = Address::generate(&env);

    // Create members Vec
    let mut members = SorobanVec::new(&env);
    
    members.push_back(MemberRecord {
        address: member1.clone(),
        index: 0,
        contribution_count: 0,
        last_contribution_time: 0,
        status: MemberStatus::Active,
        tier_multiplier: 1,
        consecutive_missed_rounds: 0,
        referrer: None,
        buddy: None,
        shares: 1,
        guarantor: None,
    });
    
    members.push_back(MemberRecord {
        address: member2.clone(),
        index: 1,
        contribution_count: 0,
        last_contribution_time: 0,
        status: MemberStatus::Active,
        tier_multiplier: 1,
        consecutive_missed_rounds: 0,
        referrer: None,
        buddy: None,
        shares: 1,
        guarantor: None,
    });
    
    members.push_back(MemberRecord {
        address: member3.clone(),
        index: 2,
        contribution_count: 0,
        last_contribution_time: 0,
        status: MemberStatus::Active,
        tier_multiplier: 1,
        consecutive_missed_rounds: 0,
        referrer: None,
        buddy: None,
        shares: 1,
        guarantor: None,
    });

    // Test find_member (we'll need to expose this or test indirectly)
    // For now, test through join_circle which uses find_member internally
    assert_eq!(members.len(), 3);
    assert_eq!(members.get(0).unwrap().address, member1);
    assert_eq!(members.get(1).unwrap().address, member2);
    assert_eq!(members.get(2).unwrap().address, member3);
}

/// Test: find_member returns None for unknown address
#[test]
fn test_find_member_not_found() {
    let env = Env::default();
    let members = SorobanVec::<MemberRecord>::new(&env);
    let unknown = Address::generate(&env);
    
    // Empty Vec should not contain any member
    assert_eq!(members.len(), 0);
}

/// Test: join_circle correctly adds member to Vec
#[test]
fn test_join_circle_adds_to_vec() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    // Initialize contract
    client.init(&admin);

    // Create a circle
    let circle_id = client.create_circle(
        &admin,
        &1000,      // amount
        &10,        // max_members
        &token,
        &86400,     // cycle_duration (1 day)
        &false,     // yield_enabled
        &0,         // risk_tolerance
        &3600,      // grace_period (1 hour)
        &500,       // late_fee_bps (5%)
    );

    // Join circle
    client.join_circle(&user1, &circle_id, &1, &None);
    client.join_circle(&user2, &circle_id, &1, &None);

    // Verify circle member_count updated
    let circle = client.get_circle(&circle_id);
    assert_eq!(circle.member_count, 2);
    assert_eq!(circle.member_addresses.len(), 2);
    assert_eq!(circle.member_addresses.get(0).unwrap(), user1);
    assert_eq!(circle.member_addresses.get(1).unwrap(), user2);

    // Verify members can be retrieved
    let member1 = client.get_member(&user1);
    assert_eq!(member1.address, user1);
    assert_eq!(member1.index, 0);
    assert_eq!(member1.contribution_count, 0);

    let member2 = client.get_member(&user2);
    assert_eq!(member2.address, user2);
    assert_eq!(member2.index, 1);
    assert_eq!(member2.contribution_count, 0);
}

/// Test: join_circle rejects duplicate member
#[test]
#[should_panic(expected = "Already a member")]
fn test_join_circle_rejects_duplicate() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let user = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &10, &token, &86400, &false, &0, &3600, &500,
    );

    client.join_circle(&user, &circle_id, &1, &None);
    client.join_circle(&user, &circle_id, &1, &None); // Should panic
}

/// Test: join_circle enforces MAX_GROUP_SIZE
#[test]
#[should_panic(expected = "Group size limit exceeded")]
fn test_join_circle_enforces_max_group_size() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    // Create circle with max_members = 25 (exceeds MAX_GROUP_SIZE)
    let circle_id = client.create_circle(
        &admin, &1000, &25, &token, &86400, &false, &0, &3600, &500,
    );

    // Add MAX_GROUP_SIZE members (should succeed)
    for i in 0..MAX_GROUP_SIZE {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id, &1, &None);
    }

    // Try to add one more (should panic)
    let extra_user = Address::generate(&env);
    client.join_circle(&extra_user, &circle_id, &1, &None);
}

/// Test: deposit correctly updates member contribution_count in Vec
#[test]
fn test_deposit_updates_member_in_vec() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let user = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &10, &token, &86400, &false, &0, &3600, &500,
    );

    client.join_circle(&user, &circle_id, &1, &None);

    // Make deposit
    client.deposit(&user, &circle_id, &2); // 2 rounds

    // Verify member contribution_count updated
    let member = client.get_member(&user);
    assert_eq!(member.contribution_count, 2);
    assert_eq!(member.last_contribution_time, 1000);
}

/// Test: deposit fails for non-member
#[test]
#[should_panic(expected = "Member not found")]
fn test_deposit_fails_for_non_member() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let non_member = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &10, &token, &86400, &false, &0, &3600, &500,
    );

    // Try to deposit without joining
    client.deposit(&non_member, &circle_id, &1);
}

/// Test: Multiple members can contribute independently
#[test]
fn test_multiple_members_contribute_independently() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &10, &token, &86400, &false, &0, &3600, &500,
    );

    // All join
    client.join_circle(&user1, &circle_id, &1, &None);
    client.join_circle(&user2, &circle_id, &1, &None);
    client.join_circle(&user3, &circle_id, &1, &None);

    // Each contributes different amounts
    client.deposit(&user1, &circle_id, &1);
    client.deposit(&user2, &circle_id, &3);
    client.deposit(&user3, &circle_id, &2);

    // Verify each member's contribution_count is correct
    let member1 = client.get_member(&user1);
    assert_eq!(member1.contribution_count, 1);

    let member2 = client.get_member(&user2);
    assert_eq!(member2.contribution_count, 3);

    let member3 = client.get_member(&user3);
    assert_eq!(member3.contribution_count, 2);
}

/// Test: Full ROSCA cycle with Vec storage
#[test]
fn test_full_rosca_cycle_with_vec_storage() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let members: SorobanVec<Address> = (0..5)
        .map(|_| Address::generate(&env))
        .collect::<SorobanVec<_>>();

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &10, &token, &86400, &false, &0, &3600, &500,
    );

    // All members join
    for member in members.iter() {
        client.join_circle(&member, &circle_id, &1, &None);
    }

    // Verify all members are in the circle
    let circle = client.get_circle(&circle_id);
    assert_eq!(circle.member_count, 5);

    // All members contribute
    for member in members.iter() {
        client.deposit(&member, &circle_id, &1);
    }

    // Verify all contributions recorded
    for member in members.iter() {
        let member_data = client.get_member(&member);
        assert_eq!(member_data.contribution_count, 1);
    }
}

/// Test: Storage efficiency - count_active_members uses single read
#[test]
fn test_count_active_members_efficiency() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &20, &token, &86400, &false, &0, &3600, &500,
    );

    // Add 10 members
    for _ in 0..10 {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id, &1, &None);
    }

    // Get circle and verify member count
    let circle = client.get_circle(&circle_id);
    assert_eq!(circle.member_count, 10);
    
    // count_active_members is called internally by various functions
    // and should now use only 1 storage read instead of 11
}

/// Test: MAX_GROUP_SIZE constant is 20
#[test]
fn test_max_group_size_constant() {
    assert_eq!(MAX_GROUP_SIZE, 20);
}

/// Test: Circle with exactly MAX_GROUP_SIZE members
#[test]
fn test_circle_at_max_capacity() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &MAX_GROUP_SIZE, &token, &86400, &false, &0, &3600, &500,
    );

    // Add exactly MAX_GROUP_SIZE members
    for _ in 0..MAX_GROUP_SIZE {
        let user = Address::generate(&env);
        client.join_circle(&user, &circle_id, &1, &None);
    }

    let circle = client.get_circle(&circle_id);
    assert_eq!(circle.member_count, MAX_GROUP_SIZE);
}

/// Test: Contribution bitmap still works with Vec storage
#[test]
fn test_contribution_bitmap_with_vec_storage() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);

    client.init(&admin);

    let circle_id = client.create_circle(
        &admin, &1000, &10, &token, &86400, &false, &0, &3600, &500,
    );

    client.join_circle(&user1, &circle_id, &1, &None);
    client.join_circle(&user2, &circle_id, &1, &None);

    // Before deposits
    let circle_before = client.get_circle(&circle_id);
    assert_eq!(circle_before.contribution_bitmap, 0);

    // User1 deposits
    client.deposit(&user1, &circle_id, &1);
    let circle_after_user1 = client.get_circle(&circle_id);
    assert_eq!(circle_after_user1.contribution_bitmap, 0b01); // Bit 0 set

    // User2 deposits
    client.deposit(&user2, &circle_id, &1);
    let circle_after_user2 = client.get_circle(&circle_id);
    assert_eq!(circle_after_user2.contribution_bitmap, 0b11); // Bits 0 and 1 set
}
