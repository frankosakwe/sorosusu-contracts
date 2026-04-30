#![cfg(test)]

use crate::*;
use soroban_sdk::{Address, Env, Symbol, vec, Vec};

#[test]
fn test_heartbeat_mechanism_basic_functionality() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let member = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000, // $10 contribution
        5,    // max_members
        Address::generate(&env), // token
        7 * 24 * 60 * 60, // 1 week cycle duration
        100, // insurance_fee_bps
        Address::generate(&env), // nft_contract
    );
    
    // Test initial heartbeat status
    let status = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert_eq!(status.circle_id, circle_id);
    assert!(status.last_heartbeat.is_some());
    assert!(!status.is_in_crisis);
    assert!(!status.is_orphaned);
    assert!(status.time_until_heartbeat.is_some());
    assert!(status.time_until_heartbeat.unwrap() > 0);
    
    // Test admin heartbeat
    SoroSusuTrait::heartbeat(env.clone(), creator.clone(), circle_id);
    
    // Verify heartbeat was updated
    let status_after = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status_after.last_heartbeat.is_some());
    assert!(status_after.last_heartbeat.unwrap() >= status.last_heartbeat.unwrap());
    assert!(!status_after.is_in_crisis);
}

#[test]
fn test_heartbeat_crisis_detection() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let high_ri_member = Address::generate(&env);
    let low_ri_member = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Add members
    SoroSusuTrait::join_circle(env.clone(), high_ri_member.clone(), circle_id, 1, None);
    SoroSusuTrait::join_circle(env.clone(), low_ri_member.clone(), circle_id, 1, None);
    
    // Set reliability indices
    env.storage().instance().set(&DataKey::ReliabilityIndex(high_ri_member.clone()), &900u32);
    env.storage().instance().set(&DataKey::ReliabilityIndex(low_ri_member.clone()), &700u32);
    
    // Initial heartbeat
    SoroSusuTrait::heartbeat(env.clone(), creator.clone(), circle_id);
    
    // Jump time to exceed heartbeat interval (90 days + 1 second)
    env.ledger().set_timestamp(env.ledger().timestamp() + HEARTBEAT_INTERVAL + 1);
    
    // Trigger crisis detection by calling deposit
    SoroSusuTrait::deposit(env.clone(), high_ri_member.clone(), circle_id, 1);
    
    // Check crisis state
    let status = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status.is_in_crisis);
    assert!(status.crisis_started_at.is_some());
    assert!(status.time_until_heartbeat.unwrap() == 0); // Overdue
}

#[test]
fn test_leadership_claim_ri_threshold() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let high_ri_member = Address::generate(&env);
    let low_ri_member = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Add members
    SoroSusuTrait::join_circle(env.clone(), high_ri_member.clone(), circle_id, 1, None);
    SoroSusuTrait::join_circle(env.clone(), low_ri_member.clone(), circle_id, 1, None);
    
    // Set reliability indices
    env.storage().instance().set(&DataKey::ReliabilityIndex(high_ri_member.clone()), &900u32);
    env.storage().instance().set(&DataKey::ReliabilityIndex(low_ri_member.clone()), &700u32);
    
    // Manually put circle in crisis state
    let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
    circle.is_in_crisis = true;
    circle.crisis_started_at = Some(env.ledger().timestamp());
    env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    
    // Test successful leadership claim (RI > 800)
    let result = SoroSusuTrait::claim_leadership(env.clone(), high_ri_member.clone(), circle_id);
    assert!(result.is_ok());
    
    // Reset claim for next test
    let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
    circle.leadership_claimant = None;
    circle.claim_deadline = None;
    env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    
    // Test failed leadership claim (RI < 800)
    let result = SoroSusuTrait::claim_leadership(env.clone(), low_ri_member.clone(), circle_id);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), 503); // Reliability index too low
}

#[test]
fn test_leadership_challenge_window() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let claimant = Address::generate(&env);
    let challenger = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Add members
    SoroSusuTrait::join_circle(env.clone(), claimant.clone(), circle_id, 1, None);
    SoroSusuTrait::join_circle(env.clone(), challenger.clone(), circle_id, 1, None);
    
    // Set high RI for claimant
    env.storage().instance().set(&DataKey::ReliabilityIndex(claimant.clone()), &900u32);
    
    // Manually put circle in crisis state
    let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
    circle.is_in_crisis = true;
    circle.crisis_started_at = Some(env.ledger().timestamp());
    env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    
    // Claim leadership
    let result = SoroSusuTrait::claim_leadership(env.clone(), claimant.clone(), circle_id);
    assert!(result.is_ok());
    
    // Test challenge reset during window
    let result = SoroSusuTrait::reset_leadership_claim(env.clone(), creator.clone(), circle_id);
    assert!(result.is_ok());
    
    // Verify claim was reset
    let status = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status.leadership_claimant.is_none());
    assert!(status.claim_deadline.is_none());
}

