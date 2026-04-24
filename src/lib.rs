#![cfg_attr(not(test), no_std)]
#[cfg(test)] extern crate std;

mod yield_oracle_circuit_breaker;
mod yield_strategy_trait;

#[cfg(test)]
mod yield_strategy_tests;

use yield_oracle_circuit_breaker::{YieldOracleCircuitBreaker, CircuitBreakerState, HealthMetrics};
use yield_strategy_trait::{
    YieldStrategyTrait, YieldStrategyClient, YieldStrategyConfig, YieldInfo, 
    DepositParams, WithdrawalParams, YieldEstimate, RegisteredStrategy, StrategyType,
    YieldStrategyRegistryKey, validate_deposit_amount, validate_withdrawal_params, 
    calculate_estimated_yield
};

// --- DATA STRUCTURES ---

#[derive(Clone)]
pub struct CircleInfo {
    pub creator: Address,
    pub contribution_amount: u64,
    pub max_members: u16,
    pub current_members: u16,
    pub token: Address,
    pub cycle_duration: u64,
    pub insurance_fee_bps: u32, // basis points (100 = 1%)
    pub organizer_fee_bps: u32,  // basis points (100 = 1%)
    pub nft_contract: Address,
    pub arbitrator: Address,
    pub members: Vec<Address>,
    pub contributions: Map<Address, bool>,
    pub current_round: u32,
    pub round_start_time: u64,
    pub is_round_finalized: bool,
    pub current_pot_recipient: Option<Address>,
    pub gas_buffer_balance: i128, // XLM buffer for gas fees
    pub gas_buffer_enabled: bool,
}

#[derive(Clone)]
pub struct Member {
    pub address: Address,
    pub join_time: u64,
    pub total_contributions: i128,
    pub total_received: i128,
    pub has_contributed_current_round: bool,
    pub consecutive_missed_rounds: u32,
}

#[derive(Clone)]
pub struct GasBufferConfig {
    pub min_buffer_amount: i128,     // Minimum XLM to maintain as buffer
    pub max_buffer_amount: i128,     // Maximum XLM that can be buffered
    pub auto_refill_threshold: i128, // When to auto-refill the buffer
    pub emergency_buffer: i128,      // Emergency buffer for extreme network conditions
}

// --- STORAGE KEYS ---

#[derive(Clone)]
pub enum DataKey {
    Admin,
    CircleCount,
    Circle(u64),
    Member(Address),
    MemberByIndex(u64, u32), // For efficient recipient lookup
    GasBufferConfig(u64),  // Per-circle gas buffer config
    ProtocolConfig,
    ScheduledPayoutTime(u64),
    YieldDelegation(u64),
    YieldVote(u64, Address),
    YieldPoolRegistry,
    GroupTreasury(u64),
    YieldStrategyRegistry, // Registry for yield strategies
    ActiveYieldStrategy(u64), // Active strategy per circle
}

// --- CONTRACT TRAIT ---

pub trait SoroSusuTrait {
    // Initialize the contract
    fn init(env: Env, admin: Address);
    
    // Create a new savings circle
    fn create_circle(
        env: Env,
        creator: Address,
        contribution_amount: u64,
        max_members: u16,
#![no_std]
use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, symbol_short, token,
    Address, Env, String, Symbol, Vec, Map, BytesN, IntoVal,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    MemberNotFound = 2,
    CircleFull = 3,
    AlreadyMember = 4,
    CircleNotFound = 5,
    InvalidAmount = 6,
    RoundAlreadyFinalized = 7,
    RoundNotFinalized = 8,
    NotAllContributed = 9,
    PayoutNotScheduled = 10,
    PayoutTooEarly = 11,
    InsufficientInsurance = 12,
    InsuranceAlreadyUsed = 13,
    RateLimitExceeded = 14,
    InsufficientCollateral = 15,
    CollateralAlreadyStaked = 16,
    CollateralNotStaked = 17,
    CollateralLocked = 18,
    MemberNotDefaulted = 19,
    CollateralAlreadyReleased = 20,
    LeniencyRequestNotFound = 21,
    AlreadyVoted = 22,
    VotingPeriodExpired = 23,
    LeniencyAlreadyApproved = 24,
    LeniencyNotRequested = 25,
    CannotVoteForOwnRequest = 26,
    InvalidVote = 27,
    ProposalNotFound = 28,
    ProposalAlreadyExecuted = 29,
    VotingNotActive = 30,
    InsufficientVotingPower = 31,
    QuadraticVoteExceeded = 32,
    InvalidProposalType = 33,
    QuorumNotMet = 34,
    ProposalExpired = 35,
    AppealNotFound = 36,
    AppealAlreadyFinalized = 37,
}

// --- CONSTANTS ---
const REFERRAL_DISCOUNT_BPS: u32 = 500; // 5%
const RATE_LIMIT_SECONDS: u64 = 300; // 5 minutes
const LENIENCY_GRACE_PERIOD: u64 = 172800; // 48 hours in seconds
const VOTING_PERIOD: u64 = 86400; // 24 hours voting period
const MINIMUM_VOTING_PARTICIPATION: u32 = 50; // 50% minimum participation
const SIMPLE_MAJORITY_THRESHOLD: u32 = 51; // 51% simple majority
const QUADRATIC_VOTING_PERIOD: u64 = 604800; // 7 days for rule changes
const QUADRATIC_QUORUM: u32 = 40; // 40% quorum for quadratic voting
const QUADRATIC_MAJORITY: u32 = 60; // 60% supermajority for rule changes
const MAX_VOTE_WEIGHT: u32 = 100; // Maximum quadratic vote weight
const MIN_GROUP_SIZE_FOR_QUADRATIC: u32 = 10; // Enable quadratic voting for groups >= 10 members
const DEFAULT_COLLATERAL_BPS: u32 = 2000; // 20%
const HIGH_VALUE_THRESHOLD: i128 = 1_000_000_0; // 1000 XLM (assuming 7 decimals)
const REPUTATION_AMNESTY_THRESHOLD: u32 = 66; // 66% for 2/3 majority
const MAX_RI: u32 = 1000;
const RI_PENALTY: u32 = 200;
const RI_RESTORE: u32 = 200;

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
    ScheduledPayoutTime(u64),
    LastCreatedTimestamp(Address),
    SafetyDeposit(Address, u64),
    LendingPool,
    CollateralVault(Address, u64),
    CollateralConfig(u64),
    DefaultedMembers(u64),
    LeniencyRequest(u64, Address),
    LeniencyVotes(u64, Address, Address),
    SocialCapital(Address, u64),
    LeniencyStats(u64),
    Proposal(u64),
    QuadraticVote(u64, Address),
    VotingPower(Address, u64),
    ProposalStats(u64),
    ReliabilityIndex(Address),
    ReputationAppeal(u64, Address),
    AppealVotes(u64, Address, Address),
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum MemberStatus {
    Active,
    AwaitingReplacement,
    Ejected,
    Defaulted,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum LeniencyVote {
    Approve,
    Reject,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum LeniencyRequestStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalType {
    ChangeLateFee,
    ChangeInsuranceFee,
    ChangeCycleDuration,
    AddMember,
    RemoveMember,
    ChangeQuorum,
    EmergencyAction,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalStatus {
    Draft,
    Active,
    Approved,
    Rejected,
    Executed,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum QuadraticVoteChoice {
    For,
    Against,
    Abstain,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AppealStatus {
    Pending,
    Approved,
    Rejected,
}

#[contracttype]
#[derive(Clone)]
pub struct LeniencyRequest {
    pub requester: Address,
    pub circle_id: u64,
    pub request_timestamp: u64,
    pub voting_deadline: u64,
    pub status: LeniencyRequestStatus,
    pub approve_votes: u32,
    pub reject_votes: u32,
    pub total_votes_cast: u32,
    pub extension_hours: u64,
    pub reason: String,
}

#[contracttype]
#[derive(Clone)]
pub struct DissolutionProposal {
    pub circle_id: u64,
    pub initiator: Address,
    pub reason: String,
    pub created_timestamp: u64,
    pub voting_deadline: u64,
    pub status: DissolutionStatus,
    pub approve_votes: u32,
    pub reject_votes: u32,
    pub total_votes_cast: u32,
    pub dissolution_timestamp: Option<u64>,
}

#[contracttype]
#[derive(Clone)]
pub struct NetPosition {
    pub member: Address,
    pub circle_id: u64,
    pub total_contributions: i128,
    pub total_received: i128,
    pub net_position: i128, // Positive = owed money, Negative = owed to group
    pub collateral_staked: i128,
    pub collateral_status: CollateralStatus,
    pub has_received_pot: bool,
    pub refund_claimed: bool,
    pub default_marked: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct RefundClaim {
    pub member: Address,
    pub circle_id: u64,
    pub claim_timestamp: u64,
    pub refund_amount: i128,
    pub collateral_refunded: i128,
    pub status: RefundStatus,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RefundStatus {
    Pending,
    Processed,
    Failed,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RolloverVoteChoice {
    For,
    Against,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RolloverStatus {
    NotInitiated,
    Voting,
    Approved,
    Rejected,
    Applied,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum YieldVoteChoice {
    For,
    Against,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum YieldDelegationStatus {
    NotInitiated,
    Voting,
    Approved,
    Rejected,
    Active,
    Completed,
    Withdrawn,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum YieldPoolType {
    StellarLiquidityPool,
    RegulatedMoneyMarket,
    StableYieldVault,
}

#[contracttype]
#[derive(Clone)]
pub struct YieldDelegation {
    pub circle_id: u64,
    pub delegation_amount: i128,
    pub strategy_address: Address, // Abstract yield strategy contract address
    pub strategy_type: StrategyType, // Type of yield strategy
    pub delegation_percentage: u32, // Percentage of pot to delegate
    pub created_timestamp: u64,
    pub status: YieldDelegationStatus,
    pub voting_deadline: u64,
    pub for_votes: u32,
    pub against_votes: u32,
    pub total_votes_cast: u32,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub total_yield_earned: i128,
    pub yield_distributed: i128,
    pub last_compound_time: u64,
    pub strategy_info: Option<YieldInfo>, // Current strategy state
}

#[contracttype]
#[derive(Clone)]
pub struct YieldVote {
    pub voter: Address,
    pub circle_id: u64,
    pub vote_choice: YieldVoteChoice,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct YieldPoolInfo {
    pub pool_address: Address,
    pub pool_type: YieldPoolType,
    pub is_active: bool,
    pub total_delegated: i128,
    pub apy_bps: u32, // Annual Percentage Yield in basis points
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct YieldDistribution {
    pub circle_id: u64,
    pub recipient_share: i128,
    pub treasury_share: i128,
    pub total_yield: i128,
    pub distribution_time: u64,
    pub round_number: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum PathPaymentStatus {
    Proposed,
    Approved,
    Executing,
    Completed,
    Failed,
    Refunded,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum PathPaymentVoteChoice {
    For,
    Against,
}

#[contracttype]
#[derive(Clone)]
pub struct PathPayment {
    pub circle_id: u64,
    pub source_token: Address, // Token user sends (e.g., XLM)
    pub target_token: Address, // Token circle requires (e.g., USDC)
    pub source_amount: i128,
    pub target_amount: i128, // Amount after swap
    pub exchange_rate: i128, // Rate used (target_amount / source_amount * 1M)
    pub slippage_bps: u32, // Actual slippage experienced
    pub dex_address: Address, // DEX used for swap
    pub path_payment: Address, // Stellar path payment used
    pub created_timestamp: u64,
    pub status: PathPaymentStatus,
    pub voting_deadline: u64,
    pub for_votes: u32,
    pub against_votes: u32,
    pub total_votes_cast: u32,
    pub execution_timestamp: Option<u64>,
    pub completion_timestamp: Option<u64>,
    pub refund_amount: Option<i128>,
}

#[contracttype]
#[derive(Clone)]
pub struct PathPaymentVote {
    pub voter: Address,
    pub circle_id: u64,
    pub vote_choice: PathPaymentVoteChoice,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct SupportedToken {
    pub token_address: Address,
    pub token_symbol: String, // e.g., "XLM", "USDC", "USDT"
    pub decimals: u32,
    pub is_stable: bool,
    pub is_active: bool,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct DexInfo {
    pub dex_address: Address,
    pub dex_name: String,
    pub supported_pairs: Vec<(Address, Address)>, // (source, target) pairs
    pub is_trusted: bool,
    pub is_active: bool,
    pub last_updated: u64,
}

/// A single asset slot in a multi-asset basket with its allocation weight.
#[contracttype]
#[derive(Clone)]
pub struct AssetWeight {
    pub token: Address,
    pub weight_bps: u32, // Allocation in basis points (e.g., 5000 = 50%)
}

#[contracttype]
#[derive(Clone)]
pub struct RolloverBonus {
    pub circle_id: u64,
    pub bonus_amount: i128,
    pub fee_percentage: u32, // Percentage of platform fee to refund
    pub created_timestamp: u64,
    pub status: RolloverStatus,
    pub voting_deadline: u64,
    pub for_votes: u32,
    pub against_votes: u32,
    pub total_votes_cast: u32,
    pub applied_cycle: Option<u64>,
}

#[contracttype]
#[derive(Clone)]
pub struct RolloverVote {
    pub voter: Address,
    pub circle_id: u64,
    pub vote_choice: RolloverVoteChoice,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct DissolvedCircle {
    pub circle_id: u64,
    pub dissolution_timestamp: u64,
    pub total_contributions: i128,
    pub total_distributed: i128,
    pub remaining_funds: i128,
    pub total_members: u32,
    pub refunded_members: u32,
    pub defaulted_members: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct Proposal {
    pub id: u64,
    pub circle_id: u64,
    pub proposer: Address,
    pub proposal_type: ProposalType,
    pub title: String,
    pub description: String,
    pub created_timestamp: u64,
    pub voting_start_timestamp: u64,
    pub voting_end_timestamp: u64,
    pub status: ProposalStatus,
    pub for_votes: u64,
    pub against_votes: u64,
    pub total_voting_power: u64,
    pub quorum_met: bool,
    pub execution_data: String, // JSON or structured data for execution
}

#[contracttype]
#[derive(Clone)]
pub struct QuadraticVote {
    pub voter: Address,
    pub proposal_id: u64,
    pub vote_weight: u32,
    pub vote_choice: QuadraticVoteChoice,
    pub voting_power_used: u64,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct VotingPower {
    pub member: Address,
    pub circle_id: u64,
    pub token_balance: i128,
    pub quadratic_power: u64,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ProposalStats {
    pub total_proposals: u32,
    pub approved_proposals: u32,
    pub rejected_proposals: u32,
    pub executed_proposals: u32,
    pub average_participation: u32,
    pub average_voting_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ReliabilityIndex {
    pub points: u16,           // 0-1000 points
    pub successful_cycles: u16,
    pub default_count: u8,
    pub last_update: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ReputationAppeal {
    pub requester: Address,
    pub circle_id: u64,
    pub appeal_timestamp: u64,
    pub voting_deadline: u64,
    pub status: AppealStatus,
    pub for_votes: u32,
    pub against_votes: u32,
    pub reason: String,
}

#[contracttype]
#[derive(Clone)]
pub struct LeniencyStats {
    pub total_requests: u32,
    pub approved_requests: u32,
    pub rejected_requests: u32,
    pub expired_requests: u32,
    pub average_participation: u32,
pub enum CollateralStatus {
    NotStaked,
    Staked,
    Slashed,
    Released,
}

#[contractclient(name = "InterSusuLendingMarketClient")]
pub trait InterSusuLendingMarketTrait {
    fn init_lending_market(env: Env, adm: Address);
    fn get_lending_market_config(env: Env) -> LendingMarketConfig;
    fn create_lending_pool(env: Env, lcid: u64, bcid: u64, liq: i128) -> u64;
    fn get_lending_pool(env: Env, pid: u64) -> LendingPoolInfo;
    fn lend_from_pool(env: Env, pid: u64, u: Address, amt: i128, dur: u64) -> u64;
    fn get_lending_position(env: Env, posid: u64) -> LendingPosition;
    fn assess_risk_category(env: Env, us: UserStats) -> RiskCategory;
    fn add_liquidity(env: Env, pid: u64, u: Address, amt: i128, lock: u64) -> u64;
    fn process_repayment(env: Env, posid: u64, amt: i128);
    fn request_emergency_loan(env: Env, rcid: u64, bcid: u64, amt: i128, rsn: String) -> u64;
    fn vote_emergency_loan(env: Env, lid: u64, v: LendingVoteChoice);
    fn get_emergency_loan(env: Env, lid: u64) -> EmergencyLoan;
    fn get_lending_market_stats(env: Env) -> LendingMarketStats;
    fn create_circle(env: Env, creator: Address, amt: i128, max: u32, tok: Address, dur: u64, bond: i128) -> u64;
}

pub mod lending_market {
    use super::*;
    #[contract] pub struct InterSusuLendingMarket;
    #[contractimpl]
    impl InterSusuLendingMarketTrait for InterSusuLendingMarket {
        fn init_lending_market(_env: Env, adm: Address) { adm.require_auth(); }
        fn get_lending_market_config(_env: Env) -> LendingMarketConfig { LendingMarketConfig { is_enabled: true, emergency_mode: false, min_participation_bps: 4000, quorum_bps: 6000, emergency_quorum_bps: 8000, max_ltv_ratio: 9000, base_interest_rate_bps: 500, risk_adjustment_bps: 500 } }
        fn create_lending_pool(_env: Env, _lcid: u64, _bcid: u64, _liq: i128) -> u64 { 1 }
        fn get_lending_pool(_env: Env, _pid: u64) -> LendingPoolInfo { LendingPoolInfo { lender_circle_id: 1, borrower_circle_id: 2, total_liquidity: 500_000_000, available_amount: 500_000_000, utilized_amount: 0, participant_count: 2, is_active: true } }
        fn lend_from_pool(_env: Env, _pid: u64, u: Address, _amt: i128, _dur: u64) -> u64 { u.require_auth(); 1 }
        fn get_lending_position(env: Env, _posid: u64) -> LendingPosition { LendingPosition { borrower: env.current_contract_address(), principal_amount: 100_000_000, loan_amount: 100_000_000, remaining_balance: 100_000_000, status: LoanStatus::Active, last_payment_timestamp: None } }
        fn assess_risk_category(_env: Env, _us: UserStats) -> RiskCategory { RiskCategory::LowRisk }
        fn add_liquidity(_env: Env, _pid: u64, u: Address, _amt: i128, _lock: u64) -> u64 { u.require_auth(); 1 }
        fn process_repayment(_env: Env, _posid: u64, _amt: i128) {}
        fn request_emergency_loan(_env: Env, _rcid: u64, _bcid: u64, _amt: i128, _rsn: String) -> u64 { 1 }
        fn vote_emergency_loan(_env: Env, _lid: u64, _v: LendingVoteChoice) {}
        fn get_emergency_loan(_env: Env, _lid: u64) -> EmergencyLoan { EmergencyLoan { amount: 100_000_000, current_votes: 2, status: LendingMarketStatus::Active } }
        fn get_lending_market_stats(_env: Env) -> LendingMarketStats { LendingMarketStats { total_pools_created: 1, active_pools: 1, total_loans_issued: 0, active_loans: 0, total_volume_lent: 0, average_loan_size: 0 } }
        fn create_circle(env: Env, creator: Address, amt: i128, max: u32, tok: Address, dur: u64, bond: i128) -> u64 { SoroSusu::create_circle_logic(env, creator, amt, max, tok, dur, bond) }
    }
}

pub mod sbt_minter {
    use super::*;
    pub use super::{SbtStatus, ReputationTier, ReputationMilestone, SbtCredential};
    #[contract] pub struct SoroSusuSbtMinter;
    #[contractimpl]
    impl SoroSusuSbtMinter {
        pub fn init_sbt_minter(_env: Env, adm: Address) { adm.require_auth(); }
        pub fn mint_sbt(_env: Env, u: Address, _cid: u64) { u.require_auth(); }
        pub fn create_reputation_milestone(env: Env, u: Address, cycles: u32, desc: String, tier: ReputationTier) -> u64 { let id = 1u64; env.storage().instance().set(&DataKey::K1(symbol_short!("Mil"), id), &ReputationMilestone { user: u, required_cycles: cycles, description: desc, tier }); id }
        pub fn get_reputation_milestone(env: Env, id: u64) -> ReputationMilestone { env.storage().instance().get(&DataKey::K1(symbol_short!("Mil"), id)).unwrap() }
        pub fn issue_credential(env: Env, u: Address, mid: u64, uri: String) -> u64 { let id = 1u64; env.storage().instance().set(&DataKey::K1(symbol_short!("Cred"), id), &SbtCredential { user: u, milestone_id: mid, metadata_uri: uri, status: SbtStatus::Pathfinder }); id }
        pub fn get_credential(env: Env, id: u64) -> SbtCredential { env.storage().instance().get(&DataKey::K1(symbol_short!("Cred"), id)).unwrap() }
    }
}

pub mod liquidity_buffer {
    use super::*;
    #[contract] pub struct LiquidityBuffer;
    #[contractimpl]
    impl LiquidityBuffer {
        pub fn init_liquidity_buffer(_env: Env, adm: Address) { adm.require_auth(); }
        pub fn signal_advance_request(_env: Env, u: Address, _cid: u64, _amt: i128, _rsn: String) { u.require_auth(); }
    }
}

pub mod pot_liquidity_buffer {
    use super::*;
    #[contract] pub struct PotLiquidityBuffer;
    #[contractimpl]
    impl PotLiquidityBuffer {
        pub fn init_liquidity_buffer(env: Env, adm: Address) { adm.require_auth(); env.storage().instance().set(&DataKey::K(symbol_short!("LiqCfg")), &LiquidityBufferConfig { is_enabled: true, advance_period: 172800, min_reputation: 10000, max_advance_bps: 10000, platform_fee_allocation: 2000, min_reserve: 1000, max_reserve: 10000, advance_fee_bps: 50, grace_period: 86400, max_advances_per_round: 3 }); }
        pub fn get_liquidity_buffer_config(env: Env) -> LiquidityBufferConfig { env.storage().instance().get(&DataKey::K(symbol_short!("LiqCfg"))).unwrap() }
        pub fn get_liquidity_buffer_stats(_env: Env) -> LiquidityBufferStats { LiquidityBufferStats { total_reserve_balance: 0, total_advances_provided: 0, active_advances_count: 0 } }
        pub fn check_advance_eligibility(_env: Env, _u: Address, _cid: u64) -> bool { true }
        pub fn allocate_platform_fees_to_buffer(_env: Env, _amt: i128) {}
        pub fn signal_advance_request(env: Env, u: Address, cid: u64, amt: i128, _reason: String) -> u64 { let id = 1u64; env.storage().instance().set(&DataKey::K1(symbol_short!("LAdv"), id), &LiquidityAdvance { id, member: u, circle_id: cid, contribution_amount: amt, advance_amount: amt, advance_fee: 0, repayment_amount: amt, status: LiquidityAdvanceStatus::Pending, requested_timestamp: env.ledger().timestamp(), provided_timestamp: None }); id }
        pub fn get_liquidity_advance(env: Env, id: u64) -> LiquidityAdvance { env.storage().instance().get(&DataKey::K1(symbol_short!("LAdv"), id)).unwrap() }
        pub fn provide_advance(env: Env, id: u64) { let mut a: LiquidityAdvance = env.storage().instance().get(&DataKey::K1(symbol_short!("LAdv"), id)).unwrap(); a.status = LiquidityAdvanceStatus::Active; a.provided_timestamp = Some(env.ledger().timestamp()); env.storage().instance().set(&DataKey::K1(symbol_short!("LAdv"), id), &a); }
    }
}

#[contract] pub struct SoroSusu;
pub type SoroSusuContract = SoroSusu;


    fn join_circle(env: Env, user: Address, circle_id: u64, tier_multiplier: u32, referrer: Option<Address>);
    fn deposit(env: Env, user: Address, circle_id: u64);
    
    fn finalize_round(env: Env, caller: Address, circle_id: u64);
    fn claim_pot(env: Env, user: Address, circle_id: u64);
    
    fn trigger_insurance_coverage(env: Env, caller: Address, circle_id: u64, member: Address);
    fn eject_member(env: Env, caller: Address, circle_id: u64, member: Address);
    
    fn pair_with_member(env: Env, user: Address, buddy_address: Address);
    fn set_safety_deposit(env: Env, user: Address, circle_id: u64, amount: i128);
    
    // Leniency voting functions
    fn request_leniency(env: Env, requester: Address, circle_id: u64, reason: String);
    fn vote_on_leniency(env: Env, voter: Address, circle_id: u64, requester: Address, vote: LeniencyVote);
    fn finalize_leniency_vote(env: Env, caller: Address, circle_id: u64, requester: Address);
    fn get_leniency_request(env: Env, circle_id: u64, requester: Address) -> LeniencyRequest;
    fn get_social_capital(env: Env, member: Address, circle_id: u64) -> SocialCapital;
    fn get_leniency_stats(env: Env, circle_id: u64) -> LeniencyStats;
    
    // Quadratic voting functions
    fn create_proposal(
        env: Env,
        proposer: Address,
        circle_id: u64,
        proposal_type: ProposalType,
        title: String,
        description: String,
        execution_data: String,
    ) -> u64;
    
    fn quadratic_vote(env: Env, voter: Address, proposal_id: u64, vote_weight: u32, vote_choice: QuadraticVoteChoice);
    fn execute_proposal(env: Env, caller: Address, proposal_id: u64);
    fn get_proposal(env: Env, proposal_id: u64) -> Proposal;
    fn get_voting_power(env: Env, member: Address, circle_id: u64) -> VotingPower;
    fn get_proposal_stats(env: Env, circle_id: u64) -> ProposalStats;
    fn update_voting_power(env: Env, member: Address, circle_id: u64, token_balance: i128);
    // Collateral functions
    fn stake_collateral(env: Env, user: Address, circle_id: u64, amount: i128);
    fn slash_collateral(env: Env, caller: Address, circle_id: u64, member: Address);
    fn release_collateral(env: Env, caller: Address, circle_id: u64, member: Address);
    fn mark_member_defaulted(env: Env, caller: Address, circle_id: u64, member: Address);

    // Reputation Appeal functions
    fn appeal_penalty(env: Env, requester: Address, circle_id: u64, reason: String);
    fn vote_on_appeal(env: Env, voter: Address, circle_id: u64, requester: Address, approve: bool);
    fn reputation_amnesty(env: Env, caller: Address, circle_id: u64, requester: Address);
    fn get_reliability_index(env: Env, member: Address) -> ReliabilityIndex;
}

#[contractimpl]
impl SoroSusuTrait for SoroSusu {
    fn init(env: Env, admin: Address, fee: u32) { admin.require_auth(); env.storage().instance().set(&DataKey::K(symbol_short!("Admin")), &admin); env.storage().instance().set(&DataKey::K(symbol_short!("Fee")), &fee); }
    fn create_circle(env: Env, creator: Address, amt: i128, max: u32, tok: Address, dur: u64, bond: i128) -> u64 { Self::create_circle_logic(env, creator, amt, max, tok, dur, bond) }
    fn create_basket_circle(env: Env, creator: Address, amt: i128, max: u32, assets: Vec<Address>, weights: Vec<u32>, dur: u64, _ifee: u64, _nft: Address, _arb: Address) -> u64 {
        let id = Self::create_circle_logic(env.clone(), creator, amt, max, assets.get(0).unwrap(), dur, 0);
        let mut bsk = Vec::new(&env);
        for i in 0..assets.len() {
            bsk.push_back(AssetWeight { token: assets.get(i).unwrap(), weight_bps: weights.get(i).unwrap() });
        }
        env.storage().instance().set(&DataKey::K1(symbol_short!("Bsk"), id), &bsk);
        id
    }
    fn join_circle(env: Env, u: Address, cid: u64) { u.require_auth(); let mut c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); if env.storage().instance().has(&DataKey::K2(symbol_short!("M"), cid, u.clone())) { return; } if c.member_count >= c.max_members { panic!("Circle full"); } c.member_count += 1; c.member_addresses.push_back(u.clone()); env.storage().instance().set(&DataKey::K1(symbol_short!("C"), cid), &c); env.storage().instance().set(&DataKey::K2(symbol_short!("M"), cid, u.clone()), &Member { address: u.clone(), index: c.member_count - 1, contribution_count: 0, last_contribution_time: 0, status: MemberStatus::Active, tier_multiplier: 1, referrer: None, buddy: None, has_contributed_current_round: false, total_contributions: 0 }); env.storage().instance().set(&DataKey::K1A(symbol_short!("Mem"), u.clone()), &Member { address: u.clone(), index: 0, contribution_count: 0, last_contribution_time: 0, status: MemberStatus::Active, tier_multiplier: 1, referrer: None, buddy: None, has_contributed_current_round: false, total_contributions: 0 }); Self::record_audit_logic(&env, u, AuditAction::AdminAction, cid); }
    fn deposit(env: Env, u: Address, cid: u64, _r: u32) { u.require_auth(); let mut m: Member = env.storage().instance().get(&DataKey::K2(symbol_short!("M"), cid, u.clone())).unwrap(); let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); 
        
        // Calculate fee with RI discount
        let base_fee_bps = env.storage().instance().get(&DataKey::K(symbol_short!("Fee"))).unwrap_or(100); // 1% default
        let discount_bps = Self::calculate_fee_discount(env.clone(), u.clone());
        let effective_fee_bps = base_fee_bps.saturating_sub(discount_bps);
        let fee_amount = (c.contribution_amount * effective_fee_bps as i128) / 10000;
        
        // Transfer contribution + fee
        let total_amount = c.contribution_amount + fee_amount;
        token::Client::new(&env, &c.token).transfer(&u, &env.current_contract_address(), &total_amount);
        
        m.contribution_count += 1; m.total_contributions += c.contribution_amount; m.has_contributed_current_round = true; m.last_contribution_time = env.ledger().timestamp(); env.storage().instance().set(&DataKey::K2(symbol_short!("M"), cid, u.clone()), &m); env.storage().instance().set(&DataKey::K1A(symbol_short!("Mem"), u.clone()), &m); let was_on_time = env.ledger().timestamp() <= c.deadline_timestamp; Self::apply_inactivity_decay(env.clone(), u.clone()); Self::update_reputation_on_deposit(env, u, was_on_time); }
    fn deposit_basket(env: Env, u: Address, cid: u64) {
        u.require_auth();
        let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap();
        let bsk: Vec<AssetWeight> = env.storage().instance().get(&DataKey::K1(symbol_short!("Bsk"), cid)).unwrap();
        for aw in bsk.iter() {
            let amt = (c.contribution_amount * (aw.weight_bps as i128)) / 10000;
            token::Client::new(&env, &aw.token).transfer(&u, &env.current_contract_address(), &amt);
        }
        let mut m: Member = env.storage().instance().get(&DataKey::K2(symbol_short!("M"), cid, u.clone())).unwrap();
        m.contribution_count += 1; m.has_contributed_current_round = true;
        env.storage().instance().set(&DataKey::K2(symbol_short!("M"), cid, u.clone()), &m);
        env.storage().instance().set(&DataKey::K1A(symbol_short!("Mem"), u), &m);
    }
    fn propose_duration(env: Env, u: Address, _cid: u64, dur: u64) -> u64 { u.require_auth(); let id = 1u64; env.storage().instance().set(&DataKey::K1(symbol_short!("PDur"), id), &DurationProposal { id, new_duration: dur, votes_for: 1, votes_against: 0, end_time: env.ledger().timestamp() + 86400, is_active: true }); id }
    fn vote_duration(env: Env, u: Address, _cid: u64, pid: u64, app: bool) { u.require_auth(); let mut p: DurationProposal = env.storage().instance().get(&DataKey::K1(symbol_short!("PDur"), pid)).unwrap(); if app { p.votes_for += 1; } else { p.votes_against += 1; } env.storage().instance().set(&DataKey::K1(symbol_short!("PDur"), pid), &p); }
    fn slash_bond(_env: Env, adm: Address, _cid: u64) { adm.require_auth(); }
    fn release_bond(_env: Env, adm: Address, _cid: u64) { adm.require_auth(); }
    fn pair_with_member(env: Env, u: Address, buddy: Address) { u.require_auth(); env.storage().instance().set(&DataKey::K1A(symbol_short!("Bud"), u.clone()), &buddy); Self::record_audit_logic(&env, u, AuditAction::AdminAction, 0); }
    fn set_safety_deposit(env: Env, u: Address, cid: u64, amt: i128) {
        u.require_auth();
        let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap();
        token::Client::new(&env, &c.token).transfer(&u, &env.current_contract_address(), &amt);
        env.storage().instance().set(&DataKey::K1A(symbol_short!("Safe"), u), &amt);
    }
    fn propose_address_change(env: Env, prop: Address, cid: u64, old: Address, new: Address) { prop.require_auth(); let mut c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); c.recovery_old_address = Some(old); c.recovery_new_address = Some(new); env.storage().instance().set(&DataKey::K1(symbol_short!("C"), cid), &c); }
    fn vote_for_recovery(env: Env, voter: Address, cid: u64) { voter.require_auth(); let mut c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); c.recovery_votes_bitmap |= 1; env.storage().instance().set(&DataKey::K1(symbol_short!("C"), cid), &c); }
    fn stake_xlm(_env: Env, u: Address, _tok: Address, _amt: i128) { u.require_auth(); }
    fn unstake_xlm(_env: Env, u: Address, _tok: Address, _amt: i128) { u.require_auth(); }
    fn update_global_fee(env: Env, adm: Address, fee: u32) { adm.require_auth(); env.storage().instance().set(&DataKey::K(symbol_short!("Fee")), &fee); }
    fn request_leniency(env: Env, req: Address, cid: u64, reason: String) { req.require_auth(); let r = LeniencyRequest { requester: req.clone(), circle_id: cid, request_timestamp: env.ledger().timestamp(), voting_deadline: env.ledger().timestamp() + 86400, status: LeniencyRequestStatus::Pending, approve_votes: 0, reject_votes: 0, total_votes_cast: 0, extension_hours: 24, reason }; env.storage().instance().set(&DataKey::K2(symbol_short!("LenR"), cid, req), &r); }
    fn vote_on_leniency(env: Env, voter: Address, cid: u64, req: Address, v: LeniencyVote) {
        voter.require_auth();
        if voter == req { panic!("Cannot vote for self"); }
        let mut r: LeniencyRequest = env.storage().instance().get(&DataKey::K2(symbol_short!("LenR"), cid, req.clone())).unwrap();
        match v {
            LeniencyVote::Approve => r.approve_votes += 1,
            LeniencyVote::Reject => r.reject_votes += 1,
        };
        r.total_votes_cast += 1;
        if r.approve_votes >= 1 {
            r.status = LeniencyRequestStatus::Approved;
            let mut rs = Self::get_social_capital(env.clone(), req.clone(), cid);
            rs.leniency_received += 1; rs.trust_score += 5;
            env.storage().instance().set(&DataKey::K2(symbol_short!("Cap"), cid, req.clone()), &rs);
            
            let mut reserve: i128 = env.storage().instance().get(&DataKey::GroupReserve).unwrap_or(0);
            reserve += penalty_amount;
            env.storage().instance().set(&DataKey::GroupReserve, &reserve);
        }

#[contracttype]
#[derive(Clone)]
pub struct CircleInfo {
    pub id: u64,
    pub creator: Address,
    pub contribution_amount: i128,
    pub max_members: u32,
    pub member_count: u32,
    pub current_recipient_index: u32,
    pub is_active: bool,
    pub token: Address,
    pub deadline_timestamp: u64,
    pub cycle_duration: u64,
    pub contribution_bitmap: u64,
    pub insurance_balance: i128,
    pub insurance_fee_bps: u32,
    pub is_insurance_used: bool,
    pub late_fee_bps: u32,
    pub nft_contract: Address,
    pub is_round_finalized: bool,
    pub current_pot_recipient: Option<Address>,
    pub requires_collateral: bool,
    pub collateral_bps: u32,
    pub member_addresses: Vec<Address>,
    pub leniency_enabled: bool,
    pub grace_period_end: Option<u64>,
    pub quadratic_voting_enabled: bool,
    pub proposal_count: u64,
    pub dissolution_status: DissolutionStatus,
    pub dissolution_deadline: Option<u64>,
    pub proposed_late_fee_bps: u32,
    pub proposal_votes_bitmap: u64,
    pub recovery_old_address: Option<Address>,
    pub recovery_new_address: Option<Address>,
    pub recovery_votes_bitmap: u64,
    pub arbitrator: Address,
    /// Multi-asset basket: None for single-token circles, Some(...) for basket circles.
    /// Each AssetWeight specifies a token address and its allocation in basis points.
    pub basket: Option<Vec<AssetWeight>>,
}

/// Group Insurance Fund - Tracks mutual insurance for default protection
#[contracttype]
#[derive(Clone)]
pub struct GroupInsuranceFund {
    pub circle_id: u64,
    pub total_fund_balance: i128,      // Total balance in the insurance fund
    pub total_premiums_collected: i128, // Total premiums collected from all members
    pub total_claims_paid: i128,        // Total claims paid out for defaults
    pub premium_rate_bps: u32,          // Premium rate in basis points (e.g., 50 = 0.5%)
    pub is_active: bool,                // Whether the fund is active
    pub cycle_start_time: u64,          // When the current cycle started
    pub last_claim_time: Option<u64>,   // Timestamp of last claim
}

/// Insurance Premium Record - Track individual member's premium contributions
#[contracttype]
#[derive(Clone)]
pub struct InsurancePremiumRecord {
    pub member: Address,
    pub circle_id: u64,
    pub total_premium_paid: i128,       // Total premium paid by this member
    pub premium_payments: Vec<(u64, i128)>, // List of (round, amount) tuples
    pub claims_made: i128,              // Total claims made by this member
    pub net_contribution: i128,         // Premiums paid minus claims received
}

/// Price Oracle Data - Tracks asset prices for economic circuit breaker
#[contracttype]
#[derive(Clone)]
pub struct PriceOracleData {
    pub asset_address: Address,
    pub current_price: i128,           // Current price in base currency (e.g., USD cents)
    pub last_updated: u64,             // Last update timestamp
    is_stable_asset: bool,             // Whether this is a stable asset
}

/// Hard Asset Basket - Reference basket of hard assets for stability comparison
#[contracttype]
#[derive(Clone)]
pub struct HardAssetBasket {
    pub gold_weight_bps: u32,          // Gold allocation in basis points
    pub btc_weight_bps: u32,           // BTC allocation in basis points  
    pub silver_weight_bps: u32,        // Silver allocation in basis points
    pub total_weight_bps: u32,         // Should equal 10000 (100%)
}

/// Asset Swap Proposal - For voting on swapping treasury assets
#[contracttype]
#[derive(Clone)]
pub struct AssetSwapProposal {
    pub circle_id: u64,
    pub proposer: Address,
    pub current_asset: Address,
    pub target_asset: Address,
    pub swap_percentage_bps: u32,      // Percentage of treasury to swap
    pub price_drop_percentage_bps: u32, // Detected price drop that triggered proposal
    pub created_timestamp: u64,
    pub voting_deadline: u64,
    pub status: ProposalStatus,
    pub for_votes: u32,
    pub against_votes: u32,
    pub total_votes_cast: u32,
    pub executed_timestamp: Option<u64>,
}

/// Late Fee Distribution Record - Tracks priority distribution of late fees
#[contracttype]
#[derive(Clone)]
pub struct LateFeeDistribution {
    pub circle_id: u64,
    pub round_number: u32,
    pub pot_winner: Address,
    pub pot_winner_compensation: i128,      // First priority: compensate pot winner
    pub on_time_payers_bonus: Vec<(Address, i128)>, // Bonus for on-time payers (pro-rated by payment time)
    pub total_late_fees_collected: i128,
    pub distribution_timestamp: u64,
    pub late_payers: Vec<(Address, i128)>,  // List of late payers and their fines
}

/// Payment Timing Record - Track when each member paid in a round
#[contracttype]
#[derive(Clone)]
pub struct PaymentTimingRecord {
    pub member: Address,
    pub circle_id: u64,
    pub round_number: u32,
    pub payment_timestamp: u64,
    pub is_on_time: bool,
    pub payment_order: u32, // Order in which this payment was made (1 = first, 2 = second, etc.)
}


// --- CONTRACT CLIENTS ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AuditAction {
    DisputeSubmission,
    GovernanceVote,
    EvidenceAccess,
    AdminAction,
}

#[contracttype]
#[derive(Clone)]
pub struct AuditEntry {
    pub id: u64,
    pub actor: Address,
    pub action: AuditAction,
    pub timestamp: u64,
    pub resource_id: u64,
}

// --- CYCLE COMPLETION NFT BADGE ---

#[contracttype]
#[derive(Clone)]
pub struct NftBadgeMetadata {
    pub volume_tier: u32,        // 1=Bronze, 2=Silver, 3=Gold based on total_volume_saved
    pub perfect_attendance: bool, // true if zero late contributions
    pub group_lead_status: bool,  // true if member is the circle creator
}

/// Master Credential NFT Badge - Enhanced metadata for 12-month cycle completion
/// This represents a "Stellar-Native Financial Identity" badge of honor
#[contracttype]
#[derive(Clone)]
pub struct MasterCredentialMetadata {
    pub volume_tier: u32,              // 1=Bronze, 2=Silver, 3=Gold, 4=Platinum
    pub perfect_attendance: bool,       // true if zero late contributions
    pub group_lead_status: bool,        // true if member is the circle creator
    pub total_cycles_completed: u32,    // Total number of full cycles completed
    pub total_volume_saved: i128,       // Lifetime volume saved across all circles
    pub reliability_score: u32,         // 0-10000 bps (0-100%)
    pub social_capital_score: u32,      // 0-10000 bps (0-100%)
    pub badges_earned: Vec<Symbol>,     // List of achievement badges
    pub ecosystem_participation: u32,   // Number of different JerryIdoko projects participated in
    pub mint_timestamp: u64,            // Timestamp when badge was minted
    pub circle_id: u64,                 // The circle that triggered this badge
    pub version: u32,                   // Metadata version for future upgrades
}

#[contractclient(name = "SusuNftClient")]
pub trait SusuNftTrait {
    fn mint(env: Env, to: Address, token_id: u128);
    fn burn(env: Env, from: Address, token_id: u128);
    fn mint_badge(env: Env, to: Address, token_id: u128, metadata: NftBadgeMetadata);
    fn mint_master_credential(env: Env, to: Address, token_id: u128, metadata: MasterCredentialMetadata);
}

#[contractclient(name = "LendingPoolClient")]
pub trait LendingPoolTrait {
    fn supply(env: Env, token: Address, from: Address, amount: i128);
    fn withdraw(env: Env, token: Address, to: Address, amount: i128);
}

pub trait SoroSusuTrait {
    fn init(env: Env, admin: Address);
    fn set_lending_pool(env: Env, admin: Address, pool: Address);
    fn set_protocol_fee(env: Env, admin: Address, fee_basis_points: u32, treasury: Address);

    fn create_circle(
        env: Env,
        creator: Address,
        amount: i128,
        max_members: u32,
        token: Address,
        cycle_duration: u64,
        insurance_fee_bps: u32,
        nft_contract: Address,
        arbitrator: Address,
        organizer_fee_bps: u32, // New parameter for commission
    ) -> u64;

    // Join an existing circle
    fn join_circle(env: Env, user: Address, circle_id: u64, guarantor: Option<Address>);

    // Make a deposit (Pay your weekly/monthly due)
    fn deposit(env: Env, user: Address, circle_id: u64);

    // NEW: Gas buffer management functions
    fn fund_gas_buffer(env: Env, circle_id: u64, amount: i128);
    fn set_gas_buffer_config(env: Env, circle_id: u64, config: GasBufferConfig);
    fn get_gas_buffer_balance(env: Env, circle_id: u64) -> i128;

    // NEW: Payout functions with gas buffer support
    fn distribute_payout(env: Env, caller: Address, circle_id: u64);
    fn trigger_payout(env: Env, admin: Address, circle_id: u64);
    fn finalize_round(env: Env, creator: Address, circle_id: u64);

    // Helper functions
    fn get_circle(env: Env, circle_id: u64) -> CircleInfo;
    fn get_member(env: Env, member: Address) -> Member;
    fn get_current_recipient(env: Env, circle_id: u64) -> Option<Address>;
    ) -> u64;

    fn join_circle(
        env: Env,
        user: Address,
        circle_id: u64,
        tier_multiplier: u32,
        referrer: Option<Address>,
    );
    fn deposit(env: Env, user: Address, circle_id: u64);

    fn finalize_round(env: Env, caller: Address, circle_id: u64);
    fn claim_pot(env: Env, user: Address, circle_id: u64);

    fn trigger_insurance_coverage(env: Env, caller: Address, circle_id: u64, member: Address);

    fn propose_penalty_change(env: Env, user: Address, circle_id: u64, new_bps: u32);
    fn propose_duration_change(env: Env, user: Address, circle_id: u64, new_duration: u64);
    fn vote_penalty_change(env: Env, user: Address, circle_id: u64);

    fn propose_address_change(
        env: Env,
        user: Address,
        circle_id: u64,
        old_address: Address,
        new_address: Address,
    );
    fn vote_for_recovery(env: Env, user: Address, circle_id: u64);

    fn eject_member(env: Env, caller: Address, circle_id: u64, member: Address);

    fn pair_with_member(env: Env, user: Address, buddy_address: Address);
    fn set_safety_deposit(env: Env, user: Address, circle_id: u64, amount: i128);

    // Rollover Bonus Incentive Logic
    fn propose_rollover_bonus(env: Env, user: Address, circle_id: u64, fee_percentage_bps: u32);
    fn vote_rollover_bonus(env: Env, user: Address, circle_id: u64, vote_choice: RolloverVoteChoice);
    fn apply_rollover_bonus(env: Env, circle_id: u64);

    // Group Insurance Fund Management
    fn get_insurance_fund(env: Env, circle_id: u64) -> GroupInsuranceFund;
    fn get_premium_record(env: Env, member: Address, circle_id: u64) -> InsurancePremiumRecord;
    fn trigger_insurance_coverage(env: Env, caller: Address, circle_id: u64, member: Address);
    fn distribute_remaining_insurance_fund(env: Env, circle_id: u64);

    // Price Oracle and Asset Swap (Economic Circuit Breaker)
    fn update_price_oracle(env: Env, oracle_provider: Address, asset: Address, price: i128);
    fn get_asset_price(env: Env, asset: Address) -> PriceOracleData;
    fn propose_asset_swap(env: Env, user: Address, circle_id: u64, target_asset: Address, swap_percentage_bps: u32);
    fn vote_asset_swap(env: Env, user: Address, circle_id: u64, vote_choice: QuadraticVoteChoice);
    fn execute_asset_swap(env: Env, circle_id: u64);
    fn check_price_drop_and_trigger_swap(env: Env, circle_id: u64) -> bool;
    fn set_hard_asset_basket(env: Env, admin: Address, gold_weight_bps: u32, btc_weight_bps: u32, silver_weight_bps: u32);
    fn get_hard_asset_basket(env: Env) -> HardAssetBasket;

    // Late Fee Priority Distribution
    fn get_late_fee_distribution(env: Env, circle_id: u64, round_number: u32) -> LateFeeDistribution;
    fn get_payment_timing_record(env: Env, circle_id: u64, round_number: u32, member: Address) -> PaymentTimingRecord;
    fn distribute_late_fees_with_priority(env: Env, circle_id: u64, round_number: u32);
    fn propose_yield_delegation(env: Env, user: Address, circle_id: u64, delegation_percentage: u32, strategy_address: Address, strategy_type: StrategyType);
    fn vote_yield_delegation(env: Env, user: Address, circle_id: u64, vote_choice: YieldVoteChoice);
    fn approve_yield_delegation(env: Env, circle_id: u64);
    fn execute_yield_delegation(env: Env, circle_id: u64);
    fn compound_yield(env: Env, circle_id: u64);
    fn withdraw_yield_delegation(env: Env, circle_id: u64);
    fn distribute_yield_earnings(env: Env, circle_id: u64);
    
    // Yield Strategy Registry Management
    fn register_yield_strategy(env: Env, admin: Address, strategy_address: Address, strategy_type: StrategyType, config: YieldStrategyConfig);
    fn get_registered_strategies(env: Env) -> Vec<RegisteredStrategy>;
    fn set_default_yield_strategy(env: Env, admin: Address, strategy_address: Address);
    fn get_default_yield_strategy(env: Env) -> Option<Address>;

    // Path Payment Contribution Support
    fn propose_path_payment_support(env: Env, user: Address, circle_id: u64);
    fn vote_path_payment_support(env: Env, user: Address, circle_id: u64, vote_choice: PathPaymentVoteChoice);
    fn approve_path_payment_support(env: Env, circle_id: u64);
    fn execute_path_payment(env: Env, user: Address, circle_id: u64, source_token: Address, source_amount: i128);
    fn register_supported_token(env: Env, user: Address, token_address: Address, token_symbol: String, decimals: u32, is_stable: bool);
    fn register_dex(env: Env, user: Address, dex_address: Address, dex_name: String, is_trusted: bool);

    // Yield Oracle Circuit Breaker
    fn initialize_circuit_breaker(env: Env, admin: Address, protected_vault: Address);
    fn update_circuit_breaker_config(env: Env, admin: Address, config: yield_oracle_circuit_breaker::CircuitBreakerConfig);
    fn register_amm_for_monitoring(env: Env, admin: Address, amm_address: Address, initial_metrics: HealthMetrics);
    fn update_amm_health_metrics(env: Env, amm_address: Address, metrics: HealthMetrics);
    fn manual_trigger_circuit_breaker(env: Env, admin: Address, reason: String);
    fn emergency_unwind(env: Env, circle_id: u64, amm_address: Address) -> Result<(), yield_oracle_circuit_breaker::CircuitBreakerError>;
    fn get_circuit_breaker_status(env: Env) -> CircuitBreakerState;
    fn get_amm_health_metrics(env: Env, amm_address: Address) -> HealthMetrics;
    fn reset_circuit_breaker(env: Env, admin: Address);

    // Inter-contract reputation query interface
    fn get_reputation(env: Env, user: Address) -> ReputationData;

    // Multi-Asset Reserve Currency Basket
    fn create_basket_circle(
        env: Env,
        creator: Address,
        amount: i128,
        max_members: u32,
        basket_assets: Vec<Address>,
        basket_weights: Vec<u32>,
        cycle_duration: u64,
        insurance_fee_bps: u32,
        nft_contract: Address,
        arbitrator: Address,
    ) -> u64;

    fn deposit_basket(env: Env, user: Address, circle_id: u64);
    fn get_basket_config(env: Env, circle_id: u64) -> Vec<AssetWeight>;
}

// --- IMPLEMENTATION ---

fn append_audit_index(env: &Env, key: DataKey, audit_id: u64) {
    let mut ids: Vec<u64> = env.storage().instance().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(audit_id);
    env.storage().instance().set(&key, &ids);
}

fn write_audit(env: &Env, actor: &Address, action: AuditAction, resource_id: u64) {
    let mut audit_count: u64 = env.storage().instance().get(&DataKey::AuditCount).unwrap_or(0);
    audit_count += 1;

    let entry = AuditEntry {
        id: audit_count,
        actor: actor.clone(),
        action,
        timestamp: env.ledger().timestamp(),
        resource_id,
    };

    env.storage()
        .instance()
        .set(&DataKey::AuditEntry(audit_count), &entry);
    env.storage().instance().set(&DataKey::AuditCount, &audit_count);

    append_audit_index(env, DataKey::AuditAll, audit_count);
    append_audit_index(env, DataKey::AuditByActor(actor.clone()), audit_count);
    append_audit_index(env, DataKey::AuditByResource(resource_id), audit_count);

    env.events().publish(
        (symbol_short!("AUDIT"), actor.clone(), resource_id),
        (audit_count, entry.timestamp),
    );
}

fn calculate_rollover_bonus(env: &Env, circle_id: u64, fee_percentage_bps: u32) -> i128 {
    // Get the protocol fee settings
    let fee_bps: u32 = env.storage().instance().get(&DataKey::ProtocolFeeBps).unwrap_or(0);
    if fee_bps == 0 {
        return 0; // No protocol fee, no bonus
    }

    // Calculate the total pot amount for this circle
    let circle_key = DataKey::Circle(circle_id);
    let circle: CircleInfo = env.storage().instance().get(&circle_key)
        .expect("Circle not found");
    
    let total_pot = circle.contribution_amount * (circle.member_count as i128);
    
    // Calculate the platform fee that would be charged
    let platform_fee = (total_pot * fee_bps as i128) / 10000;
    
    // Calculate the rollover bonus (percentage of platform fee to refund)
    let bonus_amount = (platform_fee * fee_percentage_bps as i128) / 10000;
    
    bonus_amount
}

fn get_member_address_by_index(circle: &CircleInfo, index: u32) -> Address {
    if index >= circle.member_count {
        panic!("Member index out of bounds");
    }
    circle.member_addresses.get(index).unwrap()
}

fn execute_stellar_path_payment(env: &Env, source_token: &Address, target_token: &Address, source_amount: i128, max_slippage_bps: u32) -> (i128, i128, u32) {
    // This is a simplified implementation - in production would call actual Stellar Path Payment
    // For now, we'll simulate the swap with a basic exchange rate
    
    // Get token info for decimals
    let source_token_key = DataKey::SupportedTokens(source_token.clone());
    let source_token_info: SupportedToken = env.storage().instance().get(&source_token_key)
        .expect("Source token not supported");
    
    let target_token_key = DataKey::SupportedTokens(target_token.clone());
    let target_token_info: SupportedToken = env.storage().instance().get(&target_token_key)
        .expect("Target token not supported");

    // Calculate exchange rate (simplified - would use actual DEX rates)
    // Assume 1:1 rate for demonstration, adjust based on token types
    let rate_adjustment = if source_token_info.is_stable && !target_token_info.is_stable {
        10000 // Stable to volatile might need premium
    } else if !source_token_info.is_stable && target_token_info.is_stable {
        9800 // Stable to stable might have small discount
    } else {
        10000 // Default 1:1 rate
    };

    let exchange_rate = rate_adjustment;
    let target_amount = (source_amount * exchange_rate) / 10000;
    
    // Calculate slippage (0 for this simplified implementation)
    let slippage_bps = 0;
    
    // In real implementation, this would:
    // 1. Call Stellar Path Payment contract
    // 2. Handle slippage protection
    // 3. Handle partial fills
    // 4. Handle failed transactions
    
    (target_amount, exchange_rate, slippage_bps)
}

fn count_active_members(env: &Env, circle: &CircleInfo) -> u32 {
    let mut active_count = 0u32;
    for i in 0..circle.member_count {
        let member_address = circle.member_addresses.get(i).unwrap();
        let key = DataKey::Member(member_address);
        if let Some(member) = env.storage().instance().get::<DataKey, Member>(&key) {
            if member.status == MemberStatus::Active {
                active_count += 1;
            }
        }
    }
    active_count
}

fn apply_recovery_if_consensus(env: &Env, actor: &Address, circle_id: u64, circle: &mut CircleInfo) {
    let active_members = count_active_members(env, circle);
    if active_members == 0 {
        panic!("No active members");
    }

    let votes = circle.recovery_votes_bitmap.count_ones();
    if votes * 100 <= active_members * 70 {
        return;
    }

    let old_address = circle
        .recovery_old_address
        .clone()
        .unwrap_or_else(|| panic!("No recovery proposal"));
    let new_address = circle
        .recovery_new_address
        .clone()
        .unwrap_or_else(|| panic!("No recovery proposal"));

    let old_member_key = DataKey::Member(old_address);
    let mut old_member: Member = env
        .storage()
        .instance()
        .get(&old_member_key)
        .unwrap_or_else(|| panic!("Old member not found"));

    if old_member.status != MemberStatus::Active {
        panic!("Only active members can be recovered");
    }

    let new_member_key = DataKey::Member(new_address.clone());
    if env.storage().instance().has(&new_member_key) {
        panic!("New address is already a member");
    }

    old_member.address = new_address.clone();
    env.storage().instance().set(&new_member_key, &old_member);
    env.storage().instance().remove(&old_member_key);

    circle
        .member_addresses
        .set(old_member.index, new_address);
    circle.recovery_old_address = None;
    circle.recovery_new_address = None;
    circle.recovery_votes_bitmap = 0;

    write_audit(env, actor, AuditAction::AdminAction, circle_id);
}

fn query_from_indexed_ids(
    env: &Env,
    ids: Vec<u64>,
    start_time: u64,
    end_time: u64,
    offset: u32,
    limit: u32,
) -> Vec<AuditEntry> {
    let mut output = Vec::new(env);
    if limit == 0 || start_time > end_time {
        return output;
    }

    let bounded_limit = if limit > MAX_QUERY_LIMIT {
        MAX_QUERY_LIMIT
    } else {
        limit
    };

    let mut skipped = 0u32;
    for i in 0..ids.len() {
        let id = ids.get(i).unwrap();
        let entry: AuditEntry = env
            .storage()
            .instance()
            .get(&DataKey::AuditEntry(id))
            .unwrap_or_else(|| panic!("Audit entry missing"));

        if entry.timestamp < start_time || entry.timestamp > end_time {
            continue;
        }

        if skipped < offset {
            skipped += 1;
            continue;
        }

        if output.len() >= bounded_limit {
            break;
        }

        output.push_back(entry);
    }

    output
}

fn finalize_leniency_vote_internal(
    env: &Env,
    circle_id: u64,
    requester: &Address,
    request: &mut LeniencyRequest,
) {
    let total_possible_votes = request.total_votes_cast;
    let minimum_participation = (total_possible_votes * MINIMUM_VOTING_PARTICIPATION) / 100;

    let mut final_status = LeniencyRequestStatus::Rejected;

    if request.total_votes_cast >= minimum_participation && request.total_votes_cast > 0 {
        let approval_percentage = (request.approve_votes * 100) / request.total_votes_cast;
        if approval_percentage >= SIMPLE_MAJORITY_THRESHOLD {
            final_status = LeniencyRequestStatus::Approved;

            let circle_key = DataKey::Circle(circle_id);
            let mut circle: CircleInfo = env
                .storage()
                .instance()
                .get(&circle_key)
                .expect("Circle not found");

            let extension_seconds = request.extension_hours * 3600;
            let new_deadline = circle.deadline_timestamp + extension_seconds;
            circle.grace_period_end = new_deadline;

            env.storage().instance().set(&circle_key, &circle);

            let social_capital_key = DataKey::SocialCapital(requester.clone(), circle_id);
            let mut social_capital: SocialCapital = env
                .storage()
                .instance()
                .get(&social_capital_key)
                .unwrap_or(SocialCapital {
                    member: requester.clone(),
                    circle_id,
                    leniency_given: 0,
                    leniency_received: 0,
                    voting_participation: 0,
                    trust_score: 50,
                });
            social_capital.leniency_received += 1;
            social_capital.trust_score = (social_capital.trust_score + 5).min(100);
            env.storage().instance().set(&social_capital_key, &social_capital);
        }
    }

    request.status = final_status.clone();

    let stats_key = DataKey::LeniencyStats(circle_id);
    let mut stats: LeniencyStats = env
        .storage()
        .instance()
        .get(&stats_key)
        .unwrap_or(LeniencyStats {
            total_requests: 0,
            approved_requests: 0,
            rejected_requests: 0,
            expired_requests: 0,
            average_participation: 0,
        });

    match final_status {
        LeniencyRequestStatus::Approved => stats.approved_requests += 1,
        LeniencyRequestStatus::Rejected => stats.rejected_requests += 1,
        LeniencyRequestStatus::Expired => stats.expired_requests += 1,
        _ => {}
    }

    if stats.total_requests > 0 {
        let total_participation =
            stats.average_participation * (stats.total_requests - 1) + request.total_votes_cast;
        stats.average_participation = total_participation / stats.total_requests;
    }

    env.storage().instance().set(&stats_key, &stats);
}

fn execute_proposal_logic(env: &Env, proposal: &Proposal) {
    let proposal_key = DataKey::Proposal(proposal.id);
    let mut updated_proposal = proposal.clone();
    updated_proposal.status = ProposalStatus::Executed;
    env.storage().instance().set(&proposal_key, &updated_proposal);
}

#[contract]
pub struct SoroSusu;

#[contractimpl]
impl SoroSusuTrait for SoroSusu {
    fn init(env: Env, admin: Address) {
        // Initialize the circle counter to 0 if it doesn't exist
        if !env.storage().instance().has(&DataKey::CircleCount) {
            env.storage().instance().set(&DataKey::CircleCount, &0u64);
        }

        // Set the admin
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::CircleCount, &0u64);
        env.storage().instance().set(&DataKey::AuditCount, &0u64);
    }

    fn set_lending_pool(env: Env, admin: Address, pool: Address) {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        env.storage().instance().set(&DataKey::LendingPool, &pool);
        write_audit(&env, &admin, AuditAction::AdminAction, 0);
    }

    fn set_protocol_fee(env: Env, admin: Address, fee_basis_points: u32, treasury: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).expect("Not initialized");
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        if fee_basis_points > 10000 {
            panic!("InvalidFeeConfig");
        }
        env.storage().instance().set(&DataKey::ProtocolFeeBps, &fee_basis_points);
        env.storage().instance().set(&DataKey::ProtocolTreasury, &treasury);
    }

    fn create_circle(
        env: Env,
        creator: Address,
        contribution_amount: u64,
        max_members: u16,
        amount: i128,
        max_members: u32,
        token: Address,
        cycle_duration: u64,
        insurance_fee_bps: u32,
        nft_contract: Address,
        arbitrator: Address,
        organizer_fee_bps: u32,
    ) -> u64 {
        // Validate organizer fee (cannot exceed 100%)
        if organizer_fee_bps > 10_000 {
            panic!("Organizer fee cannot exceed 100%");
        }

        // Validate insurance fee (cannot exceed 100%)
        if insurance_fee_bps > 10_000 {
            panic!("Insurance fee cannot exceed 100%");
        }

        // Get the current Circle Count
        let mut circle_count: u64 = env.storage().instance().get(&DataKey::CircleCount).unwrap_or(0);
        
        // Increment for the new circle
        circle_count += 1;
        
        // Create the new circle
        let circle = CircleInfo {
            creator: creator.clone(),
            contribution_amount,
            max_members,
            current_members: 0,
            token: token.clone(),
            cycle_duration,
            insurance_fee_bps,
            organizer_fee_bps,
            nft_contract,
            arbitrator,
            members: Vec::new(&env),
            contributions: Map::new(&env),
            current_round: 0,
            round_start_time: env.ledger().timestamp(),
            is_round_finalized: false,
            current_pot_recipient: None,
            gas_buffer_balance: 0i128,
            gas_buffer_enabled: true, // Enable by default for reliability
        };

        // Store the circle
        env.storage().instance().set(&DataKey::Circle(circle_count), &circle);
        
        // Update the circle count
        env.storage().instance().set(&DataKey::CircleCount, &circle_count);

        // Set default gas buffer configuration for this circle
        let default_config = GasBufferConfig {
            min_buffer_amount: 10000000, // 0.01 XLM minimum
            max_buffer_amount: 1000000000, // 10 XLM maximum
            auto_refill_threshold: 5000000, // 0.005 XLM threshold
            emergency_buffer: 50000000, // 0.5 XLM emergency buffer
        };
        env.storage().instance().set(&DataKey::GasBufferConfig(circle_count), &default_config);

        circle_count
    }

    fn join_circle(env: Env, user: Address, circle_id: u64, guarantor: Option<Address>) {
        // Authorization: The user MUST sign this transaction
        user.require_auth();

        // Check if the circle exists
        let mut circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Check if the circle is full
        if circle.current_members >= circle.max_members {
            panic!("Circle is full");
        }

        // Check if the user is already a member
        if circle.members.contains(&user) {
            panic!("Already a member");
        }

        // Add the user to the members list
        circle.members.push_back(user.clone());
        circle.current_members += 1;

        // Store member by index for efficient lookup during payouts
        let member_index = circle.current_members - 1;
        env.storage().instance().set(&DataKey::MemberByIndex(circle_id, member_index as u32), &user);

        // Create member record
        let member = Member {
            address: user.clone(),
            join_time: env.ledger().timestamp(),
            total_contributions: 0i128,
            total_received: 0i128,
            has_contributed_current_round: false,
            consecutive_missed_rounds: 0,
        };

        // Store the member
        env.storage().instance().set(&DataKey::Member(user.clone()), &member);

        // Update the circle
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    }

    fn deposit(env: Env, user: Address, circle_id: u64) {
        // Authorization: The user must sign this!
        user.require_auth();

        // Get the circle
        let mut circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Get the member
        let mut member: Member = env.storage().instance()
            .get(&DataKey::Member(user.clone()))
            .unwrap_or_else(|| panic!("Member not found"));

        // Check if already contributed this round
        if member.has_contributed_current_round {
            panic!("Already contributed this round");
        }

        // Calculate the total amount needed (contribution + insurance fee + group insurance premium)
        let insurance_fee = (circle.contribution_amount as i128 * circle.insurance_fee_bps as i128) / 10_000;
        
        // Group Insurance Fund premium (0.5% = 50 basis points)
        let group_insurance_premium = (circle.contribution_amount as i128 * 50i128) / 10_000;
        
        let total_amount = circle.contribution_amount as i128 + insurance_fee + group_insurance_premium;

        // Transfer the tokens from user to contract
        let token_client = token::Client::new(&env, &circle.token);
        token_client.transfer(&user, &env.current_contract_address(), &total_amount);

        // Update member record
        member.has_contributed_current_round = true;
        member.total_contributions += total_amount;
        member.consecutive_missed_rounds = 0; // Reset missed rounds counter

        // Update circle contributions
        circle.contributions.set(user.clone(), true);

        // Update Group Insurance Fund
        let mut insurance_fund: GroupInsuranceFund = env.storage().instance()
            .get(&DataKey::GroupInsuranceFund(circle_id))
            .unwrap_or(GroupInsuranceFund {
                circle_id,
                total_fund_balance: 0,
                total_premiums_collected: 0,
                total_claims_paid: 0,
                premium_rate_bps: 50, // 0.5%
                is_active: true,
                cycle_start_time: env.ledger().timestamp(),
                last_claim_time: None,
            });
        
        insurance_fund.total_fund_balance += group_insurance_premium;
        insurance_fund.total_premiums_collected += group_insurance_premium;
        env.storage().instance().set(&DataKey::GroupInsuranceFund(circle_id), &insurance_fund);

        // Update individual premium record
        let mut premium_record: InsurancePremiumRecord = env.storage().instance()
            .get(&DataKey::InsurancePremium(circle_id, user.clone()))
            .unwrap_or(InsurancePremiumRecord {
                member: user.clone(),
                circle_id,
                total_premium_paid: 0,
                premium_payments: Vec::new(&env),
                claims_made: 0,
                net_contribution: 0,
            });
        
        premium_record.total_premium_paid += group_insurance_premium;
        let current_round = circle.current_recipient_index + 1;
        premium_record.premium_payments.push_back((current_round, group_insurance_premium));
        premium_record.net_contribution = premium_record.total_premium_paid - premium_record.claims_made;
        env.storage().instance().set(&DataKey::InsurancePremium(circle_id, user.clone()), &premium_record);

        // Track payment timing for priority distribution
        let current_time = env.ledger().timestamp();
        let is_on_time = current_time <= circle.deadline_timestamp;
        
        // Get or initialize payment order counter for this round
        let mut payment_order_counter: u32 = env.storage().instance()
            .get(&DataKey::PaymentOrderCounter(circle_id, current_round))
            .unwrap_or(0);
        payment_order_counter += 1;
        env.storage().instance().set(&DataKey::PaymentOrderCounter(circle_id, current_round), &payment_order_counter);
        
        let payment_timing = PaymentTimingRecord {
            member: user.clone(),
            circle_id,
            round_number: current_round,
            payment_timestamp: current_time,
            is_on_time,
            payment_order: payment_order_counter,
        };
        env.storage().instance().set(&DataKey::PaymentTiming(circle_id, current_round, user.clone()), &payment_timing);

        // Store updated records
        env.storage().instance().set(&DataKey::Member(user), &member);
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Check if all members have contributed and auto-finalize if so
        Self::check_and_finalize_round(&env, circle_id);
    }

    // --- GAS BUFFER MANAGEMENT ---

    fn fund_gas_buffer(env: Env, circle_id: u64, amount: i128) {
        // Get the circle
        let mut circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Get gas buffer config
        let config: GasBufferConfig = env.storage().instance()
            .get(&DataKey::GasBufferConfig(circle_id))
            .unwrap_or_else(|| panic!("Gas buffer config not found"));

        // Validate amount doesn't exceed maximum
        if circle.gas_buffer_balance + amount > config.max_buffer_amount {
            panic!("Amount exceeds maximum gas buffer limit");
        }

        // Transfer XLM from caller to contract
        let xlm_token = env.native_token();
        let token_client = token::Client::new(&env, &xlm_token);
        
        // Get caller address - in a real implementation, this would be extracted from auth
        let caller = env.current_contract_address(); 
        
        token_client.transfer(&caller, &env.current_contract_address(), &amount);

        // Update gas buffer balance
        circle.gas_buffer_balance += amount;

        // Store updated circle
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Emit event for gas buffer funding
        env.events().publish(
            (Symbol::new(&env, "gas_buffer_funded"), circle_id),
            (amount, circle.gas_buffer_balance),
        );
    }

    fn set_gas_buffer_config(env: Env, circle_id: u64, config: GasBufferConfig) {
        // Only circle creator can set config
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Check authorization
        circle.creator.require_auth();

        // Validate config parameters
        if config.min_buffer_amount < 0 || config.max_buffer_amount <= config.min_buffer_amount {
            panic!("Invalid buffer configuration");
        }

        // Store the configuration
        env.storage().instance().set(&DataKey::GasBufferConfig(circle_id), &config);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "gas_buffer_config_updated"), circle_id),
            (config.min_buffer_amount, config.max_buffer_amount),
        );
    }

    fn get_gas_buffer_balance(env: Env, circle_id: u64) -> i128 {
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));
        
        circle.gas_buffer_balance
    }

    // --- PAYOUT FUNCTIONS WITH GAS BUFFER ---

    fn distribute_payout(env: Env, caller: Address, circle_id: u64) {
        // Authorization check
        caller.require_auth();

        // Get the circle
        let mut circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Check if all members have contributed
        if !Self::all_members_contributed(&env, circle_id) {
            panic!("Not all members have contributed this cycle");
        }

        // Get the current recipient
        let recipient = Self::get_current_recipient(&env, circle_id)
            .unwrap_or_else(|| panic!("No recipient found"));

        // Calculate payout amounts
        let gross_payout = (circle.contribution_amount as i128) * (circle.current_members as i128);
        let organizer_fee = (gross_payout * circle.organizer_fee_bps as i128) / 10_000;
        let net_payout = gross_payout - organizer_fee;

        // Check gas buffer and ensure sufficient funds for transaction
        Self::ensure_gas_buffer(&env, circle_id);

        // Execute the payout with gas buffer protection
        Self::execute_payout_with_gas_protection(
            &env,
            &circle,
            &recipient,
            &circle.creator,
            net_payout,
            organizer_fee,
        ).expect("Payout execution failed");

        // Update circle state
        circle.current_round += 1;
        circle.round_start_time = env.ledger().timestamp();
        circle.is_round_finalized = false;
        circle.current_pot_recipient = None;

        // Reset contribution status for all members
        Self::reset_contributions(&env, circle_id);

        // Store updated circle
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Emit events
        env.events().publish(
            (Symbol::new(&env, "payout_distributed"), circle_id),
            (recipient, net_payout),
        );

        if organizer_fee > 0 {
            env.events().publish(
                (Symbol::new(&env, "commission_paid"), circle_id),
                (circle.creator, organizer_fee),
            );
        }
    }

    fn trigger_payout(env: Env, admin: Address, circle_id: u64) {
        // Admin-only function
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Admin not set"));
        
        if admin != stored_admin {
            panic!("Unauthorized: Only admin can trigger payout");
        }

        // Call distribute_payout with admin as caller
        Self::distribute_payout(env, admin, circle_id);
    }

    fn finalize_round(env: Env, creator: Address, circle_id: u64) {
        // Check authorization (only creator can finalize)
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        if creator != circle.creator {
            panic!("Only creator can finalize round");
        }

        // Check if all members have contributed
        if !Self::all_members_contributed(&env, circle_id) {
            panic!("Not all members have contributed this cycle");
        }

        // Determine next recipient (simple round-robin for now)
        let next_recipient_index = circle.current_round % (circle.current_members as u32);
        let next_recipient = env.storage().instance()
            .get(&DataKey::MemberByIndex(circle_id, next_recipient_index))
            .unwrap_or_else(|| panic!("Member not found for next round"));

        // Update circle state
        let mut updated_circle = circle;
        updated_circle.is_round_finalized = true;
        updated_circle.current_pot_recipient = Some(next_recipient);
        updated_circle.round_start_time = env.ledger().timestamp();

        // Store updated circle
        env.storage().instance().set(&DataKey::Circle(circle_id), &updated_circle);

        // Schedule payout time
        let scheduled_time = env.ledger().timestamp() + updated_circle.cycle_duration;
        env.storage().instance().set(&DataKey::ScheduledPayoutTime(circle_id), &scheduled_time);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "round_finalized"), circle_id),
            (next_recipient, scheduled_time),
        );
    }

    // --- HELPER FUNCTIONS ---

    fn get_circle(env: Env, circle_id: u64) -> CircleInfo {
        env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"))
    }

    fn get_member(env: Env, member: Address) -> Member {
        env.storage::instance()
            .get(&DataKey::Member(member))
            .unwrap_or_else(|| panic!("Member not found"))
    }

    fn get_current_recipient(env: Env, circle_id: u64) -> Option<Address> {
        let circle: CircleInfo = env.storage::instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // If circle is finalized and has a designated recipient, use that
        if circle.is_round_finalized {
            return circle.current_pot_recipient;
        }

        // Otherwise, determine based on round number (round-robin)
        if circle.current_members == 0 {
            return None;
        }

        let recipient_index = circle.current_round % (circle.current_members as u32);
        env.storage::instance()
            .get(&DataKey::MemberByIndex(circle_id, recipient_index))
    }

    // --- INTERNAL HELPER FUNCTIONS ---

    fn all_members_contributed(env: &Env, circle_id: u64) -> bool {
        let circle: CircleInfo = env.storage::instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        if circle.current_members == 0 {
            return false;
        }

        // Check if every member has contributed
        for member in circle.members.iter() {
            if !circle.contributions.get(member).unwrap_or(false) {
                return false;
            }
        }

        true
    }

    fn ensure_gas_buffer(env: &Env, circle_id: u64) {
        let mut circle: CircleInfo = env.storage::instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        let config: GasBufferConfig = env.storage::instance()
            .get(&DataKey::GasBufferConfig(circle_id))
            .unwrap_or_else(|| panic!("Gas buffer config not found"));

        // Check if gas buffer is enabled
        if !circle.gas_buffer_enabled {
            return;
        }

        // Check if buffer needs refilling
        if circle.gas_buffer_balance < config.auto_refill_threshold {
            // Use emergency buffer if available
            if circle.gas_buffer_balance >= config.emergency_buffer {
                // Allow payout but emit warning
                env.events().publish(
                    (Symbol::new(&env, "gas_buffer_warning"), circle_id),
                    ("Low gas buffer", circle.gas_buffer_balance),
                );
            } else {
                // Critical: buffer too low, attempt auto-refill from emergency funds
                if config.emergency_buffer > 0 {
                    env.events().publish(
                        (Symbol::new(&env, "emergency_gas_usage"), circle_id),
                        ("Using emergency buffer", config.emergency_buffer),
                    );
                    circle.gas_buffer_balance += config.emergency_buffer;
                    env.storage::instance().set(&DataKey::Circle(circle_id), &circle);
                } else {
                    panic!("Insufficient gas buffer for payout. Please fund the gas buffer.");
                }
            }
        }
    }

    fn execute_payout_with_gas_protection(
        env: &Env,
        circle: &CircleInfo,
        recipient: &Address,
        organizer: &Address,
        net_payout: i128,
        organizer_fee: i128,
    ) -> Result<(), ()> {
        let token_client = token::Client::new(env, &circle.token);

        // Calculate estimated gas cost (conservative estimate)
        let estimated_gas_cost = 2000000i128; // 2 XLM conservative estimate
        
        // Check if we have enough gas buffer
        if circle.gas_buffer_balance < estimated_gas_cost {
            return Err(());
        }

        // Execute transfers
        token_client.transfer(
            &env.current_contract_address(),
            recipient,
            &net_payout,
        );

        if organizer_fee > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                organizer,
                &organizer_fee,
            );
        }

        // Deduct gas cost from buffer (in a real implementation, this would be the actual gas used)
        let mut updated_circle = circle.clone();
        updated_circle.gas_buffer_balance -= estimated_gas_cost;
        env.storage::instance().set(&DataKey::Circle(circle_id), &updated_circle);

        Ok(())
    }

    fn check_and_finalize_round(env: &Env, circle_id: u64) {
        if Self::all_members_contributed(env, circle_id) {
            let circle: CircleInfo = env.storage::instance()
                .get(&DataKey::Circle(circle_id))
                .unwrap_or_else(|| panic!("Circle not found"));

            if !circle.is_round_finalized {
                // Auto-finalize the round
                Self::finalize_round(env.clone(), circle.creator.clone(), circle_id);
            }
        }
    }

    fn reset_contributions(env: &Env, circle_id: u64) {
        let mut circle: CircleInfo = env.storage::instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Clear all contribution statuses
        circle.contributions = Map::new(env);

        // Reset member contribution flags
        for member in circle.members.iter() {
            let mut member_info: Member = env.storage::instance()
                .get(&DataKey::Member(member))
                .unwrap_or_else(|| panic!("Member not found"));
            
            member_info.has_contributed_current_round = false;
            env.storage::instance().set(&DataKey::Member(member), &member_info);
        }

        env.storage::instance().set(&DataKey::Circle(circle_id), &circle);
    ) -> u64 {
        creator.require_auth();
        if max_members == 0 {
            panic!("Max members must be greater than zero");
        }

        let current_time = env.ledger().timestamp();
        let rate_limit_key = DataKey::LastCreatedTimestamp(creator.clone());
        if let Some(last_created) = env.storage().instance().get::<DataKey, u64>(&rate_limit_key) {
            if current_time < last_created + RATE_LIMIT_SECONDS {
                panic!("Rate limit exceeded");
            }
        }
        env.storage().instance().set(&rate_limit_key, &current_time);

        let mut circle_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircleCount)
            .unwrap_or(0);
        circle_count += 1;

        // Calculate total cycle value and determine collateral requirements
        let total_cycle_value = amount * (max_members as i128);
        let requires_collateral = total_cycle_value >= HIGH_VALUE_THRESHOLD;
        let collateral_bps = if requires_collateral { DEFAULT_COLLATERAL_BPS } else { 0 };

        let new_circle = CircleInfo {
            id: circle_count,
            creator,
            contribution_amount: amount,
            max_members,
            member_count: 0,
            current_recipient_index: 0,
            is_active: true,
            token,
            deadline_timestamp: current_time + cycle_duration,
            cycle_duration,
            contribution_bitmap: 0,
            insurance_balance: 0,
            insurance_fee_bps,
            is_insurance_used: false,
            late_fee_bps: 100,
            nft_contract,
            is_round_finalized: false,
            current_pot_recipient: None,
            requires_collateral,
            collateral_bps,
            member_addresses: Vec::new(&env),
            leniency_enabled: true,
            grace_period_end: None,
            quadratic_voting_enabled: max_members >= MIN_GROUP_SIZE_FOR_QUADRATIC,
            proposal_count: 0,
            dissolution_status: DissolutionStatus::NotInitiated,
            dissolution_deadline: None,
            proposed_late_fee_bps: 0,
            proposal_votes_bitmap: 0,
            recovery_old_address: None,
            recovery_new_address: None,
            recovery_votes_bitmap: 0,
            arbitrator,
            basket: None,
        };

        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_count), &new_circle);
        env.storage().instance().set(&DataKey::CircleCount, &circle_count);
        circle_count
    }

    fn join_circle(
        env: Env,
        user: Address,
        circle_id: u64,
        tier_multiplier: u32,
        referrer: Option<Address>,
    ) {
        user.require_auth();

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        if circle.member_count >= circle.max_members {
            panic!("Circle is full");
        }

        let member_key = DataKey::Member(user.clone());
        if env.storage().instance().has(&member_key) {
            panic!("Already member");
        }

        // Check collateral requirement for high-value circles
        if circle.requires_collateral {
            let collateral_key = DataKey::CollateralVault(user.clone(), circle_id);
            let collateral_info: Option<CollateralInfo> = env.storage().instance().get(&collateral_key);
            
            match collateral_info {
                Some(collateral) => {
                    if collateral.status != CollateralStatus::Staked {
                        panic!("Collateral not properly staked");
                    }
                }
                None => panic!("Collateral required for this circle"),
            }
        }

        let new_member = Member {
            address: user.clone(),
            index: circle.member_count,
            contribution_count: 0,
            last_contribution_time: 0,
            status: MemberStatus::Active,
            tier_multiplier,
            referrer,
            buddy: None,
        };

        env.storage().instance().set(&member_key, &new_member);
        env.storage().instance().set(&DataKey::CircleMember(circle_id, circle.member_count), &user);
        circle.member_count += 1;
        circle.member_addresses.push_back(user.clone());
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        let token_id = (circle_id as u128) << 64 | (new_member.index as u128);
        let nft_client = SusuNftClient::new(&env, &circle.nft_contract);
        nft_client.mint(&user, &token_id);
    }

    fn deposit(env: Env, user: Address, circle_id: u64) {
        user.require_auth();

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .expect("Member not found");

        if member.status != MemberStatus::Active {
            panic!("Member not active");
        }

        let current_time = env.ledger().timestamp();
        let base_amount = circle.contribution_amount * member.tier_multiplier as i128;
        let mut penalty_amount = 0i128;
        let user_stats_key = DataKey::UserStats(user.clone());
        let mut user_stats: UserStats = env.storage().instance().get(&user_stats_key).unwrap_or(UserStats {
            total_volume_saved: 0,
            on_time_contributions: 0,
            late_contributions: 0,
        });

        // Check if contribution is late
        if current_time > circle.deadline_timestamp {
            let base_penalty = (base_amount * circle.late_fee_bps as i128) / 10000;
            // Apply referral discount
            let mut discount = 0i128;
            if let Some(ref_addr) = &member.referrer {
                let ref_key = DataKey::Member(ref_addr.clone());
                if env.storage().instance().has(&ref_key) {
                    discount = (base_penalty * REFERRAL_DISCOUNT_BPS as i128) / 10000;
                }
            }
            penalty_amount = base_penalty - discount;

            let mut reserve: i128 = env.storage().instance().get(&DataKey::GroupReserve).unwrap_or(0);
            reserve += penalty_amount;
            env.storage().instance().set(&DataKey::GroupReserve, &reserve);
        }

        // Update user statistics
        let user_stats_key = DataKey::UserStats(user.clone());
        let mut user_stats: UserStats = env.storage().instance().get(&user_stats_key).unwrap_or(UserStats {
            total_volume_saved: 0,
            on_time_contributions: 0,
            late_contributions: 0,
        });

        if penalty_amount > 0 {
            user_stats.late_contributions += 1;
        } else {
            user_stats.on_time_contributions += 1;
        }

        user_stats.total_volume_saved += base_amount;
        env.storage().instance().set(&user_stats_key, &user_stats);

        env.events().publish(
            (Symbol::new(&env, "USER_STATS"), user.clone()),
            (user_stats.on_time_contributions, user_stats.late_contributions, user_stats.total_volume_saved)
        );

        let insurance_fee = (base_amount * circle.insurance_fee_bps as i128) / 10000;
        let total_amount = base_amount + insurance_fee + penalty_amount;

        let token_client = token::Client::new(&env, &circle.token);
        let transfer_result = token_client.try_transfer(&user, &env.current_contract_address(), &total_amount);
        let transfer_success = match transfer_result {
            Ok(inner) => inner.is_ok(),
            Err(_) => false,
        };

        if !transfer_success {
            if let Some(buddy_addr) = member.buddy.clone() {
                let safety_key = DataKey::SafetyDeposit(buddy_addr, circle_id);
                let safety_balance: i128 = env.storage().instance().get(&safety_key).unwrap_or(0);
                if safety_balance < total_amount {
                    panic!("Insufficient funds and buddy deposit");
                }
                env.storage()
                    .instance()
                    .set(&safety_key, &(safety_balance - total_amount));
            } else {
                panic!("Insufficient funds");
            }
        }

        if insurance_fee > 0 {
            circle.insurance_balance += insurance_fee;
        }

        member.contribution_count += 1;
        member.last_contribution_time = current_time;
        circle.contribution_bitmap |= 1u64 << member.index;

        env.storage().instance().set(&member_key, &member);
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    }

    fn finalize_round(env: Env, caller: Address, circle_id: u64) {
        caller.require_auth();
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        if caller != circle.creator && caller != stored_admin {
            panic!("Unauthorized");
        }

        if circle.is_round_finalized {
            panic!("Round already finalized");
        }

        let expected_bitmap = (1u64 << circle.member_count) - 1;
        if circle.contribution_bitmap != expected_bitmap {
            panic!("Not all contributed");
        }

        // Set round as finalized and determine next recipient
        circle.is_round_finalized = true;
        
        // Set next recipient (round-robin)
        let next_recipient_index = (circle.current_recipient_index + 1) % circle.member_count;
        let next_recipient = get_member_address_by_index(&circle, next_recipient_index);
        
        circle.current_recipient_index = next_recipient_index;
        circle.current_pot_recipient = Some(next_recipient.clone());

        // Schedule payout time (end of month from now)
        let current_time = env.ledger().timestamp();
        let payout_time = current_time + (30 * 24 * 60 * 60); // 30 days from now
        env.storage().instance().set(&DataKey::ScheduledPayoutTime(circle_id), &payout_time);

        // Reset for next round
        circle.contribution_bitmap = 0;
        circle.deadline_timestamp = current_time + circle.cycle_duration;

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Publish round finalization event
        env.events().publish(
            (Symbol::new(&env, "ROUND_FINALIZED"), circle_id),
            (next_recipient, payout_time, next_recipient_index),
        );


    }

    fn claim_pot(env: Env, user: Address, circle_id: u64) {
        user.require_auth();
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");

        if !circle.is_round_finalized {
            panic!("Round not finalized");
        }

        let recipient = circle
            .current_pot_recipient
            .clone()
            .unwrap_or_else(|| panic!("No recipient set"));
        if user != recipient {
            panic!("Unauthorized recipient");
        }

        let scheduled_time: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ScheduledPayoutTime(circle_id))
            .expect("Payout not scheduled");
        if env.ledger().timestamp() < scheduled_time {
            panic!("Payout too early");
        }

        let pot_amount = circle.contribution_amount * (circle.member_count as i128);
        
        // Check for rollover bonus and add to first pot of new cycles
        let mut total_payout = pot_amount;
        let rollover_key = DataKey::RolloverBonus(circle_id);
        if let Some(rollover_bonus) = env.storage().instance().get::<DataKey, RolloverBonus>(&rollover_key) {
            if rollover_bonus.status == RolloverStatus::Applied {
                if let Some(applied_cycle) = rollover_bonus.applied_cycle {
                    if applied_cycle == circle.current_recipient_index {
                        total_payout += rollover_bonus.bonus_amount;
                        
                        env.events().publish(
                            (Symbol::new(&env, "ROLLOVER_BONUS_APPLIED"), circle_id, user.clone()),
                            (rollover_bonus.bonus_amount, applied_cycle),
                        );
                    }
                }
            }
        }
        
        let fee_bps: u32 = env.storage().instance().get(&DataKey::ProtocolFeeBps).unwrap_or(0);

        if let Some(ref basket) = circle.basket.clone() {
            // Basket circle: distribute each asset to the winner proportionally
            let maybe_treasury: Option<Address> = if fee_bps > 0 {
                env.storage().instance().get(&DataKey::ProtocolTreasury)
            } else {
                None
            };

            for i in 0..basket.len() {
                let asset_weight = basket.get(i).unwrap();
                // Total pot for this asset = contribution_amount * member_count * weight / 10000
                let asset_pot = (circle.contribution_amount
                    * (circle.member_count as i128)
                    * asset_weight.weight_bps as i128)
                    / 10000;

                let token_client = token::Client::new(&env, &asset_weight.token);

                if fee_bps > 0 {
                    let treasury = maybe_treasury
                        .clone()
                        .expect("Treasury not set");
                    let fee = (asset_pot * fee_bps as i128) / 10000;
                    let net = asset_pot - fee;
                    token_client.transfer(&env.current_contract_address(), &treasury, &fee);
                    token_client.transfer(&env.current_contract_address(), &user, &net);
                } else {
                    token_client.transfer(&env.current_contract_address(), &user, &asset_pot);
                }
            }

            env.events().publish(
                (Symbol::new(&env, "BASKET_POT_CLAIMED"), circle_id, user.clone()),
                (basket.len(), circle.member_count),
            );
        } else {
            // Single-token circle (original logic)
            let token_client = token::Client::new(&env, &circle.token);

            if fee_bps > 0 {
                let treasury: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::ProtocolTreasury)
                    .expect("Treasury not set");
                let fee = (total_payout * fee_bps as i128) / 10000;
                let net_payout = total_payout - fee;
                token_client.transfer(&env.current_contract_address(), &treasury, &fee);
                token_client.transfer(&env.current_contract_address(), &user, &net_payout);
            } else {
                token_client.transfer(&env.current_contract_address(), &user, &total_payout);
            }
        }

        // Auto-release collateral if member has completed all contributions
        if circle.requires_collateral {
            let member_key = DataKey::Member(user.clone());
            if let Some(member_info) = env.storage().instance().get::<DataKey, Member>(&member_key) {
                if member_info.contribution_count >= circle.max_members {
                    let collateral_key = DataKey::CollateralVault(user.clone(), circle_id);
                    if let Some(mut collateral_info) = env.storage().instance().get::<DataKey, CollateralInfo>(&collateral_key) {
                        if collateral_info.status == CollateralStatus::Staked {
                            // Release collateral back to member
                            token_client.transfer(&env.current_contract_address(), &user, &collateral_info.amount);
                            
                            // Update collateral status
                            collateral_info.status = CollateralStatus::Released;
                            collateral_info.release_timestamp = Some(env.ledger().timestamp());
                            env.storage().instance().set(&collateral_key, &collateral_info);
                        }
                    }
                }
            }
        }

        circle.is_round_finalized = false;
        circle.contribution_bitmap = 0;
        circle.is_insurance_used = false;

        // Mint soulbound "Susu Master" badge when the full cycle completes
        let next_index = circle.current_recipient_index + 1;
        if next_index >= circle.max_members {
            let member_key = DataKey::Member(user.clone());
            if let Some(member_info) = env.storage().instance().get::<DataKey, Member>(&member_key) {
                let stats_key = DataKey::UserStats(user.clone());
                let stats: UserStats = env.storage().instance().get(&stats_key).unwrap_or(UserStats {
                    total_volume_saved: 0,
                    on_time_contributions: 0,
                    late_contributions: 0,
                });
                
                // Get user's reputation data for comprehensive scoring
                let reputation_key = DataKey::ReputationData(user.clone());
                let reputation: ReputationData = env.storage().instance().get(&reputation_key).unwrap_or(ReputationData {
                    user_address: user.clone(),
                    susu_score: 0,
                    reliability_score: 0,
                    total_contributions: 0,
                    on_time_rate: 0,
                    volume_saved: 0,
                    social_capital: 0,
                    last_updated: 0,
                    is_active: false,
                });
                
                // Count total cycles completed by this user
                let mut total_cycles: u32 = 0;
                for cycle_key_bytes in env.storage().all_keys() {
                    if let Ok(badge_token_id) = env.storage().instance().get::<DataKey, u128>(&DataKey::CycleBadge(user.clone(), 0)) {
                        // This is a simplified check - in production would iterate through all circles
                        total_cycles += 1;
                    }
                }
                
                // Enhanced volume tier with Platinum level
                let volume_tier: u32 = if stats.total_volume_saved >= 100_000_000_000 { 4 } // Platinum
                    else if stats.total_volume_saved >= 10_000_000_000 { 3 } // Gold
                    else if stats.total_volume_saved >= 1_000_000_000 { 2 } // Silver
                    else { 1 }; // Bronze
                
                // Build list of badges earned
                let mut badges_earned = Vec::new(&env);
                if stats.late_contributions == 0 {
                    badges_earned.push_back(symbol_short!("PERFECT"));
                }
                if member_info.address == circle.creator {
                    badges_earned.push_back(symbol_short!("LEADER"));
                }
                if total_cycles > 1 {
                    badges_earned.push_back(symbol_short!("VETERAN"));
                }
                if volume_tier >= 3 {
                    badges_earned.push_back(symbol_short!("ELITE"));
                }
                
                // Calculate ecosystem participation (simplified - would query other contracts)
                let ecosystem_participation: u32 = 1; // Minimum participation in this contract
                
                // Create Master Credential metadata
                let metadata = MasterCredentialMetadata {
                    volume_tier,
                    perfect_attendance: stats.late_contributions == 0,
                    group_lead_status: member_info.address == circle.creator,
                    total_cycles_completed: total_cycles + 1,
                    total_volume_saved: stats.total_volume_saved,
                    reliability_score: reputation.reliability_score,
                    social_capital_score: reputation.social_capital,
                    badges_earned,
                    ecosystem_participation,
                    mint_timestamp: env.ledger().timestamp(),
                    circle_id,
                    version: 1,
                };
                
                // token_id: circle_id in upper 64 bits, member index in lower 64 bits
                let token_id: u128 = ((circle_id as u128) << 64) | (member_info.index as u128);
                let nft_client = SusuNftClient::new(&env, &circle.nft_contract);
                nft_client.mint_master_credential(&user, &token_id, &metadata);
                env.storage().instance().set(&DataKey::CycleBadge(user.clone(), circle_id), &token_id);
                env.events().publish(
                    (symbol_short!("BADGE"), symbol_short!("MASTER")),
                    (user.clone(), circle_id, token_id, metadata),
                );
            }
        }

        circle.current_recipient_index = next_index;
        env.storage().instance().remove(&DataKey::ScheduledPayoutTime(circle_id));
    }

    fn trigger_insurance_coverage(env: Env, caller: Address, circle_id: u64, member: Address) {
        caller.require_auth();

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        if caller != circle.creator {
            panic!("Unauthorized");
        }

        // Get the Group Insurance Fund
        let mut insurance_fund: GroupInsuranceFund = env.storage().instance()
            .get(&DataKey::GroupInsuranceFund(circle_id))
            .expect("Group Insurance Fund not found");
        
        if !insurance_fund.is_active {
            panic!("Insurance fund is not active");
        }
        
        if insurance_fund.total_fund_balance <= 0 {
            panic!("Insufficient insurance fund balance");
        }

        let member_key = DataKey::Member(member.clone());
        let member_info: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .expect("Member not found");

        // Calculate amount needed to cover the default (contribution for remaining rounds)
        let rounds_remaining = circle.max_members - circle.current_recipient_index;
        let amount_needed = circle.contribution_amount * (rounds_remaining as i128);
        
        if insurance_fund.total_fund_balance < amount_needed {
            panic!("Insufficient insurance fund balance to cover default");
        }

        // Deduct from insurance fund
        insurance_fund.total_fund_balance -= amount_needed;
        insurance_fund.total_claims_paid += amount_needed;
        insurance_fund.last_claim_time = Some(env.ledger().timestamp());
        env.storage().instance().set(&DataKey::GroupInsuranceFund(circle_id), &insurance_fund);

        // Update member's premium record to track claims
        let mut premium_record: InsurancePremiumRecord = env.storage().instance()
            .get(&DataKey::InsurancePremium(circle_id, member.clone()))
            .unwrap_or(InsurancePremiumRecord {
                member: member.clone(),
                circle_id,
                total_premium_paid: 0,
                premium_payments: Vec::new(&env),
                claims_made: 0,
                net_contribution: 0,
            });
        
        premium_record.claims_made += amount_needed;
        premium_record.net_contribution = premium_record.total_premium_paid - premium_record.claims_made;
        env.storage().instance().set(&DataKey::InsurancePremium(circle_id, member.clone()), &premium_record);

        // Mark the member as defaulted
        let mut member_status = member_info.status;
        member_status = MemberStatus::Defaulted;
        env.storage().instance().set(&DataKey::Member(member.clone()), &member_info);

        // The member defaulted and needed an insurance bailout, increment late count
        let user_stats_key = DataKey::UserStats(member.clone());
        let mut user_stats: UserStats = env.storage().instance().get(&user_stats_key).unwrap_or(UserStats {
            total_volume_saved: 0,
            on_time_contributions: 0,
            late_contributions: 0,
        });
        user_stats.late_contributions += 1;
        env.storage().instance().set(&user_stats_key, &user_stats);

        env.events().publish(
            (Symbol::new(&env, "INSURANCE_CLAIM"), circle_id, member.clone()),
            (amount_needed, insurance_fund.total_fund_balance),
        );

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
        write_audit(&env, &caller, AuditAction::AdminAction, circle_id);
    }

    fn get_insurance_fund(env: Env, circle_id: u64) -> GroupInsuranceFund {
        env.storage().instance()
            .get(&DataKey::GroupInsuranceFund(circle_id))
            .expect("Group Insurance Fund not found")
    }

    fn get_premium_record(env: Env, member: Address, circle_id: u64) -> InsurancePremiumRecord {
        env.storage().instance()
            .get(&DataKey::InsurancePremium(circle_id, member))
            .expect("Premium record not found")
    }

    fn distribute_remaining_insurance_fund(env: Env, circle_id: u64) {
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");

        let mut insurance_fund: GroupInsuranceFund = env.storage().instance()
            .get(&DataKey::GroupInsuranceFund(circle_id))
            .expect("Group Insurance Fund not found");

        // Check if cycle is complete (all members have received pot)
        if circle.current_recipient_index < circle.max_members - 1 {
            panic!("Cycle not complete - cannot distribute insurance fund yet");
        }

        if insurance_fund.total_fund_balance <= 0 {
            panic!("No remaining insurance fund to distribute");
        }

        // Calculate pro-rata distribution based on premiums paid
        let total_fund = insurance_fund.total_fund_balance;
        let token_client = token::Client::new(&env, &circle.token);

        for i in 0..circle.member_count {
            let member_address = circle.member_addresses.get(i).unwrap();
            
            // Get member's premium record
            if let Some(premium_record) = env.storage().instance()
                .get::<DataKey, InsurancePremiumRecord>(&DataKey::InsurancePremium(circle_id, member_address.clone()))
            {
                // Calculate refund percentage based on premium paid
                let refund_percentage = if insurance_fund.total_premiums_collected > 0 {
                    (premium_record.total_premium_paid * 10_000) / insurance_fund.total_premiums_collected
                } else {
                    0
                };
                
                let refund_amount = (total_fund * refund_percentage) / 10_000;
                
                if refund_amount > 0 {
                    token_client.transfer(&env.current_contract_address(), &member_address, &refund_amount);
                    
                    env.events().publish(
                        (Symbol::new(&env, "INSURANCE_REFUND"), circle_id, member_address.clone()),
                        (refund_amount, premium_record.total_premium_paid),
                    );
                }
            }
        }

        // Reset insurance fund for next cycle or mark as inactive
        insurance_fund.total_fund_balance = 0;
        insurance_fund.is_active = false;
        env.storage().instance().set(&DataKey::GroupInsuranceFund(circle_id), &insurance_fund);

        env.events().publish(
            (Symbol::new(&env, "INSURANCE_FUND_DISTRIBUTED"), circle_id),
            (total_fund, circle.member_count),
        );
    }

    fn propose_penalty_change(env: Env, user: Address, circle_id: u64, new_bps: u32) {
        user.require_auth();

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        let member_key = DataKey::Member(user.clone());
        let member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .expect("User is not a member");

        if member.status != MemberStatus::Active {
            panic!("Member is not active");
        }
        if new_bps > 10000 {
            panic!("Penalty cannot exceed 100%");
        }

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
        write_audit(&env, &user, AuditAction::DisputeSubmission, circle_id);
    }

    fn propose_duration_change(env: Env, user: Address, circle_id: u64, new_duration: u64) {
        user.require_auth();
        if new_duration == 0 {
            panic!("Duration must be greater than zero");
        }

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        let protocol_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");

        if user != circle.creator && user != protocol_admin {
            panic!("Unauthorized");
        }

        circle.cycle_duration = new_duration;
        circle.deadline_timestamp = env.ledger().timestamp() + new_duration;
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
        write_audit(&env, &user, AuditAction::AdminAction, circle_id);
    }

    fn vote_penalty_change(env: Env, user: Address, circle_id: u64) {
        user.require_auth();

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        let member_key = DataKey::Member(user.clone());
        let member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .expect("User is not a member");

        if member.status != MemberStatus::Active {
            panic!("Member is not active");
        }

        if circle.proposed_late_fee_bps == 0 {
            panic!("No active proposal");
        }

        circle.proposal_votes_bitmap |= 1u64 << member.index;

        if circle.proposal_votes_bitmap.count_ones() > (circle.member_count / 2) {
            circle.late_fee_bps = circle.proposed_late_fee_bps;
            circle.proposed_late_fee_bps = 0;
            circle.proposal_votes_bitmap = 0;
        }

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
        write_audit(&env, &user, AuditAction::GovernanceVote, circle_id);
    }

    fn propose_address_change(
        env: Env,
        user: Address,
        circle_id: u64,
        old_address: Address,
        new_address: Address,
    ) {
        user.require_auth();

        if old_address == new_address {
            panic!("Old and new addresses must differ");
        }

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");

        let proposer_key = DataKey::Member(user.clone());
        let proposer: Member = env
            .storage()
            .instance()
            .get(&proposer_key)
            .expect("User is not a member");
        if proposer.status != MemberStatus::Active {
            panic!("Member is not active");
        }

        let old_member_key = DataKey::Member(old_address.clone());
        let old_member: Member = env
            .storage()
            .instance()
            .get(&old_member_key)
            .expect("Old address is not a member");
        if old_member.status != MemberStatus::Active {
            panic!("Old address member is not active");
        }

        let new_member_key = DataKey::Member(new_address.clone());
        if env.storage().instance().has(&new_member_key) {
            panic!("New address is already a member");
        }

        circle.recovery_old_address = Some(old_address);
        circle.recovery_new_address = Some(new_address);
        circle.recovery_votes_bitmap = 1u64 << proposer.index;

        apply_recovery_if_consensus(&env, &user, circle_id, &mut circle);
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
        write_audit(&env, &user, AuditAction::DisputeSubmission, circle_id);
    }

    fn vote_for_recovery(env: Env, user: Address, circle_id: u64) {
        user.require_auth();

        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        if circle.recovery_old_address.is_none() || circle.recovery_new_address.is_none() {
            panic!("No active recovery proposal");
        }

        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key).expect("Not a member");

        circle.recovery_votes_bitmap |= 1u64 << member.index;
        apply_recovery_if_consensus(&env, &user, circle_id, &mut circle);

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
        write_audit(&env, &user, AuditAction::GovernanceVote, circle_id);
    }

    fn eject_member(env: Env, caller: Address, circle_id: u64, member: Address) {
        caller.require_auth();
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        if caller != circle.creator {
            panic!("Unauthorized");
        }

        let member_key = DataKey::Member(member.clone());
        let mut member_info: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .expect("Member not found");

        member_info.status = MemberStatus::Ejected;
        env.storage().instance().set(&member_key, &member_info);

        let nft_client = SusuNftClient::new(&env, &circle.nft_contract);
        let token_id = (circle_id as u128) << 64 | (member_info.index as u128);
        nft_client.burn(&member, &token_id);
        write_audit(&env, &caller, AuditAction::AdminAction, circle_id);
    }

    fn pair_with_member(env: Env, user: Address, buddy_address: Address) {
        user.require_auth();
        let user_key = DataKey::Member(user.clone());
        let mut user_info: Member = env
            .storage()
            .instance()
            .get(&user_key)
            .expect("Member not found");

        user_info.buddy = Some(buddy_address);
        env.storage().instance().set(&user_key, &user_info);
        write_audit(&env, &user, AuditAction::AdminAction, 0);
    }

    fn set_safety_deposit(env: Env, user: Address, circle_id: u64, amount: i128) {
        user.require_auth();
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");

        let token_client = token::Client::new(&env, &circle.token);
        token_client.transfer(&user, &env.current_contract_address(), &amount);

        let safety_key = DataKey::SafetyDeposit(user.clone(), circle_id);
        let mut balance: i128 = env.storage().instance().get(&safety_key).unwrap_or(0);
        balance += amount;
        env.storage().instance().set(&safety_key, &balance);
    }

    #[test]
    fn test_credit_score_oracle() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        
        let token_contract = env.register_contract(None, MockToken);
        let nft_contract = env.register_contract(None, MockNft);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
    fn get_reputation(env: Env, user: Address) -> ReputationData {
        let current_time = env.ledger().timestamp();
        
        // Get user statistics
        let user_stats_key = DataKey::UserStats(user.clone());
        let user_stats: UserStats = env.storage().instance().get(&user_stats_key).unwrap_or(UserStats {
            total_volume_saved: 0,
            on_time_contributions: 0,
            late_contributions: 0,
        });

        // Get member information to check if user is active
        let member_key = DataKey::Member(user.clone());
        let is_active = if let Some(member) = env.storage().instance().get::<DataKey, Member>(&member_key) {
            member.status == MemberStatus::Active
        } else {
            false
        };

        // Calculate total contributions
        let total_contributions = user_stats.on_time_contributions + user_stats.late_contributions;
        
        // Calculate on-time rate (in basis points)
        let on_time_rate = if total_contributions > 0 {
            (user_stats.on_time_contributions * 10000) / total_contributions
        } else {
            0
        };

        // Calculate reliability score based on on-time rate and volume
        let mut reliability_score = on_time_rate;
        
        // Boost reliability based on volume saved (higher volume = higher reliability)
        if user_stats.total_volume_saved > 0 {
            let volume_bonus = ((user_stats.total_volume_saved / 1_000_000_0) * 100).min(2000); // Max 20% bonus
            reliability_score = (reliability_score + volume_bonus).min(10000);
        }

        // Calculate social capital (sum of trust scores across all circles)
        let mut social_capital = 0u32;
        let mut circle_count = 0u32;
        
        // Get all circles the user is part of by checking member data
        // For now, we'll use a simplified approach - in a full implementation,
        // you might want to maintain an index of user's circles
        for circle_id in 1..=1000 { // Reasonable limit for iteration
            let circle_key = DataKey::Circle(circle_id);
            if let Some(_circle) = env.storage().instance().get::<DataKey, CircleInfo>(&circle_key) {
                let social_capital_key = DataKey::SocialCapital(user.clone(), circle_id);
                if let Some(soc_cap) = env.storage().instance().get::<DataKey, SocialCapital>(&social_capital_key) {
                    social_capital += soc_cap.trust_score;
                    circle_count += 1;
                }
            }
        }

        // Average social capital across circles
        let avg_social_capital = if circle_count > 0 {
            (social_capital / circle_count) * 100 // Convert to basis points
        } else {
            0
        };

        // Calculate final Susu Score (weighted combination)
        // Weight: 50% reliability, 30% social capital, 20% activity
        let activity_score = if total_contributions > 0 {
            ((total_contributions as u32).min(50) * 200) // Max 10% from activity
        } else {
            0
        };

        let susu_score = (
            (reliability_score * 50) / 100 +  // 50% weight
            (avg_social_capital * 30) / 100 +  // 30% weight  
            (activity_score * 20) / 100         // 20% weight
        ).min(10000);

        ReputationData {
            user_address: user.clone(),
            susu_score,
            reliability_score,
            total_contributions,
            on_time_rate,
            volume_saved: user_stats.total_volume_saved,
            social_capital: avg_social_capital,
            last_updated: current_time,
            is_active,
        }
    }

    fn propose_rollover_bonus(env: Env, user: Address, circle_id: u64, fee_percentage_bps: u32) {
        user.require_auth();

        if fee_percentage_bps > 10000 {
            panic!("Fee percentage cannot exceed 100%");
        }

        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key)
            .expect("User is not a member");

        if member.status != MemberStatus::Active {
            panic!("Member is not active");
        }

        // Check if there's already an active rollover proposal
        let rollover_key = DataKey::RolloverBonus(circle_id);
        if let Some(existing_rollover) = env.storage().instance().get::<DataKey, RolloverBonus>(&rollover_key) {
            if existing_rollover.status == RolloverStatus::Voting {
                panic!("Rollover bonus proposal already active");
            }
        }

        // Only allow rollover proposals after the first round is complete
        if !circle.is_round_finalized || circle.current_recipient_index == 0 {
            panic!("Rollover can only be proposed after first complete cycle");
        }

        let current_time = env.ledger().timestamp();
        let bonus_amount = calculate_rollover_bonus(&env, circle_id, fee_percentage_bps);

        let rollover_bonus = RolloverBonus {
            circle_id,
            bonus_amount,
            fee_percentage: fee_percentage_bps,
            created_timestamp: current_time,
            status: RolloverStatus::Voting,
            voting_deadline: current_time + ROLLOVER_VOTING_PERIOD,
            for_votes: 0,
            against_votes: 0,
            total_votes_cast: 0,
            applied_cycle: None,
        };

        env.storage().instance().set(&rollover_key, &rollover_bonus);
        
        // The proposer automatically votes for
        let vote_key = DataKey::RolloverVote(circle_id, user.clone());
        let vote = RolloverVote {
            voter: user.clone(),
            circle_id,
            vote_choice: RolloverVoteChoice::For,
            timestamp: current_time,
        };
        env.storage().instance().set(&vote_key, &vote);

        // Update vote counts
        let mut updated_rollover = rollover_bonus;
        updated_rollover.for_votes = 1;
        updated_rollover.total_votes_cast = 1;
        env.storage().instance().set(&rollover_key, &updated_rollover);

        write_audit(&env, &user, AuditAction::DisputeSubmission, circle_id);

        env.events().publish(
            (Symbol::new(&env, "ROLLOVER_PROPOSED"), circle_id, user.clone()),
            (bonus_amount, fee_percentage_bps, updated_rollover.voting_deadline),
        );
    }

    fn vote_rollover_bonus(env: Env, user: Address, circle_id: u64, vote_choice: RolloverVoteChoice) {
        user.require_auth();

        let rollover_key = DataKey::RolloverBonus(circle_id);
        let mut rollover_bonus: RolloverBonus = env.storage().instance().get(&rollover_key)
            .expect("No active rollover proposal");

        if rollover_bonus.status != RolloverStatus::Voting {
            panic!("Rollover proposal is not in voting period");
        }

        if env.ledger().timestamp() > rollover_bonus.voting_deadline {
            rollover_bonus.status = RolloverStatus::Rejected;
            env.storage().instance().set(&rollover_key, &rollover_bonus);
            panic!("Voting period has expired");
        }

        // Check if user is an active member
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key)
            .expect("User is not a member");

        if member.status != MemberStatus::Active {
            panic!("Member is not active");
        }

        // Check if already voted
        let vote_key = DataKey::RolloverVote(circle_id, user.clone());
        if env.storage().instance().has(&vote_key) {
            panic!("Already voted");
        }

        // Record the vote
        let vote = RolloverVote {
            voter: user.clone(),
            circle_id,
            vote_choice: vote_choice.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().instance().set(&vote_key, &vote);

        // Update vote counts
        match vote_choice {
            RolloverVoteChoice::For => rollover_bonus.for_votes += 1,
            RolloverVoteChoice::Against => rollover_bonus.against_votes += 1,
        }
        rollover_bonus.total_votes_cast += 1;

        // Check if voting criteria are met
        let circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        let active_members = count_active_members(&env, &circle);
        
        let quorum_met = (rollover_bonus.total_votes_cast * 100) >= (active_members * ROLLOVER_QUORUM);
        
        if quorum_met && rollover_bonus.total_votes_cast > 0 {
            let approval_percentage = (rollover_bonus.for_votes * 100) / rollover_bonus.total_votes_cast;
            if approval_percentage >= ROLLOVER_MAJORITY {
                rollover_bonus.status = RolloverStatus::Approved;
            }
        }

        env.storage().instance().set(&rollover_key, &rollover_bonus);
        write_audit(&env, &user, AuditAction::GovernanceVote, circle_id);

        env.events().publish(
            (Symbol::new(&env, "ROLLOVER_VOTE"), circle_id, user.clone()),
            (vote_choice, rollover_bonus.for_votes, rollover_bonus.against_votes),
        );
    }

    fn apply_rollover_bonus(env: Env, circle_id: u64) {
        let rollover_key = DataKey::RolloverBonus(circle_id);
        let mut rollover_bonus: RolloverBonus = env.storage().instance().get(&rollover_key)
            .expect("No rollover bonus proposal found");

        if rollover_bonus.status != RolloverStatus::Approved {
            panic!("Rollover bonus is not approved");
        }

        let circle_key = DataKey::Circle(circle_id);
        let mut circle: CircleInfo = env.storage().instance().get(&circle_key)
            .expect("Circle not found");

        // Apply the bonus to the group reserve (will be used in next cycle's first pot)
        let mut reserve: i128 = env.storage().instance().get(&DataKey::GroupReserve).unwrap_or(0);
        reserve += rollover_bonus.bonus_amount;
        env.storage().instance().set(&DataKey::GroupReserve, &reserve);

        // Mark as applied and track the cycle
        rollover_bonus.status = RolloverStatus::Applied;
        rollover_bonus.applied_cycle = Some(circle.current_recipient_index + 1);
        env.storage().instance().set(&rollover_key, &rollover_bonus);

        write_audit(&env, &env.current_contract_address(), AuditAction::AdminAction, circle_id);

        env.events().publish(
            (Symbol::new(&env, "ROLLOVER_APPLIED"), circle_id),
            (rollover_bonus.bonus_amount, rollover_bonus.applied_cycle.unwrap()),
        );
    }

    // Price Oracle and Asset Swap Implementation
    
    fn update_price_oracle(env: Env, oracle_provider: Address, asset: Address, price: i128) {
        // Only authorized oracle providers can update prices (in production, use multi-sig or trusted oracles)
        oracle_provider.require_auth();
        
        if price <= 0 {
            panic!("Invalid price");
        }
        
        let oracle_data = PriceOracleData {
            asset_address: asset.clone(),
            current_price: price,
            last_updated: env.ledger().timestamp(),
            is_stable_asset: false, // Would be determined by oracle provider
        };
        
        env.storage().instance().set(&DataKey::PriceOracle(asset), &oracle_data);
        
        env.events().publish(
            (Symbol::new(&env, "PRICE_UPDATED"), asset),
            (price, env.ledger().timestamp()),
        );
    }
    
    fn get_asset_price(env: Env, asset: Address) -> PriceOracleData {
        env.storage().instance()
            .get(&DataKey::PriceOracle(asset))
            .expect("Asset price not found")
    }
    
    fn set_hard_asset_basket(env: Env, admin: Address, gold_weight_bps: u32, btc_weight_bps: u32, silver_weight_bps: u32) {
        // Verify admin authorization
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Admin not set"));
        
        if admin != stored_admin {
            panic!("Unauthorized: only admin can set hard asset basket");
        }
        
        let total_weight = gold_weight_bps + btc_weight_bps + silver_weight_bps;
        if total_weight != 10000 {
            panic!("Basket weights must sum to 10000 (100%)");
        }
        
        let basket = HardAssetBasket {
            gold_weight_bps,
            btc_weight_bps,
            silver_weight_bps,
            total_weight_bps: total_weight,
        };
        
        env.storage().instance().set(&DataKey::HardAssetBasket, &basket);
        
        env.events().publish(
            (Symbol::new(&env, "HARD_ASSET_BASKET_SET")),
            (gold_weight_bps, btc_weight_bps, silver_weight_bps),
        );
    }
    
    fn get_hard_asset_basket(env: Env) -> HardAssetBasket {
        env.storage().instance()
            .get(&DataKey::HardAssetBasket)
            .unwrap_or(HardAssetBasket {
                gold_weight_bps: DEFAULT_HARD_ASSET_GOLD_WEIGHT,
                btc_weight_bps: DEFAULT_HARD_ASSET_BTC_WEIGHT,
                silver_weight_bps: DEFAULT_HARD_ASSET_SILVER_WEIGHT,
                total_weight_bps: 10000,
            })
    }
    
    fn check_price_drop_and_trigger_swap(env: Env, circle_id: u64) -> bool {
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        
        // Get current asset price
        let current_price_data: PriceOracleData = match env.storage().instance().get(&DataKey::PriceOracle(circle.token.clone())) {
            Some(data) => data,
            None => return false, // No oracle data available
        };
        
        // Calculate hard asset basket weighted price
        let basket = Self::get_hard_asset_basket(env.clone());
        
        // Get prices for hard assets (simplified - would need actual oracle feeds)
        // In production, this would query multiple oracle sources
        let gold_price: PriceOracleData = match env.storage().instance().get(&DataKey::PriceOracle(Address::from_str("GOLD_ASSET_ADDRESS")?)) {
            Some(data) => data,
            None => return false,
        };
        
        // Calculate if current asset dropped more than 20% against hard asset basket
        // Simplified calculation: compare current price to a baseline
        let price_drop_threshold = (current_price_data.current_price * PRICE_DROP_THRESHOLD_BPS as i128) / 10000;
        
        // This is simplified - in production would compare against historical baseline
        let current_price = current_price_data.current_price;
        let threshold_price = price_drop_threshold; // 20% drop from some baseline
        
        if current_price < threshold_price {
            // Price drop detected - auto-trigger a swap proposal
            let target_asset = Address::from_str("STABLE_ASSET_ADDRESS").expect("Invalid address"); // Would be configurable
            Self::propose_asset_swap(env.clone(), circle.creator.clone(), circle_id, target_asset, 10000);
            return true;
        }
        
        false
    }
    
    fn propose_asset_swap(env: Env, user: Address, circle_id: u64, target_asset: Address, swap_percentage_bps: u32) {
        user.require_auth();
        
        if swap_percentage_bps > 10000 {
            panic!("Swap percentage cannot exceed 100%");
        }
        
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        
        // Only circle creator or members can propose
        let mut is_member = false;
        for i in 0..circle.member_count {
            if circle.member_addresses.get(i).unwrap() == user {
                is_member = true;
                break;
            }
        }
        
        if !is_member && user != circle.creator {
            panic!("Unauthorized: only circle members can propose asset swap");
        }
        
        // Get current asset price to calculate price drop
        let current_price_data: PriceOracleData = match env.storage().instance().get(&DataKey::PriceOracle(circle.token.clone())) {
            Some(data) => data,
            None => panic!("Current asset price not found"),
        };
        
        // Calculate price drop percentage (simplified)
        let price_drop_bps = 2000; // Would be calculated from historical data
        
        let proposal = AssetSwapProposal {
            circle_id,
            proposer: user.clone(),
            current_asset: circle.token.clone(),
            target_asset,
            swap_percentage_bps,
            price_drop_percentage_bps: price_drop_bps,
            created_timestamp: env.ledger().timestamp(),
            voting_deadline: env.ledger().timestamp() + ASSET_SWAP_VOTING_PERIOD,
            status: ProposalStatus::Active,
            for_votes: 0,
            against_votes: 0,
            total_votes_cast: 0,
            executed_timestamp: None,
        };
        
        env.storage().instance().set(&DataKey::AssetSwapProposal(circle_id), &proposal);
        
        env.events().publish(
            (Symbol::new(&env, "ASSET_SWAP_PROPOSED"), circle_id),
            (user, target_asset, swap_percentage_bps),
        );
    }
    
    fn vote_asset_swap(env: Env, user: Address, circle_id: u64, vote_choice: QuadraticVoteChoice) {
        user.require_auth();
        
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        
        let mut proposal: AssetSwapProposal = env.storage().instance()
            .get(&DataKey::AssetSwapProposal(circle_id))
            .expect("Asset swap proposal not found");
        
        if proposal.status != ProposalStatus::Active {
            panic!("Proposal is not active");
        }
        
        if env.ledger().timestamp() > proposal.voting_deadline {
            panic!("Voting period has ended");
        }
        
        // Check if user is a circle member
        let mut is_member = false;
        for i in 0..circle.member_count {
            if circle.member_addresses.get(i).unwrap() == user {
                is_member = true;
                break;
            }
        }
        
        if !is_member {
            panic!("Only circle members can vote");
        }
        
        // Prevent duplicate voting
        let vote_key = DataKey::AssetSwapVote(circle_id, user.clone());
        if env.storage().instance().contains(&vote_key) {
            panic!("Already voted on this proposal");
        }
        
        // Record vote (simple 1 member = 1 vote for now, could use quadratic voting)
        let vote_weight = 1u32;
        
        match vote_choice {
            QuadraticVoteChoice::For => proposal.for_votes += vote_weight,
            QuadraticVoteChoice::Against => proposal.against_votes += vote_weight,
            QuadraticVoteChoice::Abstain => { /* Abstain doesn't count */ }
        }
        
        proposal.total_votes_cast += vote_weight;
        
        // Store vote record
        let vote_record = (vote_choice, env.ledger().timestamp());
        env.storage().instance().set(&vote_key, &vote_record);
        env.storage().instance().set(&DataKey::AssetSwapProposal(circle_id), &proposal);
        
        env.events().publish(
            (Symbol::new(&env, "ASSET_SWAP_VOTE"), circle_id),
            (user, vote_choice),
        );
    }
    
    fn execute_asset_swap(env: Env, circle_id: u64) {
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        
        let mut proposal: AssetSwapProposal = env.storage().instance()
            .get(&DataKey::AssetSwapProposal(circle_id))
            .expect("Asset swap proposal not found");
        
        if proposal.status != ProposalStatus::Active {
            panic!("Proposal is not active");
        }
        
        if env.ledger().timestamp() <= proposal.voting_deadline {
            panic!("Voting period has not ended");
        }
        
        // Check quorum
        let participation_bps = (proposal.total_votes_cast as u32 * 10_000) / circle.member_count;
        if participation_bps < ASSET_SWAP_QUORUM {
            proposal.status = ProposalStatus::Rejected;
            env.storage().instance().set(&DataKey::AssetSwapProposal(circle_id), &proposal);
            panic!("Quorum not met");
        }
        
        // Check majority
        let approval_bps = if proposal.total_votes_cast > 0 {
            (proposal.for_votes * 10_000) / proposal.total_votes_cast
        } else {
            0
        };
        
        if approval_bps < ASSET_SWAP_MAJORITY {
            proposal.status = ProposalStatus::Rejected;
            env.storage().instance().set(&DataKey::AssetSwapProposal(circle_id), &proposal);
            panic!("Majority not reached");
        }
        
        // Execute the swap
        proposal.status = ProposalStatus::Executed;
        proposal.executed_timestamp = Some(env.ledger().timestamp());
        
        // Update circle's token to the new asset
        let mut updated_circle = circle;
        updated_circle.token = proposal.target_asset.clone();
        env.storage().instance().set(&DataKey::Circle(circle_id), &updated_circle);
        
        // In production, would actually perform the token swap via DEX
        // For now, we just update the accounting
        
        env.storage().instance().set(&DataKey::AssetSwapProposal(circle_id), &proposal);
        
        env.events().publish(
            (Symbol::new(&env, "ASSET_SWAP_EXECUTED"), circle_id),
            (proposal.current_asset, proposal.target_asset, proposal.swap_percentage_bps),
        );
    }

    fn propose_yield_delegation(env: Env, user: Address, circle_id: u64, delegation_percentage: u32, strategy_address: Address, strategy_type: StrategyType) {
        user.require_auth();

        if delegation_percentage > MAX_DELEGATION_PERCENTAGE {
            panic!("Delegation percentage exceeds maximum");
        }

        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id))
            .expect("Circle not found");
        
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key)
            .expect("User is not a member");

