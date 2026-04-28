// Adaptive Quorum for Global Protocol Governance
// Issue #376: Implement Adaptive Quorum for rapid response during security crises

use soroban_sdk::{contracttype, Address, Env, Symbol, Vec, Map, u64, u32, i128};

// ============================================
// DATA STRUCTURES
// ============================================

/// Proposal type determines quorum behavior
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ProposalType {
    Emergency = 0,  // Security crisis, circuit breaker activation
    Standard = 1,   // Protocol upgrades, parameter changes
}

/// Current status of a proposal
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ProposalStatus {
    Active = 0,
    Approved = 1,
    Rejected = 2,
    Expired = 3,
    Executed = 4,
}

/// Adaptive quorum configuration for a proposal
#[contracttype]
#[derive(Clone)]
pub struct AdaptiveQuorumConfig {
    pub proposal_type: ProposalType,
    pub initial_quorum_bps: u32,      // Initial quorum in basis points (10000 = 100%)
    pub minimum_quorum_bps: u32,     // Minimum quorum floor (never below this)
    pub decay_start_timestamp: u64,  // When decay starts
    pub decay_duration_seconds: u64,  // How long decay takes
    pub current_quorum_bps: u32,      // Current calculated quorum
    pub is_decayed: bool,             // Whether decay is complete
}

/// Participation velocity tracking for last 10 successful votes
#[contracttype]
#[derive(Clone)]
pub struct ParticipationVelocity {
    pub vote_records: Vec<ParticipationRecord>,  // Last 10 successful votes
    pub average_participation: u32,             // Average participation rate (bps)
    pub velocity_trend: i32,                     // Trend: positive = increasing, negative = decreasing
}

/// Individual vote participation record
#[contracttype]
#[derive(Clone)]
pub struct ParticipationRecord {
    pub proposal_id: u64,
    pub total_eligible_voters: u32,
    pub actual_participants: u32,
    pub participation_rate_bps: u32,  // 10000 = 100%
    pub timestamp: u64,
}

/// Contest tracking to prevent Silent Sabotage
#[contracttype]
#[derive(Clone)]
pub struct ContestTracker {
    pub contest_count: u32,           // Number of contest votes cast
    pub contest_threshold: u32,       // Threshold to trigger reset
    pub last_contest_timestamp: u64,  // Last time a contest was registered
    pub decay_reset_count: u32,       // How many times decay has been reset
}

/// Adaptive quorum state for a proposal
#[contracttype]
#[derive(Clone)]
pub struct AdaptiveQuorumState {
    pub proposal_id: u64,
    pub circle_id: u64,
    pub config: AdaptiveQuorumConfig,
    pub contest_tracker: ContestTracker,
    pub total_eligible_voters: u32,
    pub current_participants: u32,
    pub created_timestamp: u64,
    pub voting_end_timestamp: u64,
}

/// Global adaptive quorum settings
#[contracttype]
#[derive(Clone)]
pub struct AdaptiveQuorumSettings {
    pub emergency_initial_quorum_bps: u32,      // 5000 = 50%
    pub emergency_minimum_quorum_bps: u32,       // 2500 = 25%
    pub emergency_decay_duration_seconds: u64,    // 48 hours = 172800 seconds
    pub standard_quorum_bps: u32,                // 7000 = 70% (high threshold)
    pub contest_reset_threshold_bps: u32,        // 1500 = 15% contest triggers reset
    pub min_velocity_samples: u32,                // Need at least 3 samples for velocity
    pub silent_sabotage_threshold: u32,          // 1000 = 10% minimum to prevent gaming
}

// ============================================
// CONSTANTS
// ============================================

/// 48 hours in seconds for emergency decay
pub const EMERGENCY_DECAY_SECONDS: u64 = 48 * 60 * 60; // 172800

/// Fixed-point scaling factor for u128 math (10000 = 1.0)
pub const FIXED_POINT_SCALE: u128 = 10000;

/// Maximum number of participation records to track
pub const MAX_VELOCITY_SAMPLES: u32 = 10;

/// Minimum quorum floor to prevent gaming (never below 10%)
pub const MIN_QUORUM_FLOOR_BPS: u32 = 1000;

// ============================================
// CORE ADAPTIVE QUORUM LOGIC
// ============================================

/// Initialize adaptive quorum settings with default values
pub fn initialize_adaptive_quorum_settings(env: &Env) -> AdaptiveQuorumSettings {
    AdaptiveQuorumSettings {
        emergency_initial_quorum_bps: 5000,      // 50%
        emergency_minimum_quorum_bps: 2500,       // 25%
        emergency_decay_duration_seconds: EMERGENCY_DECAY_SECONDS,
        standard_quorum_bps: 7000,                // 70%
        contest_reset_threshold_bps: 1500,        // 15%
        min_velocity_samples: 3,
        silent_sabotage_threshold: 1000,          // 10%
    }
}

