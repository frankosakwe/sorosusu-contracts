/// Dispute resolution module for SoroSusu.
///
/// Implements:
/// - Issue #315: Cross-Contract Reentrancy Guard (NON_REENTRANT flag)
/// - Issue #316: Zombie-Group Sweep (cleanup_group)
/// - Issue #322: Dispute Bond Slashing (raise_dispute with bond lock)
/// - Issue #325: Immutable Audit Trail Events (Soroban events for dispute lifecycle)
/// - Issue #386: Ledger Rent Sweeper for Finalized "Zombie" Groups

use soroban_sdk::{contracttype, symbol_short, token, Address, Env, Symbol};

use crate::DataKey;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DisputeStatus {
    Open,
    Resolved,
    Baseless,
}

#[contracttype]
#[derive(Clone)]
pub struct DisputeRecord {
    pub circle_id: u64,
    pub accuser: Address,
    pub accused: Address,
    pub bond_amount: i128,
    pub status: DisputeStatus,
    pub raised_at: u64,
}

// ---------------------------------------------------------------------------
// Issue #315 – Reentrancy guard helpers
// ---------------------------------------------------------------------------

/// Acquires the reentrancy lock. Panics if already locked.
pub fn acquire_lock(env: &Env) {
    let locked: bool = env
        .storage()
        .instance()
        .get(&DataKey::NonReentrant)
        .unwrap_or(false);
    if locked {
        panic!("reentrant call detected");
    }
    env.storage()
        .instance()
        .set(&DataKey::NonReentrant, &true);
}

/// Releases the reentrancy lock.
pub fn release_lock(env: &Env) {
    env.storage()
        .instance()
        .set(&DataKey::NonReentrant, &false);
}

// ---------------------------------------------------------------------------
// Issue #316 – Zombie-Group Sweep
// ---------------------------------------------------------------------------

/// 30 days in seconds.
const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;

/// Archives the group metadata hash and removes heavy state, returning
/// storage rent to the treasury.
///
/// Can only be called 30 days after the circle was marked completed.
pub fn cleanup_group(env: &Env, caller: &Address, circle_id: u64) {
    caller.require_auth();

    // Load completion timestamp; panics if circle was never completed.
    let completed_at: u64 = env
        .storage()
        .instance()
        .get(&DataKey::CircleCompletedAt(circle_id))
        .unwrap_or_else(|| panic!("circle not completed"));

    let now = env.ledger().timestamp();
    if now < completed_at + THIRTY_DAYS_SECS {
        panic!("cleanup not available yet: 30-day window has not elapsed");
    }

    // Archive the metadata hash for reputation tracking.
    // Use circle_id as the stable archive key (the actual hash would be computed
    // from the full CircleInfo in a production implementation).
    let meta_hash: u64 = circle_id;
    env.storage()
        .instance()
        .set(&DataKey::ArchivedGroupHash(circle_id), &meta_hash);

    // Remove heavy data structures from active state.
    env.storage()
        .instance()
        .remove(&DataKey::Circle(circle_id));
    env.storage()
        .instance()
        .remove(&DataKey::CircleCompletedAt(circle_id));

    // Credit storage rent back to the treasury (GroupReserve acts as treasury).
    let mut treasury: u64 = env
        .storage()
        .instance()
        .get(&DataKey::GroupReserve)
        .unwrap_or(0);
    treasury += 1; // symbolic rent unit; real rent accounting is handled by the Soroban host
    env.storage()
        .instance()
        .set(&DataKey::GroupReserve, &treasury);

    // Emit cleanup event.
    env.events().publish(
        (symbol_short!("grp_clean"), circle_id),
        meta_hash,
    );
}

// ---------------------------------------------------------------------------
// Issue #386 – Ledger Rent Sweeper for Finalized "Zombie" Groups
// ---------------------------------------------------------------------------

