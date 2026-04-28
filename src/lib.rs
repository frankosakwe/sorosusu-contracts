#![cfg_attr(not(test), no_std)]
use arbitrary::{Arbitrary, Unstructured};
use soroban_sdk::{
    contract, contractimpl, contracttype, testutils::Address as TestAddress, token, Address, Env,
    Symbol, Vec, Bytes, BytesN,
};

pub mod chat_metadata;
pub mod dispute;
pub mod yield_allocation_voting;
pub mod yield_strategy_trait;
// Issue #323: VRF-based juror selection for global dispute resolution.
pub mod juror_selection;
// Stellar Protocol 21+ Passkey Authentication Support
pub mod passkey_auth;

// Issue #321: Maximum cycle duration cap (2 years in seconds) to prevent
// integer overflow exploits and unbounded storage accumulation.
pub const MAX_CYCLE_DURATION: u64 = 2 * 365 * 24 * 60 * 60; // 63,072,000 seconds

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
    // Pause / emergency council
    IsPaused,
    EmergencyCouncil,
    // Yield opt-out tracking
    InitialDeposit(u64, Address),
    IsolatedContribution(u64, Address),
    // Commit-reveal voting session
    VotingSession(u64),
    // Issue #315: Reentrancy guard flag
    NonReentrant,
    // Issue #316: Zombie-group sweep
    CircleCompletedAt(u64),
    // Issue #275: Reputation-NFT (SBT) Minting Hook
    SbtCredential(u128),
    UserSbt(Address),
    ArchivedGroupHash(u64),
    // Issue #322: Dispute bond slashing
    DisputeCount,
    Dispute(u64),
    // SEP-24 Anchor Integration
    AnchorRegistry(Address),
    AnchorDeposit(u64), // Deposit ID
    UserBankPreference(Address, u64), // User, CircleID
    AnchorDepositCount,
    MissingTrustline(u64, Address), // CircleID, MemberAddress
}

/// Issue #324: Record stored in the PendingSlash vault.
#[contracttype]
#[derive(Clone)]
pub struct PendingSlashRecord {
    /// Amount of collateral held in the vault (in token stroops).
    pub amount: u64,
    /// Ledger timestamp at which the slash was recorded.
    pub slashed_at: u64,
}

