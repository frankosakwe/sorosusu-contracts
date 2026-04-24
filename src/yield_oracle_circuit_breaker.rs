#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token,
    Address, Env, String, Symbol, Vec, i128, u64, u32, Map,
};

// --- ERROR CODES ---

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CircuitBreakerError {
    Unauthorized = 401,
    CircuitBreakerNotTriggered = 402,
    CircuitBreakerAlreadyTriggered = 403,
    InsufficientHealthFactor = 404,
    EmergencyUnwindFailed = 405,
    OracleDataStale = 406,
    InvalidThreshold = 407,
    YieldDelegationNotFound = 408,
    VaultTransferFailed = 409,
}

// --- DATA STRUCTURES ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum CircuitBreakerStatus {
    Normal,          // Operating normally
    Warning,         // Health factor declining
    Triggered,       // Circuit breaker activated
    EmergencyUnwind, // Emergency unwind in progress
    Cooldown,        // Cooldown period after unwind
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct HealthMetrics {
    pub current_apy: u32,           // Current APY in basis points
    pub volatility_index: u32,      // Volatility index in basis points
    pub liquidity_ratio: u32,      // Liquidity ratio in basis points
    pub price_impact_score: u32,   // Price impact score in basis points
    pub yield_rate: i32,           // Current yield rate (can be negative)
    pub last_updated: u64,         // Last update timestamp
    pub is_healthy: bool,          // Overall health status
}

#[contracttype]
#[derive(Clone)]
pub struct CircuitBreakerConfig {
    pub min_health_factor: u32,        // Minimum health factor (10000 = 100%)
    pub volatility_threshold: u32,      // Volatility threshold in bps
    pub negative_yield_threshold: i32, // Negative yield threshold
    pub stale_data_period: u64,        // Period before data considered stale
    pub cooldown_period: u64,          // Cooldown period after emergency unwind
    pub auto_unwind_enabled: bool,     // Enable automatic emergency unwind
    pub manual_override_allowed: bool, // Allow manual override
}

#[contracttype]
#[derive(Clone)]
pub struct CircuitBreakerState {
    pub status: CircuitBreakerStatus,
    pub health_factor: u32,            // Current health factor (10000 = 100%)
    pub triggered_at: Option<u64>,     // When circuit breaker was triggered
    pub last_health_check: u64,        // Last health check timestamp
    pub emergency_unwind_count: u32,   // Number of emergency unwinds performed
    pub total_protected_funds: i128,   // Total funds protected via circuit breaker
    pub last_unwind_amount: i128,      // Amount of last emergency unwind
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct EmergencyUnwindRecord {
    pub circle_id: u64,
    pub unwind_amount: i128,
    pub unwind_reason: String,
    pub health_factor_before: u32,
    pub health_factor_after: u32,
    pub timestamp: u64,
    pub success: bool,
}

// --- STORAGE KEYS ---

#[contracttype]
#[derive(Clone)]
pub enum CircuitBreakerDataKey {
    Config,                           // Circuit breaker configuration
    State,                           // Current circuit breaker state
    HealthMetrics(Address),          // Health metrics per AMM/pool
    EmergencyUnwindRecord(u64),     // Emergency unwind records
    UnwindRecordCounter,            // Counter for unwind records
    ProtectedVault,                 // Address of protected vault
    AMMRegistry(Address),           // Registry of monitored AMMs
    LastHealthCheck(u64),           // Last health check per circle
}

// --- CONSTANTS ---

const DEFAULT_MIN_HEALTH_FACTOR: u32 = 7000;      // 70% health factor threshold
const DEFAULT_VOLATILITY_THRESHOLD: u32 = 1500;  // 15% volatility threshold
const DEFAULT_NEGATIVE_YIELD_THRESHOLD: i32 = -500; // -5% yield threshold
const DEFAULT_STALE_DATA_PERIOD: u64 = 3600;      // 1 hour
const DEFAULT_COOLDOWN_PERIOD: u64 = 86400;      // 24 hours
const HEALTH_CHECK_INTERVAL: u64 = 300;          // 5 minutes

// --- CONTRACT ---

#[contract]
pub struct YieldOracleCircuitBreaker;

#[contractimpl]
impl YieldOracleCircuitBreaker {
    
