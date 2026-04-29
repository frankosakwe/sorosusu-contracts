//! Tests for Contribution Security Module
//! 
//! Tests atomic transactions, double-spend prevention, and Merkle proof generation

#![cfg(test)]

use soroban_sdk::{Address, Env, Symbol, BytesN, Vec};
use crate::contribution_security::*;
use crate::DataKey;
use crate::{SoroSusuTrait, SoroSusu, CircleInfo, Member, MemberStatus};

#[test]
fn test_atomic_contribution_transaction() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        Address::generate(&env), // token
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle
    SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
    
    // Start atomic transaction
    let tx_id = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    
    // Verify transaction is pending
    let state = ContributionSecurityTrait::get_transaction_state(env.clone(), tx_id.clone());
    assert_eq!(state, Some(TransactionState::Pending));
    
    // Commit transaction
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id.clone()).unwrap();
    
    // Verify transaction is committed
    let state = ContributionSecurityTrait::get_transaction_state(env.clone(), tx_id.clone());
    assert_eq!(state, Some(TransactionState::Committed));
}

#[test]
fn test_double_spend_prevention() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        Address::generate(&env), // token
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle
    SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
    
    // Start first transaction
    let tx_id1 = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    
    // Try to start second transaction (should fail)
    let result = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    );
    
    assert!(matches!(result, Err(ContributionSecurityError::DoubleSpendAttempt)));
    
    // Commit first transaction
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id1).unwrap();
    
    // Now second transaction should work
    let tx_id2 = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    
    assert_ne!(tx_id1, tx_id2);
}

#[test]
fn test_transaction_rollback() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        Address::generate(&env), // token
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle
    SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
    
    // Get initial state
    let member_key = DataKey::Member(user.clone());
    let initial_member: Member = env.storage().instance().get(&member_key).unwrap();
    let initial_circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
    
    // Start transaction
    let tx_id = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    
    // Simulate some state changes (in real scenario, this would be done by deposit function)
    let mut circle = initial_circle.clone();
    let mut member = initial_member.clone();
    
    member.contribution_count += 1;
    member.last_contribution_time = env.ledger().timestamp();
    circle.contribution_bitmap |= 1u64 << member.index;
    
    env.storage().instance().set(&member_key, &member);
    env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    
    // Rollback transaction
    ContributionSecurityTrait::rollback_contribution_transaction(
        env.clone(),
        tx_id,
        Symbol::short(&env, "test_rollback"),
    ).unwrap();
    
    // Verify state was restored
    let rolled_back_member: Member = env.storage().instance().get(&member_key).unwrap();
    let rolled_back_circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
    
    assert_eq!(rolled_back_member.contribution_count, initial_member.contribution_count);
    assert_eq!(rolled_back_circle.contribution_bitmap, initial_circle.contribution_bitmap);
    
    // Verify transaction state
    let state = ContributionSecurityTrait::get_transaction_state(env.clone(), tx_id);
    assert_eq!(state, Some(TransactionState::RolledBack));
}

#[test]
fn test_merkle_proof_generation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        Address::generate(&env), // token
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle
    SoroSusuTrait::join_circle(env.clone(), user1.clone(), circle_id);
    SoroSusuTrait::join_circle(env.clone(), user2.clone(), circle_id);
    
    // Make contributions
    let tx_id1 = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user1.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id1).unwrap();
    
    let tx_id2 = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user2.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id2).unwrap();
    
    // Generate Merkle proof for user1
    let proof = SoroSusuTrait::generate_contribution_proof(
        env.clone(),
        user1.clone(),
        circle_id,
        0, // round 0
    ).unwrap();
    
    // Verify the proof
    let is_valid = SoroSusuTrait::verify_contribution_proof(env.clone(), proof.clone()).unwrap();
    assert!(is_valid);
    
    // Check Merkle root
    let root = SoroSusuTrait::get_circle_merkle_root(env.clone(), circle_id);
    assert!(root.is_some());
    assert_eq!(root.unwrap(), proof.root);
}