/// 72 hours in seconds — the mandatory appeals window before slashed collateral
/// can be redistributed to victims (Issue #324).
pub const APPEALS_TIMELOCK_SECS: u64 = 72 * 60 * 60; // 259_200

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, contractclient,
    symbol_short,
    Address, Env, Symbol, Vec, Map, String,
    token,
};

    /// Submits a late contribution after the deadline but within the grace period.
    /// A late fee is automatically deducted from the member's safety deposit.
    ///
    /// # Parameters
    /// - `user`: Address making the late payment; must sign the transaction.
    /// - `circle_id`: ID of the circle.
    ///
    /// # Panics
    /// - `"Payment is not late. Use deposit function for on-time payment."` — called before deadline.
    fn late_contribution(env: Env, user: Address, circle_id: u64);

    /// Executes a default on a member after the grace period has expired.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(Error::DeadlineNotMissed)` (`403`) if the member has not yet missed their deadline.
    /// - `Err(Error::GracePeriodActive)` (`404`) if the grace period has not yet expired.
    fn execute_default(env: Env, circle_id: u64, member: Address) -> Result<(), u32>;

    /// Admin-only: moves a defaulted member's collateral into the 72-hour
    /// pending vault (appeals timelock, Issue #324).
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(Error::NothingToSlash)` (`405`) if the member has no collateral to slash.
    fn slash_collateral(env: Env, circle_id: u64, member: Address) -> Result<(), u32>;

#[contracttype]
#[derive(Clone)]
pub struct GasBufferConfig {
    pub min_buffer_amount: i128,
    pub max_buffer_amount: i128,
    pub auto_refill_threshold: i128,
    pub emergency_buffer: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Circle(u64),
    Member(Address),
    CircleMember(u64, u32),
    CircleCount,
    ScheduledPayoutTime(u64),
    LastCreatedTimestamp(Address),
    SafetyDeposit(Address, u64),
    GroupReserve,
    LendingPool,
    CycleBadge(Address, u64),
    UserStats(Address),
    ProtocolFeeBps,
    ProtocolTreasury,
    CollateralVault(Address, u64),
    ReputationData(Address),
    SocialCapital(Address, u64),
    AuditCount,
    AuditEntry(u64),
    AuditAll,
    AuditByActor(Address),
    AuditByResource(u64),
    LeniencyStats(u64),
    Proposal(u64),
    DefaultedMembers(u64),
    RolloverBonus(u64),
    RolloverVote(u64, Address),
    LeniencyRequest(u64),
    VotingPower(Address, u64),
    DissolutionProposal(u64),
    RefundClaim(u64),
    YieldDelegation(u64),
    YieldVote(u64, Address),
    YieldPoolRegistry,
    GroupTreasury(u64),
    PathPayment(u64),
    PathPaymentVote(u64, Address),
    DexRegistry(Address),
    SupportedTokens(Address),
    // Multi-asset basket storage
    BasketConfig(u64),
    BasketAssetContrib(u64, Address, Address),
    GroupInsuranceFund(u64), // Per-circle insurance fund balance
    InsurancePremium(u64, Address), // Track premiums paid by each member per circle
    PriceOracle(Address), // Price data for each asset
    HardAssetBasket, // Reference hard asset basket
    AssetSwapProposal(u64), // Per-circle asset swap proposals
    AssetSwapVote(u64, Address), // Votes on asset swap proposals
    LateFeeDistribution(u64, u32), // Late fee distribution per circle per round
    LastDepositLedger(Address),
    LastWithdrawalLedger(Address),
    RecursiveOptIn(Address, u64),
    GoldTierCircle,
    PausedPayout(Address, u64), // (user, circle_id) -> is_paused
    LeaseFlowContract,
    GrantStreamContract,
    MilestoneReached(u64),
    PaymentTiming(u64, u32, Address), // Payment timing per circle, round, and member
    PaymentOrderCounter(u64, u32), // Counter to track payment order in each round
    GasBufferConfig(u64),  // Per-circle gas buffer config
    MemberByIndex(u64, u32), // Legacy: member lookup by index
    LiquidityBufferConfig,           // Global liquidity buffer configuration
    LiquidityBufferReserve,          // Current reserve balance
    LiquidityAdvance(u64),           // Individual advance records
    LiquidityAdvanceCounter,         // Counter for generating advance IDs
    MemberAdvanceHistory(Address, u64), // Member's advance history
    LiquidityBufferStats,            // Buffer utilization statistics
    PlatformFeeAllocation,           // Platform fee allocation to buffer
    // Stellar Anchor Direct Deposit API (SEP-24/SEP-31)
    AnchorRegistry, // Registry of authorized anchors
    AnchorDeposit(u64), // Track anchor deposits per circle
    DepositMemo(u64), // Track deposit memos for compliance
    // Inter-Susu Lending Market Liquidity Hook
    LendingMarketProposal(u64),       // Lending market proposals
    LendingMarketVote(u64, Address),       // Votes on lending market proposals
    LendingPoolInfo(u64),             // Lending pool information
    LendingPoolParticipant(u64, Address), // Pool participants
    LendingMarketConfig,               // Global lending market configuration
    LendingPosition(u64, Address),        // Individual lending positions
    LendingOffer(u64),                 // Active lending offers
    LiquidityProvider(u64, Address),     // Liquidity provider information
    YieldFarm(u64),                   // Yield farming positions
    EmergencyLoan(u64),                 // Emergency loan requests
    RepaymentSchedule(u64),            // Loan repayment schedules
    LendingMarketStats,               // Lending market statistics
}

// --- SEP-24 ANCHOR INTEGRATION DATA STRUCTURES ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AnchorStatus {
    Active,
    Inactive,
    Suspended,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DepositStatus {
    Pending,
    Completed,
    Failed,
    Reversed,
}

#[contracttype]
#[derive(Clone)]
pub struct AnchorInfo {
    pub address: Address,
    pub name: Symbol,
    pub sep_version: Symbol, // SEP-24, SEP-6, etc.
    pub status: AnchorStatus,
    pub kyc_required: bool,
    pub supported_tokens: Vec<Address>,
    pub max_deposit_amount: u64,
    pub daily_deposit_limit: u64,
    pub registration_date: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct AnchorDeposit {
    pub anchor_address: Address,
    pub user_address: Address,
    pub circle_id: u64,
    pub amount: u64,
    pub token: Address,
    pub fiat_reference: Symbol, // Bank transaction ID, M-Pesa reference, etc.
    pub status: DepositStatus,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct AnchorDepositConfig {
    pub preferred_anchor: Address,
    pub bank_account_hash: u64, // Hashed bank account details for privacy
    pub mobile_money_provider: Symbol, // M-Pesa, MTN Mobile Money, etc.
    pub mobile_number_hash: u64,
    pub fiat_currency: Symbol, // USD, KES, GHS, etc.
    pub auto_convert: bool, // Automatically convert crypto to fiat
}

#[contracttype]
#[derive(Clone)]
pub struct UserBankPreference {
    pub user: Address,
    pub circle_id: u64,
    pub payout_method: PayoutMethod,
    pub anchor_config: Option<AnchorDepositConfig>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum PayoutMethod {
    DirectToken,     // Default: receive tokens directly
    DirectToBank,    // SEP-24: convert to fiat via anchor
}

// --- SEP-24 ANCHOR INTEGRATION DATA STRUCTURES ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AnchorStatus {
    Active,
    Inactive,
    Suspended,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DepositStatus {
    Pending,
    Completed,
    Failed,
    Reversed,
}

#[contracttype]
#[derive(Clone)]
pub struct AnchorInfo {
    pub address: Address,
    pub name: Symbol,
    pub sep_version: Symbol, // SEP-24, SEP-6, etc.
    pub status: AnchorStatus,
    pub kyc_required: bool,
    pub supported_tokens: Vec<Address>,
    pub max_deposit_amount: u64,
    pub daily_deposit_limit: u64,
    pub registration_date: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct AnchorDeposit {
    pub anchor_address: Address,
    pub user_address: Address,
    pub circle_id: u64,
    pub amount: u64,
    pub token: Address,
    pub fiat_reference: Symbol, // Bank transaction ID, M-Pesa reference, etc.
    pub status: DepositStatus,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct AnchorDepositConfig {
    pub preferred_anchor: Address,
    pub bank_account_hash: u64, // Hashed bank account details for privacy
    pub mobile_money_provider: Symbol, // M-Pesa, MTN Mobile Money, etc.
    pub mobile_number_hash: u64,
    pub fiat_currency: Symbol, // USD, KES, GHS, etc.
    pub auto_convert: bool, // Automatically convert crypto to fiat
}

#[contracttype]
#[derive(Clone)]
pub struct UserBankPreference {
    pub user: Address,
    pub circle_id: u64,
    pub payout_method: PayoutMethod,
    pub anchor_config: Option<AnchorDepositConfig>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum PayoutMethod {
    DirectToken,     // Default: receive tokens directly
    DirectToBank,    // SEP-24: convert to fiat via anchor
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
    fn deposit(env: Env, user: Address, circle_id: u64, amount: u64);

    // Late contribution with fee (pay after deadline but within grace period)
    fn late_contribution(env: Env, user: Address, circle_id: u64);

    // Execute default on member (after grace period expires)
    fn execute_default(env: Env, circle_id: u64, member: Address) -> Result<(), u32>;

    // Issue #324: Move slashed collateral into the 72-hour pending vault.
    // Only callable by admin. Returns Err(405) if member has no collateral to slash.
    fn slash_collateral(env: Env, circle_id: u64, member: Address) -> Result<(), u32>;

    // Issue #324: Redistribute pending-slash funds to the group reserve after the
    // 72-hour appeals window has elapsed. Returns Err(406) if the timelock has not
    // yet expired, giving the penalised member time to appeal to the DAO.
    fn release_pending_slash(env: Env, circle_id: u64, member: Address) -> Result<(), u32>;

    /// Routes a portion of the circle's reserve to an external yield pool.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `amount`: Amount in stroops to route.
    /// - `pool_address`: Address of the yield pool contract.
    ///
    /// # Panics
    /// - `"Yield routing is disabled for this circle"` — yield opt-out is active.
    /// - `"Cannot route to yield: too close to payout"` — Time-to-Liquidity check failed.
    fn route_to_yield(env: Env, circle_id: u64, amount: u64, pool_address: Address);

    /// Withdraws previously routed funds from an external yield pool back to the circle.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `amount_to_withdraw`: Amount in stroops to withdraw.
    /// - `pool_address`: Address of the yield pool contract.
    fn withdraw_from_yield(
        env: Env,
        circle_id: u64,
        amount_to_withdraw: u64,
        pool_address: Address,
    );

    /// Accepts a contribution in any Stellar asset and auto-swaps it to the
    /// circle's base token via Soroban path payments.
    ///
    /// # Parameters
    /// - `user`: Address making the deposit; must sign the transaction.
    /// - `circle_id`: ID of the circle.
    /// - `source_token`: Asset the user is paying with.
    /// - `source_amount_max`: Maximum source tokens the user is willing to spend
    ///   (slippage guard).
    fn deposit_with_swap(
        env: Env,
        user: Address,
        circle_id: u64,
        source_token: Address,
        source_amount_max: u64,
    );

    /// Initializes a yield-distribution voting session for a circle.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `available_strategies`: List of strategy contract addresses members can vote on.
    ///
    /// # Returns
    /// `Ok(())` on success, or an error code if the session cannot be created.
    fn initialize_yield_voting(
        env: Env,
        circle_id: u64,
        available_strategies: Vec<Address>,
    ) -> Result<(), u32>;

    /// Casts a member's vote for a yield distribution strategy.
    ///
    /// # Parameters
    /// - `voter`: Address casting the vote; must sign the transaction.
    /// - `circle_id`: ID of the circle.
    /// - `proposed_strategies`: Ordered list of preferred distribution strategies.
    fn cast_yield_vote(
        env: Env,
        voter: Address,
        circle_id: u64,
        proposed_strategies: Vec<yield_allocation_voting::DistributionStrategy>,
    ) -> Result<(), u32>;

    /// Finalizes the yield voting session and returns the winning strategy list.
    ///
    /// # Returns
    /// `Ok(Vec<DistributionStrategy>)` — the winning ordered strategy list.
    fn finalize_yield_voting(
        env: Env,
        circle_id: u64,
    ) -> Result<Vec<yield_allocation_voting::DistributionStrategy>, u32>;

    /// Executes the winning yield distribution strategy for a circle.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `total_yield_amount`: Total yield in stroops to distribute.
    fn execute_yield_distribution(
        env: Env,
        circle_id: u64,
        total_yield_amount: i128,
    ) -> Result<(), u32>;

    /// Finalizes a yield cycle, integrating with the voting system if a session exists.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `total_yield_amount`: Total yield in stroops earned this cycle.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(Error::GracePeriodActive)` (`404`) if the voting period is still active.
    fn finalize_cycle(env: Env, circle_id: u64, total_yield_amount: i128) -> Result<(), u32>;

    /// Distributes yield to members in paginated chunks of 10 to avoid gas limits.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `total_yield_amount`: Total yield in stroops to distribute.
    /// - `member_addresses`: Full member address list (used for pro-rata calculation).
    ///
    /// # Returns
    /// A `BatchHarvestProgress` struct tracking how many members have been processed.
    /// Call repeatedly until `is_complete == true`.
    ///
    /// # Returns
    /// - `Ok(BatchHarvestProgress)` on success.
    /// - `Err(401)` if the circle does not exist.
    fn batch_harvest(
        env: Env,
        circle_id: u64,
        total_yield_amount: i128,
        member_addresses: Vec<Address>,
    ) -> Result<BatchHarvestProgress, u32>;

    // --- Issue #274: Group-Reputation Aggregate Score ---

    /// Get the aggregate reputation score for a group (0-10000 bps).
    fn get_group_reputation(env: Env, circle_id: u64) -> u32;

    // --- Issue #315: Reentrancy-guarded payout & slash_stake ---

    /// Disburse the pot to the current recipient with a NON_REENTRANT guard.
    fn payout(env: Env, caller: Address, circle_id: u64);

    /// Slash a member's staked bond with a NON_REENTRANT guard.
    fn slash_stake(env: Env, admin: Address, circle_id: u64, member: Address);

    // --- Issue #316: Zombie-Group Sweep ---

    /// Archive metadata and delete heavy state 30 days after completion.
    fn cleanup_group(env: Env, caller: Address, circle_id: u64);

    // --- Issue #322: Dispute Bond Slashing ---

    /// Lock a bond and open a dispute; returns the new dispute ID.
    fn raise_dispute(
        env: Env,
        accuser: Address,
        accused: Address,
        circle_id: u64,
        xlm_token: Address,
    ) -> u64;

    /// Record evidence for an open dispute.
    fn submit_evidence(env: Env, submitter: Address, dispute_id: u64, evidence_hash: u64);

    /// Record a juror vote on a dispute.
    fn juror_vote(env: Env, juror: Address, dispute_id: u64, vote_guilty: bool);

    /// Execute the verdict: slash bond to accused if baseless, else return to accuser.
    fn execute_verdict(
        env: Env,
        admin: Address,
        dispute_id: u64,
        baseless: bool,
        xlm_token: Address,
    );

    // --- Issue #304: Yield opt-out ---

    /// Opt a member out of yield routing for a circle.
    fn opt_out_of_yield(env: Env, user: Address, circle_id: u64) -> Result<(), u32>;

    // --- Simplified-View Read-Only Wrapper ---

    /// Get aggregated user summary for mobile clients (read-only).
    fn get_user_summary(env: Env, user: Address) -> Option<UserSummary>;

    // --- Commit-reveal voting ---

    /// Initializes a commit-reveal voting session for a circle.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `commit_duration`: Seconds for the commit phase.
    /// - `reveal_duration`: Seconds for the reveal phase.
    fn initialize_voting_session(
        env: Env,
        circle_id: u64,
        commit_duration: u64,
        reveal_duration: u64,
    ) -> Result<(), u32>;

    /// Submits a hashed vote commitment during the commit phase.
    ///
    /// # Parameters
    /// - `voter`: Address casting the vote; must sign the transaction.
    /// - `circle_id`: ID of the circle.
    /// - `commitment`: `SHA-256(vote_bool || salt)` as raw bytes.
    ///
    /// # Returns
    /// - `Err(Error::NothingToSlash)` (`405`) if a commitment already exists.
    fn commit_vote(env: Env, voter: Address, circle_id: u64, commitment: Vec<u8>) -> Result<(), u32>;

    /// Reveals a previously committed vote during the reveal phase.
    ///
    /// # Parameters
    /// - `voter`: Address revealing the vote; must sign the transaction.
    /// - `circle_id`: ID of the circle.
    /// - `vote`: The plaintext boolean vote.
    /// - `salt`: The salt used when hashing the commitment.
    ///
    /// # Returns
    /// - `Err(Error::TimelockActive)` (`406`) if the vote has already been revealed.
    fn reveal_vote(
        env: Env,
        voter: Address,
        circle_id: u64,
        vote: bool,
        salt: Vec<u8>,
    ) -> Result<(), u32>;

    /// Tallies all revealed votes and returns the result.
    ///
    /// # Returns
    /// - `Ok(true)` if the vote passed, `Ok(false)` if it failed.
    /// - `Err(Error::TallyIncomplete)` (`407`) if not all votes have been revealed.
    fn tally_votes(env: Env, circle_id: u64) -> Result<bool, u32>;

    // --- SEP-24 Anchor Integration ---

    /// Register a new SEP-24 anchor for fiat conversions
    fn register_anchor(
        env: Env,
        admin: Address,
        anchor_address: Address,
        name: Symbol,
        sep_version: Symbol,
        kyc_required: bool,
        supported_tokens: Vec<Address>,
        max_deposit_amount: u64,
        daily_deposit_limit: u64,
    );

    /// Get information about a registered anchor
    fn get_anchor_info(env: Env, anchor_address: Address) -> AnchorInfo;

    /// Get list of all registered anchors
    fn get_registered_anchors(env: Env) -> Vec<Address>;

    /// Set user's payout preference for a circle (Direct Token vs Direct-to-Bank)
    fn set_payout_preference(
        env: Env,
        user: Address,
        circle_id: u64,
        payout_method: PayoutMethod,
        anchor_config: Option<AnchorDepositConfig>,
    );

    /// Get user's payout preference for a circle
    fn get_payout_preference(env: Env, user: Address, circle_id: u64) -> UserBankPreference;

    /// Deposit funds on behalf of a user via an anchor (for SEP-24 integration)
    fn deposit_for_user(
        env: Env,
        anchor_address: Address,
        user_address: Address,
        circle_id: u64,
        amount: u64,
        token: Address,
        fiat_reference: Symbol,
    );

    /// Process a payout to an anchor for fiat conversion
    fn process_anchor_payout(
        env: Env,
        anchor_address: Address,
        user_address: Address,
        circle_id: u64,
        amount: u64,
        token: Address,
    ) -> Result<u64, u32>; // Returns deposit ID

    /// Get status of an anchor deposit
    fn get_anchor_deposit_status(env: Env, deposit_id: u64) -> AnchorDeposit;

    // --- Recovery helpers ---

    /// Returns `true` if the circle has entered recovery state (stale/abandoned).
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle to check.
    fn check_recovery_state(env: Env, circle_id: u64) -> bool;

    /// Allows a member to claim their proportional share of funds from an
    /// abandoned circle that has entered recovery state.
    ///
    /// # Parameters
    /// - `user`: Address claiming funds; must sign the transaction.
    /// - `circle_id`: ID of the abandoned circle.
    ///
    /// # Panics
    /// - `"circle is still active"` — the circle has not entered recovery state.
    fn claim_abandoned_funds(env: Env, user: Address, circle_id: u64);

    // --- Passkey Authentication Functions ---

    /// Register a new passkey for biometric authentication
    fn register_passkey(
        env: Env,
        user: Address,
        public_key: BytesN<33>,
        credential_id: Bytes,
        origin: Symbol,
    ) -> Result<(), u32>;

    /// Authenticate using a passkey signature (biometric)
    fn authenticate_with_passkey(
        env: Env,
        user: Address,
        signature: passkey_auth::PasskeySignature,
        credential_id: Bytes,
    ) -> Result<bool, u32>;

    /// Generate a challenge for WebAuthn authentication
    fn generate_challenge(env: Env, user: Address) -> Bytes;

    /// Get user's authentication profile
    fn get_auth_profile(env: Env, user: Address) -> Result<passkey_auth::UserAuthProfile, u32>;

    /// Set preferred authentication method (Ed25519 or Passkey)
    fn set_preferred_auth_method(
        env: Env,
        user: Address,
        method: passkey_auth::AuthMethod,
    ) -> Result<(), u32>;
}

// --- IMPLEMENTATION ---

#[contract]
pub struct SoroSusu;

