#![cfg(test)]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, token, i128, u64, u32};
use crate::{
    yield_strategy_trait::{
        YieldStrategyTrait, YieldStrategyClient, YieldStrategyConfig, YieldInfo, 
        DepositParams, WithdrawalParams, YieldEstimate, StrategyType,
        YieldStrategyError
    },
    DataKey,
};

// --- MOCK YIELD STRATEGY IMPLEMENTATION ---

#[contract]
pub struct MockYieldStrategy;

#[contractimpl]
impl YieldStrategyTrait for MockYieldStrategy {
    fn initialize(env: Env, admin: Address, config: YieldStrategyConfig) {
        let admin_key = crate::DataKey::Admin;
        env.storage().instance().set(&admin_key, &admin);
        env.storage().instance().set(&DataKey::ActiveYieldStrategy(0), &config);
    }
    
    fn deposit(env: Env, from: Address, amount: i128, params: DepositParams) -> Result<YieldInfo, YieldStrategyError> {
        // Validate deposit
        crate::yield_strategy_trait::validate_deposit_amount(
            amount, 
            1_000_000, // Min deposit
            Some(10_000_000_000) // Max deposit
        )?;
        
        // Create yield info
        let yield_info = YieldInfo {
            total_deposited: amount,
            current_balance: amount,
            total_yield_earned: 0,
            apy_bps: params.min_apy_bps.unwrap_or(500), // Default 5% APY
            last_updated: env.ledger().timestamp(),
            is_active: true,
        };
        
        // Store user yield info
        let user_key = crate::DataKey::Member(from);
        env.storage().instance().set(&user_key, &yield_info);
        
        Ok(yield_info)
    }
    
    fn withdraw(env: Env, to: Address, params: WithdrawalParams) -> Result<YieldInfo, YieldStrategyError> {
        let user_key = crate::DataKey::Member(to.clone());
        let mut yield_info: YieldInfo = env.storage().instance()
            .get(&user_key)
            .ok_or(YieldStrategyError::StrategyNotFound)?;
        
        // Validate withdrawal
        crate::yield_strategy_trait::validate_withdrawal_params(&params, yield_info.current_balance)?;
        
        // Calculate yield earned
        let current_time = env.ledger().timestamp();
        let time_elapsed = current_time - yield_info.last_updated;
        let yield_earned = crate::yield_strategy_trait::calculate_estimated_yield(
            yield_info.total_deposited,
            yield_info.apy_bps,
            time_elapsed,
            8000, // 80% confidence
        ).estimated_yield;
        
        // Update yield info
        yield_info.total_yield_earned += yield_earned;
        yield_info.current_balance = params.amount;
        yield_info.last_updated = current_time;
        
        if params.amount >= yield_info.current_balance {
            yield_info.is_active = false;
        }
        
        env.storage().instance().set(&user_key, &yield_info);
        
        Ok(yield_info)
    }
    
    fn get_estimated_yield(env: Env, amount: i128, period_seconds: u64) -> Result<YieldEstimate, YieldStrategyError> {
        if amount <= 0 {
            return Err(YieldStrategyError::InvalidAmount);
        }
        
        let yield_estimate = crate::yield_strategy_trait::calculate_estimated_yield(
            amount,
            500, // 5% APY
            period_seconds,
            8000, // 80% confidence
        );
        
        Ok(yield_estimate)
    }
    
    fn get_yield_info(env: Env, user: Address) -> Result<YieldInfo, YieldStrategyError> {
        let user_key = crate::DataKey::Member(user);
        env.storage().instance()
            .get(&user_key)
            .ok_or(YieldStrategyError::StrategyNotFound)
    }
    
    fn get_strategy_info(env: Env) -> Result<YieldStrategyConfig, YieldStrategyError> {
        env.storage().instance()
            .get(&DataKey::ActiveYieldStrategy(0))
            .ok_or(YieldStrategyError::StrategyNotFound)
    }
    
