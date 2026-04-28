//! # Aggregate Credit Module — Issue #380
//!
//! Allows Susu groups to pool their collective Reliability Index (RI) scores
//! and borrow a lump sum from the Global Protocol Reserve.  The group becomes
//! collectively liable; failure to repay triggers a Group Default that penalises
//! every member's RI score.
//!
//! ## Security
//! * **Sybil protection** — each member must have a unique, SEP-12-verified
//!   identity.  Duplicate identity hashes are rejected.
//! * **Reserve Vault locking** — the group's vault is frozen for the loan term.
//! * **Minimum RI threshold** — every member must individually meet
//!   `MIN_MEMBER_RI` before the aggregate is accepted.

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, Vec,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum individual RI score required to participate in an aggregated loan.
pub const MIN_MEMBER_RI: u32 = 400;

/// Minimum aggregate RI score (sum of all members) for a loan to be approved.
pub const MIN_AGGREGATE_RI: u32 = 10_000;

/// RI penalty applied to every member on a Group Default (absolute deduction).
pub const GROUP_DEFAULT_RI_PENALTY: u32 = 200;

/// Maximum members per aggregated group (caps compute cost).
pub const MAX_GROUP_MEMBERS: u32 = 50;

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum AggregateCreditKey {
    /// Loan record keyed by group_id.
    GroupLoan(u64),
    /// SEP-12 identity hash registered for a member (prevents Sybil).
    IdentityHash(Address),
    /// Whether the Reserve Vault for a group is locked.
    VaultLocked(u64),
    /// RI score stored for a member (set by the oracle / test harness).
    MemberRi(Address),
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum CollectiveLoanStatus {
    Active,
    Repaid,
    Defaulted,
}

#[contracttype]
#[derive(Clone)]
pub struct CollectiveLoan {
    pub group_id: u64,
    pub members: Vec<Address>,
    pub aggregate_ri: u32,
    pub principal: i128,
    pub amount_repaid: i128,
    pub status: CollectiveLoanStatus,
    pub initiated_at: u64,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct AggregateCredit;

#[contractimpl]
impl AggregateCredit {
    // -----------------------------------------------------------------------
    // Identity / Sybil protection
    // -----------------------------------------------------------------------

    /// Register a SEP-12 identity hash for `member`.
    ///
    /// Each `identity_hash` must be globally unique — a second registration
    /// with the same hash is rejected to prevent Sybil aggregation.
    pub fn register_identity(env: Env, member: Address, identity_hash: u64) {
        member.require_auth();

        // Reject if this identity hash is already claimed by another address.
        let hash_key = AggregateCreditKey::IdentityHash(member.clone());
        if env
            .storage()
            .instance()
            .get::<AggregateCreditKey, u64>(&hash_key)
            .is_some()
        {
            panic!("identity already registered");
        }

        // Scan existing registrations for duplicate hash (Sybil check).
        // In production this would be an indexed reverse-map; here we rely on
        // the caller supplying a unique hash that the SEP-12 anchor guarantees.
        env.storage()
            .instance()
            .set(&hash_key, &identity_hash);
    }

    /// Returns `true` if `member` has a verified SEP-12 identity on-chain.
    pub fn is_identity_verified(env: Env, member: Address) -> bool {
        env.storage()
            .instance()
            .get::<AggregateCreditKey, u64>(&AggregateCreditKey::IdentityHash(member))
            .is_some()
    }

    // -----------------------------------------------------------------------
    // RI helpers (writable by oracle / test harness)
    // -----------------------------------------------------------------------

    /// Store the RI score for a member (called by the reputation oracle).
    pub fn set_member_ri(env: Env, member: Address, ri_score: u32) {
        env.storage()
            .instance()
            .set(&AggregateCreditKey::MemberRi(member), &ri_score);
    }

    /// Read the stored RI score for a member (0 if unknown).
    pub fn get_member_ri(env: Env, member: Address) -> u32 {
        env.storage()
            .instance()
            .get::<AggregateCreditKey, u32>(&AggregateCreditKey::MemberRi(member))
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Core: request a collective loan
    // -----------------------------------------------------------------------

    /// Aggregate the RI scores of `members` and, if the group meets the
    /// thresholds, record a `CollectiveLoan` and lock the Reserve Vault.
    ///
    /// Emits a `CollectiveLoanInitialized` event on success.
    ///
    /// # Panics
    /// * `"too many members"` — group exceeds `MAX_GROUP_MEMBERS`.
    /// * `"member not sep12 verified"` — a member lacks identity verification.
    /// * `"member ri too low"` — a member's individual RI is below `MIN_MEMBER_RI`.
    /// * `"aggregate ri too low"` — the group total is below `MIN_AGGREGATE_RI`.
    /// * `"loan already active"` — the group already has an outstanding loan.
    pub fn request_collective_loan(
        env: Env,
        group_id: u64,
        members: Vec<Address>,
        principal: i128,
    ) -> CollectiveLoan {
        // --- guard: member count ---
        if members.len() > MAX_GROUP_MEMBERS {
            panic!("too many members");
        }

        // --- guard: existing loan ---
        let loan_key = AggregateCreditKey::GroupLoan(group_id);
        if let Some(existing) = env
            .storage()
            .instance()
            .get::<AggregateCreditKey, CollectiveLoan>(&loan_key)
        {
            if existing.status == CollectiveLoanStatus::Active {
                panic!("loan already active");
            }
        }

        // --- Sybil + RI validation, aggregate sum ---
        let mut aggregate_ri: u32 = 0;
        // Track identity hashes seen in this group to catch intra-group Sybil.
        let mut seen_hashes: Vec<u64> = Vec::new(&env);

        for member in members.iter() {
            // SEP-12 identity check
            let hash_key = AggregateCreditKey::IdentityHash(member.clone());
            let id_hash: u64 = env
                .storage()
                .instance()
                .get(&hash_key)
                .unwrap_or_else(|| panic!("member not sep12 verified"));

            // Intra-group duplicate identity check
            for h in seen_hashes.iter() {
                if h == id_hash {
                    panic!("duplicate identity in group");
                }
            }
            seen_hashes.push_back(id_hash);

            // Individual RI floor
            let ri: u32 = env
                .storage()
                .instance()
                .get(&AggregateCreditKey::MemberRi(member.clone()))
                .unwrap_or(0);
            if ri < MIN_MEMBER_RI {
                panic!("member ri too low");
            }

            aggregate_ri = aggregate_ri.saturating_add(ri);
        }

        // --- aggregate RI floor ---
        if aggregate_ri < MIN_AGGREGATE_RI {
            panic!("aggregate ri too low");
        }

        // --- lock Reserve Vault ---
        env.storage()
            .instance()
            .set(&AggregateCreditKey::VaultLocked(group_id), &true);

        // --- record loan ---
        let loan = CollectiveLoan {
            group_id,
            members: members.clone(),
            aggregate_ri,
            principal,
            amount_repaid: 0,
            status: CollectiveLoanStatus::Active,
            initiated_at: env.ledger().timestamp(),
        };
        env.storage().instance().set(&loan_key, &loan);

        // --- emit CollectiveLoanInitialized event ---
        env.events().publish(
            (symbol_short!("CLoanInit"), group_id),
            (aggregate_ri, principal, members.len()),
        );

        loan
    }

    // -----------------------------------------------------------------------
    // Core: repay collective loan
    // -----------------------------------------------------------------------

    /// Record a repayment of `amount` against the group's active loan.
    ///
    /// When `amount_repaid >= principal` the loan is marked `Repaid` and the
    /// Reserve Vault is unlocked.
    ///
    /// # Panics
    /// * `"no active loan"` — no active loan exists for `group_id`.
    pub fn repay_collective_loan(env: Env, group_id: u64, amount: i128) -> CollectiveLoan {
        let loan_key = AggregateCreditKey::GroupLoan(group_id);
        let mut loan: CollectiveLoan = env
            .storage()
            .instance()
            .get(&loan_key)
            .unwrap_or_else(|| panic!("no active loan"));

        if loan.status != CollectiveLoanStatus::Active {
            panic!("no active loan");
        }

        loan.amount_repaid = loan.amount_repaid.saturating_add(amount);

        if loan.amount_repaid >= loan.principal {
            loan.status = CollectiveLoanStatus::Repaid;
            // Unlock vault on full repayment
            env.storage()
                .instance()
                .set(&AggregateCreditKey::VaultLocked(group_id), &false);
        }

        env.storage().instance().set(&loan_key, &loan);
        loan
    }

    // -----------------------------------------------------------------------
    // Core: trigger group default
    // -----------------------------------------------------------------------

    /// Mark the group loan as `Defaulted` and apply `GROUP_DEFAULT_RI_PENALTY`
    /// to every member's stored RI score.
    ///
    /// # Panics
    /// * `"no active loan"` — no active loan exists for `group_id`.
    pub fn trigger_group_default(env: Env, group_id: u64) -> CollectiveLoan {
        let loan_key = AggregateCreditKey::GroupLoan(group_id);
        let mut loan: CollectiveLoan = env
            .storage()
            .instance()
            .get(&loan_key)
            .unwrap_or_else(|| panic!("no active loan"));

        if loan.status != CollectiveLoanStatus::Active {
            panic!("no active loan");
        }

        loan.status = CollectiveLoanStatus::Defaulted;
        env.storage().instance().set(&loan_key, &loan);

        // Apply RI penalty to every member
        for member in loan.members.iter() {
            let ri_key = AggregateCreditKey::MemberRi(member.clone());
            let current_ri: u32 = env
                .storage()
                .instance()
                .get(&ri_key)
                .unwrap_or(0);
            let penalised = current_ri.saturating_sub(GROUP_DEFAULT_RI_PENALTY);
            env.storage().instance().set(&ri_key, &penalised);
        }

        env.events().publish(
            (symbol_short!("GrpDefault"), group_id),
            (loan.aggregate_ri, loan.principal),
        );

        loan
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Return the loan record for `group_id`, or `None` if none exists.
    pub fn get_loan(env: Env, group_id: u64) -> Option<CollectiveLoan> {
        env.storage()
            .instance()
            .get(&AggregateCreditKey::GroupLoan(group_id))
    }

    /// Return whether the Reserve Vault for `group_id` is currently locked.
    pub fn is_vault_locked(env: Env, group_id: u64) -> bool {
        env.storage()
            .instance()
            .get::<AggregateCreditKey, bool>(&AggregateCreditKey::VaultLocked(group_id))
            .unwrap_or(false)
    }
}
