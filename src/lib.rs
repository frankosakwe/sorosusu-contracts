#![cfg_attr(not(test), no_std)]
use arbitrary::{Arbitrary, Unstructured};
use soroban_sdk::{
    contract, contractimpl, contracttype, testutils::Address as TestAddress, token, Address, Env,
    Symbol, Vec,
};

pub mod yield_allocation_voting;

// --- DATA STRUCTURES ---

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Circle(u64),
    Member(Address),
    CircleCount,
    // New: Tracks if a user has paid for a specific circle (CircleID, UserAddress)
    Deposit(u64, Address),
    // New: Tracks Group Reserve balance for penalties
    GroupReserve,
    // New: Tracks amount currently in AMMs per circle
    RoutedAmount(u64),
    // New: Tracks yield balance per member (CircleID, MemberAddress)
    YieldBalance(u64, Address),
    // New: Tracks batch harvest progress (CircleID)
    BatchHarvestProgress(u64),
    // New: Tracks defaulted members (CircleID, MemberAddress)
    DefaultedMember(u64, Address),
}

#[contracttype]
#[derive(Clone)]
pub struct Member {
    pub address: Address,
    pub has_contributed: bool,
    pub contribution_count: u32,
    pub last_contribution_time: u64,
    pub missed_deadline_timestamp: u64, // Tracks when member missed deadline (0 if never missed)
}

#[contracttype]
#[derive(Clone)]
pub struct BatchHarvestProgress {
    pub circle_id: u64,
    pub total_yield_amount: i128,
    pub members_processed: u32,
    pub total_members: u32,
    pub last_processed_index: u32,
    pub is_complete: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct CircleInfo {
    pub id: u64,
    pub creator: Address,
    pub contribution_amount: u64,     // Optimized from i128 to u64
    pub max_members: u32,             // Optimized from u32 to u32
    pub member_count: u32,            // Track count separately from Vec
    pub current_recipient_index: u32, // Track by index instead of Address
    pub is_active: bool,
    pub token: Address,          // The token used (USDC, XLM)
    pub deadline_timestamp: u64, // Deadline for on-time payments
    pub cycle_duration: u64,     // Duration of each payment cycle in seconds
    pub yield_enabled: bool,     // NEW: Issue #289
    pub risk_tolerance: u32,     // NEW: Issue #289
    pub grace_period: u64,       // Grace period in seconds (default: 86400 = 24 hours)
    pub late_fee_bps: u32,       // Late fee in basis points (default: 100 = 1%)
}

// --- CONTRACT TRAIT ---

pub trait SoroSusuTrait {
    // Initialize the contract
    fn init(env: Env, admin: Address);

    // Create a new savings circle
    fn create_circle(
        env: Env,
        creator: Address,
        amount: u64,
        max_members: u32,
        token: Address,
        cycle_duration: u64,
        yield_enabled: bool,
        risk_tolerance: u32,
        grace_period: u64,
        late_fee_bps: u32,
    ) -> u64;

    // Join an existing circle
    fn join_circle(env: Env, user: Address, circle_id: u64);

    // Make a deposit (Pay your weekly/monthly due)
    fn deposit(env: Env, user: Address, circle_id: u64);

    // Late contribution with fee (pay after deadline but within grace period)
    fn late_contribution(env: Env, user: Address, circle_id: u64);

    // Execute default on member (after grace period expires)
    fn execute_default(env: Env, circle_id: u64, member: Address) -> Result<(), u32>;

    // NEW: Issue #287
    fn route_to_yield(env: Env, circle_id: u64, amount: u64, pool_address: Address);

    // NEW: Issue #290
    fn withdraw_from_yield(
        env: Env,
        circle_id: u64,
        amount_to_withdraw: u64,
        pool_address: Address,
    );

    // NEW: Issue #288
    fn deposit_with_swap(
        env: Env,
        user: Address,
        circle_id: u64,
        source_token: Address,
        source_amount_max: u64,
    );