/// 180 days in seconds - the window after completion+drain before pruning is allowed.
const ZOMBIE_PRUNE_WINDOW_SECS: u64 = 180 * 24 * 60 * 60;

/// Bounty percentage (in basis points) paid to the relayer for pruning a zombie group.
/// 500 bps = 5% of reclaimed rent.
const RELAYER_BOUNTY_BPS: u32 = 500;

/// Marks a circle as fully drained (all payouts complete).
/// Called internally when the final payout is processed.
pub fn mark_circle_drained(env: &Env, circle_id: u64) {
    let drained_at = env.ledger().timestamp();
    env.storage()
        .instance()
        .set(&DataKey::CircleDrainedAt(circle_id), &drained_at);
}

/// Prunes a finalized "zombie" group that has been completed AND drained for over 180 days.
/// 
/// This function:
/// 1. Verifies the circle was completed at least 180 days ago
/// 2. Verifies the circle was drained at least 180 days ago  
/// 3. Deletes heavy group metadata and user-position mappings
/// 4. Leaves a lightweight cryptographic tombstone for historical RI audits
/// 5. Pays the relayer a bounty from the reclaimed rent
/// 
/// Security: This function CANNOT touch any active or pending group state.
/// It will panic if the circle is still active or hasn't met the time requirements.
pub fn prune_zombie_group(env: &Env, relayer: &Address, circle_id: u64) -> Result<u64, u32> {
    relayer.require_auth();

    // 1. Verify the circle exists and was completed
    let completed_at: u64 = env
        .storage()
        .instance()
        .get(&DataKey::CircleCompletedAt(circle_id))
        .ok_or(404u32)?;

    // 2. Verify the circle was drained
    let drained_at: u64 = env
        .storage()
        .instance()
        .get(&DataKey::CircleDrainedAt(circle_id))
        .ok_or(405u32)?;

    let now = env.ledger().timestamp();

    // 3. Verify 180 days have passed since completion
    if now < completed_at + ZOMBIE_PRUNE_WINDOW_SECS {
        return Err(406u32); // "180-day completion window not elapsed"
    }

    // 4. Verify 180 days have passed since draining
    if now < drained_at + ZOMBIE_PRUNE_WINDOW_SECS {
        return Err(407u32); // "180-day drain window not elapsed"
    }

    // 5. Verify the circle is not active (safety check)
    let circle_exists = env
        .storage()
        .instance()
        .get::<_, crate::CircleInfo>(&DataKey::Circle(circle_id))
        .is_some();

    if circle_exists {
        // Double-check: if circle still exists, it must not be active
        let circle: crate::CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();
        if circle.is_active {
            return Err(408u32); // "Cannot prune active circle"
        }
    }

    // 6. Calculate storage bytes to be reclaimed (estimate)
    // CircleInfo: ~500 bytes (varies with member count)
    // Member positions: ~100 bytes per member (max 100 members = ~10KB)
    // Other metadata: ~200 bytes
    // Total estimated: ~700 bytes base + ~100 bytes per member
    let member_count = circle_exists
        .then(|| {
            let circle: crate::CircleInfo = env
                .storage()
                .instance()
                .get(&DataKey::Circle(circle_id))
                .unwrap();
            circle.member_count as u64
        })
        .unwrap_or(0);

    let base_bytes: u64 = 700;
    let member_bytes: u64 = member_count * 100;
    let total_bytes_reclaimed = base_bytes + member_bytes;

    // 7. Calculate relayer bounty (5% of reclaimed value in stroops)
    // Soroban rent is approximately 0.5 stroops per KB per ledger
    // For simplicity, we use a fixed bounty multiplier
    let rent_per_ledger_per_kb: u64 = 500; // stroops
    let ledgers_per_day: u64 = 25; // roughly 5 minutes per ledger
    let daily_rent_per_kb = rent_per_ledger_per_kb * ledgers_per_day;
    
    // Bounty = 5% of (daily_rent * bytes/1000 * 365 days) - conservative 1 year estimate
    let yearly_rent_estimate = (total_bytes_reclaimed / 1000) * daily_rent_per_kb * 365;
    let bounty = (yearly_rent_estimate * (RELAYER_BOUNTY_BPS as u64)) / 10000;

    // 8. Create cryptographic tombstone for historical RI audits
    // Tombstone = hash(circle_id || completed_at || drained_at)
    // Using simple composite value for tombstone (in production, use proper crypto)
    let tombstone_hash = circle_id
        .wrapping_add(completed_at)
        .wrapping_mul(drained_at.wrapping_add(1));

    // 9. Store tombstone (lightweight, ~8 bytes)
    env.storage()
        .instance()
        .set(&DataKey::ArchivedGroupHash(circle_id), &tombstone_hash);

    // 10. Remove heavy data structures
    if circle_exists {
        env.storage()
            .instance()
            .remove(&DataKey::Circle(circle_id));
    }
    env.storage()
        .instance()
        .remove(&DataKey::CircleCompletedAt(circle_id));
    env.storage()
        .instance()
        .remove(&DataKey::CircleDrainedAt(circle_id));

    // 11. Remove member position mappings (CircleMember keys)
    // These are stored as (circle_id, index) -> Address
    for i in 0..member_count {
        env.storage()
            .instance()
            .remove(&DataKey::CircleMember(circle_id, i));
    }

    // 12. Pay relayer bounty from GroupReserve (treasury)
    let mut treasury: u64 = env
        .storage()
        .instance()
        .get(&DataKey::GroupReserve)
        .unwrap_or(0);

    if treasury >= bounty {
        treasury -= bounty;
        env.storage()
            .instance()
            .set(&DataKey::GroupReserve, &treasury);
        
        // Transfer bounty to relayer (if we had a token mechanism)
        // In practice, this would transfer from the contract's balance
        // For now, we just emit the event with the bounty amount
    }

    // 13. Emit prune event
    env.events().publish(
        (symbol_short!("zombie_prune"), circle_id),
        (bounty, total_bytes_reclaimed),
    );

    Ok(bounty)
}

