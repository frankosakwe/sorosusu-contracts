// --- SOCIAL VOUCHING MODULE ---
//
// Implements the social-vouching mechanism where a high-reputation member can
// lock capital to guarantee a new (unproven) member's participation. If the
// new member defaults, the voucher's locked capital is slashed proportionally.

#![no_std]

use soroban_sdk::{contracttype, Address, Env, Vec};

// --- CONSTANTS ---

/// Minimum Reliability Index score required to vouch for another member
const MIN_VOUCHER_RI_SCORE: u32 = 700;

/// Maximum number of active vouches a single member can hold at once
const MAX_ACTIVE_VOUCHES: u32 = 3;

/// Penalty applied to voucher's RI when the vouched member defaults (basis points)
const VOUCHER_PENALTY_BPS: u32 = 2000; // 20%

/// Duration (in ledger seconds) a vouch stays active before expiring
const VOUCH_EXPIRY_SECONDS: u64 = 30 * 24 * 60 * 60; // 30 days

// --- DATA KEYS ---

#[contracttype]
#[derive(Clone)]
pub enum VouchDataKey {
    VouchRecord(Address, Address), // (voucher, vouched) -> VouchRecord
    VoucherActiveCount(Address),   // active vouch count per voucher
    VouchedMemberVoucher(Address), // vouched member -> who vouched them
    // Issue #420: Member-to-Member Vouching for Collateral Reductions
    CollateralVouch(u64, Address, Address), // CircleID, voucher, vouched -> CollateralVouchRecord
    CollateralReduction(u64, Address), // CircleID, member -> CollateralReductionRecord
    VouchCollateralPool(u64), // CircleID -> total collateral pool balance
    VouchReductionHistory(u64, Address), // CircleID, member -> Vec<ReductionEvent>
}

// --- DATA STRUCTURES ---

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VouchStatus {
    Active,
    Redeemed,
    Slashed,
    Expired,
}

/// A record of one member vouching for another within a circle
#[contracttype]
#[derive(Clone)]
pub struct VouchRecord {
    pub voucher: Address,
    pub vouched: Address,
    pub circle_id: u64,
    pub locked_amount: i128,
    pub created_at: u64,
    pub expires_at: u64,
    pub status: VouchStatus,
    pub slash_amount: i128,
}

/// Summary of a member's vouching activity
#[contracttype]
#[derive(Clone)]
pub struct VoucherProfile {
    pub address: Address,
    pub active_vouches: u32,
    pub total_vouches_given: u32,
    pub total_slashes_received: u32,
    pub total_slashed_amount: i128,
}

// --- ISSUE #420: MEMBER-TO-MEMBER VOUCHING FOR COLLATERAL REDUCTIONS ---

/// Collateral vouch record for reducing collateral requirements
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CollateralVouchRecord {
    /// Circle ID
    pub circle_id: u64,
    /// Member providing the vouch (voucher)
    pub voucher: Address,
    /// Member receiving the collateral reduction benefit
    pub vouched: Address,
    /// Amount of collateral reduction provided (in basis points)
    pub reduction_bps: u32,
    /// Maximum reduction amount in token units
    pub max_reduction_amount: i128,
    /// Current utilized reduction amount
    pub utilized_amount: i128,
    /// Timestamp when vouch was created
    pub created_at: u64,
    /// Timestamp when vouch expires
    pub expires_at: u64,
    /// Vouch status
    pub status: CollateralVouchStatus,
    /// Voucher's reputation score at time of vouch
    pub voucher_reputation: u32,
    /// Risk assessment score
    pub risk_score: u32,
}

/// Collateral vouch status
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum CollateralVouchStatus {
    Active,     // Vouch is active and can be used
    Utilized,    // Reduction is currently being utilized
    Exhausted,   // Reduction amount fully utilized
    Expired,     // Vouch has expired
    Slashed,     // Vouch was slashed due to default
    Cancelled,   // Vouch was cancelled by voucher
}