    /// Initialize the circuit breaker with default configuration
    pub fn initialize(env: Env, admin: Address, protected_vault: Address) {
        // Only allow initialization once
        if env.storage().instance().get(&CircuitBreakerDataKey::Config).is_some() {
            panic!("Circuit breaker already initialized");
        }
        
        // Set admin
        env.storage().instance().set(&CircuitBreakerDataKey::State, &admin);
        
        // Set protected vault
        env.storage().instance().set(&CircuitBreakerDataKey::ProtectedVault, &protected_vault);
        
        // Initialize default configuration
        let config = CircuitBreakerConfig {
            min_health_factor: DEFAULT_MIN_HEALTH_FACTOR,
            volatility_threshold: DEFAULT_VOLATILITY_THRESHOLD,
            negative_yield_threshold: DEFAULT_NEGATIVE_YIELD_THRESHOLD,
            stale_data_period: DEFAULT_STALE_DATA_PERIOD,
            cooldown_period: DEFAULT_COOLDOWN_PERIOD,
            auto_unwind_enabled: true,
            manual_override_allowed: true,
        };
        env.storage().instance().set(&CircuitBreakerDataKey::Config, &config);
        
        // Initialize state
        let state = CircuitBreakerState {
            status: CircuitBreakerStatus::Normal,
            health_factor: 10000, // Start at 100% health
            triggered_at: None,
            last_health_check: env.ledger().timestamp(),
            emergency_unwind_count: 0,
            total_protected_funds: 0,
            last_unwind_amount: 0,
        };
        env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
        
        // Initialize unwind record counter
        env.storage().instance().set(&CircuitBreakerDataKey::UnwindRecordCounter, &0u64);
        
        env.events().publish(
            (Symbol::new(&env, "circuit_breaker_initialized"),),
            (admin, protected_vault),
        );
    }
    
    /// Update circuit breaker configuration (admin only)
    pub fn update_config(env: Env, admin: Address, config: CircuitBreakerConfig) {
        // Verify admin authorization
        let stored_admin: Address = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker not initialized");
        
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        
        // Validate configuration
        if config.min_health_factor == 0 || config.min_health_factor > 10000 {
            panic!("Invalid health factor threshold");
        }
        
        if config.volatility_threshold > 10000 {
            panic!("Invalid volatility threshold");
        }
        
        // Update configuration
        env.storage().instance().set(&CircuitBreakerDataKey::Config, &config);
        
        env.events().publish(
            (Symbol::new(&env, "config_updated"),),
            (admin,),
        );
    }
    
    /// Register a yield strategy for monitoring
    pub fn register_yield_strategy(env: Env, admin: Address, strategy_address: Address, initial_metrics: HealthMetrics) {
        // Verify admin authorization
        let stored_admin: Address = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker not initialized");
        
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        
        // Register yield strategy
        env.storage().instance().set(&CircuitBreakerDataKey::AMMRegistry(strategy_address.clone()), &true);
        
        // Store initial health metrics
        env.storage().instance().set(&CircuitBreakerDataKey::HealthMetrics(strategy_address), &initial_metrics);
        
        env.events().publish(
            (Symbol::new(&env, "yield_strategy_registered"),),
            (strategy_address, initial_metrics.is_healthy),
        );
    }
    
    /// Update health metrics for a monitored yield strategy
    pub fn update_health_metrics(env: Env, strategy_address: Address, metrics: HealthMetrics) {
        // Verify strategy is registered
        let is_registered: bool = env.storage().instance()
            .get(&CircuitBreakerDataKey::AMMRegistry(strategy_address.clone()))
            .unwrap_or(false);
        
        if !is_registered {
            panic!("Yield strategy not registered for monitoring");
        }
        
        // Update metrics
        env.storage().instance().set(&CircuitBreakerDataKey::HealthMetrics(strategy_address.clone()), &metrics);
        
        // Check if circuit breaker should be triggered
        Self::check_circuit_breaker_conditions(&env, &strategy_address, &metrics);
        
        env.events().publish(
            (Symbol::new(&env, "health_metrics_updated"),),
            (strategy_address, metrics.health_factor),
        );
    }
    
    /// Manual trigger of circuit breaker (admin only)
    pub fn manual_trigger_circuit_breaker(env: Env, admin: Address, reason: String) {
        // Verify admin authorization
        let stored_admin: Address = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker not initialized");
        
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        
        // Check if manual override is allowed
        let config: CircuitBreakerConfig = env.storage().instance()
            .get(&CircuitBreakerDataKey::Config)
            .expect("Configuration not found");
        
        if !config.manual_override_allowed {
            panic!("Manual override not allowed");
        }
        
        // Trigger circuit breaker
        Self::trigger_circuit_breaker_internal(&env, &reason);
    }
    
