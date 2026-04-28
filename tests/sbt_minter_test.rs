#![cfg(test)]
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env, String};
use sorosusu_contracts::sbt_minter::{
    ReputationTier, SbtStatus, SoroSusuSbtMinter, SoroSusuSbtMinterClient,
};

#[test]
fn test_sbt_minter_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let minter_id = env.register_contract(None, SoroSusuSbtMinter);
    let client = SoroSusuSbtMinterClient::new(&env, &minter_id);

    client.init_sbt_minter(&admin);

    client.update_user_reputation_metrics(
        &admin,
        &user,
        &8_500u32,
        &7_000u32,
        &12u32,
        &12u32,
        &12_000i128,
    );

    let desc = String::from_str(&env, "Complete a 12-month cycle");
    let mid = client.create_reputation_milestone(&user, &12u32, &desc, &ReputationTier::Platinum);

    let m = client.get_reputation_milestone(&mid);
    assert_eq!(m.required_cycles, 12);

    let tid = client.issue_credential(&user, &mid, &String::from_str(&env, ""));
    let cred = client.get_credential(&tid);
    assert_eq!(cred.user, user);
    assert_eq!(cred.status, SbtStatus::Luminary);
    assert_eq!(
        cred.metadata_uri,
        String::from_str(&env, "ipfs://sorosusu/reputation/platinum")
    );
}

#[test]
fn test_12_month_cycle_mints_dynamic_reputation_badge() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let minter_id = env.register_contract(None, SoroSusuSbtMinter);
    let client = SoroSusuSbtMinterClient::new(&env, &minter_id);
    client.init_sbt_minter(&admin);

    let token_id =
        client.record_cycle_completion(&admin, &user, &1u64, &12_000i128, &12u32, &12u32);

    let cred = client.get_user_credential(&user).unwrap();
    assert_eq!(cred.token_id, token_id);
    assert_eq!(cred.status, SbtStatus::SusuLegend);
    assert_eq!(client.get_current_tier(&token_id), ReputationTier::Diamond);
    assert_eq!(
        client.metadata_uri(&token_id),
        String::from_str(&env, "ipfs://sorosusu/reputation/diamond")
    );
    assert!(!env.events().all().is_empty());
}

#[test]
fn test_reputation_badge_tier_updates_as_on_time_cycles_improve() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let minter_id = env.register_contract(None, SoroSusuSbtMinter);
    let client = SoroSusuSbtMinterClient::new(&env, &minter_id);
    client.init_sbt_minter(&admin);

    let token_id = client.record_cycle_completion(&admin, &user, &1u64, &12_000i128, &12u32, &6u32);
    assert_eq!(client.get_current_tier(&token_id), ReputationTier::Bronze);

    let same_token_id =
        client.record_cycle_completion(&admin, &user, &2u64, &12_000i128, &12u32, &12u32);
    assert_eq!(same_token_id, token_id);
    assert_eq!(client.get_current_tier(&token_id), ReputationTier::Gold);
    assert_eq!(
        client.metadata_uri(&token_id),
        String::from_str(&env, "ipfs://sorosusu/reputation/gold")
    );
}

#[test]
fn test_zero_value_cycle_cannot_mint_reputation_badge() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let minter_id = env.register_contract(None, SoroSusuSbtMinter);
    let client = SoroSusuSbtMinterClient::new(&env, &minter_id);
    client.init_sbt_minter(&admin);

    let result = client.try_record_cycle_completion(&admin, &user, &1u64, &0i128, &12u32, &12u32);
    assert!(result.is_err());
}

#[test]
fn test_soulbound_transfer_always_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let other = Address::generate(&env);

    let minter_id = env.register_contract(None, SoroSusuSbtMinter);
    let client = SoroSusuSbtMinterClient::new(&env, &minter_id);
    client.init_sbt_minter(&admin);
    let token_id =
        client.record_cycle_completion(&admin, &user, &1u64, &12_000i128, &12u32, &12u32);

    let result = client.try_transfer(&user, &other, &token_id);
    assert!(result.is_err());
}

#[test]
fn test_default_switches_metadata_to_delinquent() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let minter_id = env.register_contract(None, SoroSusuSbtMinter);
    let client = SoroSusuSbtMinterClient::new(&env, &minter_id);
    client.init_sbt_minter(&admin);
    let token_id =
        client.record_cycle_completion(&admin, &user, &1u64, &12_000i128, &12u32, &12u32);

    client.mark_defaulted(&user);

    let cred = client.get_credential(&token_id);
    assert_eq!(cred.status, SbtStatus::Delinquent);
    assert_eq!(
        cred.metadata_uri,
        String::from_str(&env, "ipfs://sorosusu/reputation/delinquent")
    );
}

#[test]
fn test_admin_can_revoke_and_burn_badge_for_fraud() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let minter_id = env.register_contract(None, SoroSusuSbtMinter);
    let client = SoroSusuSbtMinterClient::new(&env, &minter_id);
    client.init_sbt_minter(&admin);
    let token_id =
        client.record_cycle_completion(&admin, &user, &1u64, &12_000i128, &12u32, &12u32);

    client.revoke_credential(&token_id, &String::from_str(&env, "fraud"));
    assert_eq!(client.get_credential(&token_id).status, SbtStatus::Revoked);

    client.burn_credential(&token_id);
    let cred = client.get_credential(&token_id);
    assert_eq!(cred.status, SbtStatus::Burned);
    assert_eq!(
        cred.metadata_uri,
        String::from_str(&env, "ipfs://sorosusu/reputation/burned")
    );
}