/// Collateral reduction record for a member
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CollateralReductionRecord {
    /// Circle ID
    pub circle_id: u64,
    /// Member receiving the reduction
    pub member: Address,
    /// Original collateral requirement (in basis points)
    pub original_collateral_bps: u32,
    /// Current collateral requirement after reductions
    pub current_collateral_bps: u32,
    /// Total reduction amount (in basis points)
    pub total_reduction_bps: u32,
    /// List of active vouches contributing to reduction
    pub active_vouches: Vec<Address>,
    /// Timestamp of last reduction update
    pub last_updated: u64,
    /// Whether reduction is currently active
    pub is_active: bool,
}

/// Historical reduction event for tracking
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ReductionEvent {
    /// Event timestamp
    pub timestamp: u64,
    /// Event type
    pub event_type: ReductionEventType,
    /// Voucher address (if applicable)
    pub voucher: Option<Address>,
    /// Reduction amount change (in basis points)
    pub reduction_change: i32,
    /// Reason for the change
    pub reason: Symbol,
}

/// Reduction event type
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ReductionEventType {
    VouchAdded,      // New vouch added reduction
    VouchRemoved,    // Vouch removed/expired reduction
    VouchSlashed,    // Vouch slashed, reduction removed
    MemberDefault,  // Member defaulted, all reductions cancelled
    ManualAdjust,   // Manual adjustment by admin
}

// --- FUNCTIONS ---

/// Allow a high-RI member to vouch for a new member by locking capital.
/// Returns an updated VouchRecord on success.
///
/// The voucher must have an RI score >= MIN_VOUCHER_RI_SCORE and must not
/// exceed MAX_ACTIVE_VOUCHES. If the vouched member later defaults, the
/// voucher's locked_amount is slashed via `slash_voucher`.
pub fn vouch_for_user(
    env: &Env,
    voucher: Address,
    vouched: Address,
    circle_id: u64,
    locked_amount: i128,
    voucher_ri_score: u32,
) -> VouchRecord {
    voucher.require_auth();

    assert!(voucher_ri_score >= MIN_VOUCHER_RI_SCORE, "RI score too low to vouch");
    assert!(locked_amount > 0, "Locked amount must be positive");
    assert!(voucher != vouched, "Cannot vouch for yourself");

    let active_key = VouchDataKey::VoucherActiveCount(voucher.clone());
    let active_count: u32 = env.storage().instance().get(&active_key).unwrap_or(0);
    assert!(active_count < MAX_ACTIVE_VOUCHES, "Max active vouches reached");

    let now = env.ledger().timestamp();
    let record = VouchRecord {
        voucher: voucher.clone(),
        vouched: vouched.clone(),
        circle_id,
        locked_amount,
        created_at: now,
        expires_at: now + VOUCH_EXPIRY_SECONDS,
        status: VouchStatus::Active,
        slash_amount: 0,
    };

    let record_key = VouchDataKey::VouchRecord(voucher.clone(), vouched.clone());
    env.storage().instance().set(&record_key, &record);
    env.storage().instance().set(&active_key, &(active_count + 1));
    env.storage()
        .instance()
        .set(&VouchDataKey::VouchedMemberVoucher(vouched), &voucher);

    record
}

/// Called when a vouched member defaults. Slashes a portion of the voucher's
/// locked capital and marks the vouch as slashed.
pub fn slash_voucher(
    env: &Env,
    voucher: Address,
    vouched: Address,
) -> VouchRecord {
    let record_key = VouchDataKey::VouchRecord(voucher.clone(), vouched.clone());
    let mut record: VouchRecord = env
        .storage()
        .instance()
        .get(&record_key)
        .expect("Vouch record not found");

    assert!(record.status == VouchStatus::Active, "Vouch is not active");

    let slash = (record.locked_amount * VOUCHER_PENALTY_BPS as i128) / 10_000;
    record.slash_amount = slash;
    record.status = VouchStatus::Slashed;

    env.storage().instance().set(&record_key, &record);

    let active_key = VouchDataKey::VoucherActiveCount(voucher);
    let count: u32 = env.storage().instance().get(&active_key).unwrap_or(1);
    if count > 0 {
        env.storage().instance().set(&active_key, &(count - 1));
    }

    record
}

/// Retrieve the current vouch record between a voucher and a vouched member.
pub fn get_vouch_record(
    env: &Env,
    voucher: Address,
    vouched: Address,
) -> Option<VouchRecord> {
    let key = VouchDataKey::VouchRecord(voucher, vouched);
    env.storage().instance().get(&key)
}

