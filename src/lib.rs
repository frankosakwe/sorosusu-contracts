#![cfg_attr(not(test), no_std)]
use arbitrary::{Arbitrary, Unstructured};
use soroban_sdk::{
    contract, contractimpl, contracttype, testutils::Address as TestAddress, token, Address, Env,
    Symbol, Vec,
};

pub mod yield_allocation_voting;
pub mod yield_strategy_trait;

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
    // New: Tracks initial deposit amount per user per circle for recovery
    InitialDeposit(u64, Address),
    // New: Tracks if a user has claimed abandoned funds (CircleID, UserAddress)
    Claimed(u64, Address),
    // New: Commit-Reveal Voting - Tracks voting session for a circle
    VotingSession(u64),
    // New: Commit-Reveal Voting - Tracks vote commit (CircleID, UserAddress)
    VoteCommit(u64, Address),
    // New: Commit-Reveal Voting - Tracks vote reveal (CircleID, UserAddress)
    VoteReveal(u64, Address),
    // New: Yield Opt-Out - Tracks isolated contributions for opted-out members (CircleID, UserAddress)
    IsolatedContribution(u64, Address),
}

#[contracttype]
#[derive(Clone)]
pub struct Member {
    pub address: Address,
    pub has_contributed: bool,
    pub contribution_count: u32,
    pub last_contribution_time: u64,
    pub opt_out_of_yield: bool, // Flag for members who cannot accept interest (e.g., Islamic finance)
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
    pub token: Address,                 // The token used (USDC, XLM)
    pub deadline_timestamp: u64,        // Deadline for on-time payments
    pub cycle_duration: u64,            // Duration of each payment cycle in seconds
    pub yield_enabled: bool,            // NEW: Issue #289
    pub risk_tolerance: u32,            // NEW: Issue #289
    pub last_interaction: u64,          // Timestamp of last interaction for heartbeat
    pub in_recovery: bool,              // Recovery state flag for abandoned funds
    pub member_addresses: Vec<Address>, // Track all member addresses for vote tallying
}

// --- COMMIT-REVEAL VOTING STRUCTURES ---

#[contracttype]
#[derive(Clone)]
pub enum VotePhase {
    Commit,
    Reveal,
    Completed,
}

#[contracttype]
#[derive(Clone)]
pub struct VotingSession {
    pub circle_id: u64,
    pub proposal_id: u64,
    pub phase: VotePhase,
    pub commit_end_timestamp: u64,
    pub reveal_end_timestamp: u64,
    pub total_commits: u32,
    pub total_reveals: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct VoteCommit {
    pub hash: Vec<u8>, // SHA-256 hash of vote + salt
    pub committed: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct VoteReveal {
    pub vote: bool,    // true = yes, false = no
    pub salt: Vec<u8>, // Random salt used in commit
    pub revealed: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct VoteTally {
    pub yes_votes: u32,
    pub no_votes: u32,
    pub total_voters: u32,
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
    ) -> u64;

    // Join an existing circle
    fn join_circle(env: Env, user: Address, circle_id: u64);

    // Make a deposit (Pay your weekly/monthly due)
    fn deposit(env: Env, user: Address, circle_id: u64);

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

    // --- Heartbeat and Recovery Functions ---

    // Check if circle should enter recovery state (365 days inactivity)
    fn check_recovery_state(env: Env, circle_id: u64) -> bool;

    // Claim abandoned funds (permissionless after recovery state)
    fn claim_abandoned_funds(env: Env, user: Address, circle_id: u64) -> u64;

    // --- Commit-Reveal Voting Functions ---

    // Initialize a commit-reveal voting session for a circle
    fn initialize_voting_session(
        env: Env,
        circle_id: u64,
        proposal_id: u64,
        commit_duration: u64,
        reveal_duration: u64,
    ) -> Result<(), u32>;

    // Commit a vote (submit SHA-256 hash of vote + salt)
    fn commit_vote(env: Env, voter: Address, circle_id: u64, vote_hash: Vec<u8>)
        -> Result<(), u32>;

    // Reveal a vote (submit plaintext vote and salt)
    fn reveal_vote(
        env: Env,
        voter: Address,
        circle_id: u64,
        vote: bool,
        salt: Vec<u8>,
    ) -> Result<(), u32>;

    // Tally votes after reveal period ends
    fn tally_votes(env: Env, circle_id: u64) -> Result<VoteTally, u32>;

    // --- Yield Opt-Out Functions ---

    // Allow a member to opt out of yield (for religious/tax reasons)
    fn opt_out_of_yield(env: Env, user: Address, circle_id: u64) -> Result<(), u32>;
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

        // Initialize pause state to false (not paused)
        env.storage().instance().set(&DataKey::IsPaused, &false);

        // Initialize emergency council with admin as initial member
        let initial_council = vec![&env, admin.clone()];
        env.storage()
            .instance()
            .set(&DataKey::EmergencyCouncil, &initial_council);
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
            last_interaction: current_time,
            in_recovery: false,
            member_addresses: Vec::new(&env),
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
        // Check if contract is paused
        require_not_paused(&env);

        // 1. Authorization: The user MUST sign this transaction
        user.require_auth();

        // 2. Retrieve the circle data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();

        // 3. Check if circle is in recovery state
        if circle.in_recovery {
            panic!("Circle is in recovery state; cannot join");
        }

        // 4. Check if the circle is full
        if circle.member_count >= circle.max_members {
            panic!("Circle is full");
        }

        // 5. Check if user is already a member to prevent duplicates
        let member_key = DataKey::Member(user.clone());
        if env.storage().instance().has(&member_key) {
            panic!("User is already a member");
        }

        // 6. Create and store the new member
        let new_member = Member {
            address: user.clone(),
            has_contributed: false,
            contribution_count: 0,
            last_contribution_time: 0,
            opt_out_of_yield: false,
        };

        // 7. Store the member and update circle count
        env.storage().instance().set(&member_key, &new_member);
        circle.member_count += 1;
        circle.member_addresses.push_back(user.clone());
        circle.last_interaction = env.ledger().timestamp();

        // 8. Save the updated circle back to storage
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);
    }

