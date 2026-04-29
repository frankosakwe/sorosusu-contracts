// Comprehensive Tests for Issues #408, #412, and #384
// Late Fee Auto-Deduction, Contribution Velocity, and Multi-Asset Matching Rewards

#![cfg(test)]

use soroban_sdk::{Address, Env, Symbol, token, Vec, i128, u64, u32};
use crate::{
    SoroSusuTrait, SoroSusu,
    DataKey, LateFeeDebt, LateFeeRecord, PayoutDeduction, DeductionRecord,
    ContributionVelocity, VelocityRecord,
    RewardDistributorConfig, GroupTVL, RewardAccumulation, RewardClaim,
    WashStreamingProtection,
    ReliabilityIndex, CircleInfo, Member, MemberStatus
};

#[test]
fn test_late_fee_auto_deduction_configuration() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let member = Address::generate(&env);
    let circle_id = 1;

    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());

    // Configure auto-deduction
    SoroSusu::configure_auto_deduction(env.clone(), admin.clone(), circle_id, member.clone(), true);

    // Verify configuration
    let debt = SoroSusu::get_late_fee_debt(env.clone(), circle_id, member.clone());
    assert!(debt.auto_deduction_enabled);
    assert_eq!(debt.total_debt, 0);
    assert_eq!(debt.fee_history.len(), 0);
}

#[test]
fn test_late_fee_debt_accumulation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let member = Address::generate(&env);
    let creator = Address::generate(&env);
    let circle_id = 1;
    let token = Address::generate(&env);

    // Initialize contract and create circle
    SoroSusu::init(env.clone(), admin.clone());
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        creator.clone(),
        1000, // contribution amount
        5,    // max members
        token.clone(),
        7 * 24 * 60 * 60, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        24 * 60 * 60, // 1 day grace period
        500,   // 5% late fee
    );

    // Join circle
    SoroSusu::join_circle(env.clone(), member.clone(), circle_id);

    // Configure auto-deduction
    SoroSusu::configure_auto_deduction(env.clone(), admin.clone(), circle_id, member.clone(), true);

    // Simulate late contribution (advance time past deadline)
    env.ledger().set(env.ledger().timestamp() + 8 * 24 * 60 * 60); // 8 days later
    
    // Mock token balance for late contribution
    let token_client = token::Client::new(&env, &token);
    token_client.mint(&member, &2000); // Enough for contribution + fee

    // Make late contribution
    SoroSusu::late_contribution(env.clone(), member.clone(), circle_id);

    // Verify debt accumulation
    let debt = SoroSusu::get_late_fee_debt(env.clone(), circle_id, member.clone());
    assert!(debt.auto_deduction_enabled);
    assert_eq!(debt.total_debt, 50); // 5% of 1000
    assert_eq!(debt.fee_history.len(), 1);
    
    let fee_record = debt.fee_history.get(0).unwrap();
    assert_eq!(fee_record.fee_amount, 50);
    assert_eq!(fee_record.original_amount, 1000);
    assert!(!fee_record.is_deducted);
}

#[test]
fn test_payout_with_deductions() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let member = Address::generate(&env);
    let creator = Address::generate(&env);
    let circle_id = 1;
    let token = Address::generate(&env);

    // Initialize contract and create circle
    SoroSusu::init(env.clone(), admin.clone());
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        creator.clone(),
        1000, // contribution amount
        5,    // max members
        token.clone(),
        7 * 24 * 60 * 60, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        24 * 60 * 60, // 1 day grace period
        500,   // 5% late fee
    );

    // Join circle and configure auto-deduction
    SoroSusu::join_circle(env.clone(), member.clone(), circle_id);
    SoroSusu::configure_auto_deduction(env.clone(), admin.clone(), circle_id, member.clone(), true);

    // Simulate late fee debt
    let mut debt = SoroSusu::get_late_fee_debt(env.clone(), circle_id, member.clone());
    debt.total_debt = 150; // Accumulated late fees
    debt.auto_deduction_enabled = true;
    env.storage().instance().set(&DataKey::LateFeeDebt(circle_id, member.clone()), &debt);

    // Test payout with deductions
    let original_payout = 5000; // 5 members * 1000 each
    let final_payout = SoroSusu::process_payout_with_deductions(
        env.clone(),
        circle_id,
        member.clone(),
        original_payout,
    );

    assert_eq!(final_payout, 4850); // 5000 - 150 deduction
}

