# Dynamic "Susu-to-DeFi" Yield Bridge Adapter - Implementation Guide

## Overview
This implementation addresses Issue #377: Dynamic "Susu-to-DeFi" Yield Bridge Adapter for the SoroSusu Protocol. The adapter optimizes capital efficiency of idle funds sitting in Susu cycle vaults by allowing deployment to external yield-generating protocols on Stellar.

## Architecture

### 1. DeFi Adapter Trait (`defi_adapter_trait.rs`)
A standard trait that all external yield vaults must implement to be compatible with the SoroSusu yield bridge adapter.

**Key Components:**
- `DeFiAdapterTrait`: Standard interface for external vaults
- `VaultInfo`: Metadata about vault state and performance
- `VaultType`: Enum for different vault types (AMM, Lending, LiquidStaking, etc.)
- `DepositParams`/`WithdrawalParams`: Parameter structures with slippage protection
- `YieldHarvestResult`: Structure for harvest operation results
- `VaultHealth`: Health check information
- Helper functions for validation and slippage calculation

**Methods:**
- `initialize()`: Initialize the adapter
- `deposit()`: Deposit funds with slippage protection
- `withdraw()`: Withdraw funds with slippage protection
- `harvest_yield()`: Harvest yield only (keep principal)
- `get_vault_info()`: Get current vault information
- `check_health()`: Check vault health status
- `is_paused()`: Check if vault is paused
- `get_estimated_yield()`: Get estimated yield for a period
- `emergency_withdraw()`: Emergency withdrawal
- `update_config()`: Update configuration (admin only)

### 2. Yield Bridge Adapter (`yield_bridge_adapter.rs`)
Main contract that manages the bridge between SoroSusu circles and external yield vaults.

**Key Features:**
- **Whitelist Management**: Admin can approve/revoke external vaults
- **Deposit Share Ratio**: Configurable ratio (10%-80%) to protect principal
- **Automated Harvest**: Executes 2 ledger closes before scheduled payout
- **Slippage Protection**: Strict 0.5% (50 basis points) limit on all DEX interactions
- **Paused Vault Handling**: Immediate group bailout if vault is paused
- **IL Protection**: Taps Group Reserve Vault for impermanent loss
- **Event Emission**: YieldBridgeDeployed, YieldHarvested, VaultPaused events
- **Cross-Contract Authorization**: Secure permission management via Soroban

**Data Structures:**
- `YieldBridgeConfig`: Bridge configuration
- `WhitelistedVault`: Whitelisted vault information with risk level
- `CircleBridgeState`: Per-circle bridge state
- `HarvestSchedule`: Harvest scheduling information
- Events: `YieldBridgeDeployedEvent`, `YieldHarvestedEvent`, `VaultPausedEvent`

**Key Methods:**
- `initialize()`: Initialize the bridge adapter
- `add_vault_to_whitelist()`: Add vault to approved list
- `remove_vault_from_whitelist()`: Remove vault from approved list
- `deploy_to_yield_vault()`: Deploy idle funds to yield vault
- `schedule_harvest()`: Schedule harvest for a specific payout
- `execute_harvest()`: Execute harvest (automated or manual)
- `add_to_reserve()`: Add funds to group reserve
- `update_config()`: Update bridge configuration
- `get_config()`: Get bridge configuration
- `get_circle_bridge_state()`: Get circle bridge state
- `get_reserve_balance()`: Get group reserve balance

## Security Features

### 1. Principal Protection
- **Deposit Share Ratio**: Only a configurable percentage (10%-80%) of funds are deployed to yield vaults
- The remainder (20%-90%) is kept as protected principal
- Example: With 50% ratio, 50% of funds earn yield while 50% are protected

### 2. Slippage Bounds
- **Strict 0.5% limit** on all external DEX interactions
- Enforced at both deposit and withdrawal
- Configurable by admin but defaults to 50 basis points

### 3. Paused Vault Handling
- Automatic detection of paused vaults
- Immediate group bailout triggered
- Emergency withdrawal executed
- Reserve vault tapped if needed

### 4. Impermanent Loss (IL) Protection
- If AMM withdrawal returns less than expected due to IL
- Group Reserve Vault automatically covers the shortfall
- Ensures principal is never at risk

### 5. Cross-Contract Authorization
- Utilizes Soroban's native cross-contract authorization
- Admin-only operations protected
- Vault operations require proper authorization

## Constants

```rust
const MAX_SLIPPAGE_BPS: u32 = 50; // 0.5%
const MIN_PRINCIPAL_PROTECTION_BPS: u32 = 9500; // 95%
const HARVEST_LEDGER_OFFSET: u64 = 2; // 2 ledger closes before payout
const DEFAULT_DEPOSIT_SHARE_RATIO: u32 = 5000; // 50%
const MIN_DEPOSIT_SHARE_RATIO: u32 = 1000; // 10%
const MAX_DEPOSIT_SHARE_RATIO: u32 = 8000; // 80%
```

## Usage Example

### 1. Initialize the Bridge
```rust
YieldBridgeAdapter::initialize(env, admin_address);
```

### 2. Whitelist a Vault
```rust
YieldBridgeAdapter::add_vault_to_whitelist(
    env,
    admin,
    vault_address,
    Symbol::short("SoroswapAMM"),
    VaultType::AMM,
    30, // risk level 0-100
);
```

