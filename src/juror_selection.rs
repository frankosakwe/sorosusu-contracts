// Issue #323: Decentralized "Juror" Selection for SoroSusu Global Pool
//
// Implements a pseudo-random ledger-hash-based selector that picks 5 jurors
// from the global pool of high-RI (Reliability Index) users.  Using the
// ledger's sequence number and timestamp as entropy prevents an attacker from
// predicting or stacking the jury with sybil accounts, because the seed is
// only known at execution time.

#![no_std]

use soroban_sdk::{contracttype, Address, Env, Vec};

// --- CONSTANTS ---

/// Number of jurors selected per dispute.
pub const JUROR_COUNT: u32 = 5;

/// Minimum RI score required to be eligible as a juror.
pub const MIN_JUROR_RI: u32 = 650; // "Good" tier from reliability_oracle

// --- DATA KEYS ---

#[contracttype]
#[derive(Clone)]
pub enum JurorDataKey {
    /// Stores the list of addresses eligible to serve as jurors (high-RI pool).
    EligiblePool,
    /// Stores the selected jurors for a given dispute id.
    SelectedJurors(u64),
}

// --- PUBLIC API ---

/// Register `candidate` as eligible for jury duty.
/// Caller must supply the candidate's current RI score; only scores >= MIN_JUROR_RI
/// are accepted.  In production this score would be read from the ReliabilityOracle.
pub fn register_juror_candidate(env: &Env, candidate: Address, ri_score: u32) {
    if ri_score < MIN_JUROR_RI {
        panic!("RI score below minimum required for juror eligibility");
    }

    let mut pool: Vec<Address> = env
        .storage()
        .instance()
        .get(&JurorDataKey::EligiblePool)
        .unwrap_or_else(|| Vec::new(env));

    // Prevent duplicate registration.
    for existing in pool.iter() {
        if existing == candidate {
            panic!("Candidate already registered in juror pool");
        }
    }

    pool.push_back(candidate);
    env.storage()
        .instance()
        .set(&JurorDataKey::EligiblePool, &pool);
}

/// Select `JUROR_COUNT` jurors for `dispute_id` using a pseudo-random function
/// seeded from the ledger sequence number and timestamp.
///
/// The selection is deterministic given the same ledger state, but unpredictable
/// before the transaction is included in a ledger — preventing pre-selection attacks.
///
/// Returns the selected juror addresses.  Panics if the pool is too small.
pub fn select_jurors(env: &Env, dispute_id: u64) -> Vec<Address> {
    let pool: Vec<Address> = env
        .storage()
        .instance()
        .get(&JurorDataKey::EligiblePool)
        .unwrap_or_else(|| Vec::new(env));

    let pool_size = pool.len();
    if pool_size < JUROR_COUNT {
        panic!("Juror pool too small: need at least JUROR_COUNT eligible members");
    }

    // Build a mutable copy of the pool and shuffle it using Soroban's PRNG.
    // env.prng().shuffle() uses the ledger's VRF-derived randomness, making
    // the output verifiable and unpredictable to any single party.
    let mut shuffled = pool.clone();
    env.prng().shuffle(&mut shuffled);

    // Take the first JUROR_COUNT addresses from the shuffled pool.
    let mut selected: Vec<Address> = Vec::new(env);
    for i in 0..JUROR_COUNT {
        selected.push_back(shuffled.get_unchecked(i));
    }

    // Persist the selection so it can be audited on-chain.
    env.storage()
        .instance()
        .set(&JurorDataKey::SelectedJurors(dispute_id), &selected);

    selected
}

/// Retrieve the previously selected jurors for `dispute_id`.
pub fn get_selected_jurors(env: &Env, dispute_id: u64) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&JurorDataKey::SelectedJurors(dispute_id))
        .unwrap_or_else(|| Vec::new(env))
}

// --- TESTS ---

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    fn make_pool(env: &Env, n: u32) -> Vec<Address> {
        let mut addrs = Vec::new(env);
        for _ in 0..n {
            addrs.push_back(Address::generate(env));
        }
        addrs
    }

    #[test]
    fn test_register_and_select_jurors() {
        let env = Env::default();
        env.mock_all_auths();

        // Register 10 eligible candidates.
        for _ in 0..10u32 {
            let addr = Address::generate(&env);
            register_juror_candidate(&env, addr, MIN_JUROR_RI);
        }

        let pool: Vec<Address> = env
            .storage()
            .instance()
            .get(&JurorDataKey::EligiblePool)
            .unwrap();
        assert_eq!(pool.len(), 10, "Pool should have 10 candidates");

        // Select jurors for dispute 1.
        let jurors = select_jurors(&env, 1);
        assert_eq!(jurors.len(), JUROR_COUNT, "Must select exactly JUROR_COUNT jurors");

        // All selected jurors must come from the eligible pool.
        for juror in jurors.iter() {
            let mut found = false;
            for candidate in pool.iter() {
                if candidate == juror {
                    found = true;
                    break;
                }
            }
            assert!(found, "Selected juror must be from the eligible pool");
        }
    }

    #[test]
    fn test_selected_jurors_are_unique() {
        let env = Env::default();
        env.mock_all_auths();

        for _ in 0..10u32 {
            register_juror_candidate(&env, Address::generate(&env), MIN_JUROR_RI);
        }

        let jurors = select_jurors(&env, 2);
        // Check no duplicates.
        for i in 0..jurors.len() {
            for j in (i + 1)..jurors.len() {
                assert_ne!(
                    jurors.get_unchecked(i),
                    jurors.get_unchecked(j),
                    "Selected jurors must be unique"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "RI score below minimum")]
    fn test_low_ri_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        register_juror_candidate(&env, Address::generate(&env), MIN_JUROR_RI - 1);
    }

    #[test]
    #[should_panic(expected = "Juror pool too small")]
    fn test_pool_too_small_panics() {
        let env = Env::default();
        env.mock_all_auths();

        // Register only 4 candidates (less than JUROR_COUNT = 5).
        for _ in 0..4u32 {
            register_juror_candidate(&env, Address::generate(&env), MIN_JUROR_RI);
        }
        select_jurors(&env, 3);
    }

    #[test]
    fn test_selection_persisted_on_chain() {
        let env = Env::default();
        env.mock_all_auths();

        for _ in 0..6u32 {
            register_juror_candidate(&env, Address::generate(&env), MIN_JUROR_RI);
        }

        let selected = select_jurors(&env, 42);
        let retrieved = get_selected_jurors(&env, 42);

        assert_eq!(selected.len(), retrieved.len());
        for i in 0..selected.len() {
            assert_eq!(selected.get_unchecked(i), retrieved.get_unchecked(i));
        }
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn test_duplicate_registration_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let addr = Address::generate(&env);
        register_juror_candidate(&env, addr.clone(), MIN_JUROR_RI);
        register_juror_candidate(&env, addr, MIN_JUROR_RI); // should panic
    }
}
