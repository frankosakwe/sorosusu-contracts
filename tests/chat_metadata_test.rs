#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env, String, Vec};
use sorosusu_contracts::chat_metadata::ChatMetadataClient;
use sorosusu_contracts::chat_metadata::ChatMetadata;

fn setup_contract() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let group_admin = Address::generate(&env);
    let contract_id = env.register_contract(None, ChatMetadata);
    let client = ChatMetadataClient::new(&env, &contract_id);
    client.init(&admin);
    client.authorize_group_admin(&admin, &1u64, &group_admin);
    (env, admin, group_admin, contract_id)
}

#[test]
fn test_record_chat_hash_success() {
    let (env, _admin, group_admin, contract_id) = setup_contract();
    let client = ChatMetadataClient::new(&env, &contract_id);
    let cid = String::from_slice(&env, "QmYwAPJzv5CZsnAzt8auVTLhRBrY2xYK27n5aLqGvFootd");

    client.record_chat_hash(&group_admin, &1u64, &cid.clone());
    let record = client.get_latest_chat_hash(&1u64).expect("record missing");

    assert_eq!(record.cid, cid);
    assert_eq!(record.timestamp, env.ledger().timestamp());
}

#[test]
fn test_reject_within_24h_window() {
    let (env, _admin, group_admin, contract_id) = setup_contract();
    let client = ChatMetadataClient::new(&env, &contract_id);
    let cid1 = String::from_slice(&env, "QmYwAPJzv5CZsnAzt8auVTLhRBrY2xYK27n5aLqGvFootd");
    let cid2 = String::from_slice(&env, "QmZxoqJzv5CZwqAzt8auVTLhRBrY2xYK27n5aLqGvFooXy");

    client.record_chat_hash(&group_admin, &1u64, &cid1);
    let result = std::panic::catch_unwind(|| {
        client.record_chat_hash(&group_admin, &1u64, &cid2);
    });

    assert!(result.is_err());
}

#[test]
fn test_authorization_enforced() {
    let (env, _admin, _group_admin, contract_id) = setup_contract();
    let client = ChatMetadataClient::new(&env, &contract_id);
    let unauthorized = Address::generate(&env);
    let cid = String::from_slice(&env, "QmYwAPJzv5CZsnAzt8auVTLhRBrY2xYK27n5aLqGvFootd");

    let result = std::panic::catch_unwind(|| {
        client.record_chat_hash(&unauthorized, &1u64, &cid);
    });

    assert!(result.is_err());
}

#[test]
fn test_invalid_cid_format() {
    let (env, _admin, group_admin, contract_id) = setup_contract();
    let client = ChatMetadataClient::new(&env, &contract_id);
    let invalid_cid = String::from_slice(&env, "   ");

    let result = std::panic::catch_unwind(|| {
        client.record_chat_hash(&group_admin, &1u64, &invalid_cid);
    });

    assert!(result.is_err());
}

#[test]
fn test_multiple_groups_independent_windows() {
    let (env, _admin, group_admin, contract_id) = setup_contract();
    let client = ChatMetadataClient::new(&env, &contract_id);
    let cid1 = String::from_slice(&env, "QmYwAPJzv5CZsnAzt8auVTLhRBrY2xYK27n5aLqGvFootd");
    let cid2 = String::from_slice(&env, "QmZxoqJzv5CZwqAzt8auVTLhRBrY2xYK27n5aLqGvFooXy");

    client.record_chat_hash(&group_admin, &1u64, &cid1);
    client.record_chat_hash(&group_admin, &2u64, &cid2);

    let record1 = client.get_latest_chat_hash(&1u64).expect("group1 missing");
    let record2 = client.get_latest_chat_hash(&2u64).expect("group2 missing");

    assert_eq!(record1.cid, cid1);
    assert_eq!(record2.cid, cid2);
}

#[test]
fn test_exactly_24h_boundary_allowed() {
    let (env, _admin, group_admin, contract_id) = setup_contract();
    let client = ChatMetadataClient::new(&env, &contract_id);
    let cid1 = String::from_slice(&env, "QmYwAPJzv5CZsnAzt8auVTLhRBrY2xYK27n5aLqGvFootd");
    let cid2 = String::from_slice(&env, "QmZxoqJzv5CZwqAzt8auVTLhRBrY2xYK27n5aLqGvFooXy");

    client.record_chat_hash(&group_admin, &1u64, &cid1);
    env.ledger().set_timestamp(env.ledger().timestamp() + 24 * 60 * 60);
    client.record_chat_hash(&group_admin, &1u64, &cid2);

    let record = client.get_latest_chat_hash(&1u64).expect("latest missing");
    assert_eq!(record.cid, cid2);
}
