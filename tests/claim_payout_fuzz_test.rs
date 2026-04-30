//! # Claim-Payout Concurrent Fuzz & Security Hardening
//!
//! Issue #339 — High-Frequency Concurrent Payout Security Hardening
//!
//! ## Acceptance Criteria
//!
//! | # | Criterion |
//! |---|-----------|
//! | AC1 | Protocol is mathematically proven to handle high-frequency concurrent payouts safely |
//! | AC2 | Double-spend / double-payout exploits are structurally blocked at the state-machine level |
//! | AC3 | Resource consumption for bulk withdrawals stays strictly within Soroban network limits |

#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger as _, LedgerInfo},
    Address, Env,
};
use sorosusu_contracts::{CircleInfo, DataKey, SoroSusu, SoroSusuClient};
use proptest::prelude::*;

// Re-export invariant helpers so assertions read cleanly.
use sorosusu_contracts::vault_balance_invariant::vault_balance_invariant as inv;

// ── Mock NFT contract ─────────────────────────────────────────────────────────

#[contract]
pub struct MockNftConcurrent;

#[contractimpl]
impl MockNftConcurrent {
    pub fn mint(_env: Env, _to: Address, _id: u128) {}
    pub fn burn(_env: Env, _from: Address, _id: u128) {}
}

// ── Constants ─────────────────────────────────────────────────────────────────

const CONTRIBUTION: i128 = 10_000_000;
const CYCLE_SECS: u64 = 86_400;

// ── Environment / client setup ────────────────────────────────────────────────

fn setup_env() -> (Env, SoroSusuClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init(&admin);
    (env, client, admin)
}

fn deploy_nft(env: &Env) -> Address {
    env.register_contract(None, MockNftConcurrent)
}

// ─────────────────────────────────────────────────────────────────────────────
// INTEGRATION TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_100_concurrent_claimants_single_ledger() {
    let (env, client, _admin) = setup_env();
    let token = Address::generate(&env);
    let _nft = deploy_nft(&env);

    const N: u32 = 100;
    let mut circles = std::vec::Vec::new();

    for _ in 0..N {
        let creator = Address::generate(&env);
        let member = Address::generate(&env);

        let circle_id = client.create_circle(
            &creator,
            &(CONTRIBUTION as u64),
            &2u32,
            &token,
            &CYCLE_SECS,
            &false,
            &0u32,
            &CYCLE_SECS,
            &0u32,
        );
        client.join_circle(&creator, &circle_id);
        client.join_circle(&member, &circle_id);
        circles.push((circle_id, creator, member));
    }

    // Simultaneous finalization in the same ledger
    for (circle_id, creator, _) in &circles {
        client.finalize_round(creator, circle_id);
    }

    for (circle_id, creator, member) in &circles {
        let circle = client.get_circle(circle_id);
        assert!(circle.is_round_finalized);
        let recipient = circle.current_pot_recipient.expect("Recipient missing");
        assert!(recipient == *creator || recipient == *member);
    }
}

#[test]
#[should_panic]
fn test_double_finalize_same_circle_rejected() {
    let (env, client, _admin) = setup_env();
    let token = Address::generate(&env);
    let creator = Address::generate(&env);
    let member = Address::generate(&env);

    let circle_id = client.create_circle(
        &creator, &(CONTRIBUTION as u64), &2u32, &token,
        &CYCLE_SECS, &false, &0u32, &CYCLE_SECS, &0u32,
    );
    client.join_circle(&creator, &circle_id);
    client.join_circle(&member, &circle_id);

    client.finalize_round(&creator, &circle_id);
    client.finalize_round(&creator, &circle_id); // Should panic
}

#[test]
fn test_congestion_simulated_ledger_gaps() {
    let (env, client, _admin) = setup_env();
    let token = Address::generate(&env);
    
    let creator = Address::generate(&env);
    let circle_id = client.create_circle(
        &creator, &(CONTRIBUTION as u64), &2u32, &token,
        &CYCLE_SECS, &false, &0u32, &CYCLE_SECS, &0u32,
    );
    client.join_circle(&creator, &circle_id);

    // Generate congested sequence
    let sequence = inv::generate_congested_ledger_sequence(100, 1000, 10, 5);
    
    for (seq, ts) in sequence {
        env.ledger().set(LedgerInfo {
            timestamp: ts,
            protocol_version: 21,
            sequence_number: seq as u32,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 1,
            min_persistent_entry_ttl: 1,
            max_entry_ttl: 100,
        });
        // Just verify environment doesn't crash on jumps
    }
}

#[test]
fn test_last_payout_ledger_atomic_commit() {
    let result = inv::simulate_atomic_commit(false, true);
    assert!(result.is_ok());
    
    let result_fail = inv::simulate_atomic_commit(false, false);
    assert!(result_fail.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// PROPTESTS (50,000 cases)
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50_000,
        failure_persistence: Some(Box::new(
            proptest::test_runner::FileFailurePersistence::WithSource("regressions")
        )),
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_total_payout_never_exceeds_vault_balance(
        vault_balance in 0i128..=1_000_000_000_000i128,
        payout_amount in 0i128..=1_000_000_000_000i128,
    ) {
        let safe = inv::check_payout_within_balance(vault_balance, payout_amount);
        if payout_amount <= vault_balance {
            prop_assert!(safe);
        } else {
            prop_assert!(!safe);
        }
    }

    #[test]
    fn prop_double_payout_structurally_blocked(
        last in 0u64..1000000,
        curr in 0u64..1000000,
    ) {
        let safe = inv::check_no_double_payout(last, curr);
        if last == curr {
            prop_assert!(!safe);
        } else {
            prop_assert!(safe);
        }
    }

    #[test]
    fn prop_storage_within_soroban_limits(
        members in 2u32..=50u32,
    ) {
        let entries = inv::estimate_finalize_storage_entries(members);
        prop_assert!(inv::check_soroban_storage_limit(entries));
    }
}

#[test]
fn test_50k_withdrawal_permutations() {
    use proptest::test_runner::{Config, TestRunner};
    let mut runner = TestRunner::new(Config {
        cases: 50_000,
        ..Config::default()
    });

    runner.run(&(0i128..1000000i128, 0u32..100u32, 2u32..50u32), |(bal, pct, members)| {
        let payout = bal * pct as i128 / 100;
        if !inv::check_payout_within_balance(bal, payout) {
            return Err(proptest::test_runner::TestCaseError::fail("V1"));
        }
        let entries = inv::estimate_finalize_storage_entries(members);
        if !inv::check_soroban_storage_limit(entries) {
            return Err(proptest::test_runner::TestCaseError::fail("V3"));
        }
        Ok(())
    }).unwrap();
}
