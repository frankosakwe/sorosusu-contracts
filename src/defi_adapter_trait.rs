#![no_std]
use soroban_sdk::{
    contractclient, contracterror, contracttype,
    Address, Env, i128, u64, u32, Map, Vec, Symbol,
};

// --- ERROR CODES ---

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DeFiAdapterError {
    Unauthorized = 501,
    InsufficientBalance = 502,
    InvalidAmount = 503,
    AdapterNotActive = 504,
    WithdrawalNotAllowed = 505,
    DepositFailed = 506,
    WithdrawalFailed = 507,
    SlippageExceeded = 508,
    VaultPaused = 509,
    InvalidVault = 510,
    InsufficientLiquidity = 511,
    UnauthorizedVault = 512,
    InvalidParameters = 513,
    PrincipalAtRisk = 514,
    HarvestNotReady = 515,
    BailedOut = 516,
}

// --- DATA STRUCTURES ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VaultInfo {
    pub vault_address: Address,
    pub vault_name: Symbol,
    pub vault_type: VaultType,
    pub total_deposited: i128,
    pub current_balance: i128,
    pub total_yield_earned: i128,
    pub apy_bps: u32,
    pub is_active: bool,
    pub is_paused: bool,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum VaultType {
    AMM,
    Lending,
    LiquidStaking,
    YieldAggregator,
    Custom,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositParams {
    pub amount: i128,
    pub min_return_amount: i128,
    pub max_slippage_bps: u32,
    pub deadline: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawalParams {
    pub amount: i128,
    pub min_return_amount: i128,
    pub max_slippage_bps: u32,
    pub deadline: u64,
    pub force_withdrawal: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldHarvestResult {
    pub harvested_amount: i128,
    pub principal_returned: i128,
    pub yield_amount: i128,
    pub slippage_bps: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VaultHealth {
    pub is_healthy: bool,
    pub total_value_locked: i128,
    pub available_liquidity: i128,
    pub utilization_rate_bps: u32,
    pub last_check: u64,
}

// --- DEFI ADAPTER TRAIT ---

/// Standard trait that all external yield vaults must implement
/// to be compatible with the SoroSusu yield bridge adapter
pub trait DeFiAdapterTrait {
    /// Initialize the adapter with configuration
    fn initialize(env: Env, admin: Address, config: DeFiAdapterConfig);
    
    /// Deposit funds into the external vault
    fn deposit(env: Env, from: Address, amount: i128, params: DepositParams) -> Result<VaultInfo, DeFiAdapterError>;
    
    /// Withdraw funds from the external vault
    fn withdraw(env: Env, to: Address, params: WithdrawalParams) -> Result<YieldHarvestResult, DeFiAdapterError>;
    
    /// Harvest yield only (keep principal in vault)
    fn harvest_yield(env: Env, to: Address, min_return_amount: i128) -> Result<YieldHarvestResult, DeFiAdapterError>;
    
    /// Get current vault information
    fn get_vault_info(env: Env) -> Result<VaultInfo, DeFiAdapterError>;
    
    /// Check vault health status
    fn check_health(env: Env) -> Result<VaultHealth, DeFiAdapterError>;
    
    /// Check if vault is currently paused
    fn is_paused(env: Env) -> Result<bool, DeFiAdapterError>;
    
    /// Get estimated yield for a given amount and period
    fn get_estimated_yield(env: Env, amount: i128, period_seconds: u64) -> Result<i128, DeFiAdapterError>;
    
    /// Emergency withdrawal (bypass normal restrictions)
    fn emergency_withdraw(env: Env, to: Address, amount: i128) -> Result<YieldHarvestResult, DeFiAdapterError>;
    
    /// Update adapter configuration (admin only)
    fn update_config(env: Env, admin: Address, config: DeFiAdapterConfig) -> Result<(), DeFiAdapterError>;
}

// --- CLIENT INTERFACE ---

#[contractclient(name = "DeFiAdapterClient")]
pub trait DeFiAdapterClient {
    fn deposit(env: Env, from: Address, amount: i128, params: DepositParams) -> VaultInfo;
    fn withdraw(env: Env, to: Address, params: WithdrawalParams) -> YieldHarvestResult;
    fn harvest_yield(env: Env, to: Address, min_return_amount: i128) -> YieldHarvestResult;
    fn get_vault_info(env: Env) -> VaultInfo;
    fn check_health(env: Env) -> VaultHealth;
    fn is_paused(env: Env) -> bool;
    fn get_estimated_yield(env: Env, amount: i128, period_seconds: u64) -> i128;
    fn emergency_withdraw(env: Env, to: Address, amount: i128) -> YieldHarvestResult;
}

// --- CONFIGURATION ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DeFiAdapterConfig {
    pub adapter_name: Symbol,
    pub adapter_version: u32,
    pub min_deposit_amount: i128,
    pub max_deposit_amount: Option<i128>,
    pub min_withdrawal_amount: i128,
    pub max_slippage_bps: u32,
    pub emergency_withdrawal_enabled: bool,
    pub is_active: bool,
    pub admin_address: Address,
    pub supported_tokens: Vec<Address>,
}

// --- CONSTANTS ---

const DEFAULT_MAX_SLIPPAGE_BPS: u32 = 50; // 0.5%
const MIN_HEALTH_THRESHOLD_BPS: u32 = 8000; // 80%
const MAX_UTILIZATION_RATE_BPS: u32 = 9500; // 95%

// --- IMPLEMENTATION HELPERS ---

pub fn validate_deposit_params(params: &DepositParams, max_slippage_bps: u32) -> Result<(), DeFiAdapterError> {
    if params.amount <= 0 {
        return Err(DeFiAdapterError::InvalidAmount);
    }
    
    if params.min_return_amount <= 0 {
        return Err(DeFiAdapterError::InvalidAmount);
    }
    
    if params.max_slippage_bps > max_slippage_bps {
        return Err(DeFiAdapterError::SlippageExceeded);
    }
    
    if params.min_return_amount > params.amount {
        return Err(DeFiAdapterError::InvalidAmount);
    }
    
    Ok(())
}

pub fn validate_withdrawal_params(params: &WithdrawalParams, max_slippage_bps: u32) -> Result<(), DeFiAdapterError> {
    if params.amount <= 0 {
        return Err(DeFiAdapterError::InvalidAmount);
    }
    
    if params.min_return_amount <= 0 {
        return Err(DeFiAdapterError::InvalidAmount);
    }
    
    if params.max_slippage_bps > max_slippage_bps {
        return Err(DeFiAdapterError::SlippageExceeded);
    }
    
    if params.min_return_amount > params.amount {
        return Err(DeFiAdapterError::InvalidAmount);
    }
    
    Ok(())
}

pub fn calculate_slippage(expected_amount: i128, actual_amount: i128) -> u32 {
    if expected_amount <= 0 {
        return 0;
    }
    
    let diff = expected_amount - actual_amount;
    if diff <= 0 {
        return 0;
    }
    
    let slippage_bps = (diff * 10000) / expected_amount;
    slippage_bps as u32
}

pub fn check_principal_protection(
    principal: i128,
    current_balance: i128,
    min_protection_ratio_bps: u32,
) -> Result<(), DeFiAdapterError> {
    if current_balance < (principal * min_protection_ratio_bps as i128) / 10000 {
        return Err(DeFiAdapterError::PrincipalAtRisk);
    }
    Ok(())
}