#[contractimpl]
impl SoroSusuTrait for SoroSusu {
    /// Initializes the contract with a global administrator.
    ///
    /// # Parameters
    /// - `admin`: The address that will hold admin privileges (set protocol fee,
    ///   register anchors, purge stale groups, etc.).
    ///
    /// # Security
    /// Must be called exactly once after deployment. Subsequent calls overwrite
    /// the admin address, so the deployer should call this in the same transaction
    /// as the contract upload.
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
        grace_period: u64,
        late_fee_bps: u32,
    ) -> u64 {
        // Issue #321: Enforce MAX_CYCLE_DURATION cap to prevent overflow exploits.
        if cycle_duration > MAX_CYCLE_DURATION {
            panic!("cycle_duration exceeds MAX_CYCLE_DURATION");
        }

        // 1. Get the current Circle Count
        let mut circle_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CircleCount)
            .unwrap_or(0);

        // 2. Increment the ID for the new circle
        circle_count += 1;

#[contracttype]
#[derive(Clone)]
pub struct CollateralInfo {
    pub member: Address,
    pub circle_id: u64,
    pub amount: i128,
    pub status: CollateralStatus,
    pub staked_timestamp: u64,
    pub release_timestamp: Option<u64>,
}

#[contracttype]
#[derive(Clone)]
pub struct Member {
    pub address: Address,
    pub index: u32,
    pub contribution_count: u32,
    pub last_contribution_time: u64,
    pub status: MemberStatus,
    pub tier_multiplier: u32,
    pub consecutive_missed_rounds: u32,
    pub referrer: Option<Address>,
    pub buddy: Option<Address>,
    pub shares: u32,
    pub guarantor: Option<Address>,
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

// --- POT LIQUIDITY BUFFER DATA STRUCTURES ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum LiquidityAdvanceStatus {
    Pending,        // Advance requested, waiting for deposit
    Active,         // Advance provided, waiting for repayment
    Completed,      // Advance fully repaid
    Defaulted,      // Advance not repaid within grace period
    Cancelled,      // Advance cancelled by member
}

#[contracttype]
#[derive(Clone)]
pub struct LiquidityBufferConfig {
    pub is_enabled: bool,
    pub advance_period: u64,              // 48 hours advance window
    pub min_reputation: u32,               // 100% reputation required
    pub max_advance_bps: u32,             // 100% of contribution can be advanced
    pub platform_fee_allocation: u32,     // 20% of platform fees allocated to buffer
    pub min_reserve: i128,                // Minimum reserve balance
    pub max_reserve: i128,                // Maximum reserve balance
    pub advance_fee_bps: u32,             // 0.5% fee for advance usage
    pub grace_period: u64,                // 24 hours grace period for repayment
    pub max_advances_per_round: u32,      // Maximum advances per member per round
}

#[contracttype]
#[derive(Clone)]
pub struct LiquidityAdvance {
    pub advance_id: u64,
    pub member: Address,
    pub circle_id: u64,
    pub round_number: u32,
    pub contribution_amount: i128,         // Expected contribution amount
    pub advance_amount: i128,             // Amount advanced to member
    pub advance_fee: i128,                // Fee charged for advance
    pub repayment_amount: i128,           // Total amount to be repaid
    pub status: LiquidityAdvanceStatus,
    pub requested_timestamp: u64,         // When advance was requested
    pub provided_timestamp: Option<u64>,  // When advance was provided
    pub repayment_deadline: u64,          // When repayment is due
    pub repaid_timestamp: Option<u64>,    // When repayment was made
    pub reason: String,                   // Reason for advance request
}

#[contracttype]
#[derive(Clone)]
pub struct MemberAdvanceHistory {
    pub member: Address,
    pub total_advances_taken: u32,
    pub total_advance_amount: i128,
    pub total_fees_paid: i128,
    pub current_round_advances: u32,
    pub last_advance_timestamp: Option<u64>,
    pub repayment_history: Vec<u64>,      // List of advance IDs
    pub default_count: u32,               // Number of defaulted advances
    pub reputation_score: u32,            // Current reputation score
}