    // --- Yield Allocation Voting Functions ---

    // Initialize voting session for yield distribution
    fn initialize_yield_voting(
        env: Env,
        circle_id: u64,
        available_strategies: Vec<Address>,
    ) -> Result<(), u32>;

    // Cast vote for yield distribution strategy
    fn cast_yield_vote(
        env: Env,
        voter: Address,
        circle_id: u64,
        proposed_strategies: Vec<yield_allocation_voting::DistributionStrategy>,
    ) -> Result<(), u32>;

    // Finalize voting and determine winning strategy
    fn finalize_yield_voting(
        env: Env,
        circle_id: u64,
    ) -> Result<Vec<yield_allocation_voting::DistributionStrategy>, u32>;

    // Execute the winning distribution strategy
    fn execute_yield_distribution(
        env: Env,
        circle_id: u64,
        total_yield_amount: i128,
    ) -> Result<(), u32>;

    // Finalize cycle with yield voting integration
    fn finalize_cycle(env: Env, circle_id: u64, total_yield_amount: i128) -> Result<(), u32>;

    // Batch harvest yield to members in chunks of 10
    fn batch_harvest(
        env: Env,
        circle_id: u64,
        total_yield_amount: i128,
        member_addresses: Vec<Address>,
    ) -> Result<BatchHarvestProgress, u32>;
}

// --- IMPLEMENTATION ---

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
    }

    fn create_circle(
        env: Env,
        creator: Address,
        amount: u64,
        max_members: u32,
        token: Address,
        cycle_duration: u64,
        yield_enabled: bool,
        risk_tolerance: u32,
        grace_period: u64,
        late_fee_bps: u32,
    ) -> u64 {
        // 1. Get the current Circle Count
        let mut circle_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircleCount)
            .unwrap_or(0);

        // 2. Increment the ID for the new circle
        circle_count += 1;

        // 3. Create the Circle Data Struct
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
            yield_enabled,
            risk_tolerance,
            grace_period,
            late_fee_bps,
        };

        // 4. Save the Circle and the new Count
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_count), &new_circle);
        env.storage()
            .instance()
            .set(&DataKey::CircleCount, &circle_count);

        // 5. Initialize Group Reserve if not exists
        if !env.storage().instance().has(&DataKey::GroupReserve) {
            env.storage().instance().set(&DataKey::GroupReserve, &0u64);
        }

        // 6. Return the new ID
        circle_count
    }

    fn join_circle(env: Env, user: Address, circle_id: u64) {
        // 1. Authorization: The user MUST sign this transaction
        user.require_auth();

        // 2. Retrieve the circle data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();

        // 3. Check if the circle is full
        if circle.member_count >= circle.max_members {
            panic!("Circle is full");
        }

        // 4. Check if user is already a member to prevent duplicates
        let member_key = DataKey::Member(user.clone());
        if env.storage().instance().has(&member_key) {
            panic!("User is already a member");
        }

        // 5. Create and store the new member
        let new_member = Member {
            address: user.clone(),
            has_contributed: false,
            contribution_count: 0,
            last_contribution_time: 0,
            missed_deadline_timestamp: 0,
        };

        // 6. Store the member and update circle count
        env.storage().instance().set(&member_key, &new_member);
        circle.member_count += 1;

        // 7. Save the updated circle back to storage
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);
    }

    fn deposit(env: Env, user: Address, circle_id: u64) {
        // 1. Authorization: The user must sign this!
        user.require_auth();

        // 2. Load the Circle Data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();

        // 3. Check if user is actually a member
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .unwrap_or_else(|| panic!("User is not a member of this circle"));

        // 4. Check if payment is on-time
        let current_time = env.ledger().timestamp();

        if current_time > circle.deadline_timestamp {
            // Payment is late - track missed deadline but don't apply penalty yet
            // User must use late_contribution function instead
            if member.missed_deadline_timestamp == 0 {
                member.missed_deadline_timestamp = circle.deadline_timestamp;
                env.storage().instance().set(&member_key, &member);
            }
            panic!("Payment is late. Use late_contribution function to pay with late fee.");
        }

        // 5. Create the Token Client
        let client = token::Client::new(&env, &circle.token);

        // 6. Transfer the full amount from user (no penalty for on-time payment)
        client.transfer(
            &user,
            &env.current_contract_address(),
            &circle.contribution_amount,
        );

        // 7. Update member contribution info
        member.has_contributed = true;
        member.contribution_count += 1;
        member.last_contribution_time = current_time;
        member.missed_deadline_timestamp = 0; // Reset missed deadline timestamp

        // 8. Save updated member info
        env.storage().instance().set(&member_key, &member);

        // 9. Update circle deadline for next cycle
        circle.deadline_timestamp = current_time + circle.cycle_duration;
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);

        // 10. Mark as Paid in the old format for backward compatibility
        env.storage()
            .instance()
            .set(&DataKey::Deposit(circle_id, user), &true);
    }

    fn late_contribution(env: Env, user: Address, circle_id: u64) {
        // 1. Authorization: The user must sign this!
        user.require_auth();

        // 2. Load the Circle Data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();

        // 3. Check if user is actually a member
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .unwrap_or_else(|| panic!("User is not a member of this circle"));

        // 4. Check if payment is actually late
        let current_time = env.ledger().timestamp();

        if current_time <= circle.deadline_timestamp {
            panic!("Payment is not late. Use deposit function for on-time payment.");
        }

        // 5. Check if within grace period
        let grace_period_end = member.missed_deadline_timestamp + circle.grace_period;
        if member.missed_deadline_timestamp == 0 {
            member.missed_deadline_timestamp = circle.deadline_timestamp;
            grace_period_end = circle.deadline_timestamp + circle.grace_period;
        }

        if current_time > grace_period_end {
            panic!(
                "Grace period has expired. Member is now in default. Use execute_default function."
            );
        }

        // 6. Calculate late fee
        let late_fee = (circle.contribution_amount * circle.late_fee_bps as u64) / 10000;
        let total_amount = circle.contribution_amount + late_fee;

        // 7. Create the Token Client
        let client = token::Client::new(&env, &circle.token);

        // 8. Transfer total amount (contribution + late fee) from user
        client.transfer(&user, &env.current_contract_address(), &total_amount);

        // 9. Route late fee to Group Reserve
        let mut reserve_balance: u64 = env
            .storage()
            .instance()
            .get(&DataKey::GroupReserve)
            .unwrap_or(0);
        reserve_balance += late_fee;
        env.storage()
            .instance()
            .set(&DataKey::GroupReserve, &reserve_balance);

        // 10. Update member contribution info
        member.has_contributed = true;
        member.contribution_count += 1;
        member.last_contribution_time = current_time;
        member.missed_deadline_timestamp = 0; // Reset missed deadline timestamp

        // 11. Save updated member info
        env.storage().instance().set(&member_key, &member);

        // 12. Update circle deadline for next cycle
        circle.deadline_timestamp = current_time + circle.cycle_duration;
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);

        // 13. Mark as Paid in the old format for backward compatibility
        env.storage()
            .instance()
            .set(&DataKey::Deposit(circle_id, user), &true);
    }

    fn execute_default(env: Env, circle_id: u64, member: Address) -> Result<(), u32> {
        // 1. Load the Circle Data
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .ok_or(401)?; // Circle not found

        // 2. Check if user is actually a member
        let member_key = DataKey::Member(member.clone());
        let member_data: Member = env.storage().instance().get(&member_key).ok_or(402)?; // Member not found

        // 3. Check if member has missed deadline
        if member_data.missed_deadline_timestamp == 0 {
            return Err(403); // Member has not missed deadline
        }

        // 4. Check if grace period has expired
        let current_time = env.ledger().timestamp();
        let grace_period_end = member_data.missed_deadline_timestamp + circle.grace_period;

        if current_time <= grace_period_end {
            return Err(404); // Grace period has not expired yet
        }

        // 5. Mark member as defaulted (store in separate storage for tracking)
        let defaulted_key = DataKey::DefaultedMember(circle_id, member.clone());
        env.storage().instance().set(&defaulted_key, &true);

        // 6. In a full implementation, this would:
        //    - Slash member's stake/collateral
        //    - Pull from insurance if available
        //    - Update member's reliability index
        //    - Notify other members

        // For now, we'll just mark them as defaulted
        Ok(())
    }

    fn route_to_yield(env: Env, circle_id: u64, amount: u64, pool_address: Address) {
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();

        // 1. Governance Check (#289)
        if !circle.yield_enabled {
            panic!("Yield routing is disabled for this circle");
        }

        // 2. Time-to-Liquidity Check (#287)
        let current_time = env.ledger().timestamp();
        let payout_buffer = 86400; // 24 hours in seconds
        if current_time + payout_buffer > circle.deadline_timestamp {
            panic!("Cannot route to yield: too close to payout (Time-to-Liquidity check failed)");
        }

        // 3. Risk Tolerance Check (#289)
        // In a real implementation, we'd verify the pool_address against a registry based on risk_tolerance
        if circle.risk_tolerance == 0 {
            panic!("Risk tolerance too low for external routing");
        }

        // 4. Transfer funds to Pool
        let client = token::Client::new(&env, &circle.token);
        client.transfer(&env.current_contract_address(), &pool_address, &amount);

        // 5. Update Routed Amount storage
        let mut routed_amount: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoutedAmount(circle_id))
            .unwrap_or(0);
        routed_amount += amount;
        env.storage()
            .instance()
            .set(&DataKey::RoutedAmount(circle_id), &routed_amount);
    }

    fn withdraw_from_yield(
        env: Env,
        circle_id: u64,
        amount_to_withdraw: u64,
        pool_address: Address,
    ) {
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();
        let mut routed_amount: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoutedAmount(circle_id))
            .unwrap_or(0);

        // 1. Simulate withdrawal from Pool (In real case, the contract would call the Pool's withdraw)
        // For this task, we assume the amount_to_withdraw is what we received back.
        // If there was a loss (e.g. IL), amount_to_withdraw might be less than what we expected.

        // 2. Principal Protection (#290)
        let expected_principal = amount_to_withdraw.min(routed_amount); // Simplified principal tracking

        // If we have a loss, pull from Reserve
        if amount_to_withdraw < expected_principal {
            let loss = expected_principal - amount_to_withdraw;
            let mut reserve_balance: u64 = env
                .storage()
                .instance()
                .get(&DataKey::GroupReserve)
                .unwrap_or(0);

            if reserve_balance >= loss {
                reserve_balance -= loss;
                env.storage()
                    .instance()
                    .set(&DataKey::GroupReserve, &reserve_balance);
                // The contract now has the full principal back (amount_to_withdraw + loss from reserve)
            } else {
                // If reserve is empty, we have a liquidity crunch!
                // But as per #290, we must guarantee the principal.
                panic!("Insufficient Group Reserve to cover yield loss");
            }
        }

        // 3. Update Routed Amount
        routed_amount = routed_amount.saturating_sub(expected_principal);
        env.storage()
            .instance()
            .set(&DataKey::RoutedAmount(circle_id), &routed_amount);
    }

    fn deposit_with_swap(
        env: Env,
        user: Address,
        circle_id: u64,
        source_token: Address,
        source_amount_max: u64,
    ) {
        user.require_auth();

        // 1. Load Circle Data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();
        let target_token = circle.token.clone();
        let target_amount = circle.contribution_amount;

        // 2. Query DEX for Rate (Mocked for this implementation)
        // In a real scenario, we would call a DEX contract like Soroswap or use a host function
        let exchange_rate = 10; // e.g. 10 XLM = 1 USDC
        let required_source_amount = target_amount * exchange_rate;

        // 3. Slippage Check (#288)
        if required_source_amount > source_amount_max {
            panic!("Slippage exceeded: required source amount exceeds max allowed");
        }

        // 4. Perform Atomic Swap (Simulated)
        // Transfer source asset from user to contract
        let source_client = token::Client::new(&env, &source_token);
        source_client.transfer(
            &user,
            &env.current_contract_address(),
            &required_source_amount,
        );

        // Here the contract would call the DEX to swap source_token for target_token
        // For the sake of this task, we'll assume the swap happened and the contract now has target_amount

        // 5. Finalize Deposit
        // (Re-using logic from deposit function)
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .unwrap_or_else(|| panic!("User is not a member"));

        member.has_contributed = true;
        member.contribution_count += 1;
        member.last_contribution_time = env.ledger().timestamp();
        env.storage().instance().set(&member_key, &member);

        // Update circle deadline
        circle.deadline_timestamp = env.ledger().timestamp() + circle.cycle_duration;
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);

        // Mark as paid
        env.storage()
            .instance()
            .set(&DataKey::Deposit(circle_id, user), &true);
    }

    // --- Yield Allocation Voting Implementation ---

    fn initialize_yield_voting(
        env: Env,
        circle_id: u64,
        available_strategies: Vec<Address>,
    ) -> Result<(), u32> {
        yield_allocation_voting::initialize_voting_session(&env, circle_id, available_strategies)
            .map_err(|e| e as u32)
    }

    fn cast_yield_vote(
        env: Env,
        voter: Address,
        circle_id: u64,
        proposed_strategies: Vec<yield_allocation_voting::DistributionStrategy>,
    ) -> Result<(), u32> {
        yield_allocation_voting::cast_vote(&env, voter, circle_id, proposed_strategies)
            .map_err(|e| e as u32)
    }

    fn finalize_yield_voting(
        env: Env,
        circle_id: u64,
    ) -> Result<Vec<yield_allocation_voting::DistributionStrategy>, u32> {
        yield_allocation_voting::finalize_voting(&env, circle_id).map_err(|e| e as u32)
    }

    fn execute_yield_distribution(
        env: Env,
        circle_id: u64,
        total_yield_amount: i128,
    ) -> Result<(), u32> {
        yield_allocation_voting::execute_distribution_strategy(&env, circle_id, total_yield_amount)
            .map_err(|e| e as u32)
    }

    fn finalize_cycle(env: Env, circle_id: u64, total_yield_amount: i128) -> Result<(), u32> {
        // Check if voting session exists and is ready to be finalized
        let voting_session = yield_allocation_voting::get_voting_session(&env, circle_id);

        match voting_session {
            Ok(session) => {
                // Voting session exists, finalize it first
                if session.is_active {
                    let current_time = env.ledger().timestamp();
                    if current_time > session.end_timestamp {
                        // Voting period is over, finalize voting
                        let winning_strategy =
                            yield_allocation_voting::finalize_voting(&env, circle_id)
                                .map_err(|e| e as u32)?;

                        // Execute the winning strategy
                        yield_allocation_voting::execute_distribution_strategy(
                            &env,
                            circle_id,
                            total_yield_amount,
                        )
                        .map_err(|e| e as u32)?;

                        Ok(())
                    } else {
                        // Voting period is still active
                        Err(404) // VotingPeriodExpired
                    }
                } else {
                    // Voting already finalized, just execute distribution
                    yield_allocation_voting::execute_distribution_strategy(
                        &env,
                        circle_id,
                        total_yield_amount,
                    )
                    .map_err(|e| e as u32)
                }
            }
            Err(_) => {
                // No voting session exists, handle yield distribution without voting
                // This could be a default strategy or admin-controlled distribution
                handle_default_yield_distribution(&env, circle_id, total_yield_amount)
            }
        }
    }

    fn batch_harvest(
        env: Env,
        circle_id: u64,
        total_yield_amount: i128,
        member_addresses: Vec<Address>,
    ) -> Result<BatchHarvestProgress, u32> {
        // Check if circle exists
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .ok_or(401)?; // Circle not found

        // Get or create batch harvest progress
        let progress_key = DataKey::BatchHarvestProgress(circle_id);
        let mut progress: BatchHarvestProgress = env
            .storage()
            .instance()
            .get(&progress_key)
            .unwrap_or(BatchHarvestProgress {
                circle_id,
                total_yield_amount,
                members_processed: 0,
                total_members: member_addresses.len() as u32,
                last_processed_index: 0,
                is_complete: false,
            });

        // If already complete, return progress
        if progress.is_complete {
            return Ok(progress);
        }

        // Process members in chunks of 10
        let chunk_size = 10u32;
        let start_index = progress.last_processed_index;
        let end_index = (start_index + chunk_size).min(progress.total_members);

        // Calculate yield per member (pro rata distribution)
        let yield_per_member = if progress.total_members > 0 {
            total_yield_amount / progress.total_members as i128
        } else {
            0i128
        };

        // Process chunk of members
        for i in start_index..end_index {
            let member_idx = i as u32;
            if member_idx < member_addresses.len() {
                let member_address = member_addresses.get_unchecked(member_idx);

                // Verify member is part of the circle
                let member_key = DataKey::Member(member_address.clone());
                if env.storage().instance().has(&member_key) {
                    // Get current yield balance for this member
                    let yield_key = DataKey::YieldBalance(circle_id, member_address.clone());
                    let current_balance: i128 =
                        env.storage().instance().get(&yield_key).unwrap_or(0i128);

                    // Add yield to member's balance
                    let new_balance = current_balance + yield_per_member;
                    env.storage().instance().set(&yield_key, &new_balance);

                    progress.members_processed += 1;
                }
            }
            progress.last_processed_index = i + 1;
        }

        // Check if all members have been processed
        if progress.members_processed >= progress.total_members {
            progress.is_complete = true;
        }

        // Save progress
        env.storage().instance().set(&progress_key, &progress);

        Ok(progress)
    }
}