    /// Emergency unwind function - pulls all funds from AMM and returns to vault
    pub fn emergency_unwind(env: Env, circle_id: u64, amm_address: Address) -> Result<(), CircuitBreakerError> {
        let current_time = env.ledger().timestamp();
        
        // Get circuit breaker state
        let mut state: CircuitBreakerState = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker state not found");
        
        // Check if circuit breaker is triggered
        if state.status != CircuitBreakerStatus::Triggered && 
           state.status != CircuitBreakerStatus::EmergencyUnwind {
            return Err(CircuitBreakerError::CircuitBreakerNotTriggered);
        }
        
        // Get yield delegation for this circle
        let delegation_key = crate::DataKey::YieldDelegation(circle_id);
        let delegation: crate::YieldDelegation = env.storage().instance()
            .get(&delegation_key)
            .ok_or(CircuitBreakerError::YieldDelegationNotFound)?;
        
        if delegation.status != crate::YieldDelegationStatus::Active {
            return Err(CircuitBreakerError::YieldDelegationNotFound);
        }
        
        // Get protected vault address
        let protected_vault: Address = env.storage().instance()
            .get(&CircuitBreakerDataKey::ProtectedVault)
            .expect("Protected vault not found");
        
        // Calculate total amount to unwind (principal + accrued yield)
        let total_amount = delegation.delegation_amount + delegation.total_yield_earned;
        
        // Execute emergency unwind
        state.status = CircuitBreakerStatus::EmergencyUnwind;
        state.last_unwind_amount = total_amount;
        state.total_protected_funds += total_amount;
        
        // In a real implementation, this would:
        // 1. Call the AMM contract to withdraw all funds
        // 2. Transfer funds to the protected vault
        // 3. Update yield delegation status
        
        // Use the abstract yield strategy interface for emergency withdrawal
        let strategy_client = crate::YieldStrategyClient::new(&env, &delegation.strategy_address);
        let yield_info = strategy_client.emergency_withdraw(
            &protected_vault,
            &total_amount,
        );
        
        // Update delegation status
        let mut updated_delegation = delegation.clone();
        updated_delegation.status = crate::YieldDelegationStatus::Completed;
        updated_delegation.end_time = Some(current_time);
        env.storage().instance().set(&delegation_key, &updated_delegation);
        
        // Record emergency unwind
        let record_counter: u64 = env.storage().instance()
            .get(&CircuitBreakerDataKey::UnwindRecordCounter)
            .unwrap_or(0);
        
        let unwind_record = EmergencyUnwindRecord {
            circle_id,
            unwind_amount: total_amount,
            unwind_reason: String::from_str(&env, "Circuit breaker emergency unwind"),
            health_factor_before: state.health_factor,
            health_factor_after: 0, // Reset after unwind
            timestamp: current_time,
            success: true,
        };
        
        env.storage().instance().set(&CircuitBreakerDataKey::EmergencyUnwindRecord(record_counter), &unwind_record);
        env.storage().instance().set(&CircuitBreakerDataKey::UnwindRecordCounter, &(record_counter + 1));
        
        // Update state
        state.emergency_unwind_count += 1;
        state.triggered_at = Some(current_time);
        state.status = CircuitBreakerStatus::Cooldown;
        env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
        
        env.events().publish(
            (Symbol::new(&env, "emergency_unwind_completed"),),
            (circle_id, total_amount, protected_vault),
        );
        
        Ok(())
    }
    
    /// Get current circuit breaker status
    pub fn get_circuit_breaker_status(env: Env) -> CircuitBreakerState {
        env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker state not found")
    }
    
    /// Get health metrics for a specific yield strategy
    pub fn get_health_metrics(env: Env, strategy_address: Address) -> HealthMetrics {
        env.storage().instance()
            .get(&CircuitBreakerDataKey::HealthMetrics(strategy_address))
            .expect("Health metrics not found for yield strategy")
    }
    
    /// Get emergency unwind records
    pub fn get_emergency_unwind_records(env: Env, limit: u32) -> Vec<EmergencyUnwindRecord> {
        let record_counter: u64 = env.storage().instance()
            .get(&CircuitBreakerDataKey::UnwindRecordCounter)
            .unwrap_or(0);
        
        let mut records = Vec::new(&env);
        let start_index = if record_counter > limit as u64 { 
            record_counter - limit as u64 
        } else { 
            0 
        };
        
        for i in start_index..record_counter {
            if let Some(record) = env.storage().instance()
                .get(&CircuitBreakerDataKey::EmergencyUnwindRecord(i)) {
                records.push_back(record);
            }
        }
        
        records
    }
    
    /// Reset circuit breaker after cooldown period (admin only)
    pub fn reset_circuit_breaker(env: Env, admin: Address) {
        // Verify admin authorization
        let stored_admin: Address = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker not initialized");
        
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        
        let current_time = env.ledger().timestamp();
        let config: CircuitBreakerConfig = env.storage().instance()
            .get(&CircuitBreakerDataKey::Config)
            .expect("Configuration not found");
        
        let mut state: CircuitBreakerState = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker state not found");
        
        // Check if cooldown period has passed
        if let Some(triggered_at) = state.triggered_at {
            if current_time < triggered_at + config.cooldown_period {
                panic!("Cooldown period not yet completed");
            }
        }
        
        // Reset circuit breaker
        state.status = CircuitBreakerStatus::Normal;
        state.health_factor = 10000;
        state.triggered_at = None;
        state.last_health_check = current_time;
        
        env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
        
        env.events().publish(
            (Symbol::new(&env, "circuit_breaker_reset"),),
            (admin, current_time),
        );
    }
    
