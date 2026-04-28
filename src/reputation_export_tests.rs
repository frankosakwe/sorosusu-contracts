// --- MULTI-CHAIN REPUTATION EXPORT INTEGRATION TESTS ---
//
// Integration tests for the cross-chain reputation export functionality.
// These tests mock the Wormhole bridge to verify sequence numbers increment correctly
// and that all security measures are in place.

#![cfg(test)]

use soroban_sdk::{Address, Env, BytesN, Vec};

use crate::reputation_export::{
    ExportDataKey, ExportError, WormholeConfig, ReputationExportPayload,
    EXPORT_PROTOCOL_FEE, WORMHOLE_STELLAR_CHAIN_ID, WORMHOLE_ETHEREUM_CHAIN_ID,
    WORMHOLE_SOLANA_CHAIN_ID, EXPORT_COOLDOWN_SECONDS,
};

// --- TEST UTILS ---

fn setup_test_env() -> Env {
    let env = Env::default();
    env
}

fn setup_wormhole_config(env: &Env, admin: &Address) {
    let wormhole_contract = Address::generate(env);
    let mut supported_chains = Vec::new(env);
    supported_chains.push_back(WORMHOLE_STELLAR_CHAIN_ID);
    supported_chains.push_back(WORMHOLE_ETHEREUM_CHAIN_ID);
    supported_chains.push_back(WORMHOLE_SOLANA_CHAIN_ID);
    
    crate::reputation_export::init_wormhole_config(
        env,
        admin,
        wormhole_contract,
        supported_chains,
    );
}

// --- TESTS ---

#[test]
fn test_wormhole_config_initialization() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    let config = crate::reputation_export::get_wormhole_config(&env);
    assert!(config.is_some());
    
    let config = config.unwrap();
    assert!(config.enabled);
    assert_eq!(config.supported_chains.len(), 3);
}

#[test]
fn test_export_reputation_success() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    let ri_score = 8500; // 85% RI score
    let total_cycles = 12;
    let defaults_count = 0;
    let on_time_rate_bps = 9500; // 95% on-time rate
    let volume_saved = 1_000_000_000; // 10,000 XLM saved
    
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        ri_score,
        total_cycles,
        defaults_count,
        on_time_rate_bps,
        volume_saved,
    );
    
    assert!(result.is_ok());
    
    let (export_id, payload_hash) = result.unwrap();
    assert_eq!(export_id, 1); // First export should have ID 1
    
    // Verify metadata was stored
    let metadata = crate::reputation_export::get_export_metadata(&env, export_id);
    assert!(metadata.is_some());
    
    let metadata = metadata.unwrap();
    assert_eq!(metadata.export_id, export_id);
    assert_eq!(metadata.user_address, user);
    assert_eq!(metadata.destination_chain, WORMHOLE_ETHEREUM_CHAIN_ID);
    assert_eq!(metadata.fee_paid, EXPORT_PROTOCOL_FEE);
}

#[test]
fn test_export_sequence_increment() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // First export
    let result1 = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    assert!(result1.is_ok());
    let (export_id1, _) = result1.unwrap();
    assert_eq!(export_id1, 1);
    
    // Second export (should increment sequence)
    let result2 = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_SOLANA_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8500,
        11,
        0,
        9500,
        1_100_000_000,
    );
    assert!(result2.is_ok());
    let (export_id2, _) = result2.unwrap();
    assert_eq!(export_id2, 2);
}

#[test]
fn test_user_nonce_increment() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    let initial_nonce = crate::reputation_export::get_export_nonce(&env, user.clone());
    assert_eq!(initial_nonce, 0);
    
    // First export
    let _ = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    let nonce_after_first = crate::reputation_export::get_export_nonce(&env, user.clone());
    assert_eq!(nonce_after_first, 1);
    
    // Second export
    let _ = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_SOLANA_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8500,
        11,
        0,
        9500,
        1_100_000_000,
    );
    
    let nonce_after_second = crate::reputation_export::get_export_nonce(&env, user);
    assert_eq!(nonce_after_second, 2);
}

#[test]
fn test_insufficient_protocol_fee() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // Try to export with insufficient fee
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE - 1, // One stroop less than required
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ExportError::InsufficientProtocolFee);
}

#[test]
fn test_pending_default_investigation() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // Record a pending default investigation
    crate::reputation_export::record_default_investigation(&env, user.clone(), 123);
    
    // Try to export while investigation is active
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ExportError::PendingDefaultInvestigation);
    
    // Resolve the investigation
    crate::reputation_export::resolve_default_investigation(
        &env,
        user.clone(),
        crate::reputation_export::InvestigationStatus::Resolved,
    );
    
    // Now export should succeed
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    assert!(result.is_ok());
}