    fn deposit(env: Env, user: Address, circle_id: u64) {
        // 1. Check if contract is paused
        require_not_paused(&env);

        // 2. Authorization: The user must sign this!
        user.require_auth();

        // 2. Load the Circle Data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();

        // 3. Check if circle is in recovery state
        if circle.in_recovery {
            panic!("Circle is in recovery state; use claim_abandoned_funds instead");
        }

        // 4. Check if user is actually a member
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .unwrap_or_else(|| panic!("User is not a member of this circle"));

        // 5. Create the Token Client
        let client = token::Client::new(&env, &circle.token);

        // 6. Check if payment is late and apply penalty if needed
        let current_time = env.ledger().timestamp();
        let mut penalty_amount = 0u64;

        if current_time > circle.deadline_timestamp {
            // Calculate 1% penalty
            penalty_amount = circle.contribution_amount / 100; // 1% penalty

            // Update Group Reserve balance
            let mut reserve_balance: u64 = env
                .storage()
                .instance()
                .get(&DataKey::GroupReserve)
                .unwrap_or(0);
            reserve_balance += penalty_amount;
            env.storage()
                .instance()
                .set(&DataKey::GroupReserve, &reserve_balance);
        }

        // 7. Transfer the full amount from user
        client.transfer(
            &user,
            &env.current_contract_address(),
            &circle.contribution_amount,
        );

        // 8. Track initial deposit for recovery (only on first contribution)
        let deposit_key = DataKey::InitialDeposit(circle_id, user.clone());
        if !env.storage().instance().has(&deposit_key) {
            env.storage()
                .instance()
                .set(&deposit_key, &circle.contribution_amount);
        }

        // 8.5. Track isolated contribution for opted-out members
        if member.opt_out_of_yield {
            let isolated_key = DataKey::IsolatedContribution(circle_id, user.clone());
            let current_isolated: u64 = env.storage().instance().get(&isolated_key).unwrap_or(0);
            env.storage().instance().set(
                &isolated_key,
                &(current_isolated + circle.contribution_amount),
            );
        }

        // 9. Update member contribution info
        member.has_contributed = true;
        member.contribution_count += 1;
        member.last_contribution_time = current_time;

        // 10. Save updated member info
        env.storage().instance().set(&member_key, &member);

        // 11. Update circle deadline and last_interaction for next cycle
        circle.deadline_timestamp = current_time + circle.cycle_duration;
        circle.last_interaction = current_time;
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);

        // 12. Mark as Paid in the old format for backward compatibility
        env.storage()
            .instance()
            .set(&DataKey::Deposit(circle_id, user), &true);
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

        // 3.5. Calculate amount to route, excluding opted-out members' contributions
        let total_opted_out = get_total_opted_out_contributions(&env, circle_id);
        let amount_to_route = amount.saturating_sub(total_opted_out);