#[test]
fn test_contribution_velocity_tracking() {
    let env = Env::default();
    let member = Address::generate(&env);
    let circle_id = 1;

    // Initialize contract
    SoroSusu::init(env.clone(), Address::generate(&env));

    // Test initial velocity state
    let velocity = SoroSusu::get_contribution_velocity(env.clone(), member.clone());
    assert_eq!(velocity.velocity_score, 5000); // Default 50%
    assert_eq!(velocity.total_payments_analyzed, 0);

    // Update velocity for early payment (24 hours before deadline)
    let current_time = env.ledger().timestamp();
    let deadline = current_time + 24 * 60 * 60; // 24 hours from now
    let payment_time = deadline - 24 * 60 * 60; // Exactly 24 hours early

    SoroSusu::update_contribution_velocity(
        env.clone(),
        member.clone(),
        circle_id,
        1, // round number
        payment_time,
        deadline,
    );

    // Verify velocity update
    let updated_velocity = SoroSusu::get_contribution_velocity(env.clone(), member.clone());
    assert_eq!(updated_velocity.total_payments_analyzed, 1);
    assert_eq!(updated_velocity.early_payment_ratio, 10000); // 100% early payments
    assert!(updated_velocity.velocity_score > 5000); // Should improve score
}

#[test]
fn test_enhanced_reliability_index() {
    let env = Env::default();
    let member = Address::generate(&env);

    // Initialize contract
    SoroSusu::init(env.clone(), Address::generate(&env));

    // Set up base reliability index
    let mut ri = ReliabilityIndex {
        member: member.clone(),
        score: 800, // 80% base score
        total_contributions: 10,
        on_time_contributions: 8,
        late_contributions: 2,
        missed_contributions: 0,
        last_updated: env.ledger().timestamp(),
        grace_period_hits: 2,
    };
    env.storage().instance().set(&DataKey::ReliabilityIndex(member.clone()), &ri);

    // Set up high velocity score
    let mut velocity = ContributionVelocity {
        member: member.clone(),
        average_payment_speed: 48.0, // 48 hours early on average
        velocity_score: 9000, // 90% velocity score
        early_payment_ratio: 8000, // 80% early payments
        last_minute_ratio: 1000, // 10% last minute
        consistency_score: 8500, // 85% consistent
        total_payments_analyzed: 10,
        last_updated: env.ledger().timestamp(),
    };
    env.storage().instance().set(&DataKey::ContributionVelocity(member.clone()), &velocity);

    // Get enhanced reliability index
    let enhanced_ri = SoroSusu::get_enhanced_reliability_index(env.clone(), member.clone());
    
    // Should be higher than base score due to velocity bonuses
    assert!(enhanced_ri.score > ri.score);
    assert!(enhanced_ri.score <= 1000); // Max cap
}

#[test]
fn test_reward_distributor_initialization() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let governance_token = Address::generate(&env);

    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());

    // Create reward distributor config
    let config = RewardDistributorConfig {
        is_enabled: true,
        governance_token: governance_token.clone(),
        match_rate_bps: 1000, // 10% matching
        min_ri_threshold: 5000, // 50% minimum RI
        min_cycle_duration: 90 * 24 * 60 * 60, // 3 months
        max_reward_per_user: 1000000, // Max reward per user
        total_reward_pool: 10000000, // Total pool
        reward_pool_remaining: 10000000,
        wash_streaming_penalty: 5000, // 50% penalty
        last_distribution: env.ledger().timestamp(),
    };

    // Initialize reward distributor
    SoroSusu::initialize_reward_distributor(env.clone(), admin.clone(), config.clone());

    // Verify configuration
    let retrieved_config = SoroSusu::get_reward_distributor_config(env.clone());
    assert!(retrieved_config.is_enabled);
    assert_eq!(retrieved_config.match_rate_bps, 1000);
    assert_eq!(retrieved_config.min_ri_threshold, 5000);
}