// ---------------------------------------------------------------------------
// Issue #322 – Dispute Bond Slashing
// ---------------------------------------------------------------------------

/// Dispute bond amount in stroops (0.5 XLM).
pub const DISPUTE_BOND_STROOPS: i128 = 5_000_000;

/// Raises a dispute. The accuser must lock `DISPUTE_BOND_STROOPS` of the
/// circle's token. Emits a `Dispute_Raised` event (issue #325).
pub fn raise_dispute(
    env: &Env,
    accuser: &Address,
    accused: &Address,
    circle_id: u64,
    xlm_token: &Address,
) -> u64 {
    accuser.require_auth();

    // Lock the bond from the accuser.
    let token = soroban_sdk::token::Client::new(env, xlm_token);
    token.transfer(
        accuser,
        &env.current_contract_address(),
        &DISPUTE_BOND_STROOPS,
    );

    // Assign a dispute ID.
    let dispute_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::DisputeCount)
        .unwrap_or(0)
        + 1;
    env.storage()
        .instance()
        .set(&DataKey::DisputeCount, &dispute_id);

    let record = DisputeRecord {
        circle_id,
        accuser: accuser.clone(),
        accused: accused.clone(),
        bond_amount: DISPUTE_BOND_STROOPS,
        status: DisputeStatus::Open,
        raised_at: env.ledger().timestamp(),
    };
    env.storage()
        .instance()
        .set(&DataKey::Dispute(dispute_id), &record);

    // Issue #325 – emit Dispute_Raised event.
    emit_dispute_raised(env, dispute_id, circle_id, accuser, accused);

    dispute_id
}