    fn update_config(env: Env, admin: Address, config: YieldStrategyConfig) -> Result<(), YieldStrategyError> {
        let admin_key = crate::DataKey::Admin;
        let stored_admin: Address = env.storage().instance()
            .get(&admin_key)
            .ok_or(YieldStrategyError::Unauthorized)?;
        
        if admin != stored_admin {
            return Err(YieldStrategyError::Unauthorized);
        }
        
        env.storage().instance().set(&DataKey::ActiveYieldStrategy(0), &config);
        Ok(())
    }
    
    fn emergency_withdraw(env: Env, to: Address, amount: i128) -> Result<YieldInfo, YieldStrategyError> {
        let user_key = crate::DataKey::Member(to.clone());
        let mut yield_info: YieldInfo = env.storage().instance()
            .get(&user_key)
            .ok_or(YieldStrategyError::StrategyNotFound)?;
        
        // Force withdrawal regardless of balance
        yield_info.current_balance = amount.min(yield_info.current_balance + yield_info.total_yield_earned);
        yield_info.is_active = false;
        yield_info.last_updated = env.ledger().timestamp();
        
        env.storage().instance().set(&user_key, &yield_info);
        
        Ok(yield_info)
    }
    
    fn health_check(env: Env) -> Result<bool, YieldStrategyError> {
        // Simple health check - always returns true for mock
        Ok(true)
    }
}

// --- TESTS ---

#[test]
fn test_yield_strategy_trait_deposit() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Deploy mock strategy
    let contract_id = env.register_contract(None, MockYieldStrategy);
    let strategy_client = YieldStrategyClient::new(&env, &contract_id);
    
    // Initialize strategy
    let config = YieldStrategyConfig {
        strategy_name: Symbol::new(&env, "MockStrategy"),
        strategy_version: 1,
        min_deposit_amount: 1_000_000,
        max_deposit_amount: Some(10_000_000_000),
        default_apy_bps: 500,
        auto_compound_enabled: true,
        emergency_withdrawal_enabled: true,
        is_active: true,
        admin_address: admin.clone(),
    };
    
    MockYieldStrategy::initialize(env.clone(), admin.clone(), config.clone());
    
    // Test deposit
    let deposit_params = DepositParams {
        amount: 1_000_000,
        min_apy_bps: Some(400),
        lockup_period: None,
        auto_compound: true,
    };
    
    let yield_info = strategy_client.deposit(&user, &1_000_000, &deposit_params);
    
    assert_eq!(yield_info.total_deposited, 1_000_000);
    assert_eq!(yield_info.current_balance, 1_000_000);
    assert_eq!(yield_info.total_yield_earned, 0);
    assert_eq!(yield_info.apy_bps, 400); // Should use min_apy_bps
    assert!(yield_info.is_active);
}

#[test]
fn test_yield_strategy_trait_withdrawal() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Deploy mock strategy
    let contract_id = env.register_contract(None, MockYieldStrategy);
    let strategy_client = YieldStrategyClient::new(&env, &contract_id);
    
    // Initialize strategy
    let config = YieldStrategyConfig {
        strategy_name: Symbol::new(&env, "MockStrategy"),
        strategy_version: 1,
        min_deposit_amount: 1_000_000,
        max_deposit_amount: Some(10_000_000_000),
        default_apy_bps: 500,
        auto_compound_enabled: true,
        emergency_withdrawal_enabled: true,
        is_active: true,
        admin_address: admin.clone(),
    };
    
    MockYieldStrategy::initialize(env.clone(), admin.clone(), config.clone());
    
    // Deposit first
    let deposit_params = DepositParams {
        amount: 1_000_000,
        min_apy_bps: Some(500),
        lockup_period: None,
        auto_compound: true,
    };
    
    strategy_client.deposit(&user, &1_000_000, &deposit_params);
    
    // Advance time to earn some yield
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400); // 1 day
    
    // Test withdrawal
    let withdrawal_params = WithdrawalParams {
        amount: 500_000,
        force_withdrawal: false,
        claim_yield_only: false,
    };
    
    let yield_info = strategy_client.withdraw(&user, &withdrawal_params);
    
    assert!(yield_info.total_yield_earned > 0); // Should have earned some yield
    assert_eq!(yield_info.current_balance, 500_000);
    assert!(yield_info.is_active); // Still active since not fully withdrawn
}