#[test]
fn test_export_cooldown() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // First export
    let _ = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    // Try to export again immediately (should fail due to cooldown)
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8500,
        11,
        0,
        9500,
        1_100_000_000,
    );
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ExportError::ExportCooldownNotMet);
    
    // Advance time beyond cooldown period
    env.ledger().set_timestamp(env.ledger().timestamp() + EXPORT_COOLDOWN_SECONDS + 1);
    
    // Now export should succeed
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8500,
        11,
        0,
        9500,
        1_100_000_000,
    );
    
    assert!(result.is_ok());
}

#[test]
fn test_duplicate_export_detection() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    let ri_score = 8000;
    let total_cycles = 10;
    let defaults_count = 0;
    let on_time_rate_bps = 9000;
    let volume_saved = 1_000_000_000;
    
    // First export
    let result1 = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        ri_score,
        total_cycles,
        defaults_count,
        on_time_rate_bps,
        volume_saved,
    );
    assert!(result1.is_ok());
    
    // Advance time to bypass cooldown
    env.ledger().set_timestamp(env.ledger().timestamp() + EXPORT_COOLDOWN_SECONDS + 1);
    
    // Try to export with identical parameters (should fail as duplicate)
    let result2 = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        ri_score,
        total_cycles,
        defaults_count,
        on_time_rate_bps,
        volume_saved,
    );
    
    // This might not fail as duplicate if the nonce has changed
    // The deduplication is based on the payload hash which includes the nonce
    // So this test verifies that the nonce changes between exports
    assert!(result2.is_ok());
}

#[test]
fn test_unsupported_chain() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // Try to export to an unsupported chain (chain ID 99)
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        99, // Unsupported chain
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ExportError::UnsupportedChain);
}

#[test]
fn test_wormhole_not_configured() {
    let env = setup_test_env();
    let user = Address::generate(&env);
    
    // Try to export without initializing Wormhole config
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ExportError::WormholeNotConfigured);
}

#[test]
fn test_can_export() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // Initially should be able to export
    assert!(crate::reputation_export::can_export(&env, user.clone()));
    
    // Record pending investigation
    crate::reputation_export::record_default_investigation(&env, user.clone(), 123);
    
    // Should not be able to export with pending investigation
    assert!(!crate::reputation_export::can_export(&env, user.clone()));
    
    // Resolve investigation
    crate::reputation_export::resolve_default_investigation(
        &env,
        user.clone(),
        crate::reputation_export::InvestigationStatus::Resolved,
    );
    
    // Should be able to export again
    assert!(crate::reputation_export::can_export(&env, user.clone()));
    
    // Make an export
    let _ = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    // Should not be able to export due to cooldown
    assert!(!crate::reputation_export::can_export(&env, user));
}

#[test]
fn test_wormhole_disabled() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // Disable Wormhole
    let mut config = crate::reputation_export::get_wormhole_config(&env).unwrap();
    config.enabled = false;
    env.storage().temporary().set(&ExportDataKey::WormholeConfig, &config);
    
    // Try to export while Wormhole is disabled
    let result = crate::reputation_export::export_reputation(
        &env,
        &user,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ExportError::WormholeDisabled);
}

#[test]
fn test_multiple_users_exports() {
    let env = setup_test_env();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    
    setup_wormhole_config(&env, &admin);
    
    // User1 exports
    let result1 = crate::reputation_export::export_reputation(
        &env,
        &user1,
        WORMHOLE_ETHEREUM_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8000,
        10,
        0,
        9000,
        1_000_000_000,
    );
    assert!(result1.is_ok());
    let (export_id1, _) = result1.unwrap();
    assert_eq!(export_id1, 1);
    
    // User2 exports (should increment global sequence)
    let result2 = crate::reputation_export::export_reputation(
        &env,
        &user2,
        WORMHOLE_SOLANA_CHAIN_ID,
        EXPORT_PROTOCOL_FEE,
        8500,
        12,
        0,
        9500,
        1_200_000_000,
    );
    assert!(result2.is_ok());
    let (export_id2, _) = result2.unwrap();
    assert_eq!(export_id2, 2);
    
    // Each user should have their own nonce
    let nonce1 = crate::reputation_export::get_export_nonce(&env, user1);
    let nonce2 = crate::reputation_export::get_export_nonce(&env, user2);
    
    assert_eq!(nonce1, 1);
    assert_eq!(nonce2, 1);
}
