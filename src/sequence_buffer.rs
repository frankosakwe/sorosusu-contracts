#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Env, Address, Map, Vec, symbol_short, log,
};

// ---------------------------
// STORAGE KEYS
// ---------------------------
#[contracttype]
pub enum DataKey {
    UserSeq(Address),                 // last executed sequence
    Pending(Address),                 // Map<u64, BufferedTx>
}

// ---------------------------
// BUFFERED TRANSACTION STRUCT
// ---------------------------
#[derive(Clone)]
#[contracttype]
pub struct BufferedTx {
    pub seq: u64,
    pub action: u32,      // action type (e.g. 1=contribute, 2=withdraw)
    pub amount: i128,
}

// ---------------------------
// CONTRACT
// ---------------------------
#[contract]
pub struct SusuContract;

#[contractimpl]
impl SusuContract {

    // ---------------------------
    // SUBMIT BUFFERED TX (OFFLINE SIGNED)
    // ---------------------------
    pub fn submit_buffered_tx(
        env: Env,
        user: Address,
        seq: u64,
        action: u32,
        amount: i128,
    ) {
        user.require_auth();

        let last_seq: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UserSeq(user.clone()))
            .unwrap_or(0);

        if seq <= last_seq {
            panic!("Sequence too old or already processed");
        }

        let mut pending: Map<u64, BufferedTx> = env
            .storage()
            .instance()
            .get(&DataKey::Pending(user.clone()))
            .unwrap_or(Map::new(&env));

        // Prevent overwrite
        if pending.contains_key(seq) {
            panic!("Sequence already submitted");
        }

        pending.set(
            seq,
            BufferedTx {
                seq,
                action,
                amount,
            },
        );

        env.storage()
            .instance()
            .set(&DataKey::Pending(user.clone()), &pending);

        log!(&env, "Buffered tx added: seq {}", seq);
    }

    // ---------------------------
    // PROCESS BUFFERED TXs IN ORDER
    // ---------------------------
    pub fn process_buffered(env: Env, user: Address, max_batch: u32) {
        let mut last_seq: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UserSeq(user.clone()))
            .unwrap_or(0);

        let mut pending: Map<u64, BufferedTx> = env
            .storage()
            .instance()
            .get(&DataKey::Pending(user.clone()))
            .unwrap_or(Map::new(&env));

        let mut processed = 0;

        loop {
            let next_seq = last_seq + 1;

            if !pending.contains_key(next_seq) || processed >= max_batch {
                break;
            }

            let tx = pending.get(next_seq).unwrap();

            // Execute action safely
            Self::execute_action(&env, &user, &tx);

            pending.remove(next_seq);
            last_seq = next_seq;
            processed += 1;
        }

        env.storage()
            .instance()
            .set(&DataKey::UserSeq(user.clone()), &last_seq);

        env.storage()
            .instance()
            .set(&DataKey::Pending(user.clone()), &pending);

        log!(
            &env,
            "Processed {} txs, last_seq now {}",
            processed,
            last_seq
        );
    }

    // ---------------------------
    // EXECUTION LOGIC (SAFE + IDEMPOTENT)
    // ---------------------------
    fn execute_action(env: &Env, user: &Address, tx: &BufferedTx) {
        match tx.action {
            1 => {
                // contribute
                log!(env, "Contribute {} from {:?}", tx.amount, user);
            }
            2 => {
                // withdraw
                log!(env, "Withdraw {} for {:?}", tx.amount, user);
            }
            _ => {
                panic!("Unknown action");
            }
        }

        // Emit event for tracking
        env.events().publish(
            (symbol_short!("EXEC_TX"), user.clone()),
            tx.clone(),
        );
    }

    // ---------------------------
    // VIEW: GET NEXT EXPECTED SEQ
    // ---------------------------
    pub fn get_next_sequence(env: Env, user: Address) -> u64 {
        let last_seq: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UserSeq(user))
            .unwrap_or(0);

        last_seq + 1
    }
}