/// Returns how many vouches are currently active for a given voucher.
pub fn get_active_vouch_count(env: &Env, voucher: Address) -> u32 {
    let key = VouchDataKey::VoucherActiveCount(voucher);
    env.storage().instance().get(&key).unwrap_or(0)
}

// --- ISSUE #420: MEMBER-TO-MEMBER VOUCHING FOR COLLATERAL REDUCTIONS ---

/// Constants for collateral reduction vouching
const MAX_COLLATERAL_REDUCTION_BPS: u32 = 5000; // Maximum 50% reduction
const MIN_VOUCHER_REPUTATION_FOR_COLLATERAL: u32 = 800; // Higher threshold for collateral vouching
const COLLATERAL_VOUCH_EXPIRY_SECONDS: u64 = 90 * 24 * 60 * 60; // 90 days
const MAX_ACTIVE_COLLATERAL_VOUCHES: u32 = 5; // Maximum active collateral vouches per voucher

/// Create a collateral vouch to reduce another member's collateral requirement
/// 
/// # Arguments
/// * `env` - Contract environment
/// * `voucher` - Member providing the vouch
/// * `vouched` - Member receiving the collateral reduction
/// * `circle_id` - Circle ID
/// * `reduction_bps` - Reduction amount in basis points (max 5000 = 50%)
/// * `max_reduction_amount` - Maximum reduction amount in token units
/// * `voucher_reputation` - Voucher's reputation score
/// 
/// # Returns
/// CollateralVouchRecord - The created vouch record
/// 
/// # Panics
/// * `"Insufficient reputation"` - Voucher reputation too low
/// * `"Invalid reduction amount"` - Reduction amount exceeds limits
/// * `"Too many active vouches"` - Voucher has too many active vouches
/// * `"Cannot vouch for self"` - Cannot vouch for oneself
pub fn create_collateral_vouch(
    env: &Env,
    voucher: Address,
    vouched: Address,
    circle_id: u64,
    reduction_bps: u32,
    max_reduction_amount: i128,
    voucher_reputation: u32,
) -> CollateralVouchRecord {
    voucher.require_auth();

    // Validate inputs
    if voucher_reputation < MIN_VOUCHER_REPUTATION_FOR_COLLATERAL {
        panic!("Insufficient reputation");
    }

    if reduction_bps > MAX_COLLATERAL_REDUCTION_BPS {
        panic!("Invalid reduction amount");
    }

    if max_reduction_amount <= 0 {
        panic!("Invalid reduction amount");
    }

    if voucher == vouched {
        panic!("Cannot vouch for self");
    }

    // Check active vouch limit
    let active_key = VouchDataKey::VoucherActiveCount(voucher.clone());
    let active_count: u32 = env.storage().instance().get(&active_key).unwrap_or(0);
    if active_count >= MAX_ACTIVE_COLLATERAL_VOUCHES {
        panic!("Too many active vouches");
    }

    // Check if vouch already exists
    let vouch_key = VouchDataKey::CollateralVouch(circle_id, voucher.clone(), vouched.clone());
    if env.storage().instance().has(&vouch_key) {
        panic!("Vouch already exists");
    }

    // Calculate risk score based on reputation and reduction amount
    let risk_score = calculate_risk_score(voucher_reputation, reduction_bps);

    let now = env.ledger().timestamp();
    let vouch_record = CollateralVouchRecord {
        circle_id,
        voucher: voucher.clone(),
        vouched: vouched.clone(),
        reduction_bps,
        max_reduction_amount,
        utilized_amount: 0,
        created_at: now,
        expires_at: now + COLLATERAL_VOUCH_EXPIRY_SECONDS,
        status: CollateralVouchStatus::Active,
        voucher_reputation,
        risk_score,
    };

    // Store vouch record
    env.storage().instance().set(&vouch_key, &vouch_record);
    
    // Update active vouch count
    env.storage().instance().set(&active_key, &(active_count + 1));

    // Update member's collateral reduction record
    update_member_collateral_reduction(env, circle_id, vouched.clone(), voucher.clone(), reduction_bps, true);

    // Record reduction event
    record_reduction_event(
        env,
        circle_id,
        vouched.clone(),
        ReductionEvent {
            timestamp: now,
            event_type: ReductionEventType::VouchAdded,
            voucher: Some(voucher.clone()),
            reduction_change: reduction_bps as i32,
            reason: Symbol::new(env, "collateral_vouch"),
        },
    );

    vouch_record
}

