use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol,
};

// --- SOROSUSU SOULBOUND TOKEN (SBT) SYSTEM ---

const MIN_REPUTATION_BADGE_CYCLES: u32 = 12;
const DEFAULT_RELIABILITY_SCORE: u32 = 5_000;
const DEFAULT_SOCIAL_CAPITAL_SCORE: u32 = 5_000;

#[derive(Clone)]
#[contracttype]
enum SbtDataKey {
    Admin,
    MilestoneCount,
    CredentialCount,
    Milestone(u64),
    Credential(u128),
    UserCredential(Address),
    UserReputation(Address),
    CycleAward(Address, u64, u32),
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum SbtStatus {
    Discovery,
    Pathfinder,
    Guardian,
    Luminary,
    SusuLegend,
    Delinquent,
    Revoked,
    Burned,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum ReputationTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct SoroSusuCredential {
    pub token_id: u128,
    pub user: Address,
    pub status: SbtStatus,
    pub reputation_score: u32,
    pub metadata_uri: String,
    pub issue_date: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct UserReputationMetrics {
    pub reliability_score: u32, // Based on contribution timeliness (0-10000)
    pub social_capital_score: u32, // Based on leniency/participation
    pub total_cycles: u32,
    pub perfect_cycles: u32,
    pub total_volume_saved: i128,
    pub defaults_count: u32,
    pub last_updated: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ReputationMilestone {
    pub user: Address,
    pub required_cycles: u32,
    pub description: String,
    pub tier: ReputationTier,
    pub is_completed: bool,
}

#[contract]
pub struct SoroSusuSbtMinter;

fn default_metrics(now: u64) -> UserReputationMetrics {
    UserReputationMetrics {
        reliability_score: DEFAULT_RELIABILITY_SCORE,
        social_capital_score: DEFAULT_SOCIAL_CAPITAL_SCORE,
        total_cycles: 0,
        perfect_cycles: 0,
        total_volume_saved: 0,
        defaults_count: 0,
        last_updated: now,
    }
}

fn tier_for_score(score: u32) -> ReputationTier {
    if score >= 9_500 {
        ReputationTier::Diamond
    } else if score >= 8_500 {
        ReputationTier::Platinum
    } else if score >= 7_500 {
        ReputationTier::Gold
    } else if score >= 6_500 {
        ReputationTier::Silver
    } else {
        ReputationTier::Bronze
    }
}

fn status_for_tier(tier: &ReputationTier) -> SbtStatus {
    match tier {
        ReputationTier::Bronze => SbtStatus::Discovery,
        ReputationTier::Silver => SbtStatus::Pathfinder,
        ReputationTier::Gold => SbtStatus::Guardian,
        ReputationTier::Platinum => SbtStatus::Luminary,
        ReputationTier::Diamond => SbtStatus::SusuLegend,
    }
}

fn metadata_for_status(env: &Env, status: &SbtStatus) -> String {
    match status {
        SbtStatus::Discovery => String::from_str(env, "ipfs://sorosusu/reputation/bronze"),
        SbtStatus::Pathfinder => String::from_str(env, "ipfs://sorosusu/reputation/silver"),
        SbtStatus::Guardian => String::from_str(env, "ipfs://sorosusu/reputation/gold"),
        SbtStatus::Luminary => String::from_str(env, "ipfs://sorosusu/reputation/platinum"),
        SbtStatus::SusuLegend => String::from_str(env, "ipfs://sorosusu/reputation/diamond"),
        SbtStatus::Delinquent => String::from_str(env, "ipfs://sorosusu/reputation/delinquent"),
        SbtStatus::Revoked => String::from_str(env, "ipfs://sorosusu/reputation/revoked"),
        SbtStatus::Burned => String::from_str(env, "ipfs://sorosusu/reputation/burned"),
    }
}

fn is_locked_status(status: &SbtStatus) -> bool {
    matches!(
        status,
        SbtStatus::Delinquent | SbtStatus::Revoked | SbtStatus::Burned
    )
}

fn require_admin(env: &Env) -> Address {
    let admin: Address = env
        .storage()
        .instance()
        .get(&SbtDataKey::Admin)
        .unwrap_or_else(|| panic!("SBT minter not initialized"));
    admin.require_auth();
    admin
}

fn emit_badge_update(env: &Env, credential: &SoroSusuCredential) {
    env.events().publish(
        (
            Symbol::new(env, "ReputationBadgeUpdated"),
            credential.token_id,
        ),
        (
            credential.user.clone(),
            credential.status.clone(),
            credential.metadata_uri.clone(),
        ),
    );
}

fn compute_reliability_score(total_cycles: u32, perfect_cycles: u32, defaults_count: u32) -> u32 {
    if total_cycles == 0 {
        return DEFAULT_RELIABILITY_SCORE;
    }

    let on_time_score = (perfect_cycles.min(total_cycles) * 10_000) / total_cycles;
    let default_penalty = defaults_count.saturating_mul(1_500);
    on_time_score.saturating_sub(default_penalty)
}

fn next_token_id(env: &Env) -> u128 {
    let mut count: u128 = env
        .storage()
        .instance()
        .get(&SbtDataKey::CredentialCount)
        .unwrap_or(0);
    count += 1;
    env.storage()
        .instance()
        .set(&SbtDataKey::CredentialCount, &count);
    count
}

fn refresh_credential_from_metrics(
    env: &Env,
    mut credential: SoroSusuCredential,
) -> SoroSusuCredential {
    if is_locked_status(&credential.status) {
        credential.metadata_uri = metadata_for_status(env, &credential.status);
        return credential;
    }

    let metrics: UserReputationMetrics = env
        .storage()
        .instance()
        .get(&SbtDataKey::UserReputation(credential.user.clone()))
        .unwrap_or_else(|| default_metrics(env.ledger().timestamp()));

    credential.reputation_score = metrics.reliability_score;
    credential.status = status_for_tier(&tier_for_score(metrics.reliability_score));
    credential.metadata_uri = metadata_for_status(env, &credential.status);
    credential
}

fn store_credential(env: &Env, credential: &SoroSusuCredential) {
    env.storage()
        .instance()
        .set(&SbtDataKey::Credential(credential.token_id), credential);
    env.storage().instance().set(
        &SbtDataKey::UserCredential(credential.user.clone()),
        &credential.token_id,
    );
}

#[contractimpl]
impl SoroSusuSbtMinter {
    // Initialize SBT Minter with admin
    pub fn init_sbt_minter(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&SbtDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&SbtDataKey::MilestoneCount, &0u64);
        env.storage()
            .instance()
            .set(&SbtDataKey::CredentialCount, &0u128);
    }

    // Set new admin
    pub fn set_sbt_minter_admin(env: Env, admin: Address, new_admin: Address) {
        let current_admin: Address = env.storage().instance().get(&SbtDataKey::Admin).unwrap();
        admin.require_auth();
        if admin != current_admin {
            panic!();
        }
        env.storage().instance().set(&SbtDataKey::Admin, &new_admin);
    }

    // Issue SBT credential for an admin-created 12-month milestone.
    pub fn issue_credential(
        env: Env,
        user: Address,
        milestone_id: u64,
        _metadata_uri: String,
    ) -> u128 {
        let mut milestone: ReputationMilestone = env
            .storage()
            .instance()
            .get(&SbtDataKey::Milestone(milestone_id))
            .unwrap();
        if milestone.user != user {
            panic!();
        }
        if milestone.is_completed {
            panic!();
        }
        if milestone.required_cycles < MIN_REPUTATION_BADGE_CYCLES {
            panic!("minimum 12 cycles required");
        }

        let metrics: UserReputationMetrics = env
            .storage()
            .instance()
            .get(&SbtDataKey::UserReputation(user.clone()))
            .unwrap_or_else(|| default_metrics(env.ledger().timestamp()));
        if metrics.total_cycles < milestone.required_cycles {
            panic!("milestone not complete");
        }
        if metrics.total_volume_saved <= 0 {
            panic!("zero-value cycles cannot mint reputation badges");
        }

        let reputation_score = metrics.reliability_score;
        let token_id = if let Some(existing) = env
            .storage()
            .instance()
            .get::<SbtDataKey, u128>(&SbtDataKey::UserCredential(user.clone()))
        {
            existing
        } else {
            next_token_id(&env)
        };
        let status = status_for_tier(&milestone.tier);
        let credential = SoroSusuCredential {
            token_id,
            user: user.clone(),
            status: status.clone(),
            reputation_score,
            metadata_uri: metadata_for_status(&env, &status),
            issue_date: env.ledger().timestamp(),
        };

        store_credential(&env, &credential);
        milestone.is_completed = true;
        env.storage()
            .instance()
            .set(&SbtDataKey::Milestone(milestone_id), &milestone);
        emit_badge_update(&env, &credential);

        token_id
    }

    pub fn update_credential_status(env: Env, token_id: u128, new_status: SbtStatus) {
        require_admin(&env);
        let mut credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        credential.status = new_status;
        credential.metadata_uri = metadata_for_status(&env, &credential.status);
        store_credential(&env, &credential);
        emit_badge_update(&env, &credential);
    }

    pub fn revoke_credential(env: Env, token_id: u128, reason: String) {
        require_admin(&env);
        let mut credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        credential.status = SbtStatus::Revoked;
        credential.metadata_uri = metadata_for_status(&env, &credential.status);
        store_credential(&env, &credential);
        env.events().publish(
            (symbol_short!("SBTREV"), token_id),
            (credential.user.clone(), reason),
        );
        emit_badge_update(&env, &credential);
    }

    pub fn burn_credential(env: Env, token_id: u128) {
        require_admin(&env);
        let mut credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        credential.status = SbtStatus::Burned;
        credential.metadata_uri = metadata_for_status(&env, &credential.status);
        store_credential(&env, &credential);
        emit_badge_update(&env, &credential);
    }

    pub fn mark_defaulted(env: Env, user: Address) {
        require_admin(&env);
        let token_id: u128 = env
            .storage()
            .instance()
            .get(&SbtDataKey::UserCredential(user.clone()))
            .unwrap_or_else(|| panic!("credential not found"));

        let mut metrics: UserReputationMetrics = env
            .storage()
            .instance()
            .get(&SbtDataKey::UserReputation(user.clone()))
            .unwrap_or_else(|| default_metrics(env.ledger().timestamp()));
        metrics.defaults_count += 1;
        metrics.reliability_score = compute_reliability_score(
            metrics.total_cycles,
            metrics.perfect_cycles,
            metrics.defaults_count,
        );
        metrics.last_updated = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&SbtDataKey::UserReputation(user), &metrics);

        let mut credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        credential.status = SbtStatus::Delinquent;
        credential.reputation_score = metrics.reliability_score;
        credential.metadata_uri = metadata_for_status(&env, &credential.status);
        store_credential(&env, &credential);
        emit_badge_update(&env, &credential);
    }

    pub fn record_cycle_completion(
        env: Env,
        admin: Address,
        user: Address,
        circle_id: u64,
        cycle_value: i128,
        cycles_completed: u32,
        on_time_cycles: u32,
    ) -> u128 {
        let stored_admin: Address = env.storage().instance().get(&SbtDataKey::Admin).unwrap();
        admin.require_auth();
        if admin != stored_admin {
            panic!("unauthorized");
        }
        if cycle_value <= 0 {
            panic!("zero-value cycles cannot mint reputation badges");
        }
        if cycles_completed < MIN_REPUTATION_BADGE_CYCLES {
            panic!("minimum 12 cycles required");
        }
        if on_time_cycles > cycles_completed {
            panic!("invalid on-time cycle count");
        }

        let award_key = SbtDataKey::CycleAward(user.clone(), circle_id, cycles_completed);
        if env.storage().instance().has(&award_key) {
            panic!("cycle already recorded");
        }
        env.storage().instance().set(&award_key, &true);

        let mut metrics: UserReputationMetrics = env
            .storage()
            .instance()
            .get(&SbtDataKey::UserReputation(user.clone()))
            .unwrap_or_else(|| default_metrics(env.ledger().timestamp()));
        metrics.total_cycles = metrics.total_cycles.saturating_add(cycles_completed);
        metrics.perfect_cycles = metrics.perfect_cycles.saturating_add(on_time_cycles);
        metrics.total_volume_saved = metrics.total_volume_saved.saturating_add(cycle_value);
        metrics.reliability_score = compute_reliability_score(
            metrics.total_cycles,
            metrics.perfect_cycles,
            metrics.defaults_count,
        );
        metrics.last_updated = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&SbtDataKey::UserReputation(user.clone()), &metrics);

        let token_id = env
            .storage()
            .instance()
            .get::<SbtDataKey, u128>(&SbtDataKey::UserCredential(user.clone()))
            .unwrap_or_else(|| next_token_id(&env));

        let status = status_for_tier(&tier_for_score(metrics.reliability_score));
        let credential = SoroSusuCredential {
            token_id,
            user: user.clone(),
            status: status.clone(),
            reputation_score: metrics.reliability_score,
            metadata_uri: metadata_for_status(&env, &status),
            issue_date: env.ledger().timestamp(),
        };

        store_credential(&env, &credential);
        emit_badge_update(&env, &credential);
        token_id
    }

    pub fn refresh_metadata(env: Env, token_id: u128) -> String {
        let credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        let refreshed = refresh_credential_from_metrics(&env, credential);
        store_credential(&env, &refreshed);
        emit_badge_update(&env, &refreshed);
        refreshed.metadata_uri
    }

    pub fn metadata_uri(env: Env, token_id: u128) -> String {
        let credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        refresh_credential_from_metrics(&env, credential).metadata_uri
    }

    pub fn get_current_tier(env: Env, token_id: u128) -> ReputationTier {
        let credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        if credential.status == SbtStatus::Delinquent
            || credential.status == SbtStatus::Revoked
            || credential.status == SbtStatus::Burned
        {
            return ReputationTier::Bronze;
        }
        tier_for_score(refresh_credential_from_metrics(&env, credential).reputation_score)
    }

    pub fn get_credential(env: Env, token_id: u128) -> SoroSusuCredential {
        let credential: SoroSusuCredential = env
            .storage()
            .instance()
            .get(&SbtDataKey::Credential(token_id))
            .unwrap();
        refresh_credential_from_metrics(&env, credential)
    }

    pub fn get_user_credential(env: Env, user: Address) -> Option<SoroSusuCredential> {
        let token_id: Option<u128> = env
            .storage()
            .instance()
            .get(&SbtDataKey::UserCredential(user));
        token_id.map(|id| Self::get_credential(env.clone(), id))
    }

    pub fn create_reputation_milestone(
        env: Env,
        user: Address,
        cycles: u32,
        desc: String,
        tier: ReputationTier,
    ) -> u64 {
        require_admin(&env);
        if cycles < MIN_REPUTATION_BADGE_CYCLES {
            panic!("minimum 12 cycles required");
        }
        let mut count: u64 = env
            .storage()
            .instance()
            .get(&SbtDataKey::MilestoneCount)
            .unwrap_or(0);
        count += 1;
        let milestone = ReputationMilestone {
            user,
            required_cycles: cycles,
            description: desc,
            tier,
            is_completed: false,
        };
        env.storage()
            .instance()
            .set(&SbtDataKey::Milestone(count), &milestone);
        env.storage()
            .instance()
            .set(&SbtDataKey::MilestoneCount, &count);
        count
    }

    pub fn get_reputation_milestone(env: Env, milestone_id: u64) -> ReputationMilestone {
        env.storage()
            .instance()
            .get(&SbtDataKey::Milestone(milestone_id))
            .unwrap()
    }

    pub fn update_user_reputation_metrics(
        env: Env,
        admin: Address,
        user: Address,
        reliability_score: u32,
        social_capital_score: u32,
        total_cycles: u32,
        perfect_cycles: u32,
        total_volume_saved: i128,
    ) {
        let stored_admin: Address = env.storage().instance().get(&SbtDataKey::Admin).unwrap();
        admin.require_auth();
        if admin != stored_admin {
            panic!("unauthorized");
        }
        if reliability_score > 10_000 || social_capital_score > 10_000 {
            panic!("score out of range");
        }
        let metrics = UserReputationMetrics {
            reliability_score,
            social_capital_score,
            total_cycles,
            perfect_cycles,
            total_volume_saved,
            defaults_count: 0,
            last_updated: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&SbtDataKey::UserReputation(user), &metrics);
    }

    pub fn get_user_reputation_score(env: Env, user: Address) -> (u32, u32, u32) {
        let metrics: UserReputationMetrics = env
            .storage()
            .instance()
            .get(&SbtDataKey::UserReputation(user))
            .unwrap_or_else(|| default_metrics(env.ledger().timestamp()));
        (
            metrics.reliability_score,
            metrics.social_capital_score,
            (metrics.reliability_score + metrics.social_capital_score) / 2,
        )
    }

    pub fn transfer(_env: Env, _from: Address, _to: Address, _token_id: u128) {
        panic!("SBT is non-transferable");
    }

    pub fn transfer_from(
        _env: Env,
        _spender: Address,
        _from: Address,
        _to: Address,
        _token_id: u128,
    ) {
        panic!("SBT is non-transferable");
    }

    pub fn approve(_env: Env, _owner: Address, _spender: Address, _token_id: u128) {
        panic!("SBT approvals are disabled");
    }
}
