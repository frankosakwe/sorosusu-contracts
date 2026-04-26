/// Dispute resolution module for SoroSusu.
///
/// Implements:
/// - Issue #315: Cross-Contract Reentrancy Guard (NON_REENTRANT flag)
/// - Issue #316: Zombie-Group Sweep (cleanup_group)
/// - Issue #322: Dispute Bond Slashing (raise_dispute with bond lock)
/// - Issue #325: Immutable Audit Trail Events (Soroban events for dispute lifecycle)

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