#[test]
fn test_merkle_proof_invalid_user() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        Address::generate(&env), // token
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle with user1 only
    SoroSusuTrait::join_circle(env.clone(), user1.clone(), circle_id);
    
    // Make contribution from user1
    let tx_id1 = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user1.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id1).unwrap();
    
    // Try to generate proof for user2 (who hasn't contributed)
    let result = SoroSusuTrait::generate_contribution_proof(
        env.clone(),
        user2.clone(),
        circle_id,
        0, // round 0
    );
    
    assert!(result.is_err());
}

#[test]
fn test_secure_deposit_integration() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        token.clone(),
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle
    SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
    
    // Mock token balance (in real test, you'd use token client)
    let token_client = soroban_sdk::token::Client::new(&env, &token);
    token_client.mint(&user, &10000);
    
    // Approve contract to spend tokens
    token_client.approve(&user, &env.current_contract_address(), &10000);
    
    // Make secure deposit
    SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id, 1);
    
    // Verify Merkle root exists
    let root = SoroSusuTrait::get_circle_merkle_root(env.clone(), circle_id);
    assert!(root.is_some());
    
    // Generate and verify proof
    let proof = SoroSusuTrait::generate_contribution_proof(
        env.clone(),
        user.clone(),
        circle_id,
        0,
    ).unwrap();
    
    let is_valid = SoroSusuTrait::verify_contribution_proof(env.clone(), proof).unwrap();
    assert!(is_valid);
}

#[test]
fn test_transaction_state_transitions() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        Address::generate(&env), // token
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle
    SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
    
    // Start transaction
    let tx_id = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    
    // Should be pending
    assert_eq!(
        ContributionSecurityTrait::get_transaction_state(env.clone(), tx_id.clone()),
        Some(TransactionState::Pending)
    );
    
    // Commit
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id.clone()).unwrap();
    
    // Should be committed
    assert_eq!(
        ContributionSecurityTrait::get_transaction_state(env.clone(), tx_id.clone()),
        Some(TransactionState::Committed)
    );
    
    // Try to rollback committed transaction (should fail)
    let result = ContributionSecurityTrait::rollback_contribution_transaction(
        env.clone(),
        tx_id,
        Symbol::short(&env, "should_fail"),
    );
    
    assert!(matches!(result, Err(ContributionSecurityError::TransactionAlreadyCommitted)));
}

#[test]
fn test_contribution_nonce_tracking() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Setup contract
    SoroSusuTrait::init(env.clone(), admin.clone());
    
    // Create a test circle
    let circle_id = SoroSusuTrait::create_circle(
        env.clone(),
        admin.clone(),
        1000, // contribution amount
        5,    // max members
        Address::generate(&env), // token
        86400 * 7, // 1 week cycle
        true,  // yield enabled
        500,   // risk tolerance
        86400, // grace period
        100,   // late fee bps
    );
    
    // Join circle
    SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
    
    // First contribution
    let tx_id1 = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id1).unwrap();
    
    // Second contribution
    let tx_id2 = ContributionSecurityTrait::start_contribution_transaction(
        env.clone(),
        user.clone(),
        circle_id,
        1000,
        1,
    ).unwrap();
    ContributionSecurityTrait::commit_contribution_transaction(env.clone(), tx_id2).unwrap();
    
    // Generate proofs for both contributions
    let proof1 = SoroSusuTrait::generate_contribution_proof(
        env.clone(),
        user.clone(),
        circle_id,
        0,
    ).unwrap();
    
    let proof2 = SoroSusuTrait::generate_contribution_proof(
        env.clone(),
        user.clone(),
        circle_id,
        0,
    ).unwrap();
    
    // Proofs should be different due to different nonces
    assert_ne!(proof1.leaf.nonce, proof2.leaf.nonce);
    
    // Both should be valid
    assert!(SoroSusuTrait::verify_contribution_proof(env.clone(), proof1).unwrap());
    assert!(SoroSusuTrait::verify_contribution_proof(env.clone(), proof2).unwrap());
}
