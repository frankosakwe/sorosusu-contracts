#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol, Vec};

const CHAT_HASH_WINDOW_SECS: u64 = 24 * 60 * 60;
const MAX_CID_LEN: u32 = 128;
const MIN_CID_LEN: u32 = 10;
const MAX_HISTORY_ENTRIES: u32 = 3;

#[contracttype]
#[derive(Clone)]
pub enum ChatMetadataDataKey {
    Admin,
    GroupAdmin(u64),
    GroupChatRecord(u64),
    GroupChatHistory(u64),
}

#[contracttype]
#[derive(Clone)]
pub struct ChatHashRecord {
    pub cid: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ChatHashRecordedEvent {
    pub group_id: u64,
    pub cid: String,
    pub timestamp: u64,
}

#[contract]
pub struct ChatMetadata;

#[contractimpl]
impl ChatMetadata {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&ChatMetadataDataKey::Admin, &admin);
    }

    pub fn authorize_group_admin(
        env: Env,
        admin: Address,
        group_id: u64,
        group_admin: Address,
    ) {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&ChatMetadataDataKey::Admin)
            .expect("Contract not initialized");
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        env.storage()
            .instance()
            .set(&ChatMetadataDataKey::GroupAdmin(group_id), &group_admin);
    }

    pub fn record_chat_hash(env: Env, signer: Address, group_id: u64, cid: String) {
        signer.require_auth();
        let current_time = env.ledger().timestamp();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&ChatMetadataDataKey::Admin)
            .expect("Contract not initialized");

        let group_admin: Option<Address> = env
            .storage()
            .instance()
            .get(&ChatMetadataDataKey::GroupAdmin(group_id));

        let authorized = match group_admin {
            Some(admin_addr) => signer == admin_addr || signer == stored_admin,
            None => signer == stored_admin,
        };
        if !authorized {
            panic!("Unauthorized");
        }

        Self::validate_cid(&cid);

        if let Some(existing_record) = env
            .storage()
            .instance()
            .get::<ChatMetadataDataKey, ChatHashRecord>(&ChatMetadataDataKey::GroupChatRecord(group_id))
        {
            if current_time < existing_record.timestamp + CHAT_HASH_WINDOW_SECS {
                panic!("Chat hash already recorded within 24 hours");
            }
        }

        let record = ChatHashRecord {
            cid: cid.clone(),
            timestamp: current_time,
        };

        env.storage()
            .instance()
            .set(&ChatMetadataDataKey::GroupChatRecord(group_id), &record);

        let mut history: Vec<ChatHashRecord> = env
            .storage()
            .instance()
            .get(&ChatMetadataDataKey::GroupChatHistory(group_id))
            .unwrap_or_else(|| Vec::new(&env));
        history.push_back(record.clone());

        if history.len() > MAX_HISTORY_ENTRIES {
            let mut trimmed = Vec::new(&env);
            let start = history.len() - MAX_HISTORY_ENTRIES;
            for i in start..history.len() {
                trimmed.push_back(history.get_unchecked(i));
            }
            history = trimmed;
        }

        env.storage()
            .instance()
            .set(&ChatMetadataDataKey::GroupChatHistory(group_id), &history);

        env.events().publish(
            (Symbol::new(&env, "ChatHashRecorded"),),
            ChatHashRecordedEvent {
                group_id,
                cid,
                timestamp: current_time,
            },
        );
    }

    pub fn get_latest_chat_hash(env: Env, group_id: u64) -> Option<ChatHashRecord> {
        env.storage()
            .instance()
            .get(&ChatMetadataDataKey::GroupChatRecord(group_id))
    }

    pub fn get_chat_hash_history(env: Env, group_id: u64) -> Vec<ChatHashRecord> {
        env.storage()
            .instance()
            .get(&ChatMetadataDataKey::GroupChatHistory(group_id))
            .unwrap_or_else(|| Vec::new(&env))
    }
}

impl ChatMetadata {
    fn validate_cid(cid: &String) {
        let len = cid.len();
        if len < MIN_CID_LEN || len > MAX_CID_LEN {
            panic!("Invalid CID format");
        }
        for &byte in cid.as_bytes().iter() {
            if byte <= 0x20 || byte == 0x7f {
                panic!("Invalid CID format");
            }
        }
    }
}