#[test]
fn test_yield_strategy_trait_estimated_yield() {
    let env = Env::default();
    let admin = Address::generate(&env);
    
    // Deploy mock strategy
    let contract_id = env.register_contract(None, MockYieldStrategy);
    let strategy_client = YieldStrategyClient::new(&env, &contract_id);
    
    // Initialize strategy
    let config = YieldStrategyConfig {
        strategy_name: Symbol::new(&env, "MockStrategy"),
        strategy_version: 1,
        min_deposit_amount: 1_000_000,
        max_deposit_amount: Some(10_000_000_000),
        default_apy_bps: 500,
        auto_compound_enabled: true,
        emergency_withdrawal_enabled: true,
        is_active: true,
        admin_address: admin.clone(),
    };
    
    MockYieldStrategy::initialize(env.clone(), admin.clone(), config.clone());
    
    // Test yield estimation
    let amount = 1_000_000;
    let period_seconds = 86400; // 1 day
    
    let yield_estimate = strategy_client.get_estimated_yield(&amount, &period_seconds);
    
    assert!(yield_estimate.estimated_yield > 0);
    assert_eq!(yield_estimate.period_seconds, period_seconds);
    assert_eq!(yield_estimate.confidence_score, 8000); // 80% confidence
}

#[test]
fn test_yield_strategy_trait_health_check() {
    let env = Env::default();
    let admin = Address::generate(&env);
    
    // Deploy mock strategy
    let contract_id = env.register_contract(None, MockYieldStrategy);
    let strategy_client = YieldStrategyClient::new(&env, &contract_id);
    
    // Initialize strategy
    let config = YieldStrategyConfig {
        strategy_name: Symbol::new(&env, "MockStrategy"),
        strategy_version: 1,
        min_deposit_amount: 1_000_000,
        max_deposit_amount: Some(10_000_000_000),
        default_apy_bps: 500,
        auto_compound_enabled: true,
        emergency_withdrawal_enabled: true,
        is_active: true,
        admin_address: admin.clone(),
    };
    
    MockYieldStrategy::initialize(env.clone(), admin.clone(), config.clone());
    
    // Test health check
    let is_healthy = strategy_client.health_check();
    assert!(is_healthy);
}

#[test]
fn test_yield_strategy_trait_emergency_withdrawal() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Deploy mock strategy
    let contract_id = env.register_contract(None, MockYieldStrategy);
    let strategy_client = YieldStrategyClient::new(&env, &contract_id);
    
    // Initialize strategy
    let config = YieldStrategyConfig {
        strategy_name: Symbol::new(&env, "MockStrategy"),
        strategy_version: 1,
        min_deposit_amount: 1_000_000,
        max_deposit_amount: Some(10_000_000_000),
        default_apy_bps: 500,
        auto_compound_enabled: true,
        emergency_withdrawal_enabled: true,
        is_active: true,
        admin_address: admin.clone(),
    };
    
    MockYieldStrategy::initialize(env.clone(), admin.clone(), config.clone());
    
    // Deposit first
    let deposit_params = DepositParams {
        amount: 1_000_000,
        min_apy_bps: Some(500),
        lockup_period: None,
        auto_compound: true,
    };
    
    strategy_client.deposit(&user, &1_000_000, &deposit_params);
    
    // Test emergency withdrawal
    let yield_info = strategy_client.emergency_withdraw(&user, &1_500_000); // More than balance
    
    assert!(!yield_info.is_active); // Should be deactivated
    assert!(yield_info.current_balance <= 1_500_000); // Should not exceed requested amount
}
