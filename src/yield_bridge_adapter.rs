#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    Address, Env, Symbol, Vec, Map, i128, u64, u32, token,
};
use crate::defi_adapter_trait::{
    DeFiAdapterClient, DeFiAdapterError, VaultInfo, VaultType,
    DepositParams, WithdrawalParams, YieldHarvestResult,
    validate_deposit_params, validate_withdrawal_params, calculate_slippage,
    check_principal_protection,
};

// --- ERROR CODES ---

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum YieldBridgeError {
    Unauthorized = 601,
    VaultNotWhitelisted = 602,
    InsufficientBalance = 603,
    InvalidAmount = 604,
    SlippageExceeded = 605,
    VaultPaused = 606,
    PrincipalAtRisk = 607,
    HarvestNotReady = 608,
    InvalidCircle = 609,
    BridgeNotActive = 610,
    ReserveInsufficient = 611,
    UnauthorizedAdmin = 612,
    InvalidParameters = 613,
    BailedOut = 614,
    WithdrawalFailed = 615,
    DepositFailed = 616,
}

// --- DATA STRUCTURES ---

#[contracttype]
#[derive(Clone)]
pub struct YieldBridgeConfig {
    pub admin: Address,
    pub is_active: bool,
    pub max_slippage_bps: u32,
    pub min_principal_protection_bps: u32,
    pub harvest_ledger_offset: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct WhitelistedVault {
    pub vault_address: Address,
    pub vault_name: Symbol,
    pub vault_type: VaultType,
    pub is_active: bool,
    pub added_at: u64,
    pub risk_level: u32, // 0-100, higher = riskier
}

#[contracttype]
#[derive(Clone)]
pub struct CircleBridgeState {
    pub circle_id: u64,
    pub selected_vault: Option<Address>,
    pub total_deposited: i128,
    pub principal_amount: i128,
    pub current_balance: i128,
    pub deposit_share_ratio: u32, // in basis points, 10000 = 100%
    pub last_harvest_ledger: u64,
    pub next_payout_ledger: u64,
    pub is_bailed_out: bool,
    pub yield_earned: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct HarvestSchedule {
    pub circle_id: u64,
    pub target_ledger: u64,
    pub harvest_amount: i128,
    pub is_executed: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum YieldBridgeDataKey {
    Admin,
    Config,
    WhitelistedVault(Address),
    WhitelistCount,
    CircleBridge(u64),
    HarvestSchedule(u64, u64), // circle_id, schedule_id
    HarvestScheduleCount(u64),
    GroupReserve,
    YieldBridgeDeployed(u64), // event counter
}

// --- EVENTS ---

#[contracttype]
#[derive(Clone, Debug)]
pub struct YieldBridgeDeployedEvent {
    pub circle_id: u64,
    pub vault_address: Address,
    pub vault_name: Symbol,
    pub amount_deployed: i128,
    pub deposit_share_ratio: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct YieldHarvestedEvent {
    pub circle_id: u64,
    pub vault_address: Address,
    pub amount_harvested: i128,
    pub principal_returned: i128,
    pub yield_amount: i128,
    pub slippage_bps: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct VaultPausedEvent {
    pub circle_id: u64,
    pub vault_address: Address,
    pub bailed_out: bool,
    pub reserve_used: i128,
    pub timestamp: u64,
}

// --- CONSTANTS ---

const MAX_SLIPPAGE_BPS: u32 = 50; // 0.5%
const MIN_PRINCIPAL_PROTECTION_BPS: u32 = 9500; // 95%
const HARVEST_LEDGER_OFFSET: u64 = 2; // 2 ledger closes before payout
const DEFAULT_DEPOSIT_SHARE_RATIO: u32 = 5000; // 50% by default
const MIN_DEPOSIT_SHARE_RATIO: u32 = 1000; // 10% minimum
const MAX_DEPOSIT_SHARE_RATIO: u32 = 8000; // 80% maximum

// --- CONTRACT ---

#[contract]
pub struct YieldBridgeAdapter;

#[contractimpl]
impl YieldBridgeAdapter {
    /// Initialize the yield bridge adapter
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&YieldBridgeDataKey::Admin) {
            panic!("Already initialized");
        }

        let config = YieldBridgeConfig {
            admin: admin.clone(),
            is_active: true,
            max_slippage_bps: MAX_SLIPPAGE_BPS,
            min_principal_protection_bps: MIN_PRINCIPAL_PROTECTION_BPS,
            harvest_ledger_offset: HARVEST_LEDGER_OFFSET,
        };

        env.storage().instance().set(&YieldBridgeDataKey::Admin, &admin);
        env.storage().instance().set(&YieldBridgeDataKey::Config, &config);
        env.storage().instance().set(&YieldBridgeDataKey::WhitelistCount, &0u32);
        env.storage().instance().set(&YieldBridgeDataKey::GroupReserve, &0i128);
    }

    /// Add a vault to the whitelist (admin only)
    pub fn add_vault_to_whitelist(
        env: Env,
        admin: Address,
        vault_address: Address,
        vault_name: Symbol,
        vault_type: VaultType,
        risk_level: u32,
    ) {
        Self::require_admin(&env, &admin);

        let vault = WhitelistedVault {
            vault_address: vault_address.clone(),
            vault_name,
            vault_type,
            is_active: true,
            added_at: env.ledger().sequence(),
            risk_level,
        };

        env.storage().instance().set(&YieldBridgeDataKey::WhitelistedVault(vault_address.clone()), &vault);
        
        let count: u32 = env.storage().instance().get(&YieldBridgeDataKey::WhitelistCount).unwrap_or(0);
        env.storage().instance().set(&YieldBridgeDataKey::WhitelistCount, &(count + 1));
    }

    /// Remove a vault from the whitelist (admin only)
    pub fn remove_vault_from_whitelist(env: Env, admin: Address, vault_address: Address) {
        Self::require_admin(&env, &admin);
        
        let mut vault: WhitelistedVault = env.storage().instance()
            .get(&YieldBridgeDataKey::WhitelistedVault(vault_address.clone()))
            .unwrap_or_else(|| panic!("Vault not whitelisted"));
        
        vault.is_active = false;
        env.storage().instance().set(&YieldBridgeDataKey::WhitelistedVault(vault_address), &vault);
    }

    /// Deploy idle funds to a selected yield vault
    pub fn deploy_to_yield_vault(
        env: Env,
        circle_id: u64,
        group_admin: Address,
        vault_address: Address,
        amount: i128,
        deposit_share_ratio: u32,
    ) -> Result<YieldBridgeDeployedEvent, YieldBridgeError> {
        // Check authorization
        Self::require_bridge_active(&env);
        
        // Validate vault is whitelisted
        let vault = Self::get_whitelisted_vault(&env, &vault_address)?;
        if !vault.is_active {
            return Err(YieldBridgeError::VaultNotWhitelisted);
        }

        // Validate deposit share ratio
        if deposit_share_ratio < MIN_DEPOSIT_SHARE_RATIO || deposit_share_ratio > MAX_DEPOSIT_SHARE_RATIO {
            return Err(YieldBridgeError::InvalidParameters);
        }

        // Get or create circle bridge state
        let mut bridge_state: CircleBridgeState = env.storage().instance()
            .get(&YieldBridgeDataKey::CircleBridge(circle_id))
            .unwrap_or_else(|| CircleBridgeState {
                circle_id,
                selected_vault: None,
                total_deposited: 0,
                principal_amount: 0,
                current_balance: 0,
                deposit_share_ratio: DEFAULT_DEPOSIT_SHARE_RATIO,
                last_harvest_ledger: 0,
                next_payout_ledger: 0,
                is_bailed_out: false,
                yield_earned: 0,
            });

        if bridge_state.is_bailed_out {
            return Err(YieldBridgeError::BailedOut);
        }

        // Calculate principal amount (protected portion)
        let principal_amount = amount * (10000 - deposit_share_ratio) as i128 / 10000;
        let yield_amount = amount * deposit_share_ratio as i128 / 10000;

        // Deposit to external vault with slippage protection
        let deposit_params = DepositParams {
            amount: yield_amount,
            min_return_amount: yield_amount * (10000 - MAX_SLIPPAGE_BPS) as i128 / 10000,
            max_slippage_bps: MAX_SLIPPAGE_BPS,
            deadline: env.ledger().sequence() + 100,
        };

        let vault_client = DeFiAdapterClient::new(&env, &vault_address);
        let vault_info = vault_client.deposit(&env, &group_admin, &yield_amount, &deposit_params);

        // Update bridge state
        bridge_state.selected_vault = Some(vault_address.clone());
        bridge_state.total_deposited += amount;
        bridge_state.principal_amount += principal_amount;
        bridge_state.current_balance = vault_info.current_balance;
        bridge_state.deposit_share_ratio = deposit_share_ratio;
        bridge_state.last_harvest_ledger = env.ledger().sequence();

        env.storage().instance().set(&YieldBridgeDataKey::CircleBridge(circle_id), &bridge_state);

        // Emit event
        let event = YieldBridgeDeployedEvent {
            circle_id,
            vault_address: vault_address.clone(),
            vault_name: vault.vault_name,
            amount_deployed: yield_amount,
            deposit_share_ratio,
            timestamp: env.ledger().sequence(),
        };

        // Store event
        let event_count: u64 = env.storage().instance()
            .get(&YieldBridgeDataKey::YieldBridgeDeployed(circle_id))
            .unwrap_or(0);
        env.storage().instance().set(&YieldBridgeDataKey::YieldBridgeDeployed(circle_id), &(event_count + 1));

        Ok(event)
    }

    /// Schedule a harvest for a specific circle
    pub fn schedule_harvest(
        env: Env,
        circle_id: u64,
        group_admin: Address,
        payout_ledger: u64,
    ) -> Result<(), YieldBridgeError> {
        Self::require_bridge_active(&env);

        let mut bridge_state: CircleBridgeState = env.storage().instance()
            .get(&YieldBridgeDataKey::CircleBridge(circle_id))
            .ok_or(YieldBridgeError::InvalidCircle)?;

        if bridge_state.selected_vault.is_none() {
            return Err(YieldBridgeError::InvalidParameters);
        }

        if bridge_state.is_bailed_out {
            return Err(YieldBridgeError::BailedOut);
        }

        // Calculate target ledger (2 ledger closes before payout)
        let target_ledger = payout_ledger.saturating_sub(HARVEST_LEDGER_OFFSET);
        
        // Ensure we're not scheduling in the past
        if target_ledger <= env.ledger().sequence() {
            return Err(YieldBridgeError::HarvestNotReady);
        }

        bridge_state.next_payout_ledger = payout_ledger;

        // Create harvest schedule
        let schedule_id: u64 = env.storage().instance()
            .get(&YieldBridgeDataKey::HarvestScheduleCount(circle_id))
            .unwrap_or(0);

        let schedule = HarvestSchedule {
            circle_id,
            target_ledger,
            harvest_amount: bridge_state.current_balance,
            is_executed: false,
        };

        env.storage().instance().set(&YieldBridgeDataKey::HarvestSchedule(circle_id, schedule_id), &schedule);
        env.storage().instance().set(&YieldBridgeDataKey::HarvestScheduleCount(circle_id), &(schedule_id + 1));
        env.storage().instance().set(&YieldBridgeDataKey::CircleBridge(circle_id), &bridge_state);

        Ok(())
    }

    /// Execute harvest yield (called automatically or manually)
    pub fn execute_harvest(
        env: Env,
        circle_id: u64,
        caller: Address,
    ) -> Result<YieldHarvestedEvent, YieldBridgeError> {
        Self::require_bridge_active(&env);

        let bridge_state: CircleBridgeState = env.storage().instance()
            .get(&YieldBridgeDataKey::CircleBridge(circle_id))
            .ok_or(YieldBridgeError::InvalidCircle)?;

        if bridge_state.selected_vault.is_none() {
            return Err(YieldBridgeError::InvalidParameters);
        }

        if bridge_state.is_bailed_out {
            return Err(YieldBridgeError::BailedOut);
        }

        let vault_address = bridge_state.selected_vault.unwrap();
        let vault = Self::get_whitelisted_vault(&env, &vault_address)?;

        // Check if vault is paused
        let vault_client = DeFiAdapterClient::new(&env, &vault_address);
        if vault_client.is_paused(&env) {
            return Self::handle_paused_vault(&env, circle_id, &vault_address, &bridge_state);
        }

        // Check if harvest is ready (2 ledger closes before payout)
        let current_ledger = env.ledger().sequence();
        if bridge_state.next_payout_ledger > 0 {
            let target_ledger = bridge_state.next_payout_ledger.saturating_sub(HARVEST_LEDGER_OFFSET);
            if current_ledger < target_ledger {
                return Err(YieldBridgeError::HarvestNotReady);
            }
        }

        // Harvest yield with slippage protection
        let min_return = bridge_state.current_balance * (10000 - MAX_SLIPPAGE_BPS) as i128 / 10000;
        let harvest_result = vault_client.harvest_yield(&env, &caller, &min_return);

        // Check principal protection
        check_principal_protection(
            bridge_state.principal_amount,
            harvest_result.principal_returned,
            MIN_PRINCIPAL_PROTECTION_BPS,
        ).map_err(|_| YieldBridgeError::PrincipalAtRisk)?;

        // If withdrawal returned less than expected due to IL, tap reserve
        if harvest_result.principal_returned < bridge_state.principal_amount {
            let shortfall = bridge_state.principal_amount - harvest_result.principal_returned;
            let reserve_balance: i128 = env.storage().instance()
                .get(&YieldBridgeDataKey::GroupReserve)
                .unwrap_or(0);

            if reserve_balance >= shortfall {
                env.storage().instance().set(&YieldBridgeDataKey::GroupReserve, &(reserve_balance - shortfall));
            } else {
                return Err(YieldBridgeError::ReserveInsufficient);
            }
        }

        // Update bridge state
        let mut updated_state = bridge_state;
        updated_state.current_balance = harvest_result.principal_returned + harvest_result.yield_amount;
        updated_state.yield_earned += harvest_result.yield_amount;
        updated_state.last_harvest_ledger = current_ledger;

        env.storage().instance().set(&YieldBridgeDataKey::CircleBridge(circle_id), &updated_state);

        // Emit event
        let event = YieldHarvestedEvent {
            circle_id,
            vault_address,
            amount_harvested: harvest_result.harvested_amount,
            principal_returned: harvest_result.principal_returned,
            yield_amount: harvest_result.yield_amount,
            slippage_bps: harvest_result.slippage_bps,
            timestamp: current_ledger,
        };

        Ok(event)
    }

    /// Handle paused vault scenario with immediate bailout
    fn handle_paused_vault(
        env: &Env,
        circle_id: u64,
        vault_address: &Address,
        bridge_state: &CircleBridgeState,
    ) -> Result<YieldHarvestedEvent, YieldBridgeError> {
        let vault_client = DeFiAdapterClient::new(env, vault_address);
        
        // Attempt emergency withdrawal
        let emergency_result = vault_client.emergency_withdraw(env, &bridge_state.selected_vault.clone().unwrap(), &bridge_state.current_balance);

        // Calculate shortfall
        let shortfall = bridge_state.principal_amount.saturating_sub(emergency_result.principal_returned);
        
        let reserve_balance: i128 = env.storage().instance()
            .get(&YieldBridgeDataKey::GroupReserve)
            .unwrap_or(0);

        let bailed_out = if shortfall > 0 && reserve_balance >= shortfall {
            env.storage().instance().set(&YieldBridgeDataKey::GroupReserve, &(reserve_balance - shortfall));
            true
        } else if shortfall > 0 {
            return Err(YieldBridgeError::ReserveInsufficient);
        } else {
            false
        };

        // Mark as bailed out
        let mut updated_state = bridge_state.clone();
        updated_state.is_bailed_out = true;
        env.storage().instance().set(&YieldBridgeDataKey::CircleBridge(circle_id), &updated_state);

        // Emit paused event
        let paused_event = VaultPausedEvent {
            circle_id,
            vault_address: vault_address.clone(),
            bailed_out,
            reserve_used: if bailed_out { shortfall } else { 0 },
            timestamp: env.ledger().sequence(),
        };

        // Return harvest event with bailout info
        Ok(YieldHarvestedEvent {
            circle_id,
            vault_address: vault_address.clone(),
            amount_harvested: emergency_result.harvested_amount,
            principal_returned: emergency_result.principal_returned,
            yield_amount: emergency_result.yield_amount,
            slippage_bps: emergency_result.slippage_bps,
            timestamp: env.ledger().sequence(),
        })
    }

    /// Add funds to group reserve
    pub fn add_to_reserve(env: Env, amount: i128) {
        let current: i128 = env.storage().instance()
            .get(&YieldBridgeDataKey::GroupReserve)
            .unwrap_or(0);
        env.storage().instance().set(&YieldBridgeDataKey::GroupReserve, &(current + amount));
    }

    /// Update bridge configuration (admin only)
    pub fn update_config(env: Env, admin: Address, config: YieldBridgeConfig) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&YieldBridgeDataKey::Config, &config);
    }