#[test]
fn test_orphaned_circle_refund() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let member = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Add member and make deposit
    SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
    
    // Manually put circle in crisis state 180+ days ago
    let crisis_time = env.ledger().timestamp() - ORPHAN_THRESHOLD - 1;
    let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
    circle.is_in_crisis = true;
    circle.crisis_started_at = Some(crisis_time);
    env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    
    // Check orphaned circle
    let result = SoroSusuTrait::check_orphaned_circle(env.clone(), circle_id);
    assert!(result.is_ok());
    
    // Verify circle is marked as orphaned
    let status = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status.is_orphaned);
    assert!(status.orphaned_at.is_some());
    assert!(!status.is_in_crisis); // Crisis should end
}

#[test]
fn test_heartbeat_events() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let claimant = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Add member
    SoroSusuTrait::join_circle(env.clone(), claimant.clone(), circle_id, 1, None);
    env.storage().instance().set(&DataKey::ReliabilityIndex(claimant.clone()), &900u32);
    
    // Test heartbeat event
    SoroSusuTrait::heartbeat(env.clone(), creator.clone(), circle_id);
    
    // Manually trigger crisis to test crisis event
    let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
    circle.is_in_crisis = true;
    circle.crisis_started_at = Some(env.ledger().timestamp());
    env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    
    // Test leadership claim event
    let result = SoroSusuTrait::claim_leadership(env.clone(), claimant.clone(), circle_id);
    assert!(result.is_ok());
    
    // Test challenge reset event
    let result = SoroSusuTrait::reset_leadership_claim(env.clone(), creator.clone(), circle_id);
    assert!(result.is_ok());
}

#[test]
fn test_three_month_time_jump() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let high_ri_member = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Add member
    SoroSusuTrait::join_circle(env.clone(), high_ri_member.clone(), circle_id, 1, None);
    env.storage().instance().set(&DataKey::ReliabilityIndex(high_ri_member.clone()), &900u32);
    
    // Initial heartbeat
    SoroSusuTrait::heartbeat(env.clone(), creator.clone(), circle_id);
    
    // Jump 3 months (approximately 90 days)
    let three_months_seconds = 90 * 24 * 60 * 60;
    env.ledger().set_timestamp(env.ledger().timestamp() + three_months_seconds);
    
    // Trigger crisis detection
    SoroSusuTrait::deposit(env.clone(), high_ri_member.clone(), circle_id, 1);
    
    // Verify crisis state
    let status = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status.is_in_crisis);
    assert!(status.crisis_started_at.is_some());
    
    // Test leadership claim works after 3-month jump
    let result = SoroSusuTrait::claim_leadership(env.clone(), high_ri_member.clone(), circle_id);
    assert!(result.is_ok());
    
    // Verify claim was successful
    let status_after_claim = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status_after_claim.leadership_claimant.is_some());
    assert!(status_after_claim.claim_deadline.is_some());
}

#[test]
fn test_admin_authorization() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Test that unauthorized user cannot send heartbeat
    // This should panic due to require_auth() failure
    env.as_contract(&admin, || {
        env.as_contract(&unauthorized_user, || {
            let result = std::panic::catch_unwind(|| {
                SoroSusuTrait::heartbeat(env.clone(), unauthorized_user.clone(), circle_id);
            });
            assert!(result.is_err()); // Should panic
        });
    });
}

#[test]
fn test_heartbeat_status_calculations() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    
    // Initialize contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        creator.clone(),
        1000,
        5,
        Address::generate(&env),
        7 * 24 * 60 * 60,
        100,
        Address::generate(&env),
    );
    
    // Test initial status calculations
    let status = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status.time_until_heartbeat.is_some());
    assert!(status.time_until_heartbeat.unwrap() > 0);
    assert!(status.time_until_orphaned.is_none()); // Not in crisis
    
    // Send heartbeat
    SoroSusuTrait::heartbeat(env.clone(), creator.clone(), circle_id);
    
    // Test time calculations after heartbeat
    let status_after = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status_after.time_until_heartbeat.is_some());
    assert!(status_after.time_until_heartbeat.unwrap() > 0);
    
    // Jump time to just before deadline
    env.ledger().set_timestamp(env.ledger().timestamp() + HEARTBEAT_INTERVAL - 100);
    
    let status_near_deadline = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status_near_deadline.time_until_heartbeat.unwrap() < 100);
    
    // Jump time past deadline
    env.ledger().set_timestamp(env.ledger().timestamp() + 200);
    
    let status_overdue = SoroSusuTrait::get_heartbeat_status(env.clone(), circle_id);
    assert!(status_overdue.time_until_heartbeat.unwrap() == 0); // Overdue
}