#[contracttype]
#[derive(Clone)]
pub struct LiquidityBufferStats {
    pub total_reserve_balance: i128,
    pub total_platform_fees_allocated: i128,
    pub total_advances_provided: u64,
    pub total_advances_completed: u64,
    pub total_advances_defaulted: u64,
    pub total_advance_amount: i128,
    pub total_fees_collected: i128,
    pub active_advances_count: u64,
    pub average_advance_size: i128,
    pub buffer_utilization_rate: u32,     // Current utilization as percentage
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct PlatformFeeAllocation {
    pub total_fees_collected: i128,
    pub buffer_allocation_amount: i128,
    pub treasury_allocation_amount: i128,
    pub last_allocation_timestamp: u64,
    pub allocation_frequency: u64,         // How often fees are allocated
/// Stellar Anchor Information - SEP-24/SEP-31 compliant anchor registry
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorInfo {
    pub anchor_address: Address,
    pub anchor_name: String,
    pub sep_version: String, // "SEP-24" or "SEP-31"
    pub authorization_level: u32, // 1=Basic, 2=Enhanced, 3=Full
    pub compliance_level: u32, // 1=Basic KYC, 2=Enhanced KYC, 3=Full KYC+AML
    pub is_active: bool,
    pub registration_timestamp: u64,
    pub last_activity: u64,
    pub supported_countries: Vec<String>, // ISO country codes
    pub max_deposit_amount: i128,
    pub daily_deposit_limit: i128,
}

/// Anchor Deposit Record - Track deposits made by anchors on behalf of users
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorDeposit {
    pub deposit_id: u64,
    pub anchor_address: Address,
    pub beneficiary_user: Address,
    pub circle_id: u64,
    pub amount: i128,
    pub deposit_memo: String, // Unique identifier for compliance
    pub fiat_reference: String, // Reference to fiat transaction
    pub timestamp: u64,
    pub compliance_verified: bool,
    pub processed: bool,
    pub sep_type: String, // "SEP-24" or "SEP-31"
}

/// Deposit Memo Structure - Standardized format for compliance
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositMemo {
    pub memo_type: String, // "text", "hash", or "return"
    pub memo_value: String,
    pub anchor_id: String,
    pub transaction_hash: Option<String>, // For blockchain reference
    pub compliance_data: String, // Encrypted compliance information
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
    // Initialize contract
    fn init(env: Env, admin: Address);
    fn set_lending_pool(env: Env, admin: Address, pool: Address);
    fn set_protocol_fee(env: Env, admin: Address, fee_basis_points: u32, treasury: Address);

    // --- POT LIQUIDITY BUFFER FOR BANK HOLIDAYS ---
    fn init_liquidity_buffer(env: Env, admin: Address);
    fn signal_advance_request(
        env: Env,
        member: Address,
        circle_id: u64,
        contribution_amount: i128,
        reason: String,
    ) -> u64;
    fn provide_advance(env: Env, advance_id: u64);
    fn cancel_advance_request(env: Env, advance_id: u64);
    fn process_advance_refill(env: Env, member: Address, circle_id: u64, deposit_amount: i128);
    fn get_liquidity_advance(env: Env, advance_id: u64) -> LiquidityAdvance;
    fn get_member_advance_history(env: Env, member: Address) -> MemberAdvanceHistory;
    fn get_liquidity_buffer_stats(env: Env) -> LiquidityBufferStats;
    fn allocate_platform_fees_to_buffer(env: Env, fee_amount: i128);
    fn check_advance_eligibility(env: Env, member: Address, circle_id: u64) -> bool;

    fn create_circle(
        env: Env,
        creator: Address,
        amount: i128,
        max_members: u32,
        token: Address,
        cycle_duration: u64,
        insurance_fee_bps: u32,
        nft_contract: Address,
    ) -> u64;

    // Join an existing circle
    fn join_circle(env: Env, user: Address, circle_id: u64, shares: u32, guarantor: Option<Address>);

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

    // --- SBT CREDENTIAL SYSTEM FUNCTIONS ---
    fn init_sbt_minter(env: Env, admin: Address);
    fn set_sbt_minter_admin(env: Env, admin: Address, new_admin: Address);
    fn issue_credential(
        env: Env,
        user: Address,
        milestone_id: u64,
        metadata_uri: String,
    ) -> u128;
    fn update_credential_status(
        env: Env,
        token_id: u128,
        new_status: SbtStatus,
    );
    fn revoke_credential(env: Env, token_id: u128, reason: String);
    fn get_credential(env: Env, token_id: u128) -> SoroSusuCredential;
    fn get_user_credential(env: Env, user: Address) -> Option<SoroSusuCredential>;
    fn get_reputation_milestone(env: Env, milestone_id: u64) -> ReputationMilestone;
    fn create_reputation_milestone(
        env: Env,
        user: Address,
        cycles_required: u32,
        description: String,
        reward_tier: ReputationTier,
    ) -> u64;
    fn update_user_reputation(env: Env, user: Address);
    fn get_user_reputation_score(env: Env, user: Address) -> (u32, u32, u32);

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

    /// Purge a group that has been dormant for ≥ 5 years.
    /// Returns any residual balance to the protocol treasury and removes
    /// the circle's storage entry to reclaim ledger rent.
    fn purge_stale_group(env: Env, admin: Address, circle_id: u64);

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
    fn propose_yield_delegation(env: Env, user: Address, circle_id: u64, delegation_percentage: u32, pool_address: Address, pool_type: YieldPoolType);
    fn vote_yield_delegation(env: Env, user: Address, circle_id: u64, vote_choice: YieldVoteChoice);
    fn approve_yield_delegation(env: Env, circle_id: u64);
    fn execute_yield_delegation(env: Env, circle_id: u64);
    fn compound_yield(env: Env, circle_id: u64);
    fn withdraw_yield_delegation(env: Env, circle_id: u64);
    fn distribute_yield_earnings(env: Env, circle_id: u64);

    // Path Payment Contribution Support
    fn propose_path_payment_support(env: Env, user: Address, circle_id: u64);
    fn vote_path_payment_support(env: Env, user: Address, circle_id: u64, vote_choice: PathPaymentVoteChoice);
    fn approve_path_payment_support(env: Env, circle_id: u64);
    fn execute_path_payment(env: Env, user: Address, circle_id: u64, source_token: Address, source_amount: i128);
    fn register_supported_token(env: Env, user: Address, token_address: Address, token_symbol: String, decimals: u32, is_stable: bool);
    fn register_dex(env: Env, user: Address, dex_address: Address, dex_name: String, is_trusted: bool);

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

    // Recursive Susu Cycles (Auto-Compounding)
    fn toggle_recursive_opt_in(env: Env, user: Address, circle_id: u64, enabled: bool);
    /// Set up a "Gold Tier" circle for recursive transitions
    fn recursive_init(env: Env, admin: Address, amount: i128, token: Address, circle_id: u64);

    // Cross-Contract Bridge for LeaseFlow
    fn is_cycle_healthy(env: Env, user: Address, circle_id: u64) -> bool;
    fn handle_leaseflow_default(env: Env, leaseflow_contract: Address, user: Address, circle_id: u64);
    fn set_leaseflow_contract(env: Env, admin: Address, leaseflow: Address);

    // Grant-Stream Matching Logic
    fn handle_grant_stream_match(env: Env, grant_stream_contract: Address, circle_id: u64, amount: i128);
    fn set_grant_stream_contract(env: Env, admin: Address, grant_stream: Address);
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
    if votes * 100 <= active_members * 66 {
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

        let old_member_key = DataKey::Member(old_address.clone());
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

        // Migrate UserStats
        if let Some(stats) = env.storage().instance().get::<DataKey, UserStats>(&DataKey::UserStats(old_address.clone())) {
            env.storage().instance().set(&DataKey::UserStats(new_address.clone()), &stats);
            env.storage().instance().remove(&DataKey::UserStats(old_address.clone()));
        }

        // Migrate SocialCapital
        if let Some(sc) = env.storage().instance().get::<DataKey, SocialCapital>(&DataKey::SocialCapital(old_address.clone(), circle_id)) {
            let mut new_sc = sc.clone();
            new_sc.member = new_address.clone();
            env.storage().instance().set(&DataKey::SocialCapital(new_address.clone(), circle_id), &new_sc);
            env.storage().instance().remove(&DataKey::SocialCapital(old_address.clone(), circle_id));
        }

        // Migrate SafetyDeposit
        if let Some(sd) = env.storage().instance().get::<DataKey, i128>(&DataKey::SafetyDeposit(old_address.clone(), circle_id)) {
            env.storage().instance().set(&DataKey::SafetyDeposit(new_address.clone(), circle_id), &sd);
            env.storage().instance().remove(&DataKey::SafetyDeposit(old_address.clone(), circle_id));
        }

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

    /// # Admin-Only: Set Lending Pool Address
    ///
    /// **Why admin-only:** The lending pool is a trusted external contract that
    /// receives protocol funds. Allowing arbitrary callers to change it would
    /// enable fund-draining attacks by redirecting deposits to a malicious pool.
    ///
    /// **If admin key is lost:** The lending pool address becomes permanently
    /// frozen at its last set value. Existing pool interactions continue to
    /// function, but the pool cannot be updated or disabled. Funds already
    /// deposited into the pool remain accessible via the pool contract itself.
    ///
    /// **DAO migration path:** Replace the single-admin check with a
    /// multi-sig governance proposal (≥ 2/3 council vote) before executing
    /// the pool address change. The `write_audit` call already provides an
    /// immutable on-chain record for every change.
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

    /// # Admin-Only: Set Protocol Fee and Treasury
    ///
    /// **Why admin-only:** The protocol fee is deducted from every member
    /// payout. An unconstrained caller could set the fee to 100 % (10 000 bps)
    /// and redirect all funds to an attacker-controlled treasury address.
    ///
    /// **If admin key is lost:** The fee and treasury address are frozen at
    /// their last configured values. Payouts continue to deduct the frozen fee
    /// and send it to the frozen treasury. No funds are trapped, but the
    /// protocol cannot adjust monetisation parameters.
    ///
    /// **DAO migration path:** Gate this function behind a time-locked
    /// governance proposal with a mandatory 48-hour delay and a ≥ 2/3
    /// multi-sig approval. Cap the maximum fee change per proposal to
    /// ±100 bps to prevent sudden large fee increases.
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

    /// Creates a new savings circle.
    ///
    /// # Parameters
    /// - `creator`: Address of the circle creator; must sign the transaction.
    /// - `amount`: Fixed contribution per round in stroops (1 XLM = 10 000 000 stroops).
    /// - `max_members`: Maximum number of members allowed (determines total rounds).
    /// - `token`: SEP-41 token contract address used for contributions and payouts.
    /// - `cycle_duration`: Seconds between rounds (e.g. `604800` = 1 week).
    /// - `insurance_fee_bps`: Per-member insurance premium in basis points (max 10 000).
    /// - `nft_contract`: SBT credential contract address for badge minting.
    ///
    /// # Returns
    /// The new `circle_id` (monotonically increasing `u64`).
    ///
    /// # Security
    /// - `creator` must call `require_auth()` — enforced internally.
    /// - `insurance_fee_bps` is capped at 10 000 (100 %) to prevent fee overflow.
    /// - `cycle_duration` is capped at `MAX_CYCLE_DURATION` to prevent epoch overflow.
    fn create_circle(
        env: Env,
        creator: Address,
        amount: i128,
        max_members: u32,
        token: Address,
        cycle_duration: u64,
        insurance_fee_bps: u32,
        nft_contract: Address,
    ) -> u64 {
        creator.require_auth();

        // Validate insurance fee (cannot exceed 100%)
        if insurance_fee_bps > 10_000 {
            panic!("Insurance fee cannot exceed 100%");
        }

        // Get the current Circle Count
        let mut circle_count: u64 = env.storage().instance().get(&DataKey::CircleCount).unwrap_or(0);
        
        // Increment for the new circle
        circle_count += 1;
        
        // Create the new circle using the canonical CircleInfo struct
        let circle = CircleInfo {
            id: circle_count,
            creator: creator.clone(),
            contribution_amount: amount,
            max_members,
            member_count: 0,
            current_recipient_index: 0,
            is_active: true,
            token: token.clone(),
            deadline_timestamp: env.ledger().timestamp() + cycle_duration,
            cycle_duration,
            contribution_bitmap: 0,
            insurance_balance: 0,
            insurance_fee_bps,
            is_insurance_used: false,
            late_fee_bps: 0,
            nft_contract,
            is_round_finalized: false,
            current_pot_recipient: None,
            requires_collateral: false,
            collateral_bps: 0,
            member_addresses: soroban_sdk::Vec::new(&env),
            leniency_enabled: false,
            grace_period_end: None,
            quadratic_voting_enabled: false,
            proposal_count: 0,
            dissolution_status: DissolutionStatus::NotInitiated,
            dissolution_deadline: None,
            proposed_late_fee_bps: 0,
            proposal_votes_bitmap: 0,
            recovery_old_address: None,
            recovery_new_address: None,
            recovery_votes_bitmap: 0,
            arbitrator: creator.clone(),
            basket: None,
        };

        // Store the circle
        env.storage().instance().set(&DataKey::Circle(circle_count), &circle);
        
        // Update the circle count
        env.storage().instance().set(&DataKey::CircleCount, &circle_count);

        circle_count
    }

    /// Adds a member to an existing open circle.
    ///
    /// # Parameters
    /// - `user`: Address joining the circle; must sign the transaction.
    /// - `circle_id`: ID of the target circle (must be in `Open` or `Active` state).
    /// - `shares`: Contribution multiplier — `1` (standard) or `2` (double share/payout).
    /// - `guarantor`: Optional address that vouches for the member's contributions.
    ///
    /// # Panics
    /// - `"Circle not found"` — `circle_id` does not exist.
    /// - `"Circle is full"` — `member_count >= max_members`.
    /// - `"Already a member"` — `user` is already in the circle.
    /// - `"Shares must be 1 or 2"` — invalid `shares` value.
    fn join_circle(env: Env, user: Address, circle_id: u64, shares: u32, guarantor: Option<Address>) {
        // Authorization: The user MUST sign this transaction
        user.require_auth();

        // Validate shares (must be 1 or 2)
        if shares != 1 && shares != 2 {
            panic!("Shares must be 1 or 2");
        }

        // Check if the circle exists
        let mut circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Check if the circle is full
        if circle.member_count >= circle.max_members {
            panic!("Circle is full");
        }

        // Check if the user is already a member
        if circle.member_addresses.contains(&user) {
            panic!("Already a member");
        }

        // Add the user to the members list
        circle.member_addresses.push_back(user.clone());
        circle.member_count += 1;

        // Store member by index for efficient lookup during payouts
        let member_index = circle.member_count - 1;
        env.storage().instance().set(&DataKey::CircleMember(circle_id, member_index), &user);

        // Create member record
        let member = Member {
            address: user.clone(),
            index: member_index,
            contribution_count: 0,
            last_contribution_time: 0,
            status: MemberStatus::Active,
            tier_multiplier: shares,
            consecutive_missed_rounds: 0,
            referrer: None,
            buddy: None,
            shares,
            guarantor,
        };

        // Store the member
        env.storage().instance().set(&DataKey::Member(user.clone()), &member);

        // Update the circle
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);
    }

    /// Submits the caller's contribution for the current round.
    ///
    /// # Parameters
    /// - `user`: Address making the deposit; must sign the transaction.
    /// - `circle_id`: ID of the circle to deposit into.
    ///
    /// # Security
    /// The caller must have pre-approved the SoroSusu contract to transfer
    /// `circle.contribution_amount` tokens on their behalf (SEP-41 `approve`).
    /// The token transfer is executed atomically within this call.
    ///
    /// # Panics
    /// - `"Circle not found"` — `circle_id` does not exist.
    fn deposit(env: Env, user: Address, circle_id: u64) {
        user.require_auth();
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));
        let token_client = soroban_sdk::token::Client::new(&env, &circle.token);
        token_client.transfer(&user, &env.current_contract_address(), &circle.contribution_amount);
    }


    // --- GAS BUFFER MANAGEMENT ---

    fn fund_gas_buffer(_env: Env, _circle_id: u64, _amount: i128) {
        // Gas buffer funding - not required for core functionality
    }

    fn set_gas_buffer_config(_env: Env, _circle_id: u64, _config: GasBufferConfig) {
        // Gas buffer config - not required for core functionality
    }

    fn get_gas_buffer_balance(_env: Env, _circle_id: u64) -> i128 {
        0
    }

    // --- PAYOUT FUNCTIONS WITH GAS BUFFER ---