    /// Get bridge configuration
    pub fn get_config(env: Env) -> YieldBridgeConfig {
        env.storage().instance()
            .get(&YieldBridgeDataKey::Config)
            .unwrap()
    }

    /// Get whitelisted vault
    pub fn get_whitelisted_vault(env: &Env, vault_address: &Address) -> Result<WhitelistedVault, YieldBridgeError> {
        env.storage().instance()
            .get(&YieldBridgeDataKey::WhitelistedVault(vault_address.clone()))
            .ok_or(YieldBridgeError::VaultNotWhitelisted)
    }

    /// Get circle bridge state
    pub fn get_circle_bridge_state(env: Env, circle_id: u64) -> Option<CircleBridgeState> {
        env.storage().instance()
            .get(&YieldBridgeDataKey::CircleBridge(circle_id))
    }

    /// Get group reserve balance
    pub fn get_reserve_balance(env: Env) -> i128 {
        env.storage().instance()
            .get(&YieldBridgeDataKey::GroupReserve)
            .unwrap_or(0)
    }

    /// Helper: Require admin authorization
    fn require_admin(env: &Env, admin: &Address) {
        let stored_admin: Address = env.storage().instance()
            .get(&YieldBridgeDataKey::Admin)
            .unwrap();
        if stored_admin != *admin {
            panic!("Unauthorized");
        }
    }

    /// Helper: Require bridge is active
    fn require_bridge_active(env: &Env) {
        let config: YieldBridgeConfig = env.storage().instance()
            .get(&YieldBridgeDataKey::Config)
            .unwrap();
        if !config.is_active {
            panic!("Bridge not active");
        }
    }
}