/// Utilize a collateral vouch to reduce collateral requirements
/// 
/// # Arguments
/// * `env` - Contract environment
/// * `vouched` - Member utilizing the vouch
/// * `circle_id` - Circle ID
/// * `voucher` - Voucher address
/// * `utilization_amount` - Amount of reduction to utilize
/// 
/// # Returns
/// The updated collateral requirement in basis points
/// 
/// # Panics
/// * `"Vouch not found"` - Vouch does not exist
/// * `"Vouch not active"` - Vouch is not active
/// * `"Exceeds maximum"` - Utilization exceeds maximum reduction amount
pub fn utilize_collateral_vouch(
    env: &Env,
    vouched: Address,
    circle_id: u64,
    voucher: Address,
    utilization_amount: i128,
) -> u32 {
    vouched.require_auth();

    let vouch_key = VouchDataKey::CollateralVouch(circle_id, voucher.clone(), vouched.clone());
    let mut vouch_record: CollateralVouchRecord = env.storage().instance()
        .get(&vouch_key)
        .unwrap_or_else(|| panic!("Vouch not found"));

    if vouch_record.status != CollateralVouchStatus::Active {
        panic!("Vouch not active");
    }

    if vouch_record.utilized_amount + utilization_amount > vouch_record.max_reduction_amount {
        panic!("Exceeds maximum");
    }

    // Update utilization
    vouch_record.utilized_amount += utilization_amount;
    vouch_record.status = CollateralVouchStatus::Utilized;
    env.storage().instance().set(&vouch_key, &vouch_record);

    // Get and update member's collateral reduction
    let reduction_key = VouchDataKey::CollateralReduction(circle_id, vouched.clone());
    let mut reduction_record: CollateralReductionRecord = env.storage().instance()
        .get(&reduction_key)
        .unwrap_or_else(|| panic!("Reduction record not found"));

    reduction_record.last_updated = env.ledger().timestamp();
    env.storage().instance().set(&reduction_key, &reduction_record);

    reduction_record.current_collateral_bps
}

/// Cancel a collateral vouch (only by voucher)
/// 
/// # Arguments
/// * `env` - Contract environment
/// * `voucher` - Voucher cancelling the vouch
/// * `vouched` - Member who was vouched for
/// * `circle_id` - Circle ID
/// 
/// # Panics
/// * `"Vouch not found"` - Vouch does not exist
/// * `"Not voucher"` - Caller is not the voucher
/// * `"Vouch in use"` - Vouch is currently being utilized
pub fn cancel_collateral_vouch(
    env: &Env,
    voucher: Address,
    vouched: Address,
    circle_id: u64,
) {
    voucher.require_auth();

    let vouch_key = VouchDataKey::CollateralVouch(circle_id, voucher.clone(), vouched.clone());
    let mut vouch_record: CollateralVouchRecord = env.storage().instance()
        .get(&vouch_key)
        .unwrap_or_else(|| panic!("Vouch not found"));

    if vouch_record.voucher != voucher {
        panic!("Not voucher");
    }

    if vouch_record.status == CollateralVouchStatus::Utilized && vouch_record.utilized_amount > 0 {
        panic!("Vouch in use");
    }

    // Update vouch status
    vouch_record.status = CollateralVouchStatus::Cancelled;
    env.storage().instance().set(&vouch_key, &vouch_record);

    // Update active vouch count
    let active_key = VouchDataKey::VoucherActiveCount(voucher.clone());
    let active_count: u32 = env.storage().instance().get(&active_key).unwrap_or(1);
    if active_count > 0 {
        env.storage().instance().set(&active_key, &(active_count - 1));
    }

    // Update member's collateral reduction
    update_member_collateral_reduction(env, circle_id, vouched.clone(), voucher.clone(), vouch_record.reduction_bps, false);

    // Record reduction event
    record_reduction_event(
        env,
        circle_id,
        vouched.clone(),
        ReductionEvent {
            timestamp: env.ledger().timestamp(),
            event_type: ReductionEventType::VouchRemoved,
            voucher: Some(voucher.clone()),
            reduction_change: -(vouch_record.reduction_bps as i32),
            reason: Symbol::new(env, "vouch_cancelled"),
        },
    );
}