/// Calculate current adaptive quorum based on time elapsed
pub fn calculate_adaptive_quorum(
    env: &Env,
    state: &AdaptiveQuorumState,
    current_timestamp: u64,
) -> u32 {
    let config = &state.config;
    
    // Standard proposals have static quorum
    if config.proposal_type == ProposalType::Standard {
        return config.initial_quorum_bps;
    }
    
    // Emergency proposals use decay mechanism
    if config.is_decayed {
        return config.minimum_quorum_bps;
    }
    
    // Calculate decay progress
    let elapsed = if current_timestamp > config.decay_start_timestamp {
        current_timestamp - config.decay_start_timestamp
    } else {
        0
    };
    
    if elapsed >= config.decay_duration_seconds {
        // Decay complete
        return config.minimum_quorum_bps;
    }
    
    // Linear decay using u128 fixed-point math
    let decay_progress = (elapsed as u128) * FIXED_POINT_SCALE / (config.decay_duration_seconds as u128);
    let initial_quorum = config.initial_quorum_bps as u128;
    let minimum_quorum = config.minimum_quorum_bps as u128;
    let decay_range = initial_quorum - minimum_quorum;
    
    // current = initial - (decay_range * progress / scale)
    let current_quorum = initial_quorum - (decay_range * decay_progress / FIXED_POINT_SCALE);
    
    // Ensure we never go below minimum
    let clamped_quorum = current_quorum.max(minimum_quorum);
    
    // Apply velocity-based adjustment if participation is naturally low
    let velocity_adjusted = apply_velocity_adjustment(env, clamped_quorum as u32);
    
    // Apply silent sabotage protection
    apply_silent_sabotage_protection(velocity_adjusted, state.total_eligible_voters)
}

/// Apply velocity-based adjustment to quorum
pub fn apply_velocity_adjustment(env: &Env, base_quorum: u32) -> u32 {
    // This would read from stored velocity data
    // For now, return base quorum
    base_quorum
}

/// Apply silent sabotage protection - prevent small minority from gaming decay
pub fn apply_silent_sabotage_protection(quorum: u32, total_voters: u32) -> u32 {
    // Ensure quorum never goes below silent sabotage threshold
    quorum.max(MIN_QUORUM_FLOOR_BPS)
}

/// Check if quorum is met based on current participants
pub fn is_quorum_met(
    env: &Env,
    state: &AdaptiveQuorumState,
    current_timestamp: u64,
) -> bool {
    let current_quorum_bps = calculate_adaptive_quorum(env, state, current_timestamp);
    let required_votes = (state.total_eligible_voters as u128 * current_quorum_bps as u128) / 10000;
    
    state.current_participants as u128 >= required_votes
}

/// Register a contest vote - may reset decay timer
pub fn register_contest_vote(
    env: &Env,
    state: &mut AdaptiveQuorumState,
    voter: &Address,
    current_timestamp: u64,
) -> bool {
    state.contest_tracker.contest_count += 1;
    state.contest_tracker.last_contest_timestamp = current_timestamp;
    
    // Calculate contest percentage
    let contest_bps = (state.contest_tracker.contest_count as u128 * 10000) 
        / (state.total_eligible_voters as u128).max(1);
    
    // If contest threshold reached, reset decay
    if contest_bps >= state.contest_tracker.contest_threshold as u128 {
        reset_quorum_decay(env, state, current_timestamp);
        return true;
    }
    
    false
}

/// Reset quorum decay timer when proposal is contested
pub fn reset_quorum_decay(
    env: &Env,
    state: &mut AdaptiveQuorumState,
    current_timestamp: u64,
) {
    state.config.decay_start_timestamp = current_timestamp;
    state.config.is_decayed = false;
    state.contest_tracker.decay_reset_count += 1;
    
    // Emit event
    env.events().publish(
        (Symbol::new(env, "QUORUM_DECAY_RESET"), state.proposal_id),
        (
            state.config.initial_quorum_bps,
            current_timestamp,
            state.contest_tracker.decay_reset_count,
        ),
    );
}

/// Record participation data for velocity tracking
pub fn record_participation(
    env: &Env,
    proposal_id: u64,
    total_eligible: u32,
    actual_participants: u32,
    timestamp: u64,
) -> ParticipationRecord {
    let participation_rate_bps = if total_eligible > 0 {
        (actual_participants as u128 * 10000) / (total_eligible as u128)
    } else {
        0
    } as u32;
    
    ParticipationRecord {
        proposal_id,
        total_eligible_voters: total_eligible,
        actual_participants,
        participation_rate_bps,
        timestamp,
    }
}