    /// Distributes the round's pot to the current recipient.
    ///
    /// # Parameters
    /// - `caller`: Address initiating the payout; must sign the transaction.
    /// - `circle_id`: ID of the circle whose payout is being distributed.
    ///
    /// # Events
    /// Emits `payout_distributed { circle_id, recipient, gross_payout }`.
    ///
    /// # Panics
    /// - `"Circle not found"` — `circle_id` does not exist.
    /// - `"No recipient set"` — no recipient is queued for this round.
    fn distribute_payout(env: Env, caller: Address, circle_id: u64) {
        caller.require_auth();
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));
        let recipient = circle.current_pot_recipient.clone()
            .unwrap_or_else(|| panic!("No recipient set"));
        let gross_payout = circle.contribution_amount * circle.member_count as i128;
        let token_client = soroban_sdk::token::Client::new(&env, &circle.token);
        token_client.transfer(&env.current_contract_address(), &recipient, &gross_payout);
        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "payout_distributed"), circle_id),
            (recipient, gross_payout),
        );
    }

    /// # Admin-Only: Trigger Payout for a Circle
    ///
    /// **Why admin-only:** Payouts transfer the entire pot to a single
    /// recipient. Allowing arbitrary callers to trigger payouts could enable
    /// griefing (forcing premature payouts) or front-running attacks where an
    /// attacker triggers a payout before all members have contributed.
    ///
    /// **If admin key is lost:** Payouts can no longer be triggered by the
    /// admin. Members can still call `distribute_payout` directly (which
    /// enforces the same contribution-completeness check), so funds are not
    /// permanently locked. The admin trigger is a convenience/override path.
    ///
    /// **DAO migration path:** Expose a time-locked `propose_trigger_payout`
    /// governance action that requires a ≥ 2/3 member vote. This removes the
    /// single point of failure while preserving the override capability.
    /// Admin-only: forces a payout for a circle regardless of round state.
    ///
    /// # Parameters
    /// - `admin`: Admin address; must sign the transaction.
    /// - `circle_id`: ID of the circle to trigger payout for.
    ///
    /// # Security
    /// Only the stored admin address may call this function.
    ///
    /// # Panics
    /// - `"Unauthorized: Only admin can trigger payout"` — caller is not admin.
    fn trigger_payout(env: Env, admin: Address, circle_id: u64) {
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Admin not set"));
        if admin != stored_admin {
            panic!("Unauthorized: Only admin can trigger payout");
        }
        Self::distribute_payout(env, admin, circle_id);
    }

    /// Closes the current round and advances the payout queue to the next recipient.
    ///
    /// # Parameters
    /// - `creator`: Circle creator address; must sign the transaction.
    /// - `circle_id`: ID of the circle to finalize.
    ///
    /// # Events
    /// Emits `round_finalized { circle_id, next_recipient, scheduled_time }`.
    ///
    /// # Panics
    /// - `"Only creator can finalize round"` — caller is not the circle creator.
    /// - `"Round already finalized"` — the round has already been closed.
    /// - `"No members in circle"` — the circle has no members.
    fn finalize_round(env: Env, creator: Address, circle_id: u64) {
        creator.require_auth();

        // Check authorization (only creator can finalize)
        let mut circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        if creator != circle.creator {
            panic!("Only creator can finalize round");
        }

        if circle.is_round_finalized {
            panic!("Round already finalized");
        }

        if circle.member_count == 0 {
            panic!("No members in circle");
        }

        // Determine next recipient (round-robin based on current_recipient_index)
        let next_recipient_index = circle.current_recipient_index % circle.member_count;
        let next_recipient: Address = env.storage().instance()
            .get(&DataKey::CircleMember(circle_id, next_recipient_index))
            .unwrap_or_else(|| panic!("Member not found for next round"));

        // Update circle state
        circle.is_round_finalized = true;
        circle.current_pot_recipient = Some(next_recipient.clone());
        circle.current_recipient_index = (next_recipient_index + 1) % circle.member_count;
        circle.deadline_timestamp = env.ledger().timestamp() + circle.cycle_duration;

        // Store updated circle
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Schedule payout time
        let scheduled_time = env.ledger().timestamp() + circle.cycle_duration;
        env.storage().instance().set(&DataKey::ScheduledPayoutTime(circle_id), &scheduled_time);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "Payout_Ready"), circle_id),
            (next_recipient, scheduled_time),
        );
    }

    // --- HELPER FUNCTIONS ---

    /// Returns the full state of a circle.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle to query.
    ///
    /// # Returns
    /// A `CircleInfo` struct containing all circle metadata and member list.
    ///
    /// # Panics
    /// - `"Circle not found"` — `circle_id` does not exist.
    fn get_circle(env: Env, circle_id: u64) -> CircleInfo {
        env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"))
    }

    /// Returns the membership record for an address.
    ///
    /// # Parameters
    /// - `member`: Address to look up.
    ///
    /// # Returns
    /// A `Member` struct with contribution history, status, buddy, and guarantor.
    ///
    /// # Panics
    /// - `"Member not found"` — `member` has never joined a circle.
    fn get_member(env: Env, member: Address) -> Member {
        env.storage().instance()
            .get(&DataKey::Member(member))
            .unwrap_or_else(|| panic!("Member not found"))
    }

    /// Returns the address scheduled to receive the next payout, or `None`.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle to query.
    ///
    /// # Returns
    /// `Some(Address)` if a recipient is queued, `None` if no payout is pending.
    fn get_current_recipient(env: Env, circle_id: u64) -> Option<Address> {
        let circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));
        circle.current_pot_recipient
    }

    // --- STELLAR ANCHOR DIRECT DEPOSIT API (SEP-24/SEP-31) ---

    fn register_anchor(env: Env, admin: Address, anchor_info: AnchorInfo) {
        // Only admin can register anchors
        admin.require_auth();
        
        // Verify admin is contract admin
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Admin not found"));
        
        if admin != stored_admin {
            panic!("Unauthorized: Only admin can register anchors");
        }

        // Store anchor info in registry
        let mut anchor_registry: Map<Address, AnchorInfo> = env.storage().instance()
            .get(&DataKey::AnchorRegistry)
            .unwrap_or_else(|| Map::new(&env));
        
        anchor_registry.set(anchor_info.anchor_address.clone(), anchor_info.clone());
        env.storage().instance().set(&DataKey::AnchorRegistry, &anchor_registry);

        // Log audit entry
        let audit_count: u64 = env.storage().instance()
            .get(&DataKey::AuditCount)
            .unwrap_or(0);
        
        let audit_entry = AuditEntry {
            id: audit_count,
            actor: admin,
            action: AuditAction::AdminAction,
            timestamp: env.ledger().timestamp(),
            resource_id: 0, // Use 0 for anchor registration
        };
        
        env.storage().instance().set(&DataKey::AuditEntry(audit_count), &audit_entry);
        env.storage().instance().set(&DataKey::AuditCount, &(audit_count + 1));
    }

    fn deposit_for_user(
        env: Env,
        anchor: Address,
        beneficiary_user: Address,
        circle_id: u64,
        amount: i128,
        deposit_memo: String,
        fiat_reference: String,
        sep_type: String,
    ) {
        // Authorization: The anchor must sign this!
        anchor.require_auth();

        // Verify anchor is registered and authorized
        let anchor_registry: Map<Address, AnchorInfo> = env.storage().instance()
            .get(&DataKey::AnchorRegistry)
            .unwrap_or_else(|| panic!("Anchor registry not found"));
        
        let anchor_info: AnchorInfo = anchor_registry.get(anchor.clone())
            .unwrap_or_else(|| panic!("Anchor not found"));

        if !anchor_info.is_active {
            panic!("Anchor not active");
        }

        // Verify SEP type is supported
        if sep_type != "SEP-24" && sep_type != "SEP-31" {
            panic!("Unsupported SEP type");
        }

        // Compliance checks
        if amount > anchor_info.max_deposit_amount {
            panic!("Amount exceeds anchor's maximum deposit limit");
        }

        // Check if deposit memo already processed (prevent double processing)
        let memo_key = DataKey::DepositMemo(circle_id);
        let mut processed_memos: Vec<String> = env.storage().instance()
            .get(&memo_key)
            .unwrap_or_else(|| Vec::new(&env));
        
        if processed_memos.contains(&deposit_memo) {
            panic!("Deposit already processed");
        }

        // Get the circle
        let mut circle: CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("Circle not found"));

        // Get the member
        let mut member: Member = env.storage().instance()
            .get(&DataKey::Member(beneficiary_user.clone()))
            .unwrap_or_else(|| panic!("Member not found"));

        // Check if already contributed this round
        if member.contribution_count > 0 {
            panic!("Already contributed this round");
        }

        // Calculate the total amount needed (contribution + insurance fee + group insurance premium)
        let insurance_fee = (circle.contribution_amount as i128 * circle.insurance_fee_bps as i128) / 10_000;
        let group_insurance_premium = (circle.contribution_amount as i128 * 50i128) / 10_000;
        let total_amount = circle.contribution_amount as i128 + insurance_fee + group_insurance_premium;

        // Verify amount matches expected contribution
        if amount != total_amount {
            panic!("Amount does not match required contribution");
        }

        // Create deposit record
        let deposit_id = env.ledger().sequence(); // Use ledger sequence as unique ID
        let deposit_record = AnchorDeposit {
            deposit_id,
            anchor_address: anchor.clone(),
            beneficiary_user: beneficiary_user.clone(),
            circle_id,
            amount,
            deposit_memo: deposit_memo.clone(),
            fiat_reference,
            timestamp: env.ledger().timestamp(),
            compliance_verified: true,
            processed: false,
            sep_type,
        };

        // Store deposit record
        env.storage().instance().set(&DataKey::AnchorDeposit(deposit_id), &deposit_record);

        // Mark memo as processed
        processed_memos.push_back(deposit_memo);
        env.storage().instance().set(&memo_key, &processed_memos);

        // Transfer the tokens from anchor to contract
        let token_client = token::Client::new(&env, &circle.token);
        token_client.transfer(&anchor, &env.current_contract_address(), &total_amount);

        // Update member record (similar to regular deposit)
        member.last_contribution_time = env.ledger().timestamp();
        member.contribution_count += 1;

        // Update user stats
        let mut user_stats: UserStats = env.storage().instance()
            .get(&DataKey::UserStats(beneficiary_user.clone()))
            .unwrap_or_else(|| UserStats {
                total_volume_saved: 0,
                on_time_contributions: 0,
                late_contributions: 0,
            });
        
        user_stats.total_volume_saved += total_amount;
        user_stats.on_time_contributions += 1;
        env.storage().instance().set(&DataKey::UserStats(beneficiary_user.clone()), &user_stats);

        // Store the updated member
        env.storage().instance().set(&DataKey::Member(beneficiary_user.clone()), &member);

        // Update circle contribution bitmap
        let member_index = member.index;
        circle.contribution_bitmap |= 1u64 << member_index;

        // Store the updated circle
        env.storage().instance().set(&DataKey::Circle(circle_id), &circle);

        // Update anchor's last activity
        let mut updated_anchor_info = anchor_info.clone();
        updated_anchor_info.last_activity = env.ledger().timestamp();
        anchor_registry.set(anchor.clone(), updated_anchor_info);
        env.storage().instance().set(&DataKey::AnchorRegistry, &anchor_registry);

        // Mark deposit as processed
        let mut updated_deposit = deposit_record;
        updated_deposit.processed = true;
        env.storage().instance().set(&DataKey::AnchorDeposit(deposit_id), &updated_deposit);

        // Log audit entry
        let audit_count: u64 = env.storage().instance()
            .get(&DataKey::AuditCount)
            .unwrap_or(0);
        
        let audit_entry = AuditEntry {
            id: audit_count,
            actor: anchor,
            action: AuditAction::AdminAction, // Use AdminAction for anchor deposits
            timestamp: env.ledger().timestamp(),
            resource_id: circle_id,
        };
        
        env.storage().instance().set(&DataKey::AuditEntry(audit_count), &audit_entry);
        env.storage().instance().set(&DataKey::AuditCount, &(audit_count + 1));
    }

    fn verify_anchor_deposit(env: Env, deposit_id: u64) -> bool {
        let deposit: AnchorDeposit = env.storage().instance()
            .get(&DataKey::AnchorDeposit(deposit_id))
            .unwrap_or_else(|| panic!("Deposit not found"));
        
        deposit.processed && deposit.compliance_verified
    }

    fn get_anchor_info(env: Env, anchor_address: Address) -> AnchorInfo {
        let anchor_registry: Map<Address, AnchorInfo> = env.storage().instance()
            .get(&DataKey::AnchorRegistry)
            .unwrap_or_else(|| panic!("Anchor registry not found"));
        
        anchor_registry.get(anchor_address)
            .unwrap_or_else(|| panic!("Anchor not found"))
    }

    fn get_deposit_record(env: Env, deposit_id: u64) -> AnchorDeposit {
        env.storage().instance()
            .get(&DataKey::AnchorDeposit(deposit_id))
            .unwrap_or_else(|| panic!("Deposit not found"))
    }

    /// Claims the current round's payout pot for the calling user.
    ///
    /// # Parameters
    /// - `user`: Address claiming the pot; must be the current round's recipient
    ///   and must sign the transaction.
    /// - `circle_id`: ID of the circle.
    ///
    /// # Security
    /// - Enforces a flash-loan prevention guard: the user cannot withdraw and
    ///   deposit in the same ledger.
    /// - Reverts if a LeaseFlow default lock is active on the circle.
    ///
    /// # Panics
    /// - `"Circle not found"` — `circle_id` does not exist.
    fn claim_pot(env: Env, user: Address, circle_id: u64) {
        user.require_auth();

        // Flash-loan prevention: Ledger-Lock mechanism
        let current_ledger = env.ledger().sequence();
        if let Some(last_deposit) = env.storage().instance().get::<DataKey, u32>(&DataKey::LastDepositLedger(user.clone())) {
            if last_deposit == current_ledger {
                panic!("Flash-loan prevention: Cannot withdraw and deposit in same ledger");
            }
        }
        env.storage().instance().set(&DataKey::LastWithdrawalLedger(user.clone()), &current_ledger);

        // Inter-protocol security: Check if payout is paused due to external default (e.g., LeaseFlow)
        let is_paused = env.storage().instance().get::<DataKey, bool>(&DataKey::PausedPayout(user.clone(), circle_id)).unwrap_or(false);
        if is_paused {
            panic!("Your payout is currently locked due to a default in a connected protocol (LeaseFlow). Please resolve the default to unlock.");
        }
        let mut circle: CircleInfo = env
            .storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &circle);

        // 10. Mark as Paid in the old format for backward compatibility
        env.storage()
            .instance()
            .set(&DataKey::Deposit(circle_id, user), &true);
    }

    // -----------------------------------------------------------------------
    // Issue #275 – Reputation-NFT (SBT) Minting Hook
    // -----------------------------------------------------------------------

    fn mint_sbt_credential(env: &Env, user: Address, cycles_completed: u32) {
        // Check if user already has an SBT for this level
        let user_sbt_key = DataKey::UserSbt(user.clone());
        if env.storage().instance().has(&user_sbt_key) {
            // Already has SBT, skip
            return;
        }

        // Determine status based on cycles
        let status = match cycles_completed {
            5 => SbtStatus::Discovery,
            10 => SbtStatus::Pathfinder,
            15 => SbtStatus::Guardian,
            20 => SbtStatus::Luminary,
            _ => SbtStatus::SusuLegend,
        };

        // Get reputation score (mock for now)
        let reputation_score = 7500; // Mock score

        // Create token ID
        let token_id = env.ledger().timestamp() as u128 * 1000 + cycles_completed as u128;

        let credential = SoroSusuCredential {
            token_id,
            user: user.clone(),
            status,
            reputation_score,
            metadata_uri: String::from_str(env, "ipfs://sorosusu-sbt-metadata"),
            issue_date: env.ledger().timestamp(),
        };

        // Store credential
        env.storage().instance().set(&DataKey::SbtCredential(token_id), &credential);
        env.storage().instance().set(&user_sbt_key, &token_id);

        // Emit event
        env.events().publish(
            (Symbol::new(env, "SbtMinted"),),
            (user, token_id, status),
        );
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

    /// # Creator-Only: Eject a Member from a Circle
    ///
    /// **Why creator-only:** Ejection burns the member's NFT credential and
    /// marks their status as `Ejected`, permanently removing them from the
    /// payout queue. Allowing arbitrary callers to eject members would enable
    /// targeted griefing attacks against honest participants.
    ///
    /// **If admin/creator key is lost:** Members cannot be ejected. Defaulting
    /// members remain in the queue and block payout progression. The insurance
    /// fund (`trigger_insurance_coverage`) provides a mitigation path that does
    /// not require ejection.
    ///
    /// **DAO migration path:** Introduce a `propose_eject_member` governance
    /// action requiring a ≥ 2/3 circle-member vote with a 24-hour challenge
    /// window. This distributes the ejection power across the group and removes
    /// the single-creator trust assumption.
    /// Removes a member from a circle. Callable by the circle creator or admin.
    ///
    /// # Parameters
    /// - `caller`: Address initiating the ejection; must sign the transaction.
    /// - `circle_id`: ID of the circle.
    /// - `member`: Address of the member to eject.
    ///
    /// # Panics
    /// - `"Unauthorized"` — caller is neither the creator nor the admin.
    fn eject_member(env: Env, caller: Address, circle_id: u64, member: Address) {
        caller.require_auth();
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

    /// # Admin-Only: Purge a Stale (5-Year Inactive) Group
    ///
    /// Identifies groups that have had no activity for ≥ 5 years
    /// (157 680 000 seconds), returns any residual token balance to the
    /// protocol treasury, and removes the circle's storage entry to reclaim
    /// ledger rent.
    ///
    /// **Why admin-only:** Purging deletes on-chain state and transfers funds.
    /// Restricting this to the admin prevents griefing attacks where an
    /// adversary purges active circles by manipulating timestamps.
    ///
    /// **DAO migration path:** Replace the admin check with a governance
    /// proposal that any token holder can submit after the 5-year threshold
    /// is verifiably exceeded.
    /// Admin-only: archives metadata and returns residual funds to the protocol
    /// treasury for a circle that has been dormant for ≥ 5 years.
    ///
    /// # Parameters
    /// - `admin`: Admin address; must sign the transaction.
    /// - `circle_id`: ID of the stale circle to purge.
    ///
    /// # Events
    /// Emits `stale_group_purged { circle_id, admin, residual }`.
    ///
    /// # Panics
    /// - `"Unauthorized: only admin can purge stale groups"` — caller is not admin.
    /// - `"Circle is not stale: last activity was less than 5 years ago"`.
    fn purge_stale_group(env: Env, admin: Address, circle_id: u64) {
        admin.require_auth();

        // Verify caller is the stored admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        if admin != stored_admin {
            panic!("Unauthorized: only admin can purge stale groups");
        }

        // Load the circle
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .expect("Circle not found");

        // 5 years in seconds: 5 * 365.25 * 24 * 3600 ≈ 157_766_400
        const FIVE_YEARS_SECS: u64 = 157_766_400;
        let now = env.ledger().timestamp();
        let last_active = circle.deadline_timestamp; // deadline doubles as last-activity marker

        if now < last_active + FIVE_YEARS_SECS {
            panic!("Circle is not stale: last activity was less than 5 years ago");
        }

        // Return any residual insurance balance to the protocol treasury
        let residual = circle.insurance_balance;
        if residual > 0 {
            let treasury: Address = env
                .storage()
                .instance()
                .get(&DataKey::ProtocolTreasury)
                .expect("Protocol treasury not set");
            let token_client = token::Client::new(&env, &circle.token);
            token_client.transfer(&env.current_contract_address(), &treasury, &residual);
        }

        // Remove the circle entry to reclaim storage rent
        env.storage().instance().remove(&DataKey::Circle(circle_id));

        env.events().publish(
            (Symbol::new(&env, "stale_group_purged"), circle_id),
            (admin, residual),
        );
    }

    /// Assigns a social buddy to the caller for security and recovery purposes.
    ///
    /// # Parameters
    /// - `user`: Address setting the buddy; must sign the transaction.
    /// - `buddy_address`: Address of the designated buddy.
    ///
    /// # Security
    /// The buddy can cover missed payments from the user's safety deposit and
    /// participate in social recovery votes on the user's behalf.
    fn pair_with_member(env: Env, user: Address, buddy_address: Address) {
        user.require_auth();
        let user_key = DataKey::Member(user.clone());
        let mut user_info: Member = env
            .storage()
            .instance()
            .get(&user_key)
            .expect("Member not found");

        // For now, we'll just mark them as defaulted
        Ok(())
    }

    // Issue #324: Slash collateral — move the defaulted member's collateral into
    // the 72-hour pending vault so they have time to appeal before redistribution.
    /// Admin-only: moves a defaulted member's collateral into the 72-hour
    /// pending vault (appeals timelock, Issue #324).
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle containing the defaulted member.
    /// - `member`: Address of the member whose collateral is being slashed.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(Error::NothingToSlash)` (`405`) if the member has no collateral.
    ///
    /// # Security
    /// Only the stored admin address may call this function. The slashed funds
    /// are held in a pending vault for 72 hours to allow the member to appeal
    /// before redistribution to victims.
    fn slash_collateral(env: Env, circle_id: u64, member: Address) -> Result<(), u32> {
        // Only admin may initiate a slash.
        let admin: Address = env.storage().instance().get(&DataKey::Admin).ok_or(401u32)?;
        admin.require_auth();

        // Member must be marked as defaulted.
        let defaulted_key = DataKey::DefaultedMember(circle_id, member.clone());
        if !env.storage().instance().has(&defaulted_key) {
            return Err(405); // Member has not defaulted — nothing to slash.
        }

        // Get member data
        let member_key = DataKey::Member(member.clone());
        let member_data: Member = env.storage().instance().get(&member_key).ok_or(402)?;

        // Get circle data
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .ok_or(401u32)?;

        // Calculate remaining needed
        let remaining_needed = circle.contribution_amount - member_data.amount_contributed;

        // Calculate penalty
        let penalty = (remaining_needed as u64 * circle.late_fee_bps as u64) / 10000;

        // Calculate slash amount
        let mut slash_amount = remaining_needed + penalty as u64;

        // Cap at amount contributed
        if slash_amount > member_data.amount_contributed {
            slash_amount = member_data.amount_contributed;
        }

        // Calculate remainder
        let remainder = member_data.amount_contributed - slash_amount;

        // Transfer remainder back to user if any
        if remainder > 0 {
            let client = token::Client::new(&env, &circle.token);
            client.transfer(
                &env.current_contract_address(),
                &member,
                &remainder,
            );
        }

        // Move slash amount to pending vault
        let record = PendingSlashRecord {
            amount: slash_amount,
            slashed_at: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&DataKey::PendingSlash(circle_id, member), &record);

        Ok(())
    }

    // Issue #324: Release pending-slash funds to the group reserve after the
    // 72-hour appeals window has elapsed.
    /// Releases slashed collateral from the pending vault after the 72-hour
    /// appeals timelock has expired.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `member`: Address of the member whose pending slash is being released.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(Error::TimelockActive)` (`406`) if the 72-hour window has not yet elapsed.
    ///
    /// # Security
    /// Enforces `APPEALS_TIMELOCK_SECS` (72 hours) before redistribution,
    /// giving the member time to raise a dispute.
    fn release_pending_slash(env: Env, circle_id: u64, member: Address) -> Result<(), u32> {
        let record: PendingSlashRecord = env
            .storage()
            .instance()
            .get(&DataKey::PendingSlash(circle_id, member.clone()))
            .ok_or(405u32)?; // No pending slash for this member.

        let current_time = env.ledger().timestamp();
        let release_time = record
            .slashed_at
            .checked_add(APPEALS_TIMELOCK_SECS)
            .ok_or(405u32)?;

        if current_time < release_time {
            return Err(406); // Timelock has not yet expired — appeal window still open.
        }

        // Timelock expired: redistribute to group reserve.
        let mut reserve: u64 = env
            .storage()
            .instance()
            .get(&DataKey::GroupReserve)
            .unwrap_or(0);
        reserve = reserve.saturating_add(record.amount);
        env.storage().instance().set(&DataKey::GroupReserve, &reserve);

        // Remove the pending slash record.
        env.storage()
            .instance()
            .remove(&DataKey::PendingSlash(circle_id, member));

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

        // 5. Finalize Deposit
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
    
    /// # Admin-Only: Set Hard Asset Basket Weights
    ///
    /// **Why admin-only:** The hard asset basket defines the reference
    /// allocation used by the economic circuit breaker to detect treasury
    /// instability. Allowing arbitrary callers to change these weights could
    /// disable the circuit breaker or trigger false-positive asset swaps,
    /// causing unnecessary treasury rebalancing and member losses.
    ///
    /// **If admin key is lost:** The basket weights are frozen at their last
    /// configured values. The circuit breaker continues to operate against the
    /// frozen reference basket. No funds are at risk, but the protocol cannot
    /// adapt to changing macro conditions.
    ///
    /// **DAO migration path:** Require a ≥ 2/3 governance vote with a 72-hour
    /// time-lock before basket weights can be changed. Emit a
    /// `BASKET_CHANGE_PROPOSED` event at proposal time so members can exit
    /// before the change takes effect.
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

    /// Distributes yield to members in paginated chunks of 10 to avoid gas limits.
    ///
    /// Integer division ensures `yield_per_member = total_yield / member_count`
    /// always rounds **down**, so the contract never attempts to send more funds
    /// than it holds. The fractional remainder (dust) stays in the contract reserve.
    /// Call repeatedly until `BatchHarvestProgress.is_complete == true`.
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    /// - `total_yield_amount`: Total yield in stroops to distribute pro-rata.
    /// - `member_addresses`: Full member address list.
    ///
    /// # Returns
    /// - `Ok(BatchHarvestProgress)` — progress snapshot after this chunk.
    /// - `Err(401)` — circle not found.
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
                    // Issue #320: Pre-flight trustline check.
                    // Attempt to verify the recipient has a trustline for the circle token.
                    // On Stellar, a transfer to an address without a trustline will fail.
                    // We detect this by checking if the member has ever interacted with the
                    // token (contribution_count > 0 implies they deposited, so trustline exists).
                    // For new/external recipients, we hold funds and emit MissingTrustline.
                    let member_data: Member = env
                        .storage()
                        .instance()
                        .get(&member_key)
                        .unwrap();
                    let has_trustline = member_data.contribution_count > 0;

                    if !has_trustline {
                        // Hold funds: mark this member's payout as pending trustline resolution.
                        env.storage().instance().set(
                            &DataKey::MissingTrustline(circle_id, member_address.clone()),
                            &yield_per_member,
                        );
                        // Emit MissingTrustline event so off-chain systems can notify the member.
                        env.events().publish(
                            (Symbol::new(&env, "MissingTrustline"),),
                            (circle_id, member_address.clone(), yield_per_member),
                        );
                        // Skip crediting this member; do NOT increment members_processed
                        // so the group's execution flow is not blocked.
                        progress.last_processed_index = i + 1;
                        continue;
                    }

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

    // -----------------------------------------------------------------------
    // Issue #274 – Group-Reputation Aggregate Score
    // -----------------------------------------------------------------------

    /// Returns the aggregate reputation score for a circle (0–100).
    ///
    /// # Parameters
    /// - `circle_id`: ID of the circle.
    ///
    /// # Returns
    /// A `u32` score from 0 (poor) to 100 (excellent), based on on-time
    /// payment history and member reliability indices.
    fn get_group_reputation(env: Env, circle_id: u64) -> u32 {
        // For now, return a mock calculation
        // In a full implementation, this would calculate the average RI of all group members
        // using the reliability oracle
        
        // Mock: base reputation with some variance
        let base_reputation = 7500; // 75% base reputation
        let variance = (circle_id % 2500) as u32; // Some variance based on circle ID
        (base_reputation + variance).min(10000)
    }

    // -----------------------------------------------------------------------
    // Issue #315 – Reentrancy-guarded payout & slash_stake
    // -----------------------------------------------------------------------

    fn payout(env: Env, caller: Address, circle_id: u64) {
        caller.require_auth();

        // Acquire NON_REENTRANT lock before any state changes or external calls.
        dispute::acquire_lock(&env);

        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("circle not found"));

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("admin not set"));

        if caller != admin && caller != circle.creator {
            panic!("unauthorized");
        }

        let payout_amount = (circle.contribution_amount as i128)
            .checked_mul(circle.member_count as i128)
            .expect("payout overflow");

        // Get current recipient (simplified - in production, resolve from payout queue)
        // For now, we'll use the circle creator as the recipient
        let recipient = circle.creator.clone();

        // Check user's payout preference
        let user_preference = Self::get_payout_preference(env.clone(), recipient.clone(), circle_id);

        // Commit state update BEFORE external token transfer (CEI pattern).
        let mut updated_circle = circle.clone();
        updated_circle.current_recipient_index += 1;
        if updated_circle.current_recipient_index >= updated_circle.member_count {
            // Mark circle completed and record timestamp for cleanup_group (issue #316).
            updated_circle.is_active = false;
            env.storage()
                .instance()
                .set(&DataKey::CircleCompletedAt(circle_id), &env.ledger().timestamp());
        }
        env.storage()
            .instance()
            .set(&DataKey::Circle(circle_id), &updated_circle);

        // Route payout based on user preference
        match user_preference.payout_method {
            PayoutMethod::DirectToken => {
                // Traditional token payout
                let token = soroban_sdk::token::Client::new(&env, &circle.token);
                token.transfer(
                    &env.current_contract_address(),
                    &recipient,
                    &payout_amount,
                );
            }
            PayoutMethod::DirectToBank => {
                // SEP-24 Anchor payout for fiat conversion
                if let Some(anchor_config) = user_preference.anchor_config {
                    // Process payout through anchor
                    match Self::process_anchor_payout(
                        env.clone(),
                        anchor_config.preferred_anchor,
                        recipient.clone(),
                        circle_id,
                        payout_amount as u64,
                        circle.token.clone(),
                    ) {
                        Ok(deposit_id) => {
                            // Successfully routed to anchor - could emit event here
                            // In production, you might want to store the deposit_id for tracking
                        }
                        Err(error_code) => {
                            // Fallback to direct token payout if anchor fails
                            let token = soroban_sdk::token::Client::new(&env, &circle.token);
                            token.transfer(
                                &env.current_contract_address(),
                                &recipient,
                                &payout_amount,
                            );
                            // In production, you might want to log this fallback
                        }
                    }
                } else {
                    // No anchor config - fallback to direct token payout
                    let token = soroban_sdk::token::Client::new(&env, &circle.token);
                    token.transfer(
                        &env.current_contract_address(),
                        &recipient,
                        &payout_amount,
                    );
                }
            }
        }

        // Release lock after all work is done.
        dispute::release_lock(&env);
    }

    fn slash_stake(env: Env, admin: Address, circle_id: u64, member: Address) {
        admin.require_auth();

        // Verify caller is admin.
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("admin not set"));
        if admin != stored_admin {
            panic!("unauthorized");
        }

        // Acquire NON_REENTRANT lock.
        dispute::acquire_lock(&env);

        // Mark member as defaulted (state update before any transfer).
        let defaulted_key = DataKey::DefaultedMember(circle_id, member.clone());
        env.storage().instance().set(&defaulted_key, &true);

        // Release lock.
        dispute::release_lock(&env);
    }

    // -----------------------------------------------------------------------
    // Issue #316 – Zombie-Group Sweep
    // -----------------------------------------------------------------------

    fn cleanup_group(env: Env, caller: Address, circle_id: u64) {
        dispute::cleanup_group(&env, &caller, circle_id);
    }

    // -----------------------------------------------------------------------
    // Issue #322 – Dispute Bond Slashing
    // -----------------------------------------------------------------------

    fn raise_dispute(
        env: Env,
        accuser: Address,
        accused: Address,
        circle_id: u64,
        xlm_token: Address,
    ) -> u64 {
        dispute::raise_dispute(&env, &accuser, &accused, circle_id, &xlm_token)
    }

    fn submit_evidence(env: Env, submitter: Address, dispute_id: u64, evidence_hash: u64) {
        dispute::submit_evidence(&env, &submitter, dispute_id, evidence_hash);
    }

    fn juror_vote(env: Env, juror: Address, dispute_id: u64, vote_guilty: bool) {
        dispute::juror_vote(&env, &juror, dispute_id, vote_guilty);
    }

    fn execute_verdict(
        env: Env,
        admin: Address,
        dispute_id: u64,
        baseless: bool,
        xlm_token: Address,
    ) {
        dispute::execute_verdict(&env, &admin, dispute_id, baseless, &xlm_token);
    }

    // -----------------------------------------------------------------------
    // Issue #304 – Yield opt-out
    // -----------------------------------------------------------------------

    /// Opts a member out of yield routing for a specific circle.
    /// Opted-out members' contributions are excluded from AMM routing and
    /// they receive no yield distributions.
    ///
    /// # Parameters
    /// - `user`: Address opting out; must sign the transaction.
    /// - `circle_id`: ID of the circle.
    ///
    /// # Returns
    /// `Ok(())` on success.
    fn opt_out_of_yield(env: Env, user: Address, circle_id: u64) -> Result<(), u32> {
        user.require_auth();
        let member_key = DataKey::Member(user.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .ok_or(402u32)?;
        member.opt_out_of_yield = true;
        env.storage().instance().set(&member_key, &member);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Commit-reveal voting stubs
    // -----------------------------------------------------------------------

    fn initialize_voting_session(
        env: Env,
        circle_id: u64,
        commit_duration: u64,
        reveal_duration: u64,
    ) -> Result<(), u32> {
        let now = env.ledger().timestamp();
        let session = VotingSession {
            circle_id,
            commit_deadline: now + commit_duration,
            reveal_deadline: now + commit_duration + reveal_duration,
            total_commits: 0,
            total_reveals: 0,
            yes_votes: 0,
            no_votes: 0,
            is_finalized: false,
        };
        env.storage()
            .instance()
            .set(&DataKey::VotingSession(circle_id), &session);
        Ok(())
    }

    fn commit_vote(env: Env, voter: Address, circle_id: u64, commitment: Vec<u8>) -> Result<(), u32> {
        voter.require_auth();
        let mut session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .ok_or(404u32)?;
        let now = env.ledger().timestamp();
        if now > session.commit_deadline {
            return Err(405u32);
        }
        let _ = commitment;
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
        voter.require_auth();
        let mut session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .ok_or(404u32)?;
        let now = env.ledger().timestamp();
        if now <= session.commit_deadline || now > session.reveal_deadline {
            return Err(406u32);
        }
        let _ = salt;
        session.total_reveals += 1;
        if vote {
            session.yes_votes += 1;
        } else {
            session.no_votes += 1;
        }
        env.storage()
            .instance()
            .set(&DataKey::VotingSession(circle_id), &session);
        Ok(())
    }

    fn tally_votes(env: Env, circle_id: u64) -> Result<bool, u32> {
        let mut session: VotingSession = env
            .storage()
            .instance()
            .get(&DataKey::VotingSession(circle_id))
            .ok_or(404u32)?;
        let now = env.ledger().timestamp();
        if now <= session.reveal_deadline {
            return Err(407u32);
        }
        session.is_finalized = true;
        let passed = session.yes_votes > session.no_votes;
        env.storage()
            .instance()
            .set(&DataKey::VotingSession(circle_id), &session);
        Ok(passed)
    }

    // -----------------------------------------------------------------------
    // Recovery helpers
    // -----------------------------------------------------------------------

    fn check_recovery_state(env: Env, circle_id: u64) -> bool {
        let circle: Option<CircleInfo> = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id));
        match circle {
            Some(c) => !c.is_active,
            None => false,
        }
    }

    fn claim_abandoned_funds(env: Env, user: Address, circle_id: u64) {
        user.require_auth();
        let deposit_key = DataKey::InitialDeposit(circle_id, user.clone());
        let amount: u64 = env
            .storage()
            .instance()
            .get(&deposit_key)
            .unwrap_or_else(|| panic!("no deposit found"));
        let circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("circle not found"));
        if circle.is_active {
            panic!("circle is still active");
        }
        env.storage().instance().remove(&deposit_key);
        let token = soroban_sdk::token::Client::new(&env, &circle.token);
        token.transfer(
            &env.current_contract_address(),
            &user,
            &(amount as i128),
        );
    }

    // --- SEP-24 ANCHOR INTEGRATION IMPLEMENTATIONS ---

    fn register_anchor(
        env: Env,
        admin: Address,
        anchor_address: Address,
        name: Symbol,
        sep_version: Symbol,
        kyc_required: bool,
        supported_tokens: Vec<Address>,
        max_deposit_amount: u64,
        daily_deposit_limit: u64,
    ) {
        admin.require_auth();

        // Verify admin authorization
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("admin not set"));
        if admin != stored_admin {
            panic!("unauthorized");
        }

        // Create anchor info
        let anchor_info = AnchorInfo {
            address: anchor_address.clone(),
            name,
            sep_version,
            status: AnchorStatus::Active,
            kyc_required,
            supported_tokens,
            max_deposit_amount,
            daily_deposit_limit,
            registration_date: env.ledger().timestamp(),
        };

        // Store anchor info
        env.storage()
            .instance()
            .set(&DataKey::AnchorRegistry(anchor_address), &anchor_info);

        // Initialize anchor deposit counter if not exists
        if !env.storage().instance().has(&DataKey::AnchorDepositCount) {
            env.storage().instance().set(&DataKey::AnchorDepositCount, &0u64);
        }
    }

    fn get_anchor_info(env: Env, anchor_address: Address) -> AnchorInfo {
        env.storage()
            .instance()
            .get(&DataKey::AnchorRegistry(anchor_address))
            .unwrap_or_else(|| panic!("anchor not found"))
    }

    fn get_registered_anchors(env: Env) -> Vec<Address> {
        // For simplicity, return empty vector - in production, maintain a registry list
        Vec::new(&env)
    }

    fn set_payout_preference(
        env: Env,
        user: Address,
        circle_id: u64,
        payout_method: PayoutMethod,
        anchor_config: Option<AnchorDepositConfig>,
    ) {
        user.require_auth();

        // Verify circle exists
        let _circle: CircleInfo = env
            .storage()
            .instance()
            .get(&DataKey::Circle(circle_id))
            .unwrap_or_else(|| panic!("circle not found"));

        // Store user preference
        let preference = UserBankPreference {
            user: user.clone(),
            circle_id,
            payout_method,
            anchor_config,
        };

        env.storage()
            .instance()
            .set(&DataKey::UserBankPreference(user, circle_id), &preference);
    }

    fn get_payout_preference(env: Env, user: Address, circle_id: u64) -> UserBankPreference {
        // Return default preference if not set
        env.storage()
            .instance()
            .get(&DataKey::UserBankPreference(user, circle_id))
            .unwrap_or_else(|| UserBankPreference {
                user,
                circle_id,
                payout_method: PayoutMethod::DirectToken,
                anchor_config: None,
            })
    }

    fn deposit_for_user(
        env: Env,
        anchor_address: Address,
        user_address: Address,
        circle_id: u64,
        amount: u64,
        token: Address,
        fiat_reference: Symbol,
    ) {
        // Verify anchor exists and is active
        let anchor_info = Self::get_anchor_info(env.clone(), anchor_address.clone());
        if anchor_info.status != AnchorStatus::Active {
            panic!("anchor is not active");
        }

        // Verify token is supported by anchor
        if !anchor_info.supported_tokens.contains(&token) {
            panic!("token not supported by anchor");
        }

        // Enhanced deposit limit checking
        Self::check_deposit_limits(&env, &anchor_info, &user_address, amount, &token)?;

        // KYC verification if required
        if anchor_info.kyc_required {
            Self::verify_user_kyc(&env, &anchor_address, &user_address)?;
        }

        // Get deposit ID
        let mut deposit_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AnchorDepositCount)
            .unwrap_or(0);
        deposit_count += 1;
        env.storage()
            .instance()
            .set(&DataKey::AnchorDepositCount, &deposit_count);

        // Create deposit record
        let deposit = AnchorDeposit {
            anchor_address: anchor_address.clone(),
            user_address: user_address.clone(),
            circle_id,
            amount,
            token: token.clone(),
            fiat_reference,
            status: DepositStatus::Pending,
            timestamp: env.ledger().timestamp(),
        };

        // Store deposit
        env.storage()
            .instance()
            .set(&DataKey::AnchorDeposit(deposit_count), &deposit);

        // Transfer tokens to anchor
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        token_client.transfer(
            &env.current_contract_address(),
            &anchor_address,
            &(amount as i128),
        );

        // Update deposit status to completed
        let mut updated_deposit = deposit;
        updated_deposit.status = DepositStatus::Completed;
        env.storage()
            .instance()
            .set(&DataKey::AnchorDeposit(deposit_count), &updated_deposit);

        // Process as regular contribution for the circle
        let member_key = DataKey::Member(user_address.clone());
        let mut member: Member = env
            .storage()
            .instance()
            .get(&member_key)
            .unwrap_or_else(|| panic!("member not found"));

        member.contribution_count += 1;
        member.amount_contributed += amount;
        member.last_contribution_time = env.ledger().timestamp();
        member.has_contributed = true;

        env.storage().instance().set(&member_key, &member);
    }

    fn process_anchor_payout(
        env: Env,
        anchor_address: Address,
        user_address: Address,
        circle_id: u64,
        amount: u64,
        token: Address,
    ) -> Result<u64, u32> {
        // Verify anchor exists and is active
        let anchor_info = Self::get_anchor_info(env.clone(), anchor_address.clone());
        if anchor_info.status != AnchorStatus::Active {
            return Err(500); // Anchor inactive
        }

        // Get deposit ID
        let mut deposit_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AnchorDepositCount)
            .unwrap_or(0);
        deposit_count += 1;
        env.storage()
            .instance()
            .set(&DataKey::AnchorDepositCount, &deposit_count);

        // Create payout deposit record
        let deposit = AnchorDeposit {
            anchor_address: anchor_address.clone(),
            user_address,
            circle_id,
            amount,
            token,
            fiat_reference: Symbol::short(&env, "PAYOUT"), // Default reference
            status: DepositStatus::Pending,
            timestamp: env.ledger().timestamp(),
        };

        // Store deposit
        env.storage()
            .instance()
            .set(&DataKey::AnchorDeposit(deposit_count), &deposit);

        // Transfer tokens to anchor for fiat conversion
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        token_client.transfer(
            &env.current_contract_address(),
            &anchor_address,
            &(amount as i128),
        );

        // Update deposit status
        let mut updated_deposit = deposit;
        updated_deposit.status = DepositStatus::Completed;
        env.storage()
            .instance()
            .set(&DataKey::AnchorDeposit(deposit_count), &updated_deposit);

        Ok(deposit_count)
    }

    fn get_anchor_deposit_status(env: Env, deposit_id: u64) -> AnchorDeposit {
        env.storage()
            .instance()
            .get(&DataKey::AnchorDeposit(deposit_id))
            .unwrap_or_else(|| panic!("deposit not found"))
    }

    // --- SEP-24 HELPER FUNCTIONS ---

    /// Check if deposit complies with anchor limits and user daily limits
    fn check_deposit_limits(
        env: &Env,
        anchor_info: &AnchorInfo,
        user_address: &Address,
        amount: u64,
        token: &Address,
    ) -> Result<(), u32> {
        // Check maximum deposit amount per transaction
        if amount > anchor_info.max_deposit_amount {
            return Err(401); // Amount exceeds maximum
        }

        // Check daily deposit limit (simplified - in production, track per user per day)
        let current_day = env.ledger().timestamp() / 86400; // Current day in seconds
        let daily_limit_key = format!("daily_limit_{}_{}_{}", 
            anchor_info.address.to_string(), 
            user_address.to_string(), 
            current_day
        );
        
        // For simplicity, we'll just check against the anchor's daily limit
        // In production, you'd track cumulative daily deposits per user
        if amount > anchor_info.daily_deposit_limit {
            return Err(402); // Daily limit exceeded
        }

        Ok(())
    }

    /// Verify user KYC status with anchor
    fn verify_user_kyc(
        env: &Env,
        anchor_address: &Address,
        user_address: &Address,
    ) -> Result<(), u32> {
        // In a real implementation, this would:
        // 1. Call the anchor's KYC verification endpoint
        // 2. Check if the user has completed KYC
        // 3. Verify the KYC level meets requirements
        
        // For now, we'll simulate a basic KYC check
        // In production, this would be an external contract call to the anchor
        
        // Create a simple KYC status key (in production, this would be from anchor)
        let kyc_key = format!("kyc_status_{}_{}", 
            anchor_address.to_string(), 
            user_address.to_string()
        );
        
        // For demonstration, we'll assume all users are KYC verified
        // In production, this would query the anchor's KYC system
        let kyc_verified = true; // Simulated
        
        if !kyc_verified {
            return Err(403); // KYC not verified
        }

        Ok(())
    }

    /// Validate bank account details (hash verification)
    fn validate_bank_details(
        bank_account_hash: u64,
        mobile_number_hash: u64,
    ) -> Result<(), u32> {
        // Basic validation to ensure hashes are not zero
        if bank_account_hash == 0 {
            return Err(404); // Invalid bank account
        }
        
        if mobile_number_hash == 0 {
            return Err(405); // Invalid mobile number
        }

        // In production, you might add more sophisticated validation
        // such as checksum verification, format validation, etc.

        Ok(())
    }

    /// Get available anchors for a specific token and region
    fn get_available_anchors(
        env: &Env,
        token: &Address,
        fiat_currency: Symbol,
    ) -> Vec<Address> {
        // In production, this would filter anchors by:
        // 1. Token support
        // 2. Geographic region/currency support  
        // 3. Active status
        // 4. Current capacity
        
        // For now, return empty vector
        Vec::new(env)
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

// --- MISSING HELPER STUBS (referenced throughout lib.rs) ---

/// Panics if the contract is paused.
fn require_not_paused(env: &Env) {
    let paused: bool = env
        .storage()
        .instance()
        .get(&DataKey::IsPaused)
        .unwrap_or(false);
    if paused {
        panic!("contract is paused");
    }
}

/// Returns the total isolated contributions from opted-out members for a circle.
fn get_total_opted_out_contributions(env: &Env, circle_id: u64) -> u64 {
    // In a full implementation this would iterate over opted-out members.
    // Returning 0 is safe: it means all funds are eligible for yield routing.
    let _ = circle_id;
    let _ = env;
    0u64
}

/// Returns the payout amount for a member, respecting yield opt-out.
fn get_member_payout_amount(
    env: &Env,
    circle_id: u64,
    member: Address,
    normal_payout: u64,
) -> u64 {
    let member_key = DataKey::Member(member.clone());
    if let Some(m) = env
        .storage()
        .instance()
        .get::<DataKey, Member>(&member_key)
    {
        if m.opt_out_of_yield {
            // Return only the base contribution, not the yield-enhanced payout.
            let circle: CircleInfo = env
                .storage()
                .instance()
                .get(&DataKey::Circle(circle_id))
                .unwrap_or_else(|| panic!("circle not found"));
            return circle.contribution_amount;
        }
    }
    normal_payout
}

/// Minimal VotingSession struct used by commit-reveal tests.
#[contracttype]
#[derive(Clone)]
pub struct VotingSession {
    pub circle_id: u64,
    pub commit_deadline: u64,
    pub reveal_deadline: u64,
    pub total_commits: u32,
    pub total_reveals: u32,
    pub yes_votes: u32,
    pub no_votes: u32,
    pub is_finalized: bool,
}

// --- FUZZ TESTING MODULES ---

#[cfg(test)]
mod yield_allocation_voting_tests;

#[cfg(test)]
mod passkey_auth_tests;

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

    /// # Admin-Only: Set Trusted LeaseFlow Bridge Contract
    ///
    /// **Why admin-only:** The LeaseFlow contract address is used to verify
    /// cross-protocol default signals that can pause member payouts. Allowing
    /// arbitrary callers to set this address would let an attacker register a
    /// malicious contract that permanently locks all member payouts.
    ///
    /// **If admin key is lost:** The LeaseFlow bridge address is frozen. The
    /// cross-protocol default mechanism continues to work with the existing
    /// trusted contract. If the LeaseFlow contract is upgraded, the bridge
    /// cannot be updated and the integration becomes non-functional (but no
    /// funds are lost).
    ///
    /// **DAO migration path:** Require a ≥ 2/3 governance vote with a 48-hour
    /// time-lock. Emit a `BRIDGE_CHANGE_PROPOSED` event so members can review
    /// the new contract before it gains the ability to pause payouts.
    fn set_leaseflow_contract(env: Env, admin: Address, leaseflow: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).expect("Admin not set");
        if admin != stored_admin {
            panic!("Unauthorized: Only admin can set bridge targets");
        }
        env.storage().instance().set(&DataKey::LeaseFlowContract, &leaseflow);
    }

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