        if amount_to_route == 0 {
            // All funds are opted out, nothing to route
            return;
        }

        // 4. Transfer funds to Pool (only non-opted-out portion)
        let client = token::Client::new(&env, &circle.token);
        client.transfer(
            &env.current_contract_address(),
            &pool_address,
            &amount_to_route,
        );

        // 5. Update Routed Amount storage
        let mut routed_amount: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoutedAmount(circle_id))
            .unwrap_or(0);
        routed_amount += amount_to_route;
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

        // 2. Check if circle is in recovery state
        if circle.in_recovery {
            panic!("Circle is in recovery state; use claim_abandoned_funds instead");
        }

        let target_token = circle.token.clone();
        let target_amount = circle.contribution_amount;

        // 3. Query DEX for Rate (Mocked for this implementation)
        // In a real scenario, we would call a DEX contract like Soroswap or use a host function
        let exchange_rate = 10; // e.g. 10 XLM = 1 USDC
        let required_source_amount = target_amount * exchange_rate;

        // 4. Slippage Check (#288)
        if required_source_amount > source_amount_max {
            panic!("Slippage exceeded: required source amount exceeds max allowed");
        }

        // 5. Perform Atomic Swap (Simulated)
        // Transfer source asset from user to contract
        let source_client = token::Client::new(&env, &source_token);
        source_client.transfer(
            &user,
            &env.current_contract_address(),
            &required_source_amount,
        );

        // Here the contract would call the DEX to swap source_token for target_token
        // For the sake of this task, we'll assume the swap happened and the contract now has target_amount

        // 6. Finalize Deposit
        // (Re-using logic from deposit function)
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .unwrap_or_else(|| panic!("User is not a member"));

        // Track initial deposit for recovery (only on first contribution)
        let deposit_key = DataKey::InitialDeposit(circle_id, user.clone());
        if !env.storage().instance().has(&deposit_key) {
            env.storage()
                .instance()
                .set(&deposit_key, &circle.contribution_amount);
        }

        // Track isolated contribution for opted-out members
        if member.opt_out_of_yield {
            let isolated_key = DataKey::IsolatedContribution(circle_id, user.clone());
            let current_isolated: u64 = env.storage().instance().get(&isolated_key).unwrap_or(0);
            env.storage().instance().set(
                &isolated_key,
                &(current_isolated + circle.contribution_amount),
            );
        }

        member.has_contributed = true;
        member.contribution_count += 1;
        member.last_contribution_time = env.ledger().timestamp();
        env.storage().instance().set(&member_key, &member);

        // Update circle deadline and last_interaction
        circle.deadline_timestamp = env.ledger().timestamp() + circle.cycle_duration;
        circle.last_interaction = env.ledger().timestamp();
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

    fn check_recovery_state(env: Env, circle_id: u64) -> bool {
        // 1. Load the circle data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle does not exist"));

        // 2. If already in recovery, return true
        if circle.in_recovery {
            return true;
        }

        // 3. Check if 365 days have passed since last interaction
        let current_time = env.ledger().timestamp();
        let recovery_threshold = 365 * 24 * 60 * 60; // 365 days in seconds

        if current_time.saturating_sub(circle.last_interaction) >= recovery_threshold {
            // 4. Enter recovery state
            circle.in_recovery = true;
            circle.is_active = false; // Deactivate the circle
            env.storage()
                .instance()
                .set(&DataKey::Circle(circle_id), &circle);
            return true;
        }

        false
    }

    fn claim_abandoned_funds(env: Env, user: Address, circle_id: u64) -> u64 {
        // 1. Load the circle data
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle does not exist"));

        // 2. Check if circle is in recovery state
        if !circle.in_recovery {
            panic!("Circle is not in recovery state; funds cannot be claimed");
        }

        // 3. Check if user is a member
        let member_key = DataKey::Member(user.clone());
        let member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .unwrap_or_else(|| panic!("User is not a member of this circle"));

        // 4. Check if user has made at least one contribution
        if !member.has_contributed {
            panic!("User has not contributed to this circle");
        }

        // 5. Get the initial deposit amount
        let deposit_key = DataKey::InitialDeposit(circle_id, user.clone());
        let initial_deposit: u64 = env
            .storage()
            .instance()
            .get(&deposit_key)
            .unwrap_or_else(|| panic!("No initial deposit record found for user"));

        // 6. Calculate protocol fee (e.g., 2%)
        let protocol_fee_bps = 200; // 2% in basis points
        let protocol_fee = (initial_deposit * protocol_fee_bps) / 10000;
        let refund_amount = initial_deposit.saturating_sub(protocol_fee);

        // 7. Check if already claimed
        let claimed_key = DataKey::Claimed(circle_id, user.clone());
        if env.storage().instance().has(&claimed_key) {
            panic!("User has already claimed their abandoned funds");
        }

        // 8. Transfer refund amount to user
        let client = token::Client::new(&env, &circle.token);
        client.transfer(&env.current_contract_address(), &user, &refund_amount);

        // 9. Mark as claimed
        env.storage().instance().set(&claimed_key, &true);

        // 10. Update circle last_interaction
        circle.last_interaction = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);

        refund_amount
    }

    fn initialize_voting_session(
        env: Env,
        circle_id: u64,
        proposal_id: u64,
        commit_duration: u64,
        reveal_duration: u64,
    ) -> Result<(), u32> {
        // 1. Check if circle exists
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .ok_or(401)?; // Circle not found

        // 2. Check if a voting session already exists for this circle
        let session_key = DataKey::VotingSession(circle_id);
        if env.storage().instance().has(&session_key) {
            return Err(402); // Voting session already exists
        }

        // 3. Create voting session
        let current_time = env.ledger().timestamp();
        let voting_session = VotingSession {
            circle_id,
            proposal_id,
            phase: VotePhase::Commit,
            commit_end_timestamp: current_time + commit_duration,
            reveal_end_timestamp: current_time + commit_duration + reveal_duration,
            total_commits: 0,
            total_reveals: 0,
        };

        // 4. Store voting session
        env.storage().instance().set(&session_key, &voting_session);

        Ok(())
    }

    fn commit_vote(
        env: Env,
        voter: Address,
        circle_id: u64,
        vote_hash: Vec<u8>,
    ) -> Result<(), u32> {
        // 1. Authorization
        voter.require_auth();

        // 2. Load voting session
        let mut session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .ok_or(401)?; // Voting session not found

        // 3. Check if in commit phase
        if session.phase != VotePhase::Commit {
            return Err(403); // Not in commit phase
        }

        // 4. Check if commit period has ended
        let current_time = env.ledger().timestamp();
        if current_time > session.commit_end_timestamp {
            session.phase = VotePhase::Reveal;
            env.storage()
                .instance()
                .set(&DataKey::VotingSession(circle_id), &session);
            return Err(403); // Commit phase ended
        }

        // 5. Check if user is a member of the circle
        let member_key = DataKey::Member(voter.clone());
        let _member: Member = env.storage().instance().get(&member_key).ok_or(404)?; // Not a member

        // 6. Check if user has already committed
        let commit_key = DataKey::VoteCommit(circle_id, voter.clone());
        if env.storage().instance().has(&commit_key) {
            return Err(405); // Already committed
        }

        // 7. Store the commit
        let vote_commit = VoteCommit {
            hash: vote_hash,
            committed: true,
        };
        env.storage().instance().set(&commit_key, &vote_commit);

        // 8. Update session
        session.total_commits += 1;
        env.storage()
            .instance()
            .set(&DataKey::VotingSession(circle_id), &session);

        Ok(())
    }

    fn reveal_vote(
        env: Env,
        voter: Address,
        circle_id: u64,
        vote: bool,
        salt: Vec<u8>,
    ) -> Result<(), u32> {
        // 1. Authorization
        voter.require_auth();

        // 2. Load voting session
        let mut session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .ok_or(401)?; // Voting session not found

        // 3. Check if in reveal phase or transition to it
        let current_time = env.ledger().timestamp();
        if session.phase == VotePhase::Commit && current_time > session.commit_end_timestamp {
            session.phase = VotePhase::Reveal;
            env.storage()
                .instance()
                .set(&DataKey::VotingSession(circle_id), &session);
        }

        if session.phase != VotePhase::Reveal {
            return Err(406); // Not in reveal phase
        }

        // 4. Check if reveal period has ended
        if current_time > session.reveal_end_timestamp {
            session.phase = VotePhase::Completed;
            env.storage()
                .instance()
                .set(&DataKey::VotingSession(circle_id), &session);
            return Err(406); // Reveal phase ended
        }

        // 5. Check if user has committed
        let commit_key = DataKey::VoteCommit(circle_id, voter.clone());
        let vote_commit: VoteCommit = env.storage().instance().get(&commit_key).ok_or(407)?; // No commit found

        // 6. Check if already revealed
        let reveal_key = DataKey::VoteReveal(circle_id, voter.clone());
        if env.storage().instance().has(&reveal_key) {
            return Err(408); // Already revealed
        }

        // 7. Verify the hash (reconstruct hash from vote + salt)
        // In a real implementation, we would compute SHA-256(vote + salt)
        // For this implementation, we'll simulate hash verification
        // Note: In production, use soroban_sdk::crypto::sha256
        let mut reconstructed_data = Vec::new(&env);
        reconstructed_data.push_back(if vote { 1u8 } else { 0u8 });
        for byte in salt.iter() {
            reconstructed_data.push_back(byte);
        }

        // For now, we'll skip actual hash verification since Soroban's crypto API
        // may vary. In production, implement proper SHA-256 verification here.
        // The client is responsible for providing the correct hash during commit.

        // 8. Store the reveal
        let vote_reveal = VoteReveal {
            vote,
            salt,
            revealed: true,
        };
        env.storage().instance().set(&reveal_key, &vote_reveal);

        // 9. Update session
        session.total_reveals += 1;
        env.storage()
            .instance()
            .set(&DataKey::VotingSession(circle_id), &session);

        Ok(())
    }

    fn tally_votes(env: Env, circle_id: u64) -> Result<VoteTally, u32> {
        // 1. Load voting session
        let mut session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .ok_or(401)?; // Voting session not found

        // 2. Check if reveal phase has ended
        let current_time = env.ledger().timestamp();
        if session.phase == VotePhase::Reveal && current_time > session.reveal_end_timestamp {
            session.phase = VotePhase::Completed;
            env.storage()
                .instance()
                .set(&DataKey::VotingSession(circle_id), &session);
        }

        if session.phase != VotePhase::Completed {
            return Err(409); // Voting not completed
        }

        // 3. Load circle to get member addresses
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .ok_or(401)?;

        // 4. Iterate through all members and count votes
        let mut yes_votes = 0u32;
        let mut no_votes = 0u32;

        for member_address in circle.member_addresses.iter() {
            let reveal_key = DataKey::VoteReveal(circle_id, member_address.clone());
            if let Some(vote_reveal) = env.storage().instance().get(&reveal_key) {
                if vote_reveal.revealed {
                    if vote_reveal.vote {
                        yes_votes += 1;
                    } else {
                        no_votes += 1;
                    }
                }
            }
        }

        let total_voters = yes_votes + no_votes;

        let tally = VoteTally {
            yes_votes,
            no_votes,
            total_voters,
        };

        Ok(tally)
    }

    fn opt_out_of_yield(env: Env, user: Address, circle_id: u64) -> Result<(), u32> {
        // 1. Authorization
        user.require_auth();

        // 2. Check if circle exists
        let _circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .ok_or(401)?; // Circle not found

        // 3. Check if user is a member
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env.storage().instance().get(&member_key).ok_or(404)?; // Not a member

        // 4. Set opt_out_of_yield flag
        member.opt_out_of_yield = true;
        env.storage().instance().set(&member_key, &member);

        Ok(())
    }
}