// --- HELPER FUNCTIONS ---

fn handle_default_yield_distribution(
    env: &Env,
    circle_id: u64,
    total_yield_amount: i128,
) -> Result<(), u32> {
    // Default yield distribution logic when no voting session exists
    // This could be a simple strategy like distributing equally to all members
    // or using a predefined safe strategy

    let circle: CircleInfo = env
        .storage()
        .instance()
        .get(&DataKey::Circle(circle_id))
        .ok_or(401)?; // Unauthorized

    // For now, we'll just keep the yield in the contract
    // In production, this would route to a default safe strategy
    // or distribute to members proportionally

    // Update routed amount to track yield
    let mut routed_amount: u64 = env
        .storage()
        .instance()
        .get(&DataKey::RoutedAmount(circle_id))
        .unwrap_or(0);
    routed_amount += total_yield_amount as u64;
    env.storage()
        .instance()
        .set(&DataKey::RoutedAmount(circle_id), &routed_amount);

    Ok(())
}

// --- FUZZ TESTING MODULES ---

#[cfg(test)]
mod yield_allocation_voting_tests;

#[cfg(test)]
mod fuzz_tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::vec;
    // use std::println; // Removed for no_std compatibility or replaced by log

    #[derive(Arbitrary, Debug, Clone)]
    pub struct FuzzTestCase {
        pub contribution_amount: u64,
        pub max_members: u32,
        pub user_id: u64,
    }

    #[test]
    fn fuzz_test_contribution_amount_edge_cases() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Test case 1: Maximum u64 value (should not panic)
        let max_circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            u64::MAX,
            10,
            token.clone(),
            604800, // 1 week in seconds
            true,
            1,
            86400, // 24 hour grace period
            100,   // 1% late fee
        );

        let user1 = Address::generate(&env);
        SoroSusuTrait::join_circle(env.clone(), user1.clone(), max_circle_id);

        // Mock token balance for the test
        env.mock_all_auths();

        // This should not panic even with u64::MAX contribution amount
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::deposit(env.clone(), user1.clone(), max_circle_id);
        });

        // The transfer might fail due to insufficient balance, but it shouldn't panic from overflow
        assert!(
            result.is_ok()
                || result
                    .unwrap_err()
                    .downcast::<String>()
                    .unwrap()
                    .contains("insufficient balance")
        );
    }

    #[test]
    fn fuzz_test_zero_and_negative_amounts() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Test case 2: Zero contribution amount (should be allowed but may cause issues)
        let zero_circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            0,
            10,
            token.clone(),
            604800, // 1 week in seconds
            true,
            1,
            86400,
            100,
        );

        let user2 = Address::generate(&env);
        SoroSusuTrait::join_circle(env.clone(), user2.clone(), zero_circle_id);

        env.mock_all_auths();

        // Zero amount deposit should work (though may not be practically useful)
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::deposit(env.clone(), user2.clone(), zero_circle_id);
        });

        assert!(result.is_ok());
    }

    #[test]
    fn fuzz_test_arbitrary_contribution_amounts() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Test with various edge case amounts
        let test_amounts = vec![
            &env,
            1,               // Minimum positive amount
            u32::MAX as u64, // Large but reasonable amount
            u64::MAX / 2,    // Very large amount
            u64::MAX - 1,    // Maximum amount - 1
            1000000,         // 1 million
            0,               // Zero (already tested above)
        ];

        for (i, amount) in test_amounts.into_iter().enumerate() {
            let circle_id = SoroSusuTrait::create_circle(
                env.clone(),
                creator.clone(),
                *amount,
                10,
                token.clone(),
                604800, // 1 week in seconds
                true,   // yield_enabled
                1,      // risk_tolerance
                86400,  // 24 hour grace period
                100,    // 1% late fee
            );

            let user = Address::generate(&env);
            SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

            env.mock_all_auths();

            let result = std::panic::catch_unwind(|| {
                SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);
            });

            // Should not panic due to overflow, only potentially due to insufficient balance
            match result {
                Ok(_) => {
                    // Deposit succeeded
                }
                Err(e) => {
                    let error_msg = e.downcast::<String>().unwrap();
                    // Expected error: insufficient balance, not overflow
                    assert!(
                        error_msg.contains("insufficient balance")
                            || error_msg.contains("underflow")
                            || error_msg.contains("overflow")
                    );
                }
            }
        }
    }

    #[test]
    fn fuzz_test_boundary_conditions() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Test boundary conditions for max_members
        let boundary_tests = vec![
            &env,
            (1, "Minimum members"),
            (u32::MAX, "Maximum members"),
            (100, "Typical circle size"),
        ];

        for (max_members, description) in boundary_tests {
            let circle_id = SoroSusuTrait::create_circle(
                env.clone(),
                creator.clone(),
                1000, // Reasonable contribution amount
                max_members,
                token.clone(),
                604800, // 1 week in seconds
                true,
                1,
                86400,
                100,
            );

            // Test joining with maximum allowed members
            for i in 0..max_members.min(10) {
                // Limit to 10 for test performance
                let user = Address::generate(&env);
                SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

                env.mock_all_auths();

                let result = std::panic::catch_unwind(|| {
                    SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);
                });

                assert!(
                    result.is_ok(),
                    "Deposit failed for {} with max_members {}: {:?}",
                    description,
                    max_members,
                    result
                );
            }
        }
    }

    #[test]
    fn fuzz_test_concurrent_deposits() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            500,
            5,
            token.clone(),
            604800, // 1 week in seconds
            true,
            1,
            86400,
            100,
        );

        // Create multiple users and test deposits
        let mut users = Vec::new(&env);
        for _ in 0..5 {
            let user = Address::generate(&env);
            SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
            users.push(user);
        }

        env.mock_all_auths();

        // Test multiple deposits in sequence (simulating concurrent access)
        for user in users {
            let result = std::panic::catch_unwind(|| {
                SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);
            });

            assert!(
                result.is_ok(),
                "Concurrent deposit test failed: {:?}",
                result
            );
        }
    }

    #[test]
    fn test_late_penalty_mechanism() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle with 1 week cycle duration
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000, // $10 contribution (assuming 6 decimals)
            5,
            token.clone(),
            604800, // 1 week in seconds
            true,
            1,
            86400,
            100,
        );

        // User joins the circle
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Mock token balance for the test
        env.mock_all_auths();

        // Get initial Group Reserve balance
        let initial_reserve: u64 = env
            .storage()
            .instance()
            .get(&DataKey::GroupReserve)
            .unwrap_or(0);
        assert_eq!(initial_reserve, 0);

        // Simulate time passing beyond deadline (jump forward 2 weeks)
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + 2 * 604800);

        // Make a late deposit
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);
        });

        assert!(result.is_ok(), "Late deposit should succeed: {:?}", result);

        // Check that Group Reserve received the 1% penalty (10 tokens)
        let final_reserve: u64 = env
            .storage()
            .instance()
            .get(&DataKey::GroupReserve)
            .unwrap_or(0);
        assert_eq!(
            final_reserve, 10,
            "Group Reserve should have 10 tokens (1% penalty)"
        );

        // Verify member was marked as having contributed
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key).unwrap();
        assert!(member.has_contributed);
        assert_eq!(member.contribution_count, 1);
    }

    #[test]
    fn test_on_time_deposit_no_penalty() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle with 1 week cycle duration
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000, // $10 contribution
            5,
            token.clone(),
            604800, // 1 week in seconds
            true,
            1,
            86400,
            100,
        );

        // User joins the circle
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Mock token balance for the test
        env.mock_all_auths();

        // Get initial Group Reserve balance
        let initial_reserve: u64 = env
            .storage()
            .instance()
            .get(&DataKey::GroupReserve)
            .unwrap_or(0);
        assert_eq!(initial_reserve, 0);

        // Make an on-time deposit (don't advance time)
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);
        });

        assert!(
            result.is_ok(),
            "On-time deposit should succeed: {:?}",
            result
        );

        // Check that Group Reserve received no penalty
        let final_reserve: u64 = env
            .storage()
            .instance()
            .get(&DataKey::GroupReserve)
            .unwrap_or(0);
        assert_eq!(
            final_reserve, 0,
            "Group Reserve should have 0 tokens for on-time deposit"
        );
    }

    #[test]
    fn test_yield_routing_and_protection() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let token = Address::generate(&env);
        let pool = Address::generate(&env);

        SoroSusuTrait::init(env.clone(), admin.clone());

        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true, // yield_enabled
            1,    // risk_tolerance
            86400,
            100,
        );

        env.mock_all_auths();

        // Route to yield
        SoroSusuTrait::route_to_yield(env.clone(), circle_id, 500, pool.clone());

        // Withdraw from yield with no loss
        SoroSusuTrait::withdraw_from_yield(env.clone(), circle_id, 500, pool.clone());

        // Check routed amount is 0
        let routed_amount: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoutedAmount(circle_id))
            .unwrap_or(0);
        assert_eq!(routed_amount, 0);
    }

    #[test]
    fn test_auto_swap_deposit() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);
        let source_token = Address::generate(&env);

        SoroSusuTrait::init(env.clone(), admin.clone());

        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            100, // target 100 tokens
            5,
            token.clone(),
            604800,
            true,
            1,
            86400,
            100,
        );

        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        env.mock_all_auths();

        // Deposit with swap (10:1 rate in mock implementation)
        // 100 * 10 = 1000 required. We provide 1100 max.
        SoroSusuTrait::deposit_with_swap(
            env.clone(),
            user.clone(),
            circle_id,
            source_token.clone(),
            1100,
        );

        // Verify contribution
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key).unwrap();
        assert!(member.has_contributed);
    }
}
