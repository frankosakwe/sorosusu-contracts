use soroban_sdk::{contractimpl, Env, Symbol, Address};

pub struct AutoCompounder;

/// Contract logic for auto-compounding the Group Insurance Reserve
#[contractimpl]
impl AutoCompounder {
    /// Stake reserve funds into a low-risk yield protocol
    pub fn stake_reserve(env: Env, amount: i128) {
        let current: i128 = env.storage().get(&Symbol::short("reserve_balance")).unwrap_or(0);
        env.storage().set(&Symbol::short("reserve_balance"), &(current + amount));
    }

    /// Claim rewards, swap to base asset, and re-stake
    pub fn compound_interest(env: Env) {
        // Simulate reward accrual
        let rewards: i128 = env.storage().get(&Symbol::short("pending_rewards")).unwrap_or(0);

        if rewards > 0 {
            // Add rewards to reserve balance
            let reserve: i128 = env.storage().get(&Symbol::short("reserve_balance")).unwrap_or(0);
            env.storage().set(&Symbol::short("reserve_balance"), &(reserve + rewards));

            // Reset pending rewards
            env.storage().set(&Symbol::short("pending_rewards"), &0);

            // Emit event for off-chain monitoring
            env.events().publish(
                (Symbol::short("compound_event"),),
                (rewards, reserve + rewards),
            );
        }
    }

    /// Simulate external yield accrual (called by protocol hook)
    pub fn accrue_rewards(env: Env, amount: i128) {
        let current: i128 = env.storage().get(&Symbol::short("pending_rewards")).unwrap_or(0);
        env.storage().set(&Symbol::short("pending_rewards"), &(current + amount));
    }

    /// Get current reserve balance
    pub fn get_reserve_balance(env: Env) -> i128 {
        env.storage().get(&Symbol::short("reserve_balance")).unwrap_or(0)
    }

    /// Get pending rewards
    pub fn get_pending_rewards(env: Env) -> i128 {
        env.storage().get(&Symbol::short("pending_rewards")).unwrap_or(0)
    }
}