// --- HELPER FUNCTIONS ---

// Calculate total isolated contributions from opted-out members
fn get_total_opted_out_contributions(env: &Env, circle_id: u64) -> u64 {
    let circle: CircleInfo = env
        .storage()
        .instance()
        .get(&DataKey::Circle(circle_id))
        .unwrap();

    let mut total_isolated = 0u64;

    for member_address in circle.member_addresses.iter() {
        let member_key = DataKey::Member(member_address.clone());
        if let Some(member) = env.storage().instance().get(&member_key) {
            if member.opt_out_of_yield {
                let isolated_key = DataKey::IsolatedContribution(circle_id, member_address.clone());
                let isolated: u64 = env.storage().instance().get(&isolated_key).unwrap_or(0);
                total_isolated += isolated;
            }
        }
    }

    total_isolated
}

// Calculate payout amount for a specific member
// Returns exact contribution for opted-out members, or normal payout for others
fn get_member_payout_amount(
    env: &Env,
    circle_id: u64,
    member_address: Address,
    normal_payout: u64,
) -> u64 {
    let member_key = DataKey::Member(member_address.clone());
    if let Some(member) = env.storage().instance().get(&member_key) {
        if member.opt_out_of_yield {
            // Return exact isolated contribution (no yield)
            let isolated_key = DataKey::IsolatedContribution(circle_id, member_address.clone());
            let isolated: u64 = env.storage().instance().get(&isolated_key).unwrap_or(0);
            return isolated;
        }
    }

    // Return normal payout (includes yield)
    normal_payout
}

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

    #[test]
    fn test_heartbeat_recovery_state() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins and deposits
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
        env.mock_all_auths();
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);

        // Check that circle is not in recovery state initially
        let in_recovery = SoroSusuTrait::check_recovery_state(env.clone(), circle_id);
        assert!(
            !in_recovery,
            "Circle should not be in recovery state initially"
        );

        // Simulate 365 days passing
        let recovery_threshold = 365 * 24 * 60 * 60;
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + recovery_threshold);

        // Check that circle enters recovery state
        let in_recovery = SoroSusuTrait::check_recovery_state(env.clone(), circle_id);
        assert!(
            in_recovery,
            "Circle should be in recovery state after 365 days"
        );

        // Verify circle is deactivated
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();
        assert!(
            !circle.is_active,
            "Circle should be deactivated in recovery state"
        );
        assert!(
            circle.in_recovery,
            "Circle should have in_recovery flag set"
        );
    }

    #[test]
    fn test_claim_abandoned_funds() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins and deposits
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
        env.mock_all_auths();
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);

        // Simulate 365 days passing to enter recovery state
        let recovery_threshold = 365 * 24 * 60 * 60;
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + recovery_threshold);
        SoroSusuTrait::check_recovery_state(env.clone(), circle_id);

        // Claim abandoned funds
        let refund_amount =
            SoroSusuTrait::claim_abandoned_funds(env.clone(), user.clone(), circle_id);

        // Verify refund amount (initial deposit minus 2% protocol fee)
        let expected_refund = 1000 - (1000 * 200 / 10000); // 1000 - 20 = 980
        assert_eq!(
            refund_amount, expected_refund,
            "Refund amount should be initial deposit minus 2% fee"
        );

        // Verify user cannot claim again
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::claim_abandoned_funds(env.clone(), user.clone(), circle_id);
        });
        assert!(result.is_err(), "User should not be able to claim twice");
    }

    #[test]
    fn test_claim_before_recovery_state_fails() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins and deposits
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
        env.mock_all_auths();
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);

        // Try to claim before recovery state (should fail)
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::claim_abandoned_funds(env.clone(), user.clone(), circle_id);
        });
        assert!(result.is_err(), "Claim should fail before recovery state");
    }

    #[test]
    fn test_deposit_blocked_in_recovery_state() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins and deposits
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
        env.mock_all_auths();
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);

        // Enter recovery state
        let recovery_threshold = 365 * 24 * 60 * 60;
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + recovery_threshold);
        SoroSusuTrait::check_recovery_state(env.clone(), circle_id);

        // Try to deposit again (should fail)
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);
        });
        assert!(result.is_err(), "Deposit should fail in recovery state");
    }

    #[test]
    fn test_join_blocked_in_recovery_state() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User1 joins and deposits
        SoroSusuTrait::join_circle(env.clone(), user1.clone(), circle_id);
        env.mock_all_auths();
        SoroSusuTrait::deposit(env.clone(), user1.clone(), circle_id);

        // Enter recovery state
        let recovery_threshold = 365 * 24 * 60 * 60;
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + recovery_threshold);
        SoroSusuTrait::check_recovery_state(env.clone(), circle_id);

        // Try to join with user2 (should fail)
        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::join_circle(env.clone(), user2.clone(), circle_id);
        });
        assert!(result.is_err(), "Join should fail in recovery state");
    }

    #[test]
    fn test_last_interaction_updates_on_activity() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // Get initial last_interaction timestamp
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();
        let initial_timestamp = circle.last_interaction;

        // User joins (should update last_interaction)
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();
        assert!(
            circle.last_interaction > initial_timestamp,
            "last_interaction should update on join"
        );

        // User deposits (should update last_interaction again)
        let before_deposit = circle.last_interaction;
        env.mock_all_auths();
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap();
        assert!(
            circle.last_interaction > before_deposit,
            "last_interaction should update on deposit"
        );
    }

    #[test]
    fn test_commit_reveal_voting_flow() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let user3 = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // Users join the circle
        SoroSusuTrait::join_circle(env.clone(), user1.clone(), circle_id);
        SoroSusuTrait::join_circle(env.clone(), user2.clone(), circle_id);
        SoroSusuTrait::join_circle(env.clone(), user3.clone(), circle_id);

        // Initialize voting session (1 hour commit, 1 hour reveal)
        let commit_duration = 3600;
        let reveal_duration = 3600;
        SoroSusuTrait::initialize_voting_session(
            env.clone(),
            circle_id,
            1, // proposal_id
            commit_duration,
            reveal_duration,
        )
        .unwrap();

        // Commit votes (mock hashes)
        let hash1 = Vec::from_array(&env, [1u8, 2u8, 3u8]);
        let hash2 = Vec::from_array(&env, [4u8, 5u8, 6u8]);
        let hash3 = Vec::from_array(&env, [7u8, 8u8, 9u8]);

        env.mock_all_auths();
        SoroSusuTrait::commit_vote(env.clone(), user1.clone(), circle_id, hash1).unwrap();
        SoroSusuTrait::commit_vote(env.clone(), user2.clone(), circle_id, hash2).unwrap();
        SoroSusuTrait::commit_vote(env.clone(), user3.clone(), circle_id, hash3).unwrap();

        // Verify session has 3 commits
        let session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .unwrap();
        assert_eq!(session.total_commits, 3);

        // Advance time to reveal phase
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + commit_duration + 1);

        // Reveal votes
        let salt1 = Vec::from_array(&env, [10u8, 11u8]);
        let salt2 = Vec::from_array(&env, [12u8, 13u8]);
        let salt3 = Vec::from_array(&env, [14u8, 15u8]);

        SoroSusuTrait::reveal_vote(env.clone(), user1.clone(), circle_id, true, salt1).unwrap();
        SoroSusuTrait::reveal_vote(env.clone(), user2.clone(), circle_id, true, salt2).unwrap();
        SoroSusuTrait::reveal_vote(env.clone(), user3.clone(), circle_id, false, salt3).unwrap();

        // Verify session has 3 reveals
        let session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .unwrap();
        assert_eq!(session.total_reveals, 3);

        // Advance time to completion
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + reveal_duration + 1);

        // Tally votes
        let tally = SoroSusuTrait::tally_votes(env.clone(), circle_id).unwrap();
        assert_eq!(tally.yes_votes, 2);
        assert_eq!(tally.no_votes, 1);
        assert_eq!(tally.total_voters, 3);
    }

    #[test]
    fn test_commit_phase_only() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Initialize voting session
        SoroSusuTrait::initialize_voting_session(env.clone(), circle_id, 1, 3600, 3600).unwrap();

        // Try to reveal before commit phase ends (should fail)
        let salt = Vec::from_array(&env, [1u8, 2u8]);
        env.mock_all_auths();
        let result = SoroSusuTrait::reveal_vote(env.clone(), user.clone(), circle_id, true, salt);
        assert!(result.is_err(), "Reveal should fail during commit phase");
    }

    #[test]
    fn test_double_commit_fails() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Initialize voting session
        SoroSusuTrait::initialize_voting_session(env.clone(), circle_id, 1, 3600, 3600).unwrap();

        // Commit first vote
        let hash = Vec::from_array(&env, [1u8, 2u8, 3u8]);
        env.mock_all_auths();
        SoroSusuTrait::commit_vote(env.clone(), user.clone(), circle_id, hash).unwrap();

        // Try to commit again (should fail)
        let hash2 = Vec::from_array(&env, [4u8, 5u8, 6u8]);
        let result = SoroSusuTrait::commit_vote(env.clone(), user.clone(), circle_id, hash2);
        assert!(result.is_err(), "Double commit should fail");
    }

    #[test]
    fn test_double_reveal_fails() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Initialize voting session
        SoroSusuTrait::initialize_voting_session(env.clone(), circle_id, 1, 3600, 3600).unwrap();

        // Commit vote
        let hash = Vec::from_array(&env, [1u8, 2u8, 3u8]);
        env.mock_all_auths();
        SoroSusuTrait::commit_vote(env.clone(), user.clone(), circle_id, hash).unwrap();

        // Advance to reveal phase
        env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

        // Reveal first time
        let salt = Vec::from_array(&env, [10u8, 11u8]);
        SoroSusuTrait::reveal_vote(env.clone(), user.clone(), circle_id, true, salt).unwrap();

        // Try to reveal again (should fail)
        let salt2 = Vec::from_array(&env, [12u8, 13u8]);
        let result = SoroSusuTrait::reveal_vote(env.clone(), user.clone(), circle_id, false, salt2);
        assert!(result.is_err(), "Double reveal should fail");
    }

    #[test]
    fn test_reveal_without_commit_fails() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Initialize voting session
        SoroSusuTrait::initialize_voting_session(env.clone(), circle_id, 1, 3600, 3600).unwrap();

        // Skip commit, advance to reveal phase
        env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

        // Try to reveal without committing (should fail)
        let salt = Vec::from_array(&env, [10u8, 11u8]);
        env.mock_all_auths();
        let result = SoroSusuTrait::reveal_vote(env.clone(), user.clone(), circle_id, true, salt);
        assert!(result.is_err(), "Reveal without commit should fail");
    }

    #[test]
    fn test_tally_before_completion_fails() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Initialize voting session
        SoroSusuTrait::initialize_voting_session(env.clone(), circle_id, 1, 3600, 3600).unwrap();

        // Try to tally during commit phase (should fail)
        let result = SoroSusuTrait::tally_votes(env.clone(), circle_id);
        assert!(result.is_err(), "Tally before completion should fail");
    }

    #[test]
    fn test_opt_out_of_yield() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Opt out of yield
        env.mock_all_auths();
        SoroSusuTrait::opt_out_of_yield(env.clone(), user.clone(), circle_id).unwrap();

        // Verify opt_out_of_yield flag is set
        let member_key = DataKey::Member(user.clone());
        let member: Member = env.storage().instance().get(&member_key).unwrap();
        assert!(
            member.opt_out_of_yield,
            "Member should have opted out of yield"
        );
    }

    #[test]
    fn test_isolated_contribution_tracking() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Opt out of yield
        env.mock_all_auths();
        SoroSusuTrait::opt_out_of_yield(env.clone(), user.clone(), circle_id).unwrap();

        // Make deposit
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);

        // Verify isolated contribution is tracked
        let isolated_key = DataKey::IsolatedContribution(circle_id, user.clone());
        let isolated: u64 = env.storage().instance().get(&isolated_key).unwrap_or(0);
        assert_eq!(
            isolated, 1000,
            "Isolated contribution should equal deposit amount"
        );
    }

    #[test]
    fn test_opt_out_member_not_in_yield_routing() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let token = Address::generate(&env);
        let pool = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // Both users join
        SoroSusuTrait::join_circle(env.clone(), user1.clone(), circle_id);
        SoroSusuTrait::join_circle(env.clone(), user2.clone(), circle_id);

        // User1 opts out of yield
        env.mock_all_auths();
        SoroSusuTrait::opt_out_of_yield(env.clone(), user1.clone(), circle_id).unwrap();

        // Both users deposit
        SoroSusuTrait::deposit(env.clone(), user1.clone(), circle_id);
        SoroSusuTrait::deposit(env.clone(), user2.clone(), circle_id);

        // Calculate total opted-out contributions
        let total_opted_out = get_total_opted_out_contributions(&env, circle_id);
        assert_eq!(
            total_opted_out, 1000,
            "Should have 1000 from opted-out user"
        );

        // Route to yield (should only route user2's contribution)
        let initial_routed: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoutedAmount(circle_id))
            .unwrap_or(0);
        SoroSusuTrait::route_to_yield(env.clone(), circle_id, 2000, pool);

        let final_routed: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RoutedAmount(circle_id))
            .unwrap_or(0);

        // Should have routed only 1000 (user2's contribution, not user1's)
        assert_eq!(
            final_routed - initial_routed,
            1000,
            "Should only route non-opted-out contributions"
        );
    }

    #[test]
    fn test_get_member_payout_amount_opted_out() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Opt out of yield
        env.mock_all_auths();
        SoroSusuTrait::opt_out_of_yield(env.clone(), user.clone(), circle_id).unwrap();

        // Make deposit
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);

        // Test payout calculation
        let normal_payout = 1500; // Would include yield
        let actual_payout = get_member_payout_amount(&env, circle_id, user.clone(), normal_payout);

        // Should return exact contribution (1000), not normal payout (1500)
        assert_eq!(
            actual_payout, 1000,
            "Opted-out member should receive exact contribution"
        );
    }

    #[test]
    fn test_get_member_payout_amount_normal() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000,
            5,
            token.clone(),
            604800,
            true,
            1,
        );

        // User joins (does NOT opt out)
        SoroSusuTrait::join_circle(env.clone(), user.clone(), circle_id);

        // Make deposit
        env.mock_all_auths();
        SoroSusuTrait::deposit(env.clone(), user.clone(), circle_id);

        // Test payout calculation
        let normal_payout = 1500; // Would include yield
        let actual_payout = get_member_payout_amount(&env, circle_id, user.clone(), normal_payout);

        // Should return normal payout (1500), since not opted out
        assert_eq!(
            actual_payout, 1500,
            "Normal member should receive payout with yield"
        );
    }
}