### 3. Deploy Idle Funds
```rust
let event = YieldBridgeAdapter::deploy_to_yield_vault(
    env,
    circle_id,
    group_admin,
    vault_address,
    10_000_000i128, // amount
    5000u32, // 50% to yield
)?;
```

### 4. Schedule Harvest
```rust
YieldBridgeAdapter::schedule_harvest(
    env,
    circle_id,
    group_admin,
    payout_ledger, // when payout occurs
)?;
```

### 5. Execute Harvest (2 ledger closes before payout)
```rust
let event = YieldBridgeAdapter::execute_harvest(
    env,
    circle_id,
    caller_address,
)?;
```

## Testing

### Unit Tests (`yield_bridge_adapter_tests.rs`)
Comprehensive unit tests covering:
- Initialization and configuration
- Whitelist management
- Vault deployment
- Deposit share ratio validation
- Harvest scheduling and execution
- Slippage bounds enforcement
- Paused vault handling
- Bailed out state
- Reserve management
- Multiple independent circles
- Market volatility scenarios

### Integration Tests (`yield_bridge_integration_tests.rs`)
End-to-end integration tests verifying acceptance criteria:

**Acceptance 1**: Idle capital is successfully and safely deployed to generate interest across various Stellar protocols
- Tests deployment to multiple vault types (AMM, Lending, LiquidStaking, YieldAggregator, Custom)
- Verifies independent circle states
- Confirms principal protection

**Acceptance 2**: Payout liquidity is mathematically guaranteed through automated, time-bound unbonding logic
- Tests harvest scheduling and timing
- Verifies 2-ledger-close offset before payout
- Confirms principal protection is maintained
- Tests automated execution timing

**Acceptance 3**: The adapter pattern allows for future integration with any protocol implementing the standard trait
- Tests whitelisting of different vault types
- Verifies independent vault selection
- Tests vault removal and active state management
- Confirms composability

Additional integration tests:
- End-to-end yield generation cycle
- Slippage protection across multiple vaults
- Reserve vault fallback mechanism

## Integration with Main Contract

The yield bridge adapter is designed to integrate with the main SoroSusu contract (`lib.rs`). The modules have been added:

```rust
// Issue #377: Dynamic "Susu-to-DeFi" Yield Bridge Adapter
pub mod defi_adapter_trait;
pub mod yield_bridge_adapter;
#[cfg(test)]
mod yield_bridge_adapter_tests;
#[cfg(test)]
mod yield_bridge_integration_tests;
```

## Deployment Workflow

1. **Deploy YieldBridgeAdapter contract**
2. **Initialize with admin address**
3. **Whitelist approved yield vaults** (e.g., Soroswap AMM, Blend lending, etc.)
4. **For each Susu circle:**
   - Group admin selects vault from whitelist
   - Configures deposit share ratio based on risk tolerance
   - Deploys idle funds to yield vault
   - Schedules harvest before next payout
5. **Automated harvest** executes 2 ledger closes before payout
6. **Principal + Yield** returned to circle for payout

## Risk Management

### Vault Risk Levels
- **0-30**: Low risk (e.g., lending protocols)
- **31-60**: Medium risk (e.g., AMMs)
- **61-100**: High risk (e.g., yield aggregators)

### Deposit Share Ratio Recommendations
- **Low risk vaults**: 60-80% to yield
- **Medium risk vaults**: 40-60% to yield
- **High risk vaults**: 20-40% to yield

### Reserve Vault
- Should be funded with sufficient liquidity to cover potential IL
- Recommended: 50-100% of total protected principal across all circles
- Can be replenished from successful yield harvests

## Future Enhancements

1. **Multi-Vault Diversification**: Deploy to multiple vaults simultaneously
2. **Dynamic Rebalancing**: Automatically adjust allocations based on performance
3. **Yield Optimization**: Auto-select best performing vaults
4. **Governance Integration**: DAO-controlled vault whitelist
5. **Insurance Integration**: Integration with DeFi insurance protocols

## Error Codes

### DeFiAdapterError (500-516)
- 501: Unauthorized
- 502: InsufficientBalance
- 503: InvalidAmount
- 504: AdapterNotActive
- 505: WithdrawalNotAllowed
- 506: DepositFailed
- 507: WithdrawalFailed
- 508: SlippageExceeded
- 509: VaultPaused
- 510: InvalidVault
- 511: InsufficientLiquidity
- 512: UnauthorizedVault
- 513: InvalidParameters
- 514: PrincipalAtRisk
- 515: HarvestNotReady
- 516: BailedOut

### YieldBridgeError (601-616)
- 601: Unauthorized
- 602: VaultNotWhitelisted
- 603: InsufficientBalance
- 604: InvalidAmount
- 605: SlippageExceeded
- 606: VaultPaused
- 607: PrincipalAtRisk
- 608: HarvestNotReady
- 609: InvalidCircle
- 610: BridgeNotActive
- 611: ReserveInsufficient
- 612: UnauthorizedAdmin
- 613: InvalidParameters
- 614: BailedOut
- 615: WithdrawalFailed
- 616: DepositFailed

## Compilation

To compile the contract:
```bash
cargo build --target wasm32-unknown-unknown --release
```

To run tests:
```bash
cargo test
```

## Notes

- This implementation requires Rust and the Soroban SDK to be installed
- The contract uses Soroban SDK version 21.0.0
- All storage operations use the instance storage pattern
- The implementation follows Soroban best practices for contract design
