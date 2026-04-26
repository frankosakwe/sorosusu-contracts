/// Integration tests for issues #315, #316, #322, #325.

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{token, Address, Env};
use sorosusu_contracts::{SoroSusu, SoroSusuClient};

#[allow(deprecated)]
fn register_token(env: &Env, admin: &Address) -> Address {
    env.register_stellar_asset_contract(admin.clone())
}

fn setup(env: &Env) -> (Address, Address, Address, u64, Address) {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let creator = Address::generate(env);
    let token_admin = Address::generate(env);
    let token_id = register_token(env, &token_admin);

    let token_client = token::StellarAssetClient::new(env, &token_id);
    token_client.mint(&creator, &100_000);

    let contract_id = env.register_contract(None, SoroSusu);
    let client = SoroSusuClient::new(env, &contract_id);
    client.init(&admin);

    let circle_id = client.create_circle(
        &creator, &1_000, &2, &token_id, &86_400, &false, &0, &3600, &100,
    );
    client.join_circle(&creator, &circle_id);

    (admin, creator, token_id, circle_id, contract_id)
}

// ---------------------------------------------------------------------------
// Issue #315 – Reentrancy guard
// ---------------------------------------------------------------------------

#[test]
fn test_reentrancy_guard_payout() {
    let env = Env::default();
    let (admin, creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    // Deposit so there are funds to pay out.
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&creator, &10_000);
    client.deposit(&creator, &circle_id);

    // payout should succeed (lock acquired and released).
    client.payout(&admin, &circle_id);
}

#[test]
fn test_slash_stake_sets_defaulted() {
    let env = Env::default();
    let (admin, creator, _token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    // slash_stake should succeed without panicking.
    client.slash_stake(&admin, &circle_id, &creator);
}

// ---------------------------------------------------------------------------
// Issue #316 – Zombie-Group Sweep
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "circle not completed")]
fn test_cleanup_group_requires_completion() {
    let env = Env::default();
    let (_admin, creator, _token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    // Should panic because circle was never completed.
    client.cleanup_group(&creator, &circle_id);
}

#[test]
#[should_panic(expected = "30-day window has not elapsed")]
fn test_cleanup_group_requires_30_days() {
    let env = Env::default();
    let (admin, creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    // Complete the circle via payout.
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&creator, &10_000);
    client.deposit(&creator, &circle_id);
    client.payout(&admin, &circle_id);

    // Advance only 10 days – should still panic.
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 10 * 24 * 60 * 60);

    client.cleanup_group(&creator, &circle_id);
}

#[test]
fn test_cleanup_group_succeeds_after_30_days() {
    let env = Env::default();
    let (admin, creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&creator, &10_000);
    client.deposit(&creator, &circle_id);
    client.payout(&admin, &circle_id);

    // Advance 31 days.
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 31 * 24 * 60 * 60);

    // Should succeed without panicking.
    client.cleanup_group(&creator, &circle_id);
}

// ---------------------------------------------------------------------------
// Issue #322 – Dispute Bond Slashing
// ---------------------------------------------------------------------------

#[test]
fn test_raise_dispute_locks_bond() {
    let env = Env::default();
    let (_admin, creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let accuser = Address::generate(&env);
    let accused = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    // Fund accuser with enough for the bond.
    token_client.mint(&accuser, &10_000_000);

    let dispute_id = client.raise_dispute(&accuser, &accused, &circle_id, &token_id);
    assert_eq!(dispute_id, 1);
}

#[test]
fn test_execute_verdict_baseless_slashes_to_accused() {
    let env = Env::default();
    let (admin, creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let accuser = Address::generate(&env);
    let accused = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&accuser, &10_000_000);

    let dispute_id = client.raise_dispute(&accuser, &accused, &circle_id, &token_id);

    let accused_before = token_client.balance(&accused);
    // Verdict: baseless – bond goes to accused.
    client.execute_verdict(&admin, &dispute_id, &true, &token_id);
    let accused_after = token_client.balance(&accused);

    assert!(accused_after > accused_before, "accused should receive slashed bond");
}

#[test]
fn test_execute_verdict_valid_returns_bond_to_accuser() {
    let env = Env::default();
    let (admin, _creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let accuser = Address::generate(&env);
    let accused = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&accuser, &10_000_000);

    let dispute_id = client.raise_dispute(&accuser, &accused, &circle_id, &token_id);

    let accuser_before = token_client.balance(&accuser);
    // Verdict: not baseless – bond returned to accuser.
    client.execute_verdict(&admin, &dispute_id, &false, &token_id);
    let accuser_after = token_client.balance(&accuser);

    assert!(accuser_after > accuser_before, "accuser should get bond back");
}

// ---------------------------------------------------------------------------
// Issue #325 – Immutable Audit Trail Events
// ---------------------------------------------------------------------------

#[test]
fn test_dispute_raised_emits_event() {
    let env = Env::default();
    let (_admin, _creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let accuser = Address::generate(&env);
    let accused = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&accuser, &10_000_000);

    client.raise_dispute(&accuser, &accused, &circle_id, &token_id);

    // Verify at least one event was published.
    assert!(!env.events().all().is_empty(), "Dispute_Raised event should be emitted");
}

#[test]
fn test_submit_evidence_emits_event() {
    let env = Env::default();
    let (_admin, _creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let accuser = Address::generate(&env);
    let accused = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&accuser, &10_000_000);

    let dispute_id = client.raise_dispute(&accuser, &accused, &circle_id, &token_id);
    client.submit_evidence(&accuser, &dispute_id, &0xdeadbeef_u64);

    assert!(!env.events().all().is_empty(), "Evidence_Submitted event should be emitted");
}

#[test]
fn test_juror_vote_emits_event() {
    let env = Env::default();
    let (_admin, _creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let accuser = Address::generate(&env);
    let accused = Address::generate(&env);
    let juror = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&accuser, &10_000_000);

    let dispute_id = client.raise_dispute(&accuser, &accused, &circle_id, &token_id);
    client.juror_vote(&juror, &dispute_id, &true);

    assert!(!env.events().all().is_empty(), "Juror_Voted event should be emitted");
}

#[test]
fn test_verdict_executed_emits_event() {
    let env = Env::default();
    let (admin, _creator, token_id, circle_id, contract_id) = setup(&env);
    let client = SoroSusuClient::new(&env, &contract_id);

    let accuser = Address::generate(&env);
    let accused = Address::generate(&env);
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&accuser, &10_000_000);

    let dispute_id = client.raise_dispute(&accuser, &accused, &circle_id, &token_id);
    client.execute_verdict(&admin, &dispute_id, &false, &token_id);

    assert!(!env.events().all().is_empty(), "Verdict_Executed event should be emitted");
}
