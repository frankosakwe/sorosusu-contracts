#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env};
use sorosusu_contracts::reliability_oracle::{
    archive_reputation, is_reputable_user, update_reputation, SoroSusuReputationAdapter,
    SoroSusuReputationAdapterClient, REPUTABLE_USER_CPU_BUDGET,
};

fn setup_adapter(env: &Env) -> (Address, SoroSusuReputationAdapterClient<'_>) {
    let adapter_id = env.register_contract(None, SoroSusuReputationAdapter);
    let client = SoroSusuReputationAdapterClient::new(env, &adapter_id);
    (adapter_id, client)
}

fn store_reputation(
    env: &Env,
    adapter_id: &Address,
    user: &Address,
    ri_score: u32,
    defaults_count: u32,
) {
    env.as_contract(adapter_id, || {
        update_reputation(env, user.clone(), ri_score, 24, 24, defaults_count, 0, 0, 2);
    });
}

#[test]
fn reputable_user_requires_ri_above_900_and_zero_defaults() {
    let env = Env::default();
    let (adapter_id, client) = setup_adapter(&env);
    let user = Address::generate(&env);

    store_reputation(&env, &adapter_id, &user, 901, 0);

    assert!(client.is_reputable_user(&user));
}

#[test]
fn threshold_is_strict_and_defaults_disqualify_user() {
    let env = Env::default();
    let (adapter_id, client) = setup_adapter(&env);
    let threshold_user = Address::generate(&env);
    let defaulted_user = Address::generate(&env);

    store_reputation(&env, &adapter_id, &threshold_user, 900, 0);
    store_reputation(&env, &adapter_id, &defaulted_user, 950, 1);

    assert!(!client.is_reputable_user(&threshold_user));
    assert!(!client.is_reputable_user(&defaulted_user));
}

#[test]
fn missing_or_archived_reputation_defaults_to_false() {
    let env = Env::default();
    let (adapter_id, client) = setup_adapter(&env);
    let missing_user = Address::generate(&env);
    let archived_user = Address::generate(&env);

    assert!(!client.is_reputable_user(&missing_user));

    store_reputation(&env, &adapter_id, &archived_user, 980, 0);
    env.as_contract(&adapter_id, || {
        archive_reputation(&env, archived_user.clone())
    });

    assert!(!client.is_reputable_user(&archived_user));
}

#[test]
fn adapter_emits_zero_events() {
    let env = Env::default();
    let (adapter_id, client) = setup_adapter(&env);
    let user = Address::generate(&env);
    store_reputation(&env, &adapter_id, &user, 980, 0);

    let event_count_before = env.events().all().len();

    assert!(client.is_reputable_user(&user));
    assert_eq!(env.events().all().len(), event_count_before);
}

#[test]
fn adapter_executes_under_5000_cpu_instructions() {
    let env = Env::default();
    let (adapter_id, _) = setup_adapter(&env);
    let user = Address::generate(&env);
    store_reputation(&env, &adapter_id, &user, 980, 0);

    let (result, consumed) = env.as_contract(&adapter_id, || {
        assert!(is_reputable_user(&env, user.clone()));

        let mut budget = env.budget();
        budget.reset_default();
        let before = budget.cpu_instruction_cost();
        let result = is_reputable_user(&env, user);
        (result, budget.cpu_instruction_cost() - before)
    });

    assert!(result);
    assert!(
        consumed < REPUTABLE_USER_CPU_BUDGET,
        "is_reputable_user used {} CPU instructions",
        consumed
    );
}
