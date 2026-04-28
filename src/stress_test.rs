#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Env, Vec, Address, log, symbol_short,
};

const MAX_MEMBERS: u32 = 50;
const BATCH_SIZE: u32 = 10; // Prevent CPU limit overflow

#[derive(Clone)]
#[contracttype]
pub struct Member {
    pub address: Address,
    pub balance: i128,
}

#[derive(Clone)]
#[contracttype]
pub struct SusuGroup {
    pub members: Vec<Member>,
    pub current_index: u32,
    pub total_funds: i128,
}

#[contract]
pub struct SusuContract;

#[contractimpl]
impl SusuContract {
    // ---------------------------
    // CREATE TEST GROUP (50 USERS)
    // ---------------------------
    pub fn create_test_group(env: Env) -> SusuGroup {
        let mut members: Vec<Member> = Vec::new(&env);

        for i in 0..MAX_MEMBERS {
            let addr = Address::generate(&env);

            members.push_back(Member {
                address: addr,
                balance: 1_000, // mock contribution
            });
        }

        SusuGroup {
            members,
            current_index: 0,
            total_funds: 50_000,
        }
    }

    // ---------------------------
    // OPTIMIZED YIELD DISTRIBUTION
    // ---------------------------
    pub fn distribute_yield(env: Env, mut group: SusuGroup) -> SusuGroup {
        let mut processed: u32 = 0;
        let mut index = group.current_index;

        let total_members = group.members.len();

        while index < total_members && processed < BATCH_SIZE {
            let mut member = group.members.get(index).unwrap();

            let payout = group.total_funds / total_members as i128;

            member.balance += payout;

            group.members.set(index, member);

            index += 1;
            processed += 1;
        }

        group.current_index = index;

        log!(
            &env,
            "Batch processed: {}, Next index: {}",
            processed,
            index
        );

        group
    }

    // ---------------------------
    // FINALIZE CYCLE SAFELY
    // ---------------------------
    pub fn finalize_cycle(env: Env, mut group: SusuGroup) -> SusuGroup {
        if group.current_index < group.members.len() {
            panic!("Cycle not fully processed yet");
        }

        group.current_index = 0;
        group.total_funds = 0;

        log!(&env, "Cycle finalized successfully");

        group
    }

    // ---------------------------
    // STRESS TEST ENTRYPOINT
    // ---------------------------
    pub fn stress_test(env: Env) {
        let mut group = Self::create_test_group(env.clone());

        // simulate repeated batching
        loop {
            if group.current_index >= group.members.len() {
                break;
            }

            group = Self::distribute_yield(env.clone(), group);
        }

        group = Self::finalize_cycle(env.clone(), group);

        log!(&env, "Stress test completed for 50 members");
    }
}