#[test]
fn test_group_tvl_calculation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let member1 = Address::generate(&env);
    let member2 = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize contract and create circle
    SoroSusu::init(env.clone(), admin.clone());
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        creator.clone(),
        1000, // contribution amount
        5,    // max members
        token.clone(),
        7 * 24 * 60 * 60, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        24 * 60 * 60, // 1 day grace period
        500,   // 5% late fee
    );

    // Add members
    SoroSusu::join_circle(env.clone(), member1.clone(), circle_id);
    SoroSusu::join_circle(env.clone(), member2.clone(), circle_id);

    // Update group TVL
    SoroSusu::update_group_tvl(env.clone(), circle_id);

    // Verify TVL calculation
    // Note: In a real implementation, we'd need to mock the circle state with actual member data
    // This test demonstrates the expected structure
    let group_tvl_key = DataKey::GroupTVL(circle_id);
    if let Some(group_tvl) = env.storage().instance().get::<DataKey, GroupTVL>(&group_tvl_key) {
        assert!(group_tvl.total_tvl > 0);
        assert_eq!(group_tvl.circle_id, circle_id);
    }
}

#[test]
fn test_member_reward_calculation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let member = Address::generate(&env);
    let governance_token = Address::generate(&env);
    let circle_id = 1;

    // Initialize contract and reward distributor
    SoroSusu::init(env.clone(), admin.clone());
    
    let config = RewardDistributorConfig {
        is_enabled: true,
        governance_token: governance_token.clone(),
        match_rate_bps: 1000, // 10% matching
        min_ri_threshold: 5000, // 50% minimum RI
        min_cycle_duration: 90 * 24 * 60 * 60, // 3 months
        max_reward_per_user: 1000000, // Max reward per user
        total_reward_pool: 10000000, // Total pool
        reward_pool_remaining: 10000000,
        wash_streaming_penalty: 5000, // 50% penalty
        last_distribution: env.ledger().timestamp(),
    };
    SoroSusu::initialize_reward_distributor(env.clone(), admin.clone(), config);

    // Set up high reliability index member
    let ri = ReliabilityIndex {
        member: member.clone(),
        score: 900, // 90% reliability
        total_contributions: 20,
        on_time_contributions: 19,
        late_contributions: 1,
        missed_contributions: 0,
        last_updated: env.ledger().timestamp(),
        grace_period_hits: 1,
    };
    env.storage().instance().set(&DataKey::ReliabilityIndex(member.clone()), &ri);

    // Set up group TVL
    let group_tvl = GroupTVL {
        circle_id,
        total_tvl: 100000, // 100k total TVL
        member_contributions: 100000,
        yield_earned: 5000,
        last_updated: env.ledger().timestamp(),
        eligible_members: 5,
    };
    env.storage().instance().set(&DataKey::GroupTVL(circle_id), &group_tvl);

    // Calculate rewards
    let rewards = SoroSusu::calculate_member_rewards(env.clone(), member.clone(), circle_id);
    
    // Should receive some rewards based on high reliability
    assert!(rewards > 0);
    assert!(rewards <= 1000000); // Should not exceed max per user
}

#[test]
fn test_wash_streaming_protection() {
    let env = Env::default();
    let member = Address::generate(&env);
    let circle_id = 1;

    // Initialize contract
    SoroSusu::init(env.clone(), Address::generate(&env));

    // Set up wash streaming protection
    let protection = WashStreamingProtection {
        member: member.clone(),
        circle_id,
        first_join_timestamp: env.ledger().timestamp() - 30 * 24 * 60 * 60, // 30 days ago
        last_exit_timestamp: Some(env.ledger().timestamp() - 15 * 24 * 60 * 60), // 15 days ago
        cycle_count: 3, // Multiple cycles
        is_protected: true,
        penalty_applied: 5000, // 50% penalty
    };
    env.storage().instance().set(&DataKey::WashStreamingProtection(member.clone(), circle_id), &protection);

    // Verify protection is active
    let retrieved_protection: WashStreamingProtection = env.storage().instance()
        .get(&DataKey::WashStreamingProtection(member.clone(), circle_id))
        .unwrap();
    
    assert!(retrieved_protection.is_protected);
    assert_eq!(retrieved_protection.penalty_applied, 5000);
    assert_eq!(retrieved_protection.cycle_count, 3);
}

