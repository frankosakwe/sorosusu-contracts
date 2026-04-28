#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Env, Address, Map, symbol_short, log,
};

// ---------------------------
// STORAGE KEYS
// ---------------------------
#[contracttype]
pub enum DataKey {
    Contribution(Address),
    Payout(Address),
    Yield(Address),
    LastEmittedYear(Address),
}

// ---------------------------
// STRUCT FOR EVENT OUTPUT
// ---------------------------
#[derive(Clone)]
#[contracttype]
pub struct YearlySummary {
    pub user: Address,
    pub year: u32,
    pub total_contributions: i128,
    pub total_payouts: i128,
    pub total_yield: i128,
}

// ---------------------------
// CONTRACT
// ---------------------------
#[contract]
pub struct SusuContract;

#[contractimpl]
impl SusuContract {

    // ---------------------------
    // MOCK: TRACK CONTRIBUTION
    // ---------------------------
    pub fn record_contribution(env: Env, user: Address, amount: i128) {
        let key = DataKey::Contribution(user.clone());
        let mut total: i128 = env.storage().instance().get(&key).unwrap_or(0);

        total += amount;
        env.storage().instance().set(&key, &total);
    }

    // ---------------------------
    // MOCK: TRACK PAYOUT
    // ---------------------------
    pub fn record_payout(env: Env, user: Address, amount: i128) {
        let key = DataKey::Payout(user.clone());
        let mut total: i128 = env.storage().instance().get(&key).unwrap_or(0);

        total += amount;
        env.storage().instance().set(&key, &total);
    }

    // ---------------------------
    // MOCK: TRACK YIELD
    // ---------------------------
    pub fn record_yield(env: Env, user: Address, amount: i128) {
        let key = DataKey::Yield(user.clone());
        let mut total: i128 = env.storage().instance().get(&key).unwrap_or(0);

        total += amount;
        env.storage().instance().set(&key, &total);
    }

    // ---------------------------
    // CORE: EMIT YEARLY SUMMARY
    // ---------------------------
    pub fn emit_yearly_summary(env: Env, user: Address, year: u32) {
        let current_year: u32 = Self::get_current_year(&env);

        if year >= current_year {
            panic!("Cannot emit summary for current/future year");
        }

        // Prevent duplicate emission
        let last_key = DataKey::LastEmittedYear(user.clone());
        let last_year: u32 = env.storage().instance().get(&last_key).unwrap_or(0);

        if year <= last_year {
            panic!("Summary already emitted for this year");
        }

        let contributions: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Contribution(user.clone()))
            .unwrap_or(0);

        let payouts: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Payout(user.clone()))
            .unwrap_or(0);

        let yield_earned: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Yield(user.clone()))
            .unwrap_or(0);

        let summary = YearlySummary {
            user: user.clone(),
            year,
            total_contributions: contributions,
            total_payouts: payouts,
            total_yield: yield_earned,
        };

        // Emit structured event
        env.events().publish(
            (symbol_short!("YEAR_SUM"), user.clone()),
            summary.clone(),
        );

        // Save last emitted year
        env.storage().instance().set(&last_key, &year);

        log!(&env, "Yearly summary emitted");
    }

    // ---------------------------
    // HELPER: GET CURRENT YEAR
    // ---------------------------
    fn get_current_year(env: &Env) -> u32 {
        let timestamp = env.ledger().timestamp();
        // Rough conversion (seconds → year)
        (timestamp / 31_536_000) as u32 + 1970
    }
}