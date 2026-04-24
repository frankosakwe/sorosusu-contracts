#![no_std]
use soroban_sdk::{
    contractclient, contracterror, contracttype,
    Address, Env, i128, u64, u32,
};

// --- ERROR CODES ---

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum YieldStrategyError {
    Unauthorized = 401,
    InsufficientBalance = 402,
    InvalidAmount = 403,
    StrategyNotActive = 404,
    WithdrawalNotAllowed = 405,
    DepositFailed = 406,
    WithdrawalFailed = 407,
    YieldCalculationFailed = 408,
    StrategyNotFound = 409,
    InvalidParameters = 410,
}

// --- DATA STRUCTURES ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldInfo {
    pub total_deposited: i128,
    pub current_balance: i128,
    pub total_yield_earned: i128,
    pub apy_bps: u32,           // APY in basis points (100 = 1%)
    pub last_updated: u64,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositParams {
    pub amount: i128,
    pub min_apy_bps: Option<u32>,     // Minimum acceptable APY
    pub lockup_period: Option<u64>,    // Optional lockup period in seconds
    pub auto_compound: bool,          // Enable automatic compounding
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawalParams {
    pub amount: i128,
    pub force_withdrawal: bool,       // Force withdrawal even if penalties apply
    pub claim_yield_only: bool,       // Only withdraw yield, keep principal
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldEstimate {
    pub estimated_yield: i128,        // Estimated yield for the period
    pub period_seconds: u64,          // Period for which yield is estimated
    pub confidence_score: u32,        // Confidence in estimate (0-10000 bps)
    pub last_calculated: u64,
}

// --- YIELD STRATEGY TRAIT ---

pub trait YieldStrategyTrait {
    /// Initialize the yield strategy with configuration
    fn initialize(env: Env, admin: Address, config: YieldStrategyConfig);
    
    /// Deposit funds into the yield strategy
    fn deposit(env: Env, from: Address, amount: i128, params: DepositParams) -> Result<YieldInfo, YieldStrategyError>;
    
    /// Withdraw funds from the yield strategy
    fn withdraw(env: Env, to: Address, params: WithdrawalParams) -> Result<YieldInfo, YieldStrategyError>;
    
    /// Get estimated yield for a given amount and period
    fn get_estimated_yield(env: Env, amount: i128, period_seconds: u64) -> Result<YieldEstimate, YieldStrategyError>;
    
    /// Get current yield information for an address
    fn get_yield_info(env: Env, user: Address) -> Result<YieldInfo, YieldStrategyError>;
    
    /// Get strategy configuration and status
    fn get_strategy_info(env: Env) -> Result<YieldStrategyConfig, YieldStrategyError>;
    
    /// Update strategy configuration (admin only)
    fn update_config(env: Env, admin: Address, config: YieldStrategyConfig) -> Result<(), YieldStrategyError>;
    
    /// Emergency withdrawal function (circuit breaker integration)
    fn emergency_withdraw(env: Env, to: Address, amount: i128) -> Result<YieldInfo, YieldStrategyError>;
    
    /// Check if strategy is healthy and operational
    fn health_check(env: Env) -> Result<bool, YieldStrategyError>;
}

// --- CONFIGURATION ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldStrategyConfig {
    pub strategy_name: String,
    pub strategy_version: u32,
    pub min_deposit_amount: i128,
    pub max_deposit_amount: Option<i128>,
    pub default_apy_bps: u32,
    pub auto_compound_enabled: bool,
    pub emergency_withdrawal_enabled: bool,
    pub is_active: bool,
    pub admin_address: Address,
}

// --- CLIENT INTERFACE ---

#[contractclient(name = "YieldStrategyClient")]
pub trait YieldStrategyClient {
    fn deposit(env: Env, from: Address, amount: i128, params: DepositParams) -> YieldInfo;
    fn withdraw(env: Env, to: Address, params: WithdrawalParams) -> YieldInfo;
    fn get_estimated_yield(env: Env, amount: i128, period_seconds: u64) -> YieldEstimate;
    fn get_yield_info(env: Env, user: Address) -> YieldInfo;
    fn get_strategy_info(env: Env) -> YieldStrategyConfig;
    fn emergency_withdraw(env: Env, to: Address, amount: i128) -> YieldInfo;
    fn health_check(env: Env) -> bool;
}

// --- REGISTRY FOR STRATEGIES ---

#[contracttype]
#[derive(Clone)]
pub struct RegisteredStrategy {
    pub address: Address,
    pub strategy_type: StrategyType,
    pub config: YieldStrategyConfig,
    pub registration_time: u64,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum StrategyType {
    AMM,                    // Automated Market Maker
    Lending,                // Lending Protocol
    LiquidStaking,          // Liquid Staking
    YieldAggregator,        // Yield Aggregator
    Custom,                 // Custom Strategy
}

// --- STORAGE KEYS FOR REGISTRY ---

#[contracttype]
#[derive(Clone)]
pub enum YieldStrategyRegistryKey {
    Admin,
    StrategyCount,
    Strategy(Address),       // Strategy address -> RegisteredStrategy
    StrategyByType(StrategyType), // Strategy type -> Vec<Address>
    ActiveStrategies,       // Vec<Address> of active strategies
    DefaultStrategy,         // Default strategy address
}

// --- CONSTANTS ---

const MIN_CONFIDENCE_SCORE: u32 = 5000; // 50% minimum confidence
const MAX_STRATEGIES_PER_TYPE: u32 = 100; // Limit strategies per type
const REGISTRATION_FEE: i128 = 1_000_000; // Small fee to prevent spam

// --- IMPLEMENTATION HELPERS ---

pub fn validate_deposit_amount(amount: i128, min_amount: i128, max_amount: Option<i128>) -> Result<(), YieldStrategyError> {
    if amount <= 0 {
        return Err(YieldStrategyError::InvalidAmount);
    }
    
    if amount < min_amount {
        return Err(YieldStrategyError::InsufficientBalance);
    }
    
    if let Some(max) = max_amount {
        if amount > max {
            return Err(YieldStrategyError::InvalidAmount);
        }
    }
    
    Ok(())
}

pub fn validate_withdrawal_params(params: &WithdrawalParams, current_balance: i128) -> Result<(), YieldStrategyError> {
    if params.amount <= 0 {
        return Err(YieldStrategyError::InvalidAmount);
    }
    
    if params.amount > current_balance && !params.force_withdrawal {
        return Err(YieldStrategyError::InsufficientBalance);
    }
    
    Ok(())
}

pub fn calculate_estimated_yield(
    principal: i128,
    apy_bps: u32,
    period_seconds: u64,
    confidence_score: u32,
) -> YieldEstimate {
    let seconds_in_year = 365 * 24 * 60 * 60;
    let time_fraction = period_seconds as i128 * 10000 / seconds_in_year as i128;
    let estimated_yield = (principal * apy_bps as i128 * time_fraction) / (10000 * 10000);
    
    YieldEstimate {
        estimated_yield,
        period_seconds,
        confidence_score,
        last_calculated: 0, // Will be set by caller
    }
}
