// Fuzz tests for Adaptive Quorum - Issue #376
// Tests chaotic voter turnout data to verify quorum logic never hits zero

#[cfg(test)]
mod adaptive_quorum_fuzz_tests {
    use super::super::*;
    use soroban_sdk::{testutils::Address as TestAddress, Env, Address};
    use proptest::prelude::*;
    use arbitrary::{Arbitrary, Unstructured};

    // Helper function to create test environment
    fn create_test_env() -> Env {
        Env::default()
    }

    // Helper function to create test adaptive quorum state
    fn create_test_state(
        env: &Env,
        proposal_id: u64,
        proposal_type: adaptive_quorum::ProposalType,
        total_voters: u32,
        decay_duration: u64,
    ) -> adaptive_quorum::AdaptiveQuorumState {
        let settings = adaptive_quorum::initialize_adaptive_quorum_settings(env);
        adaptive_quorum::create_adaptive_quorum_state(
            env,
            proposal_id,
            1, // circle_id
            proposal_type,
            total_voters,
            decay_duration,
            &settings,
        )
    }

    // Fuzz test: Verify quorum never hits zero with chaotic voter turnout
    proptest! {
        #[test]
        fn prop_quorum_never_hits_zero_chaotic_turnout(
            total_voters in 10u32..1000,
            participants in 0u32..1000u32,
            elapsed_seconds in 0u64..200000u64,
        ) {
            let env = create_test_env();
            let mut state = create_test_state(
                &env,
                1,
                adaptive_quorum::ProposalType::Emergency,
                total_voters,
                adaptive_quorum::EMERGENCY_DECAY_SECONDS,
            );

            // Set current participants
            state.current_participants = participants.min(total_voters);

            // Set elapsed time
            state.config.decay_start_timestamp = env.ledger().timestamp() - elapsed_seconds;

            // Calculate quorum
            let current_timestamp = env.ledger().timestamp();
            let quorum = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);

            // CRITICAL: Quorum must never be zero
            prop_assert!(quorum > 0, "Quorum hit zero with voters={}, participants={}, elapsed={}", total_voters, participants, elapsed_seconds);

            // Quorum must respect minimum floor
            prop_assert!(quorum >= adaptive_quorum::MIN_QUORUM_FLOOR_BPS, "Quorum below minimum floor: {}", quorum);

            // Quorum must not exceed initial
            prop_assert!(quorum <= state.config.initial_quorum_bps, "Quorum exceeded initial: {}", quorum);
        }
    }

    // Fuzz test: Verify decay progression is monotonic for emergency proposals
    proptest! {
        #[test]
        fn prop_decay_is_monotonic(
            total_voters in 10u32..1000,
            elapsed1 in 0u64..172800u64,
            elapsed2 in 0u64..172800u64,
        ) {
            let env = create_test_env();
            let mut state1 = create_test_state(
                &env,
                1,
                adaptive_quorum::ProposalType::Emergency,
                total_voters,
                adaptive_quorum::EMERGENCY_DECAY_SECONDS,
            );
            let mut state2 = state1.clone();

            let current_timestamp = env.ledger().timestamp();
            state1.config.decay_start_timestamp = current_timestamp - elapsed1;
            state2.config.decay_start_timestamp = current_timestamp - elapsed2;

            let quorum1 = adaptive_quorum::calculate_adaptive_quorum(&env, &state1, current_timestamp);
            let quorum2 = adaptive_quorum::calculate_adaptive_quorum(&env, &state2, current_timestamp);

            // If more time has elapsed, quorum should be lower or equal
            if elapsed1 > elapsed2 {
                prop_assert!(quorum1 <= quorum2, "Quorum not monotonic: {} at {}s vs {} at {}s", quorum1, elapsed1, quorum2, elapsed2);
            }
        }
    }

    // Fuzz test: Verify standard proposals have static quorum
    proptest! {
        #[test]
        fn prop_standard_proposals_static_quorum(
            total_voters in 10u32..1000,
            elapsed_seconds in 0u64..200000u64,
        ) {
            let env = create_test_env();
            let mut state = create_test_state(
                &env,
                1,
                adaptive_quorum::ProposalType::Standard,
                total_voters,
                86400, // 24 hours
            );

            state.config.decay_start_timestamp = env.ledger().timestamp() - elapsed_seconds;

            let current_timestamp = env.ledger().timestamp();
            let quorum = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);

            // Standard proposals should have static quorum regardless of time
            prop_assert_eq!(quorum, state.config.initial_quorum_bps, "Standard proposal quorum changed with time");
        }
    }

    // Fuzz test: Verify contest threshold reset works correctly
    proptest! {
        #[test]
        fn prop_contest_reset_threshold(
            total_voters in 10u32..1000,
            contest_count in 0u32..1000u32,
        ) {
            let env = create_test_env();
            let mut state = create_test_state(
                &env,
                1,
                adaptive_quorum::ProposalType::Emergency,
                total_voters,
                adaptive_quorum::EMERGENCY_DECAY_SECONDS,
            );

            let voter = Address::generate(&env);
            let current_timestamp = env.ledger().timestamp();

            // Simulate multiple contest votes
            for _ in 0..contest_count {
                adaptive_quorum::register_contest_vote(&env, &mut state, &voter, current_timestamp);
            }

            // Calculate contest percentage
            let contest_bps = (state.contest_tracker.contest_count as u128 * 10000) 
                / (total_voters as u128).max(1);

            // If threshold reached, reset count should be > 0
            if contest_bps >= state.contest_tracker.contest_threshold as u128 {
                prop_assert!(state.contest_tracker.decay_reset_count > 0, "Decay not reset when threshold reached");
            }
        }
    }

    // Fuzz test: Verify participation velocity tracking maintains max 10 records
    proptest! {
        #[test]
        fn prop_velocity_max_10_records(
            record_count in 0u32..50u32,
        ) {
            let env = create_test_env();
            let mut velocity = adaptive_quorum::ParticipationVelocity {
                vote_records: Vec::new(&env),
                average_participation: 0,
                velocity_trend: 0,
            };

            // Add many records
            for i in 0..record_count {
                let record = adaptive_quorum::record_participation(
                    &env,
                    i as u64,
                    100,
                    50,
                    env.ledger().timestamp(),
                );
                adaptive_quorum::update_velocity_tracking(&env, &mut velocity, record);
            }

            // Should never exceed 10 records
            prop_assert!(velocity.vote_records.len() <= 10, "Velocity records exceeded 10: {}", velocity.vote_records.len());
        }
    }

    // Fuzz test: Verify quorum calculation with extreme time values
    proptest! {
        #[test]
        fn prop_quorum_extreme_time_values(
            total_voters in 10u32..1000,
            elapsed_seconds in 0u64..1_000_000u64, // Very large elapsed time
        ) {
            let env = create_test_env();
            let mut state = create_test_state(
                &env,
                1,
                adaptive_quorum::ProposalType::Emergency,
                total_voters,
                adaptive_quorum::EMERGENCY_DECAY_SECONDS,
            );

            state.config.decay_start_timestamp = env.ledger().timestamp() - elapsed_seconds;

            let current_timestamp = env.ledger().timestamp();
            let quorum = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);

            // Even with extreme time values, quorum should be valid
            prop_assert!(quorum > 0, "Quorum hit zero with extreme elapsed time: {}", elapsed_seconds);
            prop_assert!(quorum <= 10000, "Quorum exceeded 100% with extreme elapsed time: {}", quorum);
        }
    }

    // Fuzz test: Verify silent sabotage protection
    proptest! {
        #[test]
        fn prop_silent_sabotage_protection(
            base_quorum in 1000u32..10000u32,
            total_voters in 10u32..1000,
        ) {
            let protected_quorum = adaptive_quorum::apply_silent_sabotage_protection(base_quorum, total_voters);

            // Protected quorum should never be below minimum floor
            prop_assert!(protected_quorum >= adaptive_quorum::MIN_QUORUM_FLOOR_BPS, 
                "Silent sabotage protection failed: {} < {}", protected_quorum, adaptive_quorum::MIN_QUORUM_FLOOR_BPS);
        }
    }

    // Fuzz test: Verify quorum met calculation with various participation levels
    proptest! {
        #[test]
        fn prop_quorum_met_calculation(
            total_voters in 10u32..1000,
            participants in 0u32..1000u32,
            elapsed_seconds in 0u64..172800u64,
        ) {
            let env = create_test_env();
            let mut state = create_test_state(
                &env,
                1,
                adaptive_quorum::ProposalType::Emergency,
                total_voters,
                adaptive_quorum::EMERGENCY_DECAY_SECONDS,
            );

            state.current_participants = participants.min(total_voters);
            state.config.decay_start_timestamp = env.ledger().timestamp() - elapsed_seconds;

            let current_timestamp = env.ledger().timestamp();
            let quorum_met = adaptive_quorum::is_quorum_met(&env, &state, current_timestamp);

            // If participants >= total voters, quorum must be met
            if state.current_participants >= total_voters {
                prop_assert!(quorum_met, "Quorum not met with 100% participation");
            }

            // If participants = 0, quorum should not be met (unless quorum is 0, which it shouldn't be)
            if state.current_participants == 0 {
                prop_assert!(!quorum_met, "Quorum met with 0 participants");
            }
        }
    }

    // Fuzz test: Verify velocity trend calculation
    proptest! {
        #[test]
        fn prop_velocity_trend_calculation(
            participation_rates in Vec::<u32>::with_strategy(
                proptest::collection::vec(1000u32..10000, 6..15) // 6-15 participation rates
            ),
        ) {
            let env = create_test_env();
            let mut velocity = adaptive_quorum::ParticipationVelocity {
                vote_records: Vec::new(&env),
                average_participation: 0,
                velocity_trend: 0,
            };

            // Add records with varying participation rates
            for (i, rate) in participation_rates.iter().enumerate() {
                let record = adaptive_quorum::ParticipationRecord {
                    proposal_id: i as u64,
                    total_eligible_voters: 100,
                    actual_participants: (*rate as u128 * 100 / 10000) as u32,
                    participation_rate_bps: *rate,
                    timestamp: env.ledger().timestamp() + (i as u64),
                };
                adaptive_quorum::update_velocity_tracking(&env, &mut velocity, record);
            }

            // Velocity trend should be calculated if we have enough records
            if velocity.vote_records.len() >= 6 {
                // Trend should be within reasonable bounds
                prop_assert!(velocity.velocity_trend >= -10000 && velocity.velocity_trend <= 10000,
                    "Velocity trend out of bounds: {}", velocity.velocity_trend);
            }
        }
    }

    // Unit test: Verify decay reaches minimum after 48 hours
    #[test]
    fn test_decay_reaches_minimum_after_48_hours() {
        let env = create_test_env();
        let mut state = create_test_state(
            &env,
            1,
            adaptive_quorum::ProposalType::Emergency,
            100,
            adaptive_quorum::EMERGENCY_DECAY_SECONDS,
        );

        // Set elapsed time to exactly 48 hours
        state.config.decay_start_timestamp = env.ledger().timestamp() - adaptive_quorum::EMERGENCY_DECAY_SECONDS;

        let current_timestamp = env.ledger().timestamp();
        let quorum = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);

        // After 48 hours, quorum should be at minimum
        assert_eq!(quorum, state.config.minimum_quorum_bps);
    }

    // Unit test: Verify decay starts at initial value
    #[test]
    fn test_decay_starts_at_initial() {
        let env = create_test_env();
        let state = create_test_state(
            &env,
            1,
            adaptive_quorum::ProposalType::Emergency,
            100,
            adaptive_quorum::EMERGENCY_DECAY_SECONDS,
        );

        let current_timestamp = env.ledger().timestamp();
        let quorum = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);

        // At time 0, quorum should be at initial value
        assert_eq!(quorum, state.config.initial_quorum_bps);
    }

    // Unit test: Verify contest resets decay timer
    #[test]
    fn test_contest_resets_decay_timer() {
        let env = create_test_env();
        let mut state = create_test_state(
            &env,
            1,
            adaptive_quorum::ProposalType::Emergency,
            100,
            adaptive_quorum::EMERGENCY_DECAY_SECONDS,
        );

        let voter = Address::generate(&env);
        let current_timestamp = env.ledger().timestamp();

        // Set some elapsed time
        state.config.decay_start_timestamp = current_timestamp - 86400; // 24 hours elapsed

        let quorum_before = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);

        // Add enough contests to trigger reset (15% of 100 = 15 votes)
        for _ in 0..15 {
            adaptive_quorum::register_contest_vote(&env, &mut state, &voter, current_timestamp);
        }

        // Decay timer should be reset
        assert_eq!(state.config.decay_start_timestamp, current_timestamp);
        assert!(state.contest_tracker.decay_reset_count > 0);

        // Quorum should now be back to initial
        let quorum_after = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);
        assert_eq!(quorum_after, state.config.initial_quorum_bps);
    }

    // Unit test: Verify quorum never goes below silent sabotage threshold
    #[test]
    fn test_quorum_never_below_silent_sabotage_threshold() {
        let env = create_test_env();
        let mut state = create_test_state(
            &env,
            1,
            adaptive_quorum::ProposalType::Emergency,
            100,
            adaptive_quorum::EMERGENCY_DECAY_SECONDS,
        );

        // Set elapsed time far beyond decay duration
        state.config.decay_start_timestamp = env.ledger().timestamp() - 1_000_000;

        let current_timestamp = env.ledger().timestamp();
        let quorum = adaptive_quorum::calculate_adaptive_quorum(&env, &state, current_timestamp);

        // Even with extreme elapsed time, quorum should respect minimum floor
        assert!(quorum >= adaptive_quorum::MIN_QUORUM_FLOOR_BPS);
    }

    // Unit test: Verify participation record creation
    #[test]
    fn test_participation_record_creation() {
        let env = create_test_env();
        let record = adaptive_quorum::record_participation(&env, 1, 100, 50, env.ledger().timestamp());

        assert_eq!(record.proposal_id, 1);
        assert_eq!(record.total_eligible_voters, 100);
        assert_eq!(record.actual_participants, 50);
        assert_eq!(record.participation_rate_bps, 5000); // 50%
    }

    // Unit test: Verify velocity tracking with 10+ records
    #[test]
    fn test_velocity_tracking_max_records() {
        let env = create_test_env();
        let mut velocity = adaptive_quorum::ParticipationVelocity {
            vote_records: Vec::new(&env),
            average_participation: 0,
            velocity_trend: 0,
        };

        // Add 15 records
        for i in 0..15 {
            let record = adaptive_quorum::record_participation(
                &env,
                i as u64,
                100,
                50,
                env.ledger().timestamp() + i,
            );
            adaptive_quorum::update_velocity_tracking(&env, &mut velocity, record);
        }

        // Should only keep last 10
        assert_eq!(velocity.vote_records.len(), 10);
    }
}