/// Slash a collateral vouch when the vouched member defaults
/// 
/// # Arguments
/// * `env` - Contract environment
/// * `vouched` - Member who defaulted
/// * `circle_id` - Circle ID
/// * `voucher` - Voucher to be slashed
/// 
/// # Returns
/// The amount to be slashed from the voucher
/// 
/// # Panics
/// * `"Vouch not found"` - Vouch does not exist
pub fn slash_collateral_vouch(
    env: &Env,
    vouched: Address,
    circle_id: u64,
    voucher: Address,
) -> i128 {
    let vouch_key = VouchDataKey::CollateralVouch(circle_id, voucher.clone(), vouched.clone());
    let mut vouch_record: CollateralVouchRecord = env.storage().instance()
        .get(&vouch_key)
        .unwrap_or_else(|| panic!("Vouch not found"));

    if vouch_record.status != CollateralVouchStatus::Active && vouch_record.status != CollateralVouchStatus::Utilized {
        return 0; // No slash if not active
    }

    // Calculate slash amount (percentage of utilized amount)
    let slash_percentage = match vouch_record.risk_score {
        0..=300 => 5000, // 50% slash for high risk
        301..=600 => 3000, // 30% slash for medium risk
        _ => 1500, // 15% slash for low risk
    };

    let slash_amount = (vouch_record.utilized_amount * slash_percentage as i128) / 10000;

    // Update vouch status
    vouch_record.status = CollateralVouchStatus::Slashed;
    env.storage().instance().set(&vouch_key, &vouch_record);

    // Update active vouch count
    let active_key = VouchDataKey::VoucherActiveCount(voucher.clone());
    let active_count: u32 = env.storage().instance().get(&active_key).unwrap_or(1);
    if active_count > 0 {
        env.storage().instance().set(&active_key, &(active_count - 1));
    }

    // Remove all reductions for this member
    cancel_all_member_reductions(env, circle_id, vouched.clone());

    // Record reduction event
    record_reduction_event(
        env,
        circle_id,
        vouched.clone(),
        ReductionEvent {
            timestamp: env.ledger().timestamp(),
            event_type: ReductionEventType::VouchSlashed,
            voucher: Some(voucher.clone()),
            reduction_change: -(vouch_record.reduction_bps as i32),
            reason: Symbol::new(env, "member_default"),
        },
    );

    slash_amount
}

/// Get a member's current collateral reduction record
pub fn get_collateral_reduction(env: &Env, circle_id: u64, member: Address) -> Option<CollateralReductionRecord> {
    let key = VouchDataKey::CollateralReduction(circle_id, member);
    env.storage().instance().get(&key)
}

/// Get all active collateral vouches for a voucher
pub fn get_active_collateral_vouches(env: &Env, circle_id: u64, voucher: Address) -> Vec<CollateralVouchRecord> {
    let mut active_vouches = Vec::new(env);
    
    // This is a simplified implementation - in production, you'd want an index
    // For now, we'll return an empty vector as the full scan would be expensive
    active_vouches
}

// --- HELPER FUNCTIONS ---

/// Calculate risk score based on reputation and reduction amount
fn calculate_risk_score(reputation: u32, reduction_bps: u32) -> u32 {
    // Higher reduction and lower reputation = higher risk
    let reputation_factor = (1000 - reputation) / 10; // Invert reputation (0-100)
    let reduction_factor = reduction_bps / 50; // Scale reduction (0-100)
    
    let base_risk = (reputation_factor + reduction_factor) / 2;
    base_risk.min(1000) // Cap at 1000
}