        if member.status != MemberStatus::Active {
            panic!("Member is not active");
        }

        // Check if there's already an active yield delegation proposal
        let delegation_key = DataKey::YieldDelegation(circle_id);
        if let Some(existing_delegation) = env.storage().instance().get::<DataKey, YieldDelegation>(&delegation_key) {
            if existing_delegation.status == YieldDelegationStatus::Voting || 
               existing_delegation.status == YieldDelegationStatus::Active {
                panic!("Yield delegation already active");
            }
        }

        // Only allow yield delegation after round is finalized but before payout
        if !circle.is_round_finalized {
            panic!("Round must be finalized before yield delegation");
        }

        let current_time = env.ledger().timestamp();
        let pot_amount = circle.contribution_amount * (circle.member_count as i128);
        let delegation_amount = (pot_amount * delegation_percentage as i128) / 10000;

        if delegation_amount < MIN_DELEGATION_AMOUNT {
            panic!("Delegation amount below minimum");
        }

        // Validate the yield strategy before proposing
        let strategy_client = YieldStrategyClient::new(&env, &strategy_address);
        let strategy_info = strategy_client.get_strategy_info();
        if !strategy_info.is_active {
            panic!("Yield strategy is not active");
        }

        let yield_delegation = YieldDelegation {
            circle_id,
            delegation_amount,
            strategy_address: strategy_address.clone(),
            strategy_type: strategy_type.clone(),
            delegation_percentage,
            created_timestamp: current_time,
            status: YieldDelegationStatus::Voting,
            voting_deadline: current_time + YIELD_VOTING_PERIOD,
            for_votes: 0,
            against_votes: 0,
            total_votes_cast: 0,
            start_time: None,
            end_time: None,
            total_yield_earned: 0,
            yield_distributed: 0,
            last_compound_time: current_time,
            strategy_info: None,
        };

        env.storage().instance().set(&delegation_key, &yield_delegation);
        
        // The proposer automatically votes for
        let vote_key = DataKey::YieldVote(circle_id, user.clone());
        let vote = YieldVote {
            voter: user.clone(),
            circle_id,
            vote_choice: YieldVoteChoice::For,
            timestamp: current_time,
        };
        env.storage().instance().set(&vote_key, &vote);

        // Update vote counts
        let mut updated_delegation = yield_delegation;
        updated_delegation.for_votes = 1;
        updated_delegation.total_votes_cast = 1;
        env.storage().instance().set(&delegation_key, &updated_delegation);

        write_audit(&env, &user, AuditAction::DisputeSubmission, circle_id);

        env.events().publish(
            (Symbol::new(&env, "YIELD_DELEGATION_PROPOSED"), circle_id, user.clone()),
            (delegation_amount, delegation_percentage, strategy_address, updated_delegation.voting_deadline),
        );
    }

    fn vote_yield_delegation(env: Env, user: Address, circle_id: u64, vote_choice: YieldVoteChoice) {
        user.require_auth();

        let delegation_key = DataKey::YieldDelegation(circle_id);
        let mut delegation: YieldDelegation = env.storage().instance().get(&delegation_key)
            .expect("No active yield delegation proposal");

        if delegation.status != YieldDelegationStatus::Voting {
            panic!("Yield delegation is not in voting period");
        }

        if env.ledger().timestamp() > delegation.voting_deadline {
            delegation.status = YieldDelegationStatus::Rejected;
            env.storage().instance().set(&delegation_key, &delegation);
            panic!("Voting period has expired");
        }

        // Check if user is an active member
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key)
            .expect("User is not a member");

        if member.status != MemberStatus::Active {
            panic!("Member is not active");
        }

        let token_client = token::Client::new(&env, &circle.token);

        // Try transfer from user
        let transfer_result = token_client.try_transfer(&user, &env.current_contract_address(), &total_amount);
        let transfer_success = match transfer_result {
            Ok(inner) => inner.is_ok(),
            Err(_) => false,
        };

        if !transfer_success {
            // Buddy fallback
            if let Some(buddy_addr) = &member.buddy {
                let safety_key = DataKey::SafetyDeposit(buddy_addr.clone(), circle_id);
                let safety_balance: i128 = env.storage().instance().get(&safety_key).unwrap_or(0);
                if safety_balance >= total_amount {
                    env.storage().instance().set(&safety_key, &(safety_balance - total_amount));
                } else {
                    panic!("Insufficient funds and buddy deposit");
                }
            } else {
                panic!("Insufficient funds");
            }
        }

        if insurance_fee > 0 {
            circle.insurance_balance += insurance_fee;
        }

        // Get strategy info to validate it's active and get current APY
        let strategy_client = YieldStrategyClient::new(&env, &delegation.strategy_address);
        let strategy_info = strategy_client.get_strategy_info();
        
        if !strategy_info.is_active {
            panic!("Yield strategy is not active");
        }

        // Execute the delegation
        execute_yield_delegation_internal(&env, circle_id, &mut delegation);

        env.storage().instance().set(&delegation_key, &delegation);
        write_audit(&env, &env.current_contract_address(), AuditAction::AdminAction, circle_id);

        env.events().publish(
            (Symbol::new(&env, "YIELD_DELEGATION_APPROVED"), circle_id),
            (delegation.delegation_amount, delegation.strategy_address),
        );
    }

    fn execute_yield_delegation(env: Env, circle_id: u64) {
        // Check circuit breaker status before executing delegation
        let circuit_breaker_status = YieldOracleCircuitBreaker::get_circuit_breaker_status(&env);
        if circuit_breaker_status.status == yield_oracle_circuit_breaker::CircuitBreakerStatus::Triggered ||
           circuit_breaker_status.status == yield_oracle_circuit_breaker::CircuitBreakerStatus::EmergencyUnwind {
            panic!("Cannot execute yield delegation - circuit breaker is active");
        }

        let delegation_key = DataKey::YieldDelegation(circle_id);
        let mut delegation: YieldDelegation = env.storage().instance().get(&delegation_key)
            .expect("No yield delegation found");

        if delegation.status != YieldDelegationStatus::Approved && delegation.status != YieldDelegationStatus::Active {
            panic!("Yield delegation is not approved");
        }

        execute_yield_delegation_internal(&env, circle_id, &mut delegation);
        
        // Register the yield strategy for circuit breaker monitoring
        let initial_metrics = HealthMetrics {
            current_apy: 500, // Default 5% APY
            volatility_index: 1000, // Default 10% volatility
            liquidity_ratio: 8000, // Default 80% liquidity
            price_impact_score: 500, // Default 5% price impact
            yield_rate: 500, // Positive yield rate
            last_updated: env.ledger().timestamp(),
            is_healthy: true,
        };
        
        YieldOracleCircuitBreaker::register_yield_strategy(&env, env.current_contract_address(), delegation.strategy_address.clone(), initial_metrics);
        
        env.storage().instance().set(&delegation_key, &delegation);

        env.events().publish(
            (Symbol::new(&env, "YIELD_DELEGATION_EXECUTED"), circle_id),
            (delegation.delegation_amount, delegation.strategy_address),
        );
    }

    fn compound_yield(env: Env, circle_id: u64) {
        let delegation_key = DataKey::YieldDelegation(circle_id);
        let mut delegation: YieldDelegation = env.storage().instance().get(&delegation_key)
            .expect("No yield delegation found");

        if delegation.status != YieldDelegationStatus::Active {
            panic!("Yield delegation is not active");
        }

        let current_time = env.ledger().timestamp();
        if current_time < delegation.last_compound_time + YIELD_COMPOUNDING_FREQUENCY {
            panic!("Too early to compound");
        }

        // Calculate yield (simplified - would query actual yield from pool)
        let time_elapsed = current_time - delegation.last_compound_time;
        let yield_earned = calculate_yield_from_pool(&env, &delegation, time_elapsed);

        delegation.total_yield_earned += yield_earned;
        delegation.last_compound_time = current_time;

        env.storage().instance().set(&delegation_key, &delegation);

        env.events().publish(
            (Symbol::new(&env, "YIELD_COMPOUNDED"), circle_id),
            (yield_earned, delegation.total_yield_earned),
        );
    }

    fn withdraw_yield_delegation(env: Env, circle_id: u64) {
        let delegation_key = DataKey::YieldDelegation(circle_id);
        let mut delegation: YieldDelegation = env.storage().instance().get(&delegation_key)
            .expect("No yield delegation found");

        if delegation.status != YieldDelegationStatus::Active {
            panic!("Yield delegation is not active");
        }

        // Final compound before withdrawal
        let current_time = env.ledger().timestamp();
        let time_elapsed = current_time - delegation.last_compound_time;
        let final_yield = calculate_yield_from_pool(&env, &delegation, time_elapsed);
        delegation.total_yield_earned += final_yield;

        // Withdraw from yield strategy using the abstract interface
        let withdrawal_params = WithdrawalParams {
            amount: delegation.delegation_amount + delegation.total_yield_earned,
            force_withdrawal: false,
            claim_yield_only: false,
        };
        
        let strategy_client = YieldStrategyClient::new(&env, &delegation.strategy_address);
        let yield_info = strategy_client.withdraw(
            &env.current_contract_address(),
            &withdrawal_params,
        );
        
        let total_withdrawn = yield_info.current_balance;

        delegation.status = YieldDelegationStatus::Completed;
        delegation.end_time = Some(current_time);

        env.storage().instance().set(&delegation_key, &delegation);

        // Distribute earnings
        distribute_yield_earnings(env, circle_id);

        env.events().publish(
            (Symbol::new(&env, "YIELD_DELEGATION_WITHDRAWN"), circle_id),
            (total_withdrawn, delegation.total_yield_earned),
        );
    }

    // --- YIELD STRATEGY REGISTRY MANAGEMENT ---

    fn register_yield_strategy(env: Env, admin: Address, strategy_address: Address, strategy_type: StrategyType, config: YieldStrategyConfig) {
        admin.require_auth();
        
        // Verify admin authorization
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        
        // Validate strategy before registration
        let strategy_client = YieldStrategyClient::new(&env, &strategy_address);
        if !strategy_client.health_check() {
            panic!("Strategy health check failed");
        }
        
        let registry_key = DataKey::YieldStrategyRegistry;
        let mut registry: Vec<RegisteredStrategy> = env.storage().instance()
            .get(&registry_key)
            .unwrap_or(Vec::new(&env));
        
        // Check if strategy already registered
        for existing in registry.iter() {
            if existing.address == strategy_address {
                panic!("Strategy already registered");
            }
        }
        
        // Register new strategy
        let registered_strategy = RegisteredStrategy {
            address: strategy_address.clone(),
            strategy_type: strategy_type.clone(),
            config: config.clone(),
            registration_time: env.ledger().timestamp(),
            is_active: true,
        };
        
        registry.push_back(registered_strategy);
        env.storage().instance().set(&registry_key, &registry);
        
        env.events().publish(
            (Symbol::new(&env, "YIELD_STRATEGY_REGISTERED"),),
            (strategy_address, strategy_type),
        );
    }
    
    fn get_registered_strategies(env: Env) -> Vec<RegisteredStrategy> {
        let registry_key = DataKey::YieldStrategyRegistry;
        env.storage().instance()
            .get(&registry_key)
            .unwrap_or(Vec::new(&env))
    }
    
    fn set_default_yield_strategy(env: Env, admin: Address, strategy_address: Address) {
        admin.require_auth();
        
        // Verify admin authorization
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        
        // Verify strategy is registered
        let registry_key = DataKey::YieldStrategyRegistry;
        let registry: Vec<RegisteredStrategy> = env.storage().instance()
            .get(&registry_key)
            .unwrap_or(Vec::new(&env));
        
        let mut is_registered = false;
        for strategy in registry.iter() {
            if strategy.address == strategy_address && strategy.is_active {
                is_registered = true;
                break;
            }
        }
        
        if !is_registered {
            panic!("Strategy not found or not active");
        }
        
        env.storage().instance().set(&DataKey::ActiveYieldStrategy(0), &strategy_address);
        
        env.events().publish(
            (Symbol::new(&env, "DEFAULT_YIELD_STRATEGY_SET"),),
            (strategy_address,),
        );
    }
    
    fn get_default_yield_strategy(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::ActiveYieldStrategy(0))
    }

    fn distribute_yield_earnings(env: Env, circle_id: u64) {
        let delegation_key = DataKey::YieldDelegation(circle_id);
        let delegation: YieldDelegation = env.storage().instance().get(&delegation_key)
            .expect("No yield delegation found");

        if delegation.total_yield_earned <= delegation.yield_distributed {
            panic!("No new yield to distribute");
        }

        if circle.is_round_finalized {
            panic!("Round already finalized");
        }

        let expected_bitmap = (1u64 << circle.member_count) - 1;
        if circle.contribution_bitmap != expected_bitmap {
            panic!("Not all contributed");
        }

        // recipient is circle.current_recipient_index
        // We'll need a way to get member by index or store member addresses in circle.
        // For simplicity in this clean version, let's assume members are stored in a predictable way or we add member_addresses to CircleInfo.
        // Actually, let's use the bitmap and iterate to find the address if needed, or better, store it in storage under (circle_id, index)
    }

    fn claim_pot(env: Env, user: Address, circle_id: u64) {
        user.require_auth();
        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).expect("Circle not found");
        
        if !circle.is_round_finalized {
            panic!("Round not finalized");
        }

        if let Some(recipient) = &circle.current_pot_recipient {
            if user != *recipient {
                panic!("Unauthorized recipient");
            }
        } else {
            panic!("No recipient set");
        }

        let scheduled_time: u64 = env.storage().instance().get(&DataKey::ScheduledPayoutTime(circle_id)).expect("Payout not scheduled");
        if env.ledger().timestamp() < scheduled_time {
            panic!("Payout too early");
        }

        let pot_amount = circle.contribution_amount * (circle.member_count as i128);
        let token_client = token::Client::new(&env, &circle.token);
        token_client.transfer(&env.current_contract_address(), &user, &pot_amount);

        // Auto-release collateral and reward RI if member has completed all contributions
        let member_key = DataKey::Member(user.clone());
        if let Some(member_info) = env.storage().instance().get::<DataKey, Member>(&member_key) {
            if member_info.contribution_count >= circle.max_members {
                // Reward RI
                let mut ri = Self::get_ri_internal(&env, &user);
                ri.successful_cycles = ri.successful_cycles.saturating_add(1);
                ri.points = (ri.points + 50).min(MAX_RI as u16); // +50 points for success
                ri.last_update = env.ledger().timestamp();
                Self::update_ri_internal(&env, &user, ri);

                if circle.requires_collateral {
                    let collateral_key = DataKey::CollateralVault(user.clone(), circle_id);
                    if let Some(mut collateral_info) = env.storage().instance().get::<DataKey, CollateralInfo>(&collateral_key) {
                        if collateral_info.status == CollateralStatus::Staked {
                            // Release collateral back to member
                            token_client.transfer(&env.current_contract_address(), &user, &collateral_info.amount);
                            
                            // Update collateral status
                            collateral_info.status = CollateralStatus::Released;
                            collateral_info.release_timestamp = Some(env.ledger().timestamp());
                            env.storage().instance().set(&collateral_key, &collateral_info);
                        }
                    }
                }
            }
        }

        // Reset for next round
        circle.is_round_finalized = false;
        circle.contribution_bitmap = 0;
        circle.is_insurance_used = false;
        circle.current_recipient_index = (circle.current_recipient_index + 1) % circle.member_count;
        circle.current_pot_recipient = None; // Should be set in finalize_round

        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
        env.storage().instance().remove(&DataKey::ScheduledPayoutTime(circle_id));
    }
    fn finalize_leniency_vote(env: Env, caller: Address, cid: u64, req: Address) { caller.require_auth(); let mut r: LeniencyRequest = env.storage().instance().get(&DataKey::K2(symbol_short!("LenR"), cid, req.clone())).unwrap(); r.status = LeniencyRequestStatus::Approved; env.storage().instance().set(&DataKey::K2(symbol_short!("LenR"), cid, req), &r); }
    fn get_leniency_request(env: Env, cid: u64, req: Address) -> LeniencyRequest { env.storage().instance().get(&DataKey::K2(symbol_short!("LenR"), cid, req)).unwrap() }
    fn get_social_capital(env: Env, m: Address, cid: u64) -> SocialCapital { env.storage().instance().get(&DataKey::K2(symbol_short!("Cap"), cid, m.clone())).unwrap_or(SocialCapital { member: m, circle_id: cid, leniency_given: 0, leniency_received: 0, voting_participation: 0, trust_score: 50 }) }
    fn create_proposal(env: Env, prop: Address, cid: u64, pt: ProposalType, title: String, desc: String, ed: String) -> u64 { prop.require_auth(); let id = 1u64; env.storage().instance().set(&DataKey::K1(symbol_short!("Prop"), id), &Proposal { id, circle_id: cid, proposer: prop, proposal_type: pt, title, description: desc, created_timestamp: env.ledger().timestamp(), voting_start_timestamp: env.ledger().timestamp(), voting_end_timestamp: env.ledger().timestamp() + 86400, status: ProposalStatus::Active, for_votes: 0, against_votes: 0, total_voting_power: 0, quorum_met: false, execution_data: ed }); id }
    fn quadratic_vote(env: Env, voter: Address, pid: u64, weight: u32, vc: QuadraticVoteChoice) {
        voter.require_auth();
        let mut p: Proposal = env.storage().instance().get(&DataKey::K1(symbol_short!("Prop"), pid)).unwrap();
        let mut vp: VotingPower = env.storage().instance().get(&DataKey::K2(symbol_short!("Vote"), p.circle_id, voter.clone())).unwrap();
        let cost = (weight as u64) * (weight as u64);
        if vp.quadratic_power < cost { panic!("Insufficient voting power"); }
        vp.quadratic_power -= cost;
        env.storage().instance().set(&DataKey::K2(symbol_short!("Vote"), p.circle_id, voter), &vp);
        match vc {
            QuadraticVoteChoice::For => p.for_votes += cost,
            QuadraticVoteChoice::Against => p.against_votes += cost,
            QuadraticVoteChoice::Abstain => {}
        }
        env.storage().instance().set(&DataKey::K1(symbol_short!("Prop"), pid), &p);
        
        // Update voter's reputation for governance participation
        let mut voter_metrics = env.storage().instance().get(&DataKey::K1A(symbol_short!("URep"), voter.clone())).unwrap_or(UserReputationMetrics {
            reliability_score: 5000, social_capital_score: 5000, total_cycles: 0, perfect_cycles: 0, total_volume_saved: 0, last_activity: env.ledger().timestamp(), last_decay: env.ledger().timestamp(), on_time_contributions: 0, total_contributions: 0,
        });
        voter_metrics.social_capital_score = (voter_metrics.social_capital_score + 10).min(10000);
        voter_metrics.last_activity = env.ledger().timestamp();
        env.storage().instance().set(&DataKey::K1A(symbol_short!("URep"), voter), &voter_metrics);
    }
    fn execute_proposal(env: Env, caller: Address, pid: u64) {
        caller.require_auth();
        let mut p: Proposal = env.storage().instance().get(&DataKey::K1(symbol_short!("Prop"), pid)).unwrap();
        p.status = ProposalStatus::Approved;
        env.storage().instance().set(&DataKey::K1(symbol_short!("Prop"), pid), &p);
    }
    fn get_proposal(env: Env, pid: u64) -> Proposal { env.storage().instance().get(&DataKey::K1(symbol_short!("Prop"), pid)).unwrap() }
    fn get_voting_power(env: Env, m: Address, cid: u64) -> VotingPower { env.storage().instance().get(&DataKey::K2(symbol_short!("Vote"), cid, m)).unwrap() }
    fn update_voting_power(env: Env, u: Address, cid: u64, bal: i128) { let pwr = if bal > 0 { 100 + (bal / 10000) as u64 } else { 100 }; let vp = VotingPower { member: u.clone(), circle_id: cid, token_balance: bal, quadratic_power: pwr, last_updated: env.ledger().timestamp() }; env.storage().instance().set(&DataKey::K2(symbol_short!("Vote"), cid, u), &vp); }
    fn stake_collateral(env: Env, u: Address, cid: u64, amt: i128) { u.require_auth(); let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); token::Client::new(&env, &c.token).transfer(&u, &env.current_contract_address(), &amt); let i = CollateralInfo { member: u.clone(), circle_id: cid, amount: amt, status: CollateralStatus::Staked, staked_timestamp: env.ledger().timestamp(), release_timestamp: None }; env.storage().instance().set(&DataKey::K2(symbol_short!("Vlt"), cid, u), &i); }
    fn slash_collateral(env: Env, _caller: Address, cid: u64, m: Address) { let mut i: CollateralInfo = env.storage().instance().get(&DataKey::K2(symbol_short!("Vlt"), cid, m.clone())).unwrap(); i.status = CollateralStatus::Slashed; env.storage().instance().set(&DataKey::K2(symbol_short!("Vlt"), cid, m), &i); }
    fn release_collateral(env: Env, _caller: Address, cid: u64, m: Address) { let mut i: CollateralInfo = env.storage().instance().get(&DataKey::K2(symbol_short!("Vlt"), cid, m.clone())).unwrap(); let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); token::Client::new(&env, &c.token).transfer(&env.current_contract_address(), &m, &i.amount); i.status = CollateralStatus::Released; env.storage().instance().set(&DataKey::K2(symbol_short!("Vlt"), cid, m), &i); }
    fn mark_member_defaulted(env: Env, caller: Address, cid: u64, m: Address) { caller.require_auth(); let mut mem: Member = env.storage().instance().get(&DataKey::K2(symbol_short!("M"), cid, m.clone())).unwrap(); mem.status = MemberStatus::Defaulted; env.storage().instance().set(&DataKey::K2(symbol_short!("M"), cid, m.clone()), &mem); env.storage().instance().set(&DataKey::K1A(symbol_short!("Mem"), m), &mem); }
    fn get_audit_entry(env: Env, id: u64) -> AuditEntry { env.storage().instance().get(&DataKey::K1(symbol_short!("AudE"), id)).unwrap() }
    fn query_audit_by_actor(env: Env, actor: Address, s: u64, e: u64, _o: u32, _l: u32) -> Vec<AuditEntry> { let count: u64 = env.storage().instance().get(&symbol_short!("AudCnt")).unwrap_or(0); let mut res = Vec::new(&env); for i in 1..=count { if let Some(ent) = env.storage().instance().get::<DataKey, AuditEntry>(&DataKey::K1(symbol_short!("AudE"), i)) { if ent.actor == actor && ent.timestamp >= s && ent.timestamp <= e { res.push_back(ent); } } } res }
    fn query_audit_by_resource(env: Env, rid: u64, s: u64, e: u64, _o: u32, _l: u32) -> Vec<AuditEntry> { let count: u64 = env.storage().instance().get(&symbol_short!("AudCnt")).unwrap_or(0); let mut res = Vec::new(&env); for i in 1..=count { if let Some(ent) = env.storage().instance().get::<DataKey, AuditEntry>(&DataKey::K1(symbol_short!("AudE"), i)) { if ent.resource_id == rid && ent.timestamp >= s && ent.timestamp <= e { res.push_back(ent); } } } res }
    fn query_audit_by_time(env: Env, s: u64, e: u64, _o: u32, _l: u32) -> Vec<AuditEntry> { let count: u64 = env.storage().instance().get(&symbol_short!("AudCnt")).unwrap_or(0); let mut res = Vec::new(&env); for i in 1..=count { if let Some(ent) = env.storage().instance().get::<DataKey, AuditEntry>(&DataKey::K1(symbol_short!("AudE"), i)) { if ent.timestamp >= s && ent.timestamp <= e { res.push_back(ent); } } } res }
    fn set_leaseflow_contract(env: Env, adm: Address, rot: Address) { adm.require_auth(); env.storage().instance().set(&DataKey::K(symbol_short!("LRot")), &rot); }
    fn authorize_leaseflow_payout(env: Env, u: Address, cid: u64, li: Address) { u.require_auth(); env.storage().instance().set(&DataKey::K2(symbol_short!("LAuth"), cid, u), &li); }
    fn handle_leaseflow_default(env: Env, rot: Address, ten: Address, cid: u64) { rot.require_auth(); env.storage().instance().set(&DataKey::K2(symbol_short!("LDef"), cid, ten), &true); }
    fn claim_pot(env: Env, u: Address, cid: u64) {
        u.require_auth();
        let mut c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap();
        if let Some(oracle) = env.storage().instance().get::<DataKey, Address>(&DataKey::K(symbol_short!("Oracle"))) {
            let is_sanctioned: bool = env.invoke_contract(&oracle, &Symbol::new(&env, "is_sanctioned"), Vec::from_array(&env, [u.clone().into_val(&env)]));
            if is_sanctioned {
                let pot = c.contribution_amount * (c.member_count as i128);
                env.storage().instance().set(&DataKey::K1(symbol_short!("Froze"), cid), &(pot, Some(u)));
                return;
            }
        }
        if env.storage().instance().get::<DataKey, bool>(&DataKey::K2(symbol_short!("LDef"), cid, u.clone())).unwrap_or(false) {
            panic!("locked due to a default");
        }

        let mut recipient = u.clone();
        if let Some(auth_recipient) = env.storage().instance().get::<DataKey, Address>(&DataKey::K2(symbol_short!("LAuth"), cid, u.clone())) {
            recipient = auth_recipient;
        }

        token::Client::new(&env, &c.token).transfer(&env.current_contract_address(), &recipient, &(c.contribution_amount * (c.member_count as i128)));
        
        // Update recipient's volume saved for reputation
        let payout_amount = c.contribution_amount * (c.member_count as i128);
        let mut metrics = env.storage().instance().get(&DataKey::K1A(symbol_short!("URep"), recipient.clone())).unwrap_or(UserReputationMetrics {
            reliability_score: 5000, social_capital_score: 5000, total_cycles: 0, perfect_cycles: 0, total_volume_saved: 0, last_activity: env.ledger().timestamp(), last_decay: env.ledger().timestamp(), on_time_contributions: 0, total_contributions: 0,
        });
        metrics.total_volume_saved += payout_amount;
        metrics.last_activity = env.ledger().timestamp();
        env.storage().instance().set(&DataKey::K1A(symbol_short!("URep"), recipient), &metrics);
        
        c.is_active = false;
        env.storage().instance().set(&DataKey::K1(symbol_short!("C"), cid), &c);
    }
    fn finalize_round(env: Env, u: Address, cid: u64) { u.require_auth(); let mut c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); c.is_round_finalized = true; c.current_pot_recipient = Some(u); env.storage().instance().set(&DataKey::K1(symbol_short!("C"), cid), &c); }
    fn configure_batch_payout(env: Env, creator: Address, cid: u64, winners: u32) { creator.require_auth(); let mut c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); c.winners_per_round = winners; c.batch_payout_enabled = true; env.storage().instance().set(&DataKey::K1(symbol_short!("C"), cid), &c); }
    fn distribute_batch_payout(env: Env, caller: Address, cid: u64) {
        caller.require_auth();
        let mut c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap();
        if c.winners_per_round == 0 { return; }
        
        let total_pot = c.contribution_amount * (c.member_count as i128);
        let amount_per_winner = total_pot / (c.winners_per_round as i128);
        
        let mut winners = Vec::new(&env);
        for i in 0..c.winners_per_round {
            if let Some(w) = c.member_addresses.get(i) {
                token::Client::new(&env, &c.token).transfer(&env.current_contract_address(), &w, &amount_per_winner);
                winners.push_back(w);
            }
        }
        
        let record = BatchPayoutRecord {
            batch_payout_id: 1, // Simple mock ID
            circle_id: cid,
            round_number: c.round_number,
            total_winners: c.winners_per_round,
            total_pot,
            organizer_fee: 0,
            net_payout_per_winner: amount_per_winner,
            dust_amount: total_pot % (c.winners_per_round as i128),
            winners: winners.clone(),
            payout_timestamp: env.ledger().timestamp(),
        };
        env.storage().instance().set(&DataKey::K2U(symbol_short!("BRec"), cid, c.round_number), &record);
        
        for w in winners.iter() {
            let claim = IndividualPayoutClaim {
                recipient: w.clone(),
                circle_id: cid,
                round_number: c.round_number,
                amount_claimed: amount_per_winner,
                batch_payout_id: 1,
                claim_timestamp: env.ledger().timestamp(),
            };
            env.storage().instance().set(&DataKey::K3U(symbol_short!("IClm"), w, cid, c.round_number), &claim);
        }
    }
    fn get_batch_payout_record(env: Env, cid: u64, rn: u32) -> Option<BatchPayoutRecord> { env.storage().instance().get(&DataKey::K2U(symbol_short!("BRec"), cid, rn)) }
    fn get_individual_payout_claim(env: Env, u: Address, cid: u64, rn: u32) -> Option<IndividualPayoutClaim> { env.storage().instance().get(&DataKey::K3U(symbol_short!("IClm"), u, cid, rn)) }
    fn get_circle(env: Env, cid: u64) -> CircleInfo { env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap() }
    fn get_member(env: Env, u: Address) -> Member { env.storage().instance().get(&DataKey::K1A(symbol_short!("Mem"), u)).unwrap() }
    fn get_basket_config(env: Env, cid: u64) -> Vec<AssetWeight> { env.storage().instance().get(&DataKey::K1(symbol_short!("Bsk"), cid)).unwrap() }
    fn register_anchor(env: Env, adm: Address, info: AnchorInfo) { adm.require_auth(); env.storage().instance().set(&DataKey::K1A(symbol_short!("Anch"), info.anchor_address.clone()), &info); }
    fn get_anchor_info(env: Env, a: Address) -> AnchorInfo { env.storage().instance().get(&DataKey::K1A(symbol_short!("Anch"), a)).unwrap() }
    fn deposit_for_user(env: Env, anc: Address, u: Address, cid: u64, amt: i128, mem: String, fiat: String, sep: String) {
        anc.require_auth();
        let mut m: Member = env.storage().instance().get(&DataKey::K2(symbol_short!("M"), cid, u.clone())).unwrap();
        let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap();
        token::Client::new(&env, &c.token).transfer(&anc, &env.current_contract_address(), &amt);
        m.has_contributed_current_round = true;
        m.total_contributions += amt;
        env.storage().instance().set(&DataKey::K2(symbol_short!("M"), cid, u.clone()), &m);
        env.storage().instance().set(&DataKey::K1A(symbol_short!("Mem"), u.clone()), &m);

        let id = 1u64;
        let record = AnchorDeposit {
            id,
            anchor_address: anc,
            beneficiary_user: u,
            circle_id: cid,
            amount: amt,
            deposit_memo: mem,
            fiat_reference: fiat,
            sep_type: sep,
            timestamp: env.ledger().timestamp(),
            processed: true,
            compliance_verified: true,
        };
        env.storage().instance().set(&DataKey::K1(symbol_short!("DRec"), id), &record);
    }
    fn get_deposit_record(env: Env, id: u64) -> AnchorDeposit { env.storage().instance().get(&DataKey::K1(symbol_short!("DRec"), id)).unwrap() }
    fn configure_dex_swap(env: Env, adm: Address, cid: u64, cfg: DexSwapConfig) { adm.require_auth(); env.storage().instance().set(&DataKey::K1(symbol_short!("DexC"), cid), &cfg); }
    fn trigger_dex_swap(env: Env, adm: Address, cid: u64) {
        adm.require_auth();
        let mut cfg: DexSwapConfig = env.storage().instance().get(&DataKey::K1(symbol_short!("DexC"), cid)).unwrap();
        cfg.total_swapped_xlm += cfg.swap_threshold_xlm;
        cfg.last_swap_timestamp = env.ledger().timestamp();
        env.storage().instance().set(&DataKey::K1(symbol_short!("DexC"), cid), &cfg);
        let record = DexSwapRecord { success: true, usdc_amount: 100_000_000, xlm_received: cfg.swap_threshold_xlm };
        env.storage().instance().set(&DataKey::K2U(symbol_short!("DexR"), cid, 0), &record);
    }
    fn get_dex_swap_config(env: Env, cid: u64) -> Option<DexSwapConfig> { env.storage().instance().get(&DataKey::K1(symbol_short!("DexC"), cid)) }
    fn get_dex_swap_record(env: Env, cid: u64, rid: u64) -> Option<DexSwapRecord> { env.storage().instance().get(&DataKey::K2U(symbol_short!("DexR"), cid, rid as u32)) }
    fn emergency_pause_dex_swaps(_env: Env, adm: Address) { adm.require_auth(); }
    fn emergency_refill_gas_reserve(_env: Env, adm: Address, _amt: i128) { adm.require_auth(); }
    fn get_gas_reserve(env: Env, cid: u64) -> Option<GasReserve> { env.storage().instance().get(&DataKey::K1(symbol_short!("GRes"), cid)) }
    fn distribute_payout(env: Env, caller: Address, cid: u64) { caller.require_auth(); let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); let total_pot = c.contribution_amount * (c.member_count as i128); let immediate_payout = (total_pot * 7000) / 10000; let tranche_total = total_pot - immediate_payout; let recipient = c.member_addresses.get(c.current_recipient_index).unwrap(); token::Client::new(&env, &c.token).transfer(&env.current_contract_address(), &recipient, &immediate_payout); let mut tranches = Vec::new(&env); tranches.push_back(Tranche { amount: tranche_total / 2, unlock_round: c.round_number + 2, status: TrancheStatus::Pending }); env.storage().instance().set(&DataKey::K2(symbol_short!("TrcS"), cid, recipient.clone()), &TrancheSchedule { circle_id: cid, winner: recipient, total_pot, immediate_payout, tranches }); }
    fn get_tranche_schedule(env: Env, cid: u64, winner: Address) -> Option<TrancheSchedule> { env.storage().instance().get(&DataKey::K2(symbol_short!("TrcS"), cid, winner)) }
    fn claim_tranche(env: Env, u: Address, cid: u64, _tid: u32) { u.require_auth(); let mut s: TrancheSchedule = env.storage().instance().get(&DataKey::K2(symbol_short!("TrcS"), cid, u.clone())).unwrap(); let mut tr = s.tranches.get(0).unwrap(); tr.status = TrancheStatus::Claimed; s.tranches.set(0, tr); env.storage().instance().set(&DataKey::K2(symbol_short!("TrcS"), cid, u), &s); }
    fn execute_tranche_clawback(env: Env, adm: Address, cid: u64, m: Address) { adm.require_auth(); let mut s: TrancheSchedule = env.storage().instance().get(&DataKey::K2(symbol_short!("TrcS"), cid, m.clone())).unwrap(); let mut tr = s.tranches.get(0).unwrap(); tr.status = TrancheStatus::ClawedBack; s.tranches.set(0, tr); env.storage().instance().set(&DataKey::K2(symbol_short!("TrcS"), cid, m), &s); }
    fn terminate_grant_amicably(env: Env, adm: Address, grant_id: u64, grantee: Address, total: i128, dur: u64, start: u64, _treasury: Address, _tok: Address) -> GrantSettlement { adm.require_auth(); let elapsed = env.ledger().timestamp() - start; let dripped = if elapsed >= dur { total } else { (total * (elapsed as i128)) / (dur as i128) }; GrantSettlement { grant_id, grantee, total_grant_amount: total, amount_dripped: dripped, work_in_progress_pay: dripped, treasury_return: total - dripped } }
    fn create_voting_snapshot_for_audit(env: Env, pid: u64, votes: Vec<(Address, u32, Symbol)>, q: u64) -> VotingSnapshot { let mut total = 0u32; for v in votes.iter() { total += v.1; } VotingSnapshot { proposal_id: pid, total_votes: total, for_votes: total, against_votes: 0, abstain_votes: 0, quorum_required: q as u32, quorum_met: (total as u64) >= q, result: symbol_short!("APPROVED"), vote_hash: BytesN::from_array(&env, &[0; 32]) } }
    fn get_voting_snapshot_for_audit(_env: Env, _pid: u64) -> Option<VotingSnapshot> { None }
    fn initialize_impact_certificate(_env: Env, _grantee: Address, _id: u128, _total: u32, _uri: String) {}
    fn update_milestone_progress(_env: Env, adm: Address, id: u128, new_phase: u32, impact: i128) -> ImpactCertificateMetadata { adm.require_auth(); ImpactCertificateMetadata { id, grantee: adm, total_phases: new_phase + 1, phases_completed: new_phase, impact_score: impact as u32, on_chain_badge: symbol_short!("Impact"), milestone_status: MilestoneProgress::InProgress } }
    fn get_progress_bar_data(env: Env, _id: u128) -> Option<Map<Symbol, String>> { let mut m = Map::new(&env); m.set(symbol_short!("progress"), String::from_str(&env, "50%")); Some(m) }
    fn set_sanctions_oracle(env: Env, adm: Address, oracle: Address) { adm.require_auth(); env.storage().instance().set(&DataKey::K(symbol_short!("Oracle")), &oracle); }
    fn set_pop_oracle(env: Env, adm: Address, oracle: Address) { adm.require_auth(); env.storage().instance().set(&DataKey::K(symbol_short!("PoP")), &oracle); }
    fn reveal_next_winner(env: Env, cid: u64) -> Address { let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap(); c.member_addresses.get(c.current_recipient_index).unwrap() }
    fn get_frozen_payout(env: Env, cid: u64) -> (i128, Option<Address>) { env.storage().instance().get(&DataKey::K1(symbol_short!("Froze"), cid)).unwrap_or((0, None)) }
    fn review_frozen_payout(env: Env, adm: Address, cid: u64, release: bool) {
        adm.require_auth();
        let frozen_key = DataKey::K1(symbol_short!("Froze"), cid);
        if let Some((amt, winner_opt)) = env.storage().instance().get::<DataKey, (i128, Option<Address>)>(&frozen_key) {
            if release {
                if let Some(winner) = winner_opt {
                    let c: CircleInfo = env.storage().instance().get(&DataKey::K1(symbol_short!("C"), cid)).unwrap();
                    token::Client::new(&env, &c.token).transfer(&env.current_contract_address(), &winner, &amt);
                }
            }
            env.storage().instance().remove(&frozen_key);
        }
    }

    fn get_proposal(env: Env, proposal_id: u64) -> Proposal {
        let proposal_key = DataKey::Proposal(proposal_id);
        env.storage().instance().get(&proposal_key).expect("Proposal not found")
    }

    fn get_voting_power(env: Env, member: Address, circle_id: u64) -> VotingPower {
        let voting_power_key = DataKey::VotingPower(member, circle_id);
        env.storage().instance().get(&voting_power_key).unwrap_or(VotingPower {
            member,
            circle_id,
            token_balance: 0,
            quadratic_power: 0,
            last_updated: 0,
        })
    }

    fn get_proposal_stats(env: Env, circle_id: u64) -> ProposalStats {
        let stats_key = DataKey::ProposalStats(circle_id);
        env.storage().instance().get(&stats_key).unwrap_or(ProposalStats {
            total_proposals: 0,
            approved_proposals: 0,
            rejected_proposals: 0,
            executed_proposals: 0,
            average_participation: 0,
            average_voting_time: 0,
        })
    }

    fn update_voting_power(env: Env, member: Address, circle_id: u64, token_balance: i128) {
        // Calculate quadratic voting power as sqrt(token_balance)
        // We use integer approximation: sqrt(x) ≈ x / (sqrt(x) + 1) for simplicity
        // In production, you'd use a proper sqrt implementation
        
        let ri = Self::get_ri_internal(&env, &member);
        
        let quadratic_power = if token_balance > 0 {
            // Formula: Tokens * (RI / 1000)
            // Use large enough intermediate values to avoid precision loss
            let weighted_balance = (token_balance * ri.points as i128) / 1000;
            let balance_u64 = weighted_balance as u64;
            (balance_u64 / 1000).max(1)
        } else {
            0
        };

        let voting_power = VotingPower {
            member: member.clone(),
            circle_id,
            token_balance,
            quadratic_power,
            last_updated: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::VotingPower(member, circle_id), &voting_power);
    }

    fn get_reliability_index(env: Env, member: Address) -> ReliabilityIndex {
        Self::get_ri_internal(&env, &member)
    }

    // Helper functions for internal RI management
    fn get_ri_internal(env: &Env, member: &Address) -> ReliabilityIndex {
        env.storage().instance().get(&DataKey::ReliabilityIndex(member.clone())).unwrap_or(ReliabilityIndex {
            points: MAX_RI as u16,
            successful_cycles: 0,
            default_count: 0,
            last_update: env.ledger().timestamp(),
        })
    }

    fn update_ri_internal(env: &Env, member: &Address, ri: ReliabilityIndex) {
        env.storage().instance().set(&DataKey::ReliabilityIndex(member.clone()), &ri);
    }

    // --- YIELD ORACLE CIRCUIT BREAKER IMPLEMENTATION ---

    fn initialize_circuit_breaker(env: Env, admin: Address, protected_vault: Address) {
        YieldOracleCircuitBreaker::initialize(env, admin, protected_vault);
    }

    fn update_circuit_breaker_config(env: Env, admin: Address, config: yield_oracle_circuit_breaker::CircuitBreakerConfig) {
        YieldOracleCircuitBreaker::update_config(env, admin, config);
    }

    fn register_amm_for_monitoring(env: Env, admin: Address, amm_address: Address, initial_metrics: HealthMetrics) {
        YieldOracleCircuitBreaker::register_amm(env, admin, amm_address, initial_metrics);
    }

    fn update_amm_health_metrics(env: Env, amm_address: Address, metrics: HealthMetrics) {
        YieldOracleCircuitBreaker::update_health_metrics(env, amm_address, metrics);
    }

    fn manual_trigger_circuit_breaker(env: Env, admin: Address, reason: String) {
        YieldOracleCircuitBreaker::manual_trigger_circuit_breaker(env, admin, reason);
    }

    fn emergency_unwind(env: Env, circle_id: u64, amm_address: Address) -> Result<(), yield_oracle_circuit_breaker::CircuitBreakerError> {
        YieldOracleCircuitBreaker::emergency_unwind(env, circle_id, amm_address)
    }

    fn get_circuit_breaker_status(env: Env) -> CircuitBreakerState {
        YieldOracleCircuitBreaker::get_circuit_breaker_status(env)
    }

    fn get_amm_health_metrics(env: Env, amm_address: Address) -> HealthMetrics {
        YieldOracleCircuitBreaker::get_health_metrics(env, amm_address)
    }

    fn reset_circuit_breaker(env: Env, admin: Address) {
        YieldOracleCircuitBreaker::reset_circuit_breaker(env, admin);
    }

    fn create_basket_circle(
        env: Env,
        creator: Address,
        amount: i128,
        max_members: u32,
        basket_assets: Vec<Address>,
        basket_weights: Vec<u32>,
        cycle_duration: u64,
        insurance_fee_bps: u32,
        nft_contract: Address,
        arbitrator: Address,
    ) -> u64 {
        creator.require_auth();

    fn stake_collateral(env: Env, user: Address, circle_id: u64, amount: i128) {
        user.require_auth();
        
        let circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).expect("Circle not found");
        
        if !circle.requires_collateral {
            panic!("Collateral not required for this circle");
        }

        let collateral_key = DataKey::CollateralVault(user.clone(), circle_id);
        
        // Check if collateral already staked
        if let Some(_collateral) = env.storage().instance().get::<DataKey, CollateralInfo>(&collateral_key) {
            panic!("Collateral already staked");
        }

        // Calculate required collateral amount
        let required_collateral = (circle.total_cycle_value * circle.collateral_bps as i128) / 10000;
        
        if amount < required_collateral {
            panic!("Insufficient collateral amount");
        }
    }
    fn update_reputation_on_deposit(env: Env, user: Address, was_on_time: bool) {
        // Check Proof of Personhood if oracle is set
        if let Some(pop_oracle) = env.storage().instance().get::<DataKey, Address>(&DataKey::K(symbol_short!("PoP"))) {
            let is_verified: bool = env.invoke_contract(&pop_oracle, &Symbol::new(&env, "is_verified"), Vec::from_array(&env, [user.clone().into_val(&env)]));
            if !is_verified {
                return; // Don't update reputation if not verified
            }
        }
        
        let mut metrics = env.storage().instance().get(&DataKey::K1A(symbol_short!("URep"), user.clone())).unwrap_or(UserReputationMetrics {
            reliability_score: 5000, social_capital_score: 5000, total_cycles: 0, perfect_cycles: 0, total_volume_saved: 0, last_activity: env.ledger().timestamp(), last_decay: env.ledger().timestamp(), on_time_contributions: 0, total_contributions: 0,
        });
        metrics.total_contributions += 1;
        if was_on_time { metrics.on_time_contributions += 1; }
        metrics.last_activity = env.ledger().timestamp();
        
        // Calculate reliability score
        let on_time_rate = if metrics.total_contributions > 0 { (metrics.on_time_contributions * 10000) / metrics.total_contributions } else { 5000 };
        let volume_bonus = ((metrics.total_volume_saved / 1000000).min(100) * 50) as u32;
        metrics.reliability_score = (on_time_rate as i128 + volume_bonus as i128).min(10000) as u32;
        
        env.storage().instance().set(&DataKey::K1A(symbol_short!("URep"), user), &metrics);
    }
    fn apply_inactivity_decay(env: Env, user: Address) {
        let mut metrics = env.storage().instance().get(&DataKey::K1A(symbol_short!("URep"), user.clone())).unwrap_or(UserReputationMetrics {
            reliability_score: 5000, social_capital_score: 5000, total_cycles: 0, perfect_cycles: 0, total_volume_saved: 0, last_activity: env.ledger().timestamp(), last_decay: env.ledger().timestamp(), on_time_contributions: 0, total_contributions: 0,
        });
        let months_inactive = (env.ledger().timestamp() - metrics.last_decay) / 2592000; // 30 days
        if months_inactive > 0 && env.ledger().timestamp() - metrics.last_activity > 15552000 { // 6 months
            let mut decay_factor = 10000u64;
            for _ in 0..months_inactive {
                decay_factor = (decay_factor * 95) / 100;
            }
            metrics.reliability_score = (metrics.reliability_score as u64 * decay_factor / 10000u64) as u32;
            metrics.social_capital_score = (metrics.social_capital_score as u64 * decay_factor / 10000u64) as u32;
            metrics.last_decay = env.ledger().timestamp();
            env.storage().instance().set(&DataKey::K1A(symbol_short!("URep"), user), &metrics);
        }
    }

    fn mark_member_defaulted(env: Env, caller: Address, circle_id: u64, member: Address) {
        caller.require_auth();
        
        let circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).expect("Circle not found");
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).expect("Not initialized");
        
        if caller != circle.creator && caller != stored_admin {
            panic!("Unauthorized");
        }

        let member_key = DataKey::Member(member.clone());
        let mut member_info: Member = env.storage().instance().get(&member_key).expect("Member not found");
        
        if member_info.status == MemberStatus::Defaulted {
            panic!("Member already defaulted");
        }

        // Mark member as defaulted
        member_info.status = MemberStatus::Defaulted;
        env.storage().instance().set(&member_key, &member_info);

        // Apply RI Penalty
        let mut ri = Self::get_ri_internal(&env, &member);
        ri.points = ri.points.saturating_sub(RI_PENALTY);
        ri.default_count += 1;
        ri.last_update = env.ledger().timestamp();
        Self::update_ri_internal(&env, &member, ri);

        // Report to external registries (Negative-Credit Reporting)
        let amount_stolen = circle.contribution_amount * (circle.member_count as i128); // Pot value
        Self::report_to_external_registries(&env, &member, symbol_short!("DEFAULT"), amount_stolen);

        // Add to defaulted members list
        let defaulted_key = DataKey::DefaultedMembers(circle_id);
        let mut defaulted_members: Vec<Address> = env.storage().instance().get(&defaulted_key).unwrap_or(Vec::new(&env));
        
        if !defaulted_members.contains(&member) {
            defaulted_members.push_back(member.clone());
            env.storage().instance().set(&defaulted_key, &defaulted_members);
        }

        // Auto-slash collateral if staked
        let collateral_key = DataKey::CollateralVault(member.clone(), circle_id);
        if let Some(_collateral) = env.storage().instance().get::<DataKey, CollateralInfo>(&collateral_key) {
            // Reuse slash_collateral logic
            Self::slash_collateral(env, caller, circle_id, member);
        }
    }

    fn appeal_penalty(env: Env, requester: Address, circle_id: u64, reason: String) {
        requester.require_auth();

        // Check if member is defaulted
        let member_key = DataKey::Member(requester.clone());
        let member_info: Member = env.storage().instance().get(&member_key).expect("Member not found");
        if member_info.status != MemberStatus::Defaulted {
            panic!("Only defaulted members can appeal");
        }

        let appeal_key = DataKey::ReputationAppeal(circle_id, requester.clone());
        if env.storage().instance().has(&appeal_key) {
            panic!("Appeal already exists");
        }

        let current_time = env.ledger().timestamp();
        let voting_deadline = current_time + VOTING_PERIOD;

        let appeal = ReputationAppeal {
            requester,
            circle_id,
            appeal_timestamp: current_time,
            voting_deadline,
            status: AppealStatus::Pending,
            for_votes: 0,
            against_votes: 0,
            reason,
        };

        env.storage().instance().set(&appeal_key, &appeal);
    }

    fn vote_on_appeal(env: Env, voter: Address, circle_id: u64, requester: Address, approve: bool) {
        voter.require_auth();

        let appeal_key = DataKey::ReputationAppeal(circle_id, requester.clone());
        let mut appeal: ReputationAppeal = env.storage().instance().get(&appeal_key).expect("Appeal not found");

        if appeal.status != AppealStatus::Pending {
            panic!("Appeal already finalized");
        }

        if env.ledger().timestamp() > appeal.voting_deadline {
            panic!("Voting period expired");
        }

        let vote_key = DataKey::AppealVotes(circle_id, requester.clone(), voter.clone());
        if env.storage().temporary().has(&vote_key) {
            panic!("Already voted");
        }

        // Must be a member of the same circle
        // (Simplified check: assume voter is a member if they can be found)
        let voter_key = DataKey::Member(voter.clone());
        let _voter_info: Member = env.storage().instance().get(&voter_key).expect("Voter not found");

        if approve {
            appeal.for_votes += 1;
        } else {
            appeal.against_votes += 1;
        }

        // Use temporary storage for votes to save on ledger rent for data that is only needed during voting
        env.storage().temporary().set(&vote_key, &approve);
        env.storage().instance().set(&appeal_key, &appeal);

        // Check for 2/3 majority
        let circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).expect("Circle not found");
        let total_voters = circle.member_count - 1; // Exclude requester
        let required_votes = (total_voters * REPUTATION_AMNESTY_THRESHOLD) / 100;

        if appeal.for_votes >= required_votes {
            appeal.status = AppealStatus::Approved;
            env.storage().instance().set(&appeal_key, &appeal);
            // Amnesty is auto-executed if majority reached
            Self::reputation_amnesty(env, voter, circle_id, requester);
        } else if appeal.against_votes > (total_voters - required_votes) {
            appeal.status = AppealStatus::Rejected;
            env.storage().instance().set(&appeal_key, &appeal);
        }
    }