/// Update velocity tracking with new participation record
pub fn update_velocity_tracking(
    env: &Env,
    velocity: &mut ParticipationVelocity,
    new_record: ParticipationRecord,
) {
    // Add new record
    velocity.vote_records.push_back(new_record);
    
    // Keep only last 10 records
    while velocity.vote_records.len() > MAX_VELOCITY_SAMPLES as u32 {
        velocity.vote_records.pop_front();
    }
    
    // Recalculate average
    if velocity.vote_records.len() > 0 {
        let sum: u128 = velocity.vote_records.iter()
            .map(|r| r.participation_rate_bps as u128)
            .sum();
        velocity.average_participation = (sum / velocity.vote_records.len() as u128) as u32;
    }
    
    // Calculate trend (simple: compare last 3 vs previous 3)
    if velocity.vote_records.len() >= 6 {
        let len = velocity.vote_records.len();
        let recent_sum: u128 = velocity.vote_records.iter()
            .skip((len - 3) as usize)
            .map(|r| r.participation_rate_bps as u128)
            .sum();
        let recent_avg = recent_sum / 3;
        
        let older_sum: u128 = velocity.vote_records.iter()
            .skip((len - 6) as usize)
            .take(3)
            .map(|r| r.participation_rate_bps as u128)
            .sum();
        let older_avg = older_sum / 3;
        
        velocity.velocity_trend = (recent_avg as i32) - (older_avg as i32);
    }
}

/// Get dynamic quorum adjustment based on participation velocity
pub fn get_dynamic_quorum_adjustment(
    env: &Env,
    velocity: &ParticipationVelocity,
    base_quorum: u32,
    settings: &AdaptiveQuorumSettings,
) -> u32 {
    // If not enough samples, use base quorum
    if velocity.vote_records.len() < settings.min_velocity_samples as usize {
        return base_quorum;
    }
    
    let avg = velocity.average_participation;
    
    // If participation is naturally very low, lower quorum to maintain agility
    // But never below silent sabotage threshold
    if avg < 2000 {  // Less than 20% average participation
        let adjusted = (base_quorum as u128 * 80) / 100; // Reduce by 20%
        return adjusted.max(settings.silent_sabotage_threshold as u128) as u32;
    }
    
    // If participation is trending downward, slightly lower quorum
    if velocity.velocity_trend < -500 {  // Decreasing by more than 5%
        let adjusted = (base_quorum as u128 * 90) / 100; // Reduce by 10%
        return adjusted.max(settings.silent_sabotage_threshold as u128) as u32;
    }
    
    // Otherwise, use base quorum
    base_quorum
}

/// Emit QuorumDecayTriggered event
pub fn emit_quorum_decay_triggered(
    env: &Env,
    proposal_id: u64,
    old_quorum_bps: u32,
    new_quorum_bps: u32,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "QUORUM_DECAY_TRIGGERED"), proposal_id),
        (
            old_quorum_bps,
            new_quorum_bps,
            timestamp,
        ),
    );
}

/// Create adaptive quorum state for a new proposal
pub fn create_adaptive_quorum_state(
    env: &Env,
    proposal_id: u64,
    circle_id: u64,
    proposal_type: ProposalType,
    total_eligible_voters: u32,
    voting_duration_seconds: u64,
    settings: &AdaptiveQuorumSettings,
) -> AdaptiveQuorumState {
    let current_timestamp = env.ledger().timestamp();
    
    let (initial_quorum, minimum_quorum, decay_duration) = match proposal_type {
        ProposalType::Emergency => (
            settings.emergency_initial_quorum_bps,
            settings.emergency_minimum_quorum_bps,
            settings.emergency_decay_duration_seconds,
        ),
        ProposalType::Standard => (
            settings.standard_quorum_bps,
            settings.standard_quorum_bps, // No decay for standard
            0,
        ),
    };
    
    let config = AdaptiveQuorumConfig {
        proposal_type,
        initial_quorum_bps: initial_quorum,
        minimum_quorum_bps: minimum_quorum,
        decay_start_timestamp: current_timestamp,
        decay_duration_seconds: decay_duration,
        current_quorum_bps: initial_quorum,
        is_decayed: false,
    };
    
    AdaptiveQuorumState {
        proposal_id,
        circle_id,
        config,
        contest_tracker: ContestTracker {
            contest_count: 0,
            contest_threshold: settings.contest_reset_threshold_bps,
            last_contest_timestamp: current_timestamp,
            decay_reset_count: 0,
        },
        total_eligible_voters,
        current_participants: 0,
        created_timestamp: current_timestamp,
        voting_end_timestamp: current_timestamp + voting_duration_seconds,
    }
}

/// Check if proposal has expired
pub fn is_proposal_expired(state: &AdaptiveQuorumState, current_timestamp: u64) -> bool {
    current_timestamp > state.voting_end_timestamp
}
