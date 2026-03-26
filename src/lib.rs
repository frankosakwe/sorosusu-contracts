#![no_std]
use soroban_sdk::{contract, contracttype, contractimpl, contractclient, Address, Env, Vec, Symbol, token, testutils::{Address as TestAddress, Arbitrary as TestArbitrary}, arbitrary::{Arbitrary, Unstructured}, Map};

// --- DATA STRUCTURES ---

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Circle(u64),
    Member(Address),
    CircleCount,
    Deposit(u64, Address),
    GroupReserve,
    // Governance Token Mining Keys
    GovernanceToken,
    VestingVault,
    UserVesting(Address),
    MiningConfig,
    TotalMinedTokens,
    UserMiningStats(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct Member {
    pub address: Address,
    pub index: u32,
    pub contribution_count: u32,
    pub last_contribution_time: u64,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct CircleInfo {
    pub id: u64,
    pub creator: Address,
    pub contribution_amount: u64,
    pub max_members: u16,
    pub member_count: u16,
    pub current_recipient_index: u16,
    pub is_active: bool,
    pub token: Address,
    pub deadline_timestamp: u64,
    pub cycle_duration: u64,
    pub contribution_bitmap: u64,
    pub payout_bitmap: u64,
    pub insurance_balance: u64,
    pub insurance_fee_bps: u32,
    pub is_insurance_used: bool,
    pub late_fee_bps: u32,
    pub proposed_late_fee_bps: u32,
    pub proposal_votes_bitmap: u64,
    pub nft_contract: Address,
    pub cycle_count: u32, // Track completed cycles for vesting
}

#[contracttype]
#[derive(Clone)]
pub struct MiningConfig {
    pub tokens_per_contribution: u64,
    pub vesting_duration_cycles: u32,
    pub cliff_cycles: u32,
    pub max_mining_per_circle: u64,
    pub is_mining_enabled: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct UserVestingInfo {
    pub total_allocated: u64,
    pub vested_amount: u64,
    pub claimed_amount: u64,
    pub start_cycle: u32,
    pub contributions_made: u32,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct UserMiningStats {
    pub total_contributions: u32,
    pub total_tokens_earned: u64,
    pub total_tokens_claimed: u64,
    pub join_timestamp: u64,
    pub last_mining_timestamp: u64,
}

// --- CONTRACT TRAITS ---

pub trait SoroSusuTrait {
    fn init(env: Env, admin: Address);
    fn create_circle(env: Env, creator: Address, amount: u64, max_members: u16, token: Address, cycle_duration: u64, insurance_fee_bps: u32, nft_contract: Address) -> u64;
    fn join_circle(env: Env, user: Address, circle_id: u64);
    fn deposit(env: Env, user: Address, circle_id: u64);
    fn trigger_insurance_coverage(env: Env, caller: Address, circle_id: u64, member: Address);
    fn propose_penalty_change(env: Env, user: Address, circle_id: u64, new_bps: u32);
    fn vote_penalty_change(env: Env, user: Address, circle_id: u64);
    fn eject_member(env: Env, caller: Address, circle_id: u64, member: Address);
    
    // Governance Token Mining Functions
    fn set_governance_token(env: Env, admin: Address, token_address: Address);
    fn configure_mining(env: Env, admin: Address, config: MiningConfig);
    fn claim_vested_tokens(env: Env, user: Address);
    fn get_user_vesting_info(env: Env, user: Address) -> UserVestingInfo;
    fn get_mining_stats(env: Env, user: Address) -> UserMiningStats;
    fn complete_circle_cycle(env: Env, circle_id: u64);
}

#[contractclient(name = "SusuNftClient")]
pub trait SusuNftTrait {
    fn mint(env: Env, to: Address, token_id: u128);
    fn burn(env: Env, from: Address, token_id: u128);
}

#[contractclient(name = "GovernanceTokenClient")]
pub trait GovernanceTokenTrait {
    fn mint(env: Env, to: Address, amount: u64);
}

// --- IMPLEMENTATION ---

#[contract]
pub struct SoroSusu;

#[contractimpl]
impl SoroSusuTrait for SoroSusu {
    fn init(env: Env, admin: Address) {
        if !env.storage().instance().has(&DataKey::CircleCount) {
            env.storage().instance().set(&DataKey::CircleCount, &0u64);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        
        // Initialize governance mining with default config
        let default_config = MiningConfig {
            tokens_per_contribution: 100, // 100 tokens per contribution
            vesting_duration_cycles: 12,   // 12 cycles (1 year if monthly)
            cliff_cycles: 3,               // 3 cycles cliff period
            max_mining_per_circle: 1000,    // Max 1000 tokens per circle
            is_mining_enabled: false,       // Disabled by default
        };
        env.storage().instance().set(&DataKey::MiningConfig, &default_config);
        env.storage().instance().set(&DataKey::TotalMinedTokens, &0u64);
    }

    fn create_circle(env: Env, creator: Address, amount: u64, max_members: u16, token: Address, cycle_duration: u64, insurance_fee_bps: u32, nft_contract: Address) -> u64 {
        let mut circle_count: u64 = env.storage().instance().get(&DataKey::CircleCount).unwrap_or(0);
        circle_count += 1;

        if max_members > 64 {
            panic!("Max members cannot exceed 64 for optimization");
        }

        if insurance_fee_bps > 10000 {
            panic!("Insurance fee cannot exceed 100%");
        }

        let current_time = env.ledger().timestamp();
        let new_circle = CircleInfo {
            id: circle_count,
            creator: creator.clone(),
            contribution_amount: amount,
            max_members,
            member_count: 0,
            current_recipient_index: 0,
            is_active: true,
            token,
            deadline_timestamp: current_time + cycle_duration,
            cycle_duration,
            contribution_bitmap: 0,
            payout_bitmap: 0,
            insurance_balance: 0,
            insurance_fee_bps,
            is_insurance_used: false,
            late_fee_bps: 100,
            proposed_late_fee_bps: 0,
            proposal_votes_bitmap: 0,
            nft_contract,
            cycle_count: 0,
        };

        env.storage().instance().set(&DataKey::Circle(circle_count), &new_circle);
        env.storage().instance().set(&DataKey::CircleCount, &circle_count);

        if !env.storage().instance().has(&DataKey::GroupReserve) {
            env.storage().instance().set(&DataKey::GroupReserve, &0u64);
        }

        circle_count
    }

    fn join_circle(env: Env, user: Address, circle_id: u64) {
        user.require_auth();

        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();

        if circle.member_count >= circle.max_members {
            panic!("Circle is full");
        }

        let member_key = DataKey::Member(user.clone());
        if env.storage().instance().has(&member_key) {
            panic!("User is already a member");
        }

        let new_member = Member {
            address: user.clone(),
            index: circle.member_count as u32,
            contribution_count: 0,
            last_contribution_time: 0,
            is_active: true,
        };
        
        env.storage().instance().set(&member_key, &new_member);
        circle.member_count += 1;
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Initialize user mining stats
        let stats_key = DataKey::UserMiningStats(user.clone());
        if !env.storage().instance().has(&stats_key) {
            let stats = UserMiningStats {
                total_contributions: 0,
                total_tokens_earned: 0,
                total_tokens_claimed: 0,
                join_timestamp: env.ledger().timestamp(),
                last_mining_timestamp: 0,
            };
            env.storage().instance().set(&stats_key, &stats);
        }

        // Initialize user vesting info
        let vesting_key = DataKey::UserVesting(user.clone());
        if !env.storage().instance().has(&vesting_key) {
            let vesting_info = UserVestingInfo {
                total_allocated: 0,
                vested_amount: 0,
                claimed_amount: 0,
                start_cycle: 0,
                contributions_made: 0,
                is_active: false,
            };
            env.storage().instance().set(&vesting_key, &vesting_info);
        }

        let token_id = (circle_id as u128) << 64 | (new_member.index as u128);
        let client = SusuNftClient::new(&env, &circle.nft_contract);
        client.mint(&user, &token_id);
    }

    fn deposit(env: Env, user: Address, circle_id: u64) {
        user.require_auth();

        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();

        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env.storage().instance().get(&member_key)
            .unwrap_or_else(|| panic!("User is not a member of this circle"));

        if !member.is_active {
            panic!("Member is ejected");
        }

        let client = token::Client::new(&env, &circle.token);

        let current_time = env.ledger().timestamp();
        let mut penalty_amount = 0u64;

        if current_time > circle.deadline_timestamp {
            penalty_amount = (circle.contribution_amount * circle.late_fee_bps as u64) / 10000;
            
            let mut reserve_balance: u64 = env.storage().instance().get(&DataKey::GroupReserve).unwrap_or(0);
            reserve_balance += penalty_amount;
            env.storage().instance().set(&DataKey::GroupReserve, &reserve_balance);
        }

        let insurance_fee = ((circle.contribution_amount as u128 * circle.insurance_fee_bps as u128) / 10000) as u64;
        let total_amount = circle.contribution_amount + insurance_fee;

        client.transfer(&user, &env.current_contract_address(), &total_amount);

        if insurance_fee > 0 {
            circle.insurance_balance += insurance_fee;
        }

        // ** GOVERNANCE TOKEN MINING LOGIC **
        Self::mine_governance_tokens(env.clone(), user.clone(), circle_id, &mut circle, &mut member);

        member.contribution_count += 1;
        member.last_contribution_time = current_time;
        
        env.storage().instance().set(&member_key, &member);

        circle.deadline_timestamp = current_time + circle.cycle_duration;
        circle.contribution_bitmap |= 1 << member.index;
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Check if cycle is complete and trigger payout/mining release
        Self::check_and_complete_cycle(env.clone(), circle_id);
    }

    fn set_governance_token(env: Env, admin: Address, token_address: Address) {
        // Check admin authorization
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Admin not set"));
        if admin != stored_admin {
            panic!("Unauthorized: Only admin can set governance token");
        }

        env.storage().instance().set(&DataKey::GovernanceToken, &token_address);
        
        // Enable mining by default when token is set
        let mut config: MiningConfig = env.storage().instance().get(&DataKey::MiningConfig).unwrap();
        config.is_mining_enabled = true;
        env.storage().instance().set(&DataKey::MiningConfig, &config);
    }

    fn configure_mining(env: Env, admin: Address, config: MiningConfig) {
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Admin not set"));
        if admin != stored_admin {
            panic!("Unauthorized: Only admin can configure mining");
        }

        if config.tokens_per_contribution == 0 {
            panic!("Tokens per contribution must be greater than 0");
        }

        if config.vesting_duration_cycles == 0 {
            panic!("Vesting duration must be greater than 0");
        }

        if config.cliff_cycles > config.vesting_duration_cycles {
            panic!("Cliff period cannot exceed vesting duration");
        }

        env.storage().instance().set(&DataKey::MiningConfig, &config);
    }

    fn claim_vested_tokens(env: Env, user: Address) {
        user.require_auth();

        let governance_token: Address = env.storage().instance().get(&DataKey::GovernanceToken)
            .unwrap_or_else(|| panic!("Governance token not set"));

        let vesting_key = DataKey::UserVesting(user.clone());
        let mut vesting_info: UserVestingInfo = env.storage().instance().get(&vesting_key)
            .unwrap_or_else(|| panic!("No vesting info found for user"));

        if !vesting_info.is_active || vesting_info.total_allocated == 0 {
            panic!("No active vesting found");
        }

        let current_cycle = Self::get_current_global_cycle(env.clone());
        let vested_amount = Self::calculate_vested_amount(
            vesting_info.total_allocated,
            vesting_info.start_cycle,
            current_cycle,
            vesting_info.contributions_made,
        );

        let claimable_amount = vested_amount - vesting_info.claimed_amount;
        if claimable_amount == 0 {
            panic!("No tokens available to claim");
        }

        // Update claimed amount
        vesting_info.claimed_amount += claimable_amount;
        env.storage().instance().set(&vesting_key, &vesting_info);

        // Update user stats
        let stats_key = DataKey::UserMiningStats(user.clone());
        let mut stats: UserMiningStats = env.storage().instance().get(&stats_key).unwrap();
        stats.total_tokens_claimed += claimable_amount;
        env.storage().instance().set(&stats_key, &stats);

        // Transfer tokens
        let token_client = token::Client::new(&env, &governance_token);
        token_client.transfer(&env.current_contract_address(), &user, &claimable_amount);

        // Emit event
        env.events().publish(
            (Symbol::short("tokens_claimed"), user.clone()),
            claimable_amount,
        );
    }

    fn get_user_vesting_info(env: Env, user: Address) -> UserVestingInfo {
        let vesting_key = DataKey::UserVesting(user);
        env.storage().instance().get(&vesting_key)
            .unwrap_or_else(|| UserVestingInfo {
                total_allocated: 0,
                vested_amount: 0,
                claimed_amount: 0,
                start_cycle: 0,
                contributions_made: 0,
                is_active: false,
            })
    }

    fn get_mining_stats(env: Env, user: Address) -> UserMiningStats {
        let stats_key = DataKey::UserMiningStats(user);
        env.storage().instance().get(&stats_key)
            .unwrap_or_else(|| UserMiningStats {
                total_contributions: 0,
                total_tokens_earned: 0,
                total_tokens_claimed: 0,
                join_timestamp: 0,
                last_mining_timestamp: 0,
            })
    }

    fn complete_circle_cycle(env: Env, circle_id: u64) {
        Self::check_and_complete_cycle(env, circle_id);
    }

    fn trigger_insurance_coverage(env: Env, caller: Address, circle_id: u64, member: Address) {
        caller.require_auth();

        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();

        if caller != circle.creator {
            panic!("Unauthorized: Only creator can trigger insurance");
        }

        if circle.is_insurance_used {
            panic!("Insurance already used this cycle");
        }

        if circle.insurance_balance < circle.contribution_amount {
            panic!("Insufficient insurance balance");
        }

        let member_key = DataKey::Member(member.clone());
        let member_info: Member = env.storage().instance().get(&member_key).unwrap();

        if !member_info.is_active {
            panic!("Member is ejected");
        }

        if (circle.contribution_bitmap & (1 << member_info.index)) != 0 {
            panic!("Member already contributed");
        }

        circle.contribution_bitmap |= 1 << member_info.index;
        circle.insurance_balance -= circle.contribution_amount;
        circle.is_insurance_used = true;

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    }

    fn propose_penalty_change(env: Env, user: Address, circle_id: u64, new_bps: u32) {
        user.require_auth();
        
        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
        
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key).expect("User is not a member");

        if !member.is_active {
            panic!("Member is ejected");
        }

        if new_bps > 10000 {
            panic!("Penalty cannot exceed 100%");
        }

        circle.proposed_late_fee_bps = new_bps;
        circle.proposal_votes_bitmap = 0;
        circle.proposal_votes_bitmap |= 1 << member.index;

        if circle.proposal_votes_bitmap.count_ones() > (circle.member_count as u32 / 2) {
            circle.late_fee_bps = circle.proposed_late_fee_bps;
            circle.proposed_late_fee_bps = 0;
            circle.proposal_votes_bitmap = 0;
        }

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    }

    fn vote_penalty_change(env: Env, user: Address, circle_id: u64) {
        user.require_auth();

        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
        
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key).expect("User is not a member");

        if !member.is_active {
            panic!("Member is ejected");
        }

        if circle.proposed_late_fee_bps == 0 {
            panic!("No active proposal");
        }

        circle.proposal_votes_bitmap |= 1 << member.index;

        if circle.proposal_votes_bitmap.count_ones() > (circle.member_count as u32 / 2) {
            circle.late_fee_bps = circle.proposed_late_fee_bps;
            circle.proposed_late_fee_bps = 0;
            circle.proposal_votes_bitmap = 0;
        }

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    }

    fn eject_member(env: Env, caller: Address, circle_id: u64, member: Address) {
        caller.require_auth();
        
        let circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
        
        if caller != circle.creator {
            panic!("Unauthorized: Only creator can eject members");
        }

        let member_key = DataKey::Member(member.clone());
        let mut member_info: Member = env.storage().instance().get(&member_key).expect("Member not found");

        if !member_info.is_active {
            panic!("Member already ejected");
        }

        member_info.is_active = false;
        env.storage().instance().set(&member_key, &member_info);

        // Deactivate vesting
        let vesting_key = DataKey::UserVesting(member.clone());
        if let Ok(mut vesting_info) = env.storage().instance().get::<DataKey, UserVestingInfo>(&vesting_key) {
            vesting_info.is_active = false;
            env.storage().instance().set(&vesting_key, &vesting_info);
        }

        let token_id = (circle_id as u128) << 64 | (member_info.index as u128);
        let client = SusuNftClient::new(&env, &circle.nft_contract);
        client.burn(&member, &token_id);
    }
}

// --- PRIVATE HELPER FUNCTIONS ---

impl SoroSusu {
    fn mine_governance_tokens(env: Env, user: Address, circle_id: u64, circle: &mut CircleInfo, member: &mut Member) {
        let config: MiningConfig = env.storage().instance().get(&DataKey::MiningConfig).unwrap();
        
        if !config.is_mining_enabled {
            return;
        }

        let governance_token: Address = env.storage().instance().get(&DataKey::GovernanceToken);
        if governance_token.is_none() {
            return;
        }

        let governance_token = governance_token.unwrap();

        // Check if user has already mined for this contribution cycle
        let contribution_key = DataKey::Deposit(circle_id, user.clone());
        if env.storage().instance().has(&contribution_key) {
            return; // Already mined for this contribution
        }

        // Calculate mining amount
        let mining_amount = config.tokens_per_contribution;
        
        // Check max mining per circle
        let total_mined: u64 = env.storage().instance().get(&DataKey::TotalMinedTokens).unwrap_or(0);
        if total_mined + mining_amount > config.max_mining_per_circle {
            return; // Max mining reached for this circle
        }

        // Update user vesting
        let vesting_key = DataKey::UserVesting(user.clone());
        let mut vesting_info: UserVestingInfo = env.storage().instance().get(&vesting_key).unwrap();
        
        if !vesting_info.is_active {
            vesting_info.start_cycle = circle.cycle_count;
            vesting_info.is_active = true;
        }
        
        vesting_info.total_allocated += mining_amount;
        vesting_info.contributions_made += 1;
        env.storage().instance().set(&vesting_key, &vesting_info);

        // Update user stats
        let stats_key = DataKey::UserMiningStats(user.clone());
        let mut stats: UserMiningStats = env.storage().instance().get(&stats_key).unwrap();
        stats.total_contributions += 1;
        stats.total_tokens_earned += mining_amount;
        stats.last_mining_timestamp = env.ledger().timestamp();
        env.storage().instance().set(&stats_key, &stats);

        // Update total mined tokens
        env.storage().instance().set(&DataKey::TotalMinedTokens, &(total_mined + mining_amount));

        // Mark as mined for this contribution
        env.storage().instance().set(&contribution_key, &true);

        // Mint tokens to vesting vault (contract holds them)
        let token_client = token::Client::new(&env, &governance_token);
        token_client.mint(&env.current_contract_address(), &mining_amount);

        // Emit mining event
        env.events().publish(
            (Symbol::short("tokens_mined"), user.clone(), circle_id),
            mining_amount,
        );
    }

    fn check_and_complete_cycle(env: Env, circle_id: u64) {
        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();
        
        // Check if all active members have contributed
        let required_contributions = circle.member_count;
        let current_contributions = circle.contribution_bitmap.count_ones() as u16;
        
        if current_contributions >= required_contributions {
            // Cycle complete - increment cycle count
            circle.cycle_count += 1;
            
            // Reset contribution bitmap for next cycle
            circle.contribution_bitmap = 0;
            circle.is_insurance_used = false;
            
            // Update deadline for next cycle
            circle.deadline_timestamp = env.ledger().timestamp() + circle.cycle_duration;
            
            env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
            
            // Emit cycle completion event
            env.events().publish(
                (Symbol::short("cycle_completed"), circle_id),
                circle.cycle_count,
            );
        }
    }

    fn calculate_vested_amount(
        total_allocated: u64,
        start_cycle: u32,
        current_cycle: u32,
        contributions_made: u32,
    ) -> u64 {
        if current_cycle <= start_cycle {
            return 0;
        }

        let cycles_passed = current_cycle - start_cycle;
        let config: MiningConfig = Env::default().storage().instance().get(&DataKey::MiningConfig).unwrap();
        
        if cycles_passed <= config.cliff_cycles {
            return 0;
        }

        let vesting_cycles = config.vesting_duration_cycles;
        if cycles_passed >= vesting_cycles {
            return total_allocated;
        }

        let vesting_progress = cycles_passed - config.cliff_cycles;
        let total_vesting_cycles = vesting_cycles - config.cliff_cycles;
        
        let vested_amount = (total_allocated as u128 * vesting_progress as u128) / total_vesting_cycles as u128;
        vested_amount as u64
    }

    fn get_current_global_cycle(env: Env) -> u32 {
        // Simple implementation: use ledger timestamp / average cycle duration
        let avg_cycle_duration = 604800; // 1 week in seconds
        let current_timestamp = env.ledger().timestamp();
        (current_timestamp / avg_cycle_duration) as u32
    }
}

// ... (rest of the code remains the same)