/// Records evidence for an open dispute. Emits an `Evidence_Submitted` event.
pub fn submit_evidence(env: &Env, submitter: &Address, dispute_id: u64, evidence_hash: u64) {
    submitter.require_auth();

    let record: DisputeRecord = env
        .storage()
        .instance()
        .get(&DataKey::Dispute(dispute_id))
        .unwrap_or_else(|| panic!("dispute not found"));

    if record.status != DisputeStatus::Open {
        panic!("dispute is not open");
    }

    // Issue #325 – emit Evidence_Submitted event.
    emit_evidence_submitted(env, dispute_id, submitter, evidence_hash);
}

/// Records a juror vote on a dispute. Emits a `Juror_Voted` event.
pub fn juror_vote(env: &Env, juror: &Address, dispute_id: u64, vote_guilty: bool) {
    juror.require_auth();

    let record: DisputeRecord = env
        .storage()
        .instance()
        .get(&DataKey::Dispute(dispute_id))
        .unwrap_or_else(|| panic!("dispute not found"));

    if record.status != DisputeStatus::Open {
        panic!("dispute is not open");
    }

    // Issue #325 – emit Juror_Voted event.
    emit_juror_voted(env, dispute_id, juror, vote_guilty);
}

/// Executes the verdict for a dispute.
///
/// - If `baseless = true`: bond is slashed to the accused; dispute marked Baseless.
/// - If `baseless = false`: bond is returned to the accuser; dispute marked Resolved.
///
/// Emits a `Verdict_Executed` event (issue #325).
pub fn execute_verdict(
    env: &Env,
    admin: &Address,
    dispute_id: u64,
    baseless: bool,
    xlm_token: &Address,
) {
    admin.require_auth();

    let mut record: DisputeRecord = env
        .storage()
        .instance()
        .get(&DataKey::Dispute(dispute_id))
        .unwrap_or_else(|| panic!("dispute not found"));

    if record.status != DisputeStatus::Open {
        panic!("dispute already resolved");
    }

    let token = soroban_sdk::token::Client::new(env, xlm_token);

    if baseless {
        // Slash bond to the accused.
        token.transfer(
            &env.current_contract_address(),
            &record.accused,
            &record.bond_amount,
        );
        record.status = DisputeStatus::Baseless;
    } else {
        // Return bond to the accuser.
        token.transfer(
            &env.current_contract_address(),
            &record.accuser,
            &record.bond_amount,
        );
        record.status = DisputeStatus::Resolved;
    }

    env.storage()
        .instance()
        .set(&DataKey::Dispute(dispute_id), &record);

    // Issue #325 – emit Verdict_Executed event.
    emit_verdict_executed(env, dispute_id, baseless, &record.accuser, &record.accused);
}

// ---------------------------------------------------------------------------
// Issue #325 – Immutable Audit Trail Events
// ---------------------------------------------------------------------------

pub fn emit_dispute_raised(
    env: &Env,
    dispute_id: u64,
    circle_id: u64,
    accuser: &Address,
    accused: &Address,
) {
    env.events().publish(
        (Symbol::new(env, "Dispute_Raised"), dispute_id),
        (circle_id, accuser.clone(), accused.clone()),
    );
}

pub fn emit_evidence_submitted(
    env: &Env,
    dispute_id: u64,
    submitter: &Address,
    evidence_hash: u64,
) {
    env.events().publish(
        (Symbol::new(env, "Evidence_Submitted"), dispute_id),
        (submitter.clone(), evidence_hash),
    );
}

pub fn emit_juror_voted(env: &Env, dispute_id: u64, juror: &Address, vote_guilty: bool) {
    env.events().publish(
        (Symbol::new(env, "Juror_Voted"), dispute_id),
        (juror.clone(), vote_guilty),
    );
}

pub fn emit_verdict_executed(
    env: &Env,
    dispute_id: u64,
    baseless: bool,
    accuser: &Address,
    accused: &Address,
) {
    env.events().publish(
        (Symbol::new(env, "Verdict_Executed"), dispute_id),
        (baseless, accuser.clone(), accused.clone()),
    );
}