    // --- INTERNAL FUNCTIONS ---
    
    /// Check circuit breaker conditions and trigger if necessary
    fn check_circuit_breaker_conditions(env: &Env, amm_address: &Address, metrics: &HealthMetrics) {
        let config: CircuitBreakerConfig = env.storage().instance()
            .get(&CircuitBreakerDataKey::Config)
            .expect("Configuration not found");
        
        let mut state: CircuitBreakerState = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker state not found");
        
        let current_time = env.ledger().timestamp();
        
        // Skip if already triggered or in cooldown
        if state.status == CircuitBreakerStatus::Triggered || 
           state.status == CircuitBreakerStatus::EmergencyUnwind ||
           state.status == CircuitBreakerStatus::Cooldown {
            return;
        }
        
        // Check if data is stale
        if current_time > metrics.last_updated + config.stale_data_period {
            env.events().publish(
                (Symbol::new(&env, "stale_data_warning"),),
                (amm_address, metrics.last_updated),
            );
            
            if state.status != CircuitBreakerStatus::Warning {
                state.status = CircuitBreakerStatus::Warning;
                env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
            }
            return;
        }
        
        // Calculate health factor
        let health_factor = Self::calculate_health_factor(&config, metrics);
        state.health_factor = health_factor;
        state.last_health_check = current_time;
        
        // Check trigger conditions
        let should_trigger = health_factor < config.min_health_factor ||
                           metrics.yield_rate < config.negative_yield_threshold ||
                           metrics.volatility_index > config.volatility_threshold;
        
        if should_trigger {
            let reason = if health_factor < config.min_health_factor {
                "Health factor below threshold"
            } else if metrics.yield_rate < config.negative_yield_threshold {
                "Negative yield detected"
            } else {
                "High volatility detected"
            };
            
            Self::trigger_circuit_breaker_internal(env, &String::from_str(env, reason));
        } else if health_factor < 8500 && state.status == CircuitBreakerStatus::Normal {
            // Enter warning state if health factor is declining but not critical
            state.status = CircuitBreakerStatus::Warning;
            env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
        } else if health_factor >= 9000 && state.status == CircuitBreakerStatus::Warning {
            // Return to normal if health improves
            state.status = CircuitBreakerStatus::Normal;
            env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
        }
        
        env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
    }
    
    /// Calculate overall health factor from metrics
    fn calculate_health_factor(config: &CircuitBreakerConfig, metrics: &HealthMetrics) -> u32 {
        let mut health_factor = 10000u32; // Start at 100%
        
        // Factor in yield rate (most important)
        if metrics.yield_rate < 0 {
            health_factor -= ((-metrics.yield_rate) as u32 * 200); // 20x weight for negative yield
        } else {
            health_factor += (metrics.yield_rate as u32 * 50); // Bonus for positive yield
        }
        
        // Factor in volatility
        if metrics.volatility_index > config.volatility_threshold {
            health_factor -= (metrics.volatility_index - config.volatility_threshold) * 2;
        }
        
        // Factor in liquidity ratio
        if metrics.liquidity_ratio < 5000 { // Less than 50% liquidity
            health_factor -= (5000 - metrics.liquidity_ratio) / 2;
        }
        
        // Factor in price impact
        if metrics.price_impact_score > 1000 { // More than 10% price impact
            health_factor -= (metrics.price_impact_score - 1000);
        }
        
        // Clamp between 0 and 10000
        health_factor.min(10000).max(0)
    }
    
    /// Internal circuit breaker trigger
    fn trigger_circuit_breaker_internal(env: &Env, reason: &String) {
        let mut state: CircuitBreakerState = env.storage().instance()
            .get(&CircuitBreakerDataKey::State)
            .expect("Circuit breaker state not found");
        
        let config: CircuitBreakerConfig = env.storage().instance()
            .get(&CircuitBreakerDataKey::Config)
            .expect("Configuration not found");
        
        let current_time = env.ledger().timestamp();
        
        state.status = CircuitBreakerStatus::Triggered;
        state.triggered_at = Some(current_time);
        
        env.storage().instance().set(&CircuitBreakerDataKey::State, &state);
        
        env.events().publish(
            (Symbol::new(&env, "circuit_breaker_triggered"),),
            (reason, state.health_factor, current_time),
        );
        
        // Auto-unwind if enabled
        if config.auto_unwind_enabled {
            // In a real implementation, this would iterate through all active yield delegations
            // and call emergency_unwind for each one
            env.events().publish(
                (Symbol::new(&env, "auto_unwind_initiated"),),
                (reason,),
            );
        }
    }
}