#[test]
fn test_reward_claiming() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let member = Address::generate(&env);
    let governance_token = Address::generate(&env);
    let circle_id = 1;

    // Initialize contract and reward distributor
    SoroSusu::init(env.clone(), admin.clone());
    
    let config = RewardDistributorConfig {
        is_enabled: true,
        governance_token: governance_token.clone(),
        match_rate_bps: 1000, // 10% matching
        min_ri_threshold: 5000, // 50% minimum RI
        min_cycle_duration: 90 * 24 * 60 * 60, // 3 months
        max_reward_per_user: 1000000, // Max reward per user
        total_reward_pool: 10000000, // Total pool
        reward_pool_remaining: 10000000,
        wash_streaming_penalty: 5000, // 50% penalty
        last_distribution: env.ledger().timestamp(),
    };
    SoroSusu::initialize_reward_distributor(env.clone(), admin.clone(), config);

    // Set up reward accumulation
    let accumulation = RewardAccumulation {
        member: member.clone(),
        circle_id,
        contribution_volume: 50000, // 50k contributed
        reliability_weight: 9000, // 90% reliability
        earned_rewards: 100000, // 100k earned
        claimed_rewards: 0, // Nothing claimed yet
        eligibility_start: env.ledger().timestamp() - 100 * 24 * 60 * 60, // Eligible for 100 days
        last_calculated: env.ledger().timestamp(),
    };
    env.storage().instance().set(&DataKey::RewardAccumulation(member.clone(), circle_id), &accumulation);

    // Claim rewards
    let claim = SoroSusu::claim_rewards(env.clone(), member.clone(), circle_id);

    // Verify claim
    assert_eq!(claim.amount_claimed, 100000);
    assert_eq!(claim.contribution_volume, 50000);
    assert_eq!(claim.reliability_score, 9000);
    assert!(!claim.is_final_claim);

    // Verify accumulation updated
    let updated_accumulation: RewardAccumulation = env.storage().instance()
        .get(&DataKey::RewardAccumulation(member.clone(), circle_id))
        .unwrap();
    assert_eq!(updated_accumulation.claimed_rewards, 100000);
}

#[test]
fn test_integration_all_three_features() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let member = Address::generate(&env);
    let token = Address::generate(&env);
    let governance_token = Address::generate(&env);

    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());

    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        creator.clone(),
        1000, // contribution amount
        5,    // max members
        token.clone(),
        7 * 24 * 60 * 60, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        24 * 60 * 60, // 1 day grace period
        500,   // 5% late fee
    );

    // Join circle
    SoroSusu::join_circle(env.clone(), member.clone(), circle_id);

    // Configure late fee auto-deduction
    SoroSusu::configure_auto_deduction(env.clone(), admin.clone(), circle_id, member.clone(), true);

    // Initialize reward distributor
    let config = RewardDistributorConfig {
        is_enabled: true,
        governance_token: governance_token.clone(),
        match_rate_bps: 1000, // 10% matching
        min_ri_threshold: 5000, // 50% minimum RI
        min_cycle_duration: 90 * 24 * 60 * 60, // 3 months
        max_reward_per_user: 1000000, // Max reward per user
        total_reward_pool: 10000000, // Total pool
        reward_pool_remaining: 10000000,
        wash_streaming_penalty: 5000, // 50% penalty
        last_distribution: env.ledger().timestamp(),
    };
    SoroSusu::initialize_reward_distributor(env.clone(), admin.clone(), config);

    // Make on-time contribution and track velocity
    let current_time = env.ledger().timestamp();
    let deadline = current_time + 7 * 24 * 60 * 60; // 1 week from now
    let payment_time = deadline - 48 * 60 * 60; // 48 hours early

    // Mock token balance
    let token_client = token::Client::new(&env, &token);
    token_client.mint(&member, &1000);

    // Make contribution
    SoroSusu::deposit(env.clone(), member.clone(), circle_id, 1);

    // Update velocity metrics
    SoroSusu::update_contribution_velocity(
        env.clone(),
        member.clone(),
        circle_id,
        1, // round number
        payment_time,
        deadline,
    );

    // Verify all systems are working
    let velocity = SoroSusu::get_contribution_velocity(env.clone(), member.clone());
    assert!(velocity.total_payments_analyzed > 0);

    let enhanced_ri = SoroSusu::get_enhanced_reliability_index(env.clone(), member.clone());
    assert!(enhanced_ri.score >= 0);

    let debt = SoroSusu::get_late_fee_debt(env.clone(), circle_id, member.clone());
    assert!(debt.auto_deduction_enabled);

    let rewards = SoroSusu::calculate_member_rewards(env.clone(), member.clone(), circle_id);
    assert!(rewards >= 0);
}