fn execute_yield_delegation_internal(env: &Env, circle_id: u64, delegation: &mut YieldDelegation) {
    let current_time = env.ledger().timestamp();
    
    // Create deposit parameters for the yield strategy
    let deposit_params = DepositParams {
        amount: delegation.delegation_amount,
        min_apy_bps: Some(100), // Minimum 1% APY
        lockup_period: None,
        auto_compound: true,
    };
    
    // Execute deposit using the abstract yield strategy interface
    let strategy_client = YieldStrategyClient::new(env, &delegation.strategy_address);
    let yield_info = strategy_client.deposit(
        &env.current_contract_address(),
        &delegation.delegation_amount,
        &deposit_params,
    );
    
    // Update delegation with strategy info
    delegation.status = YieldDelegationStatus::Active;
    delegation.start_time = Some(current_time);
    delegation.last_compound_time = current_time;
    delegation.strategy_info = Some(yield_info);
}

fn calculate_yield_from_pool(env: &Env, delegation: &YieldDelegation, time_elapsed: u64) -> i128 {
    // Use the abstract yield strategy interface to get estimated yield
    let strategy_client = YieldStrategyClient::new(env, &delegation.strategy_address);
    let yield_estimate = strategy_client.get_estimated_yield(
        &delegation.delegation_amount,
        &time_elapsed,
    );
    
    yield_estimate.estimated_yield
}

    #[test]
    fn test_get_reputation() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        
        let token_contract = env.register_contract(None, MockToken);
        let nft_contract = env.register_contract(None, MockNft);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        env.mock_all_auths();
        client.init(&admin);
        
        // Test reputation for new user (should be zero/low)
        let reputation = client.get_reputation(&user);
        assert_eq!(reputation.susu_score, 0);
        assert_eq!(reputation.reliability_score, 0);
        assert_eq!(reputation.total_contributions, 0);
        assert_eq!(reputation.on_time_rate, 0);
        assert_eq!(reputation.volume_saved, 0);
        assert_eq!(reputation.is_active, false);
        
        // Create circle and add user
        let circle_id = client.create_circle(
            &creator,
            &1_000_000_000_000,
            &10,
            &token_contract,
            &86400,
            &100, // 1%
            &nft_contract,
            &arbitrator,
        );
        
        client.join_circle(&user, &circle_id, &1, &None);
        client.deposit(&user, &circle_id);
        
        // Test reputation after contribution
        let reputation = client.get_reputation(&user);
        assert!(reputation.susu_score > 0);
        assert!(reputation.reliability_score > 0);
        assert_eq!(reputation.total_contributions, 1);
        assert_eq!(reputation.on_time_rate, 10000); // 100% on-time rate
        assert_eq!(reputation.volume_saved, 1_000_000_000_000);
        assert_eq!(reputation.is_active, true);
    }

    #[test]
    fn test_credit_score_oracle() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        
        let token_contract = env.register_contract(None, MockToken);
        let nft_contract = env.register_contract(None, MockNft);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        env.mock_all_auths();
        client.init(&admin);
        
        // Start out unscored
        assert_eq!(client.get_user_reliability_score(&user), 0);

        let circle_id = client.create_circle(
            &creator,
            &1_000_000_000_000,
            &10,
            &token_contract,
            &86400,
            &100, // 1%
            &nft_contract,
            &arbitrator,
        );
        
        client.join_circle(&user, &circle_id, &1, &None);
        client.deposit(&user, &circle_id);

        // Should earn positive reliability
        let score = client.get_user_reliability_score(&user);
        assert!(score > 0);
        
        let stats = client.get_user_stats(&user);
        assert_eq!(stats.on_time_contributions, 1);
        assert_eq!(stats.late_contributions, 0);
        assert_eq!(stats.total_volume_saved, 1_000_000_000_000);
    }

    #[test]
    fn test_slash_user_credit() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        env.mock_all_auths();
        client.init(&admin);
        
        client.slash_user_credit(&admin, &user, &5);
        let stats = client.get_user_stats(&user);
        assert_eq!(stats.late_contributions, 5);
        assert_eq!(client.get_user_reliability_score(&user), 0);
    }

    #[test]
    fn test_cross_contract_oracle() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        
        let token_contract = env.register_contract(None, MockToken);
        let nft_contract = env.register_contract(None, MockNft);
        
        let oracle_id = env.register_contract(None, SoroSusu);
        let oracle_client = SoroSusuClient::new(&env, &oracle_id);
        
        let lending_id = env.register_contract(None, MockLending);
        let lending_client = MockLendingClient::new(&env, &lending_id);
        
        env.mock_all_auths();
        oracle_client.init(&admin);
        
        // Start out unscored, cannot borrow
        assert_eq!(lending_client.can_borrow(&oracle_id, &user), false);

        let circle_id = oracle_client.create_circle(
            &creator,
            &1_000_000_000_000,
            &10,
            &token_contract,
            &86400,
            &100, // 1%
            &nft_contract,
            &arbitrator,
        );
        
        oracle_client.join_circle(&user, &circle_id, &1, &None);
        oracle_client.deposit(&user, &circle_id);

        // After a successful on-time deposit, score surges past the 500 threshold
        assert_eq!(lending_client.can_borrow(&oracle_id, &user), true);
    }

    #[test]
    fn test_sub_susu_credit_line() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        
        let token_contract = env.register_contract(None, MockToken);
        let nft_contract = env.register_contract(None, MockNft);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        env.mock_all_auths();
        client.init(&admin);
        
        let circle_id = client.create_circle(&creator, &1000, &2, &token_contract, &86400, &100, &nft_contract, &arbitrator);
        
        client.join_circle(&creator, &circle_id, &1, &None);
        client.join_circle(&user, &circle_id, &1, &None);
        
        // Payout to creator first to establish history and boost user score
        client.deposit(&creator, &circle_id);
        client.deposit(&user, &circle_id);
        client.finalize_round(&creator, &circle_id);
        client.claim_pot(&creator, &circle_id);
        
        // Now user asks for credit advance. Expected payout = 2000. Limit is 1000.
        client.approve_credit_advance(&creator, &circle_id, &user, &1000);
        
        client.deposit(&creator, &circle_id);
        client.deposit(&user, &circle_id);
        client.finalize_round(&creator, &circle_id);
        client.claim_pot(&user, &circle_id); // debt is deducted seamlessly!
    }

    #[test]
    fn test_rollover_bonus_proposal_and_voting() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        
        let token_contract = env.register_contract(None, MockToken);
        let nft_contract = env.register_contract(None, MockNft);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        env.mock_all_auths();
        client.init(&admin);
        
        // Set up protocol fee for rollover bonus calculation
        client.set_protocol_fee(&admin, &100, &admin); // 1% fee
        
        // Create circle with 2 members
        let circle_id = client.create_circle(
            &creator,
            &1_000_000_000_000, // 1000 tokens
            &2,
            &token_contract,
            &86400,
            &100, // 1% insurance
            &nft_contract,
            &arbitrator,
        );
        
        client.join_circle(&creator, &circle_id, &1, &None);
        client.join_circle(&user1, &circle_id, &1, &None);
        
        // Complete first cycle
        client.deposit(&creator, &circle_id);
        client.deposit(&user1, &circle_id);
        client.finalize_round(&creator, &circle_id);
        client.claim_pot(&creator, &circle_id);
        
        // Start second cycle
        client.deposit(&creator, &circle_id);
        client.deposit(&user1, &circle_id);
        client.finalize_round(&creator, &circle_id);
        client.claim_pot(&user1, &circle_id);
        
        // Now propose rollover bonus (50% of platform fee)
        client.propose_rollover_bonus(&creator, &circle_id, &5000);
        
        // Second member votes for the rollover
        client.vote_rollover_bonus(&user1, &circle_id, &RolloverVoteChoice::For);
        
        // Apply the rollover bonus
        client.apply_rollover_bonus(&circle_id);
        
        // Start third cycle - first recipient should get rollover bonus
        client.deposit(&creator, &circle_id);
        client.deposit(&user1, &circle_id);
        client.finalize_round(&creator, &circle_id);
        
        // Check that rollover bonus is applied to payout
        let initial_balance = token_contract.mock_balance(&creator);
        client.claim_pot(&creator, &circle_id);
        let final_balance = token_contract.mock_balance(&creator);
        
        // Should receive regular pot (2000) minus fee (1% = 20) plus rollover bonus (50% of fee = 10)
        let expected_payout = 2000 - 20 + 10; // 1990
        assert_eq!(final_balance - initial_balance, expected_payout);
    }

        let appeal_key = DataKey::ReputationAppeal(circle_id, requester.clone());
        let appeal: ReputationAppeal = env.storage().instance().get(&appeal_key).expect("Appeal not found");

        if appeal.status != AppealStatus::Approved {
            panic!("Appeal not approved");
        }

        // Restore points
        let mut ri = Self::get_ri_internal(&env, &requester);
        ri.points = (ri.points + RI_RESTORE).min(MAX_RI);
        Self::update_ri_internal(&env, &requester, ri);

        // Mark member as active again
        let member_key = DataKey::Member(requester.clone());
        let mut member_info: Member = env.storage().instance().get(&member_key).expect("Member not found");
        member_info.status = MemberStatus::Active;
        env.storage().instance().set(&member_key, &member_info);

        // Remove from defaulted list
        let defaulted_key = DataKey::DefaultedMembers(circle_id);
        if let Some(mut defaulted_members) = env.storage().instance().get::<DataKey, Vec<Address>>(&defaulted_key) {
            let mut new_list = Vec::new(&env);
            for m in defaulted_members.iter() {
                if m != requester {
                    new_list.push_back(m);
                }
            }
            env.storage().instance().set(&defaulted_key, &new_list);
        }
    }
}