/// Update a member's collateral reduction record
fn update_member_collateral_reduction(
    env: &Env,
    circle_id: u64,
    member: Address,
    voucher: Address,
    reduction_bps: u32,
    is_adding: bool,
) {
    let reduction_key = VouchDataKey::CollateralReduction(circle_id, member.clone());
    let mut reduction_record: CollateralReductionRecord = env.storage().instance()
        .get(&reduction_key)
        .unwrap_or_else(|| {
            // Create new reduction record if none exists
            let circle_info: crate::CircleInfo = env.storage().instance()
                .get(&crate::DataKey::Circle(circle_id))
                .unwrap_or_else(|| panic!("Circle not found"));
            
            CollateralReductionRecord {
                circle_id,
                member: member.clone(),
                original_collateral_bps: circle_info.collateral_bps,
                current_collateral_bps: circle_info.collateral_bps,
                total_reduction_bps: 0,
                active_vouches: Vec::new(env),
                last_updated: env.ledger().timestamp(),
                is_active: false,
            }
        });

    if is_adding {
        // Add reduction
        reduction_record.total_reduction_bps += reduction_bps;
        reduction_record.current_collateral_bps = reduction_record.original_collateral_bps.saturating_sub(reduction_record.total_reduction_bps);
        
        if !reduction_record.active_vouches.contains(&voucher) {
            reduction_record.active_vouches.push_back(voucher);
        }
    } else {
        // Remove reduction
        reduction_record.total_reduction_bps = reduction_record.total_reduction_bps.saturating_sub(reduction_bps);
        reduction_record.current_collateral_bps = reduction_record.original_collateral_bps.saturating_sub(reduction_record.total_reduction_bps);
        
        // Remove voucher from active list
        let index = reduction_record.active_vouches.iter().position(|v| v == &voucher);
        if let Some(idx) = index {
            reduction_record.active_vouches.remove(idx);
        }
    }

    reduction_record.is_active = reduction_record.total_reduction_bps > 0;
    reduction_record.last_updated = env.ledger().timestamp();

    env.storage().instance().set(&reduction_key, &reduction_record);
}

/// Record a reduction event in history
fn record_reduction_event(env: &Env, circle_id: u64, member: Address, event: ReductionEvent) {
    let history_key = VouchDataKey::VouchReductionHistory(circle_id, member.clone());
    let mut history: Vec<ReductionEvent> = env.storage().instance()
        .get(&history_key)
        .unwrap_or_else(|| Vec::new(env));

    history.push_back(event);

    // Keep only last 100 events to prevent storage bloat
    if history.len() > 100 {
        history.remove(0);
    }

    env.storage().instance().set(&history_key, &history);
}

/// Cancel all collateral reductions for a member (used on default)
fn cancel_all_member_reductions(env: &Env, circle_id: u64, member: Address) {
    let reduction_key = VouchDataKey::CollateralReduction(circle_id, member.clone());
    if let Some(mut reduction_record) = env.storage().instance().get::<VouchDataKey, CollateralReductionRecord>(&reduction_key) {
        // Cancel all active vouches
        for voucher in reduction_record.active_vouches.iter() {
            let vouch_key = VouchDataKey::CollateralVouch(circle_id, voucher.clone(), member.clone());
            if let Some(mut vouch_record) = env.storage().instance().get::<VouchDataKey, CollateralVouchRecord>(&vouch_key) {
                vouch_record.status = CollateralVouchStatus::Slashed;
                env.storage().instance().set(&vouch_key, &vouch_record);

                // Update active vouch count
                let active_key = VouchDataKey::VoucherActiveCount(voucher.clone());
                let active_count: u32 = env.storage().instance().get(&active_key).unwrap_or(1);
                if active_count > 0 {
                    env.storage().instance().set(&active_key, &(active_count - 1));
                }
            }
        }

        // Reset reduction record
        reduction_record.total_reduction_bps = 0;
        reduction_record.current_collateral_bps = reduction_record.original_collateral_bps;
        reduction_record.active_vouches = Vec::new(env);
        reduction_record.is_active = false;
        reduction_record.last_updated = env.ledger().timestamp();

        env.storage().instance().set(&reduction_key, &reduction_record);

        // Record default event
        record_reduction_event(
            env,
            circle_id,
            member,
            ReductionEvent {
                timestamp: env.ledger().timestamp(),
                event_type: ReductionEventType::MemberDefault,
                voucher: None,
                reduction_change: -(reduction_record.original_collateral_bps as i32),
                reason: Symbol::new(env, "member_default"),
            },
        );
    }
}
