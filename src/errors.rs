#![cfg_attr(not(test), no_std)]
use soroban_sdk::{contracttype, Env, Symbol, Vec};

/// Comprehensive error handling for SoroSusu smart contracts
/// Each error variant maps to a unique u32 code for frontend parsing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SoroSusuError {
    // General errors (1000-1999)
    Unauthorized = 1000,
    AdminNotSet = 1001,
    CircleNotFound = 1002,
    MemberNotFound = 1003,
    MemberNotActive = 1004,
    AlreadyMember = 1005,
    CircleFull = 1006,
    NoMembersInCircle = 1007,
    InvalidInput = 1008,
    Overflow = 1009,
    Underflow = 1010,
    DivisionByZero = 1011,
    InvalidAddress = 1012,
    ContractPaused = 1013,
    EmergencyMode = 1014,

    // Circle lifecycle errors (2000-2999)
    CycleNotMatured = 2000,
    RoundAlreadyFinalized = 2001,
    InvalidRound = 2002,
    CircleCompleted = 2003,
    CircleNotStarted = 2004,
    InvalidCycleDuration = 2005,
    MaxCycleDurationExceeded = 2006,
    LeadershipCrisis = 2007,
    NoRecipientSet = 2008,
    InvalidRecipient = 2009,

    // Contribution errors (3000-3999)
    InvalidContributionAmount = 3000,
    InsufficientBalance = 3001,
    ContributionOverflow = 3002,
    ZeroRounds = 3003,
    ContributionTransactionFailed = 3004,
    InvalidContributionWindow = 3005,
    MaxContributionExceeded = 3006,
    ContributionAlreadyMade = 3007,
    InvalidToken = 3008,
    TrustlineMissing = 3009,

    // Member management errors (4000-4999)
    MemberIndexOutOfBounds = 4000,
    MaxGroupSizeExceeded = 4001,
    InvalidMemberStatus = 4002,
    MemberAlreadyExists = 4003,
    InvalidShares = 4004,
    MemberSuspended = 4005,
    MemberDefaulted = 4006,
    InvalidMemberIndex = 4007,
    MemberNotEligible = 4008,

    // Financial errors (5000-5999)
    InvalidAmount = 5000,
    InsufficientFunds = 5001,
    AmountBelowMinimum = 5002,
    AmountAboveMaximum = 5003,
    InvalidFee = 5004,
    FeeExceedsLimit = 5005,
    InvalidInsuranceFee = 5006,
    InsuranceFeeExceedsLimit = 5007,
    InvalidInterestRate = 5008,
    InterestRateExceedsLimit = 5009,

    // Lending market errors (6000-6999)
    LendingMarketNotInitialized = 6000,
    LendingMarketDisabled = 6001,
    PoolNotFound = 6002,
    PoolNotActive = 6003,
    InsufficientPoolLiquidity = 6004,
    PositionNotFound = 6005,
    LoanNotActive = 6006,
    PaymentExceedsBalance = 6007,
    ScheduleNotFound = 6008,
    InvalidLTVRatio = 6009,
    MaxLTVExceeded = 6010,
    EmergencyLoanNotFound = 6011,
    VotingPeriodEnded = 6012,
    InsufficientVotes = 6013,

    // Governance errors (7000-7999)
    VotingSessionNotFound = 7000,
    VotingNotActive = 7001,
    AlreadyVoted = 7002,
    InvalidVote = 7003,
    QuorumNotMet = 7004,
    ProposalNotFound = 7005,
    ProposalExpired = 7006,
    InvalidProposal = 7007,
    UnauthorizedGovernanceAction = 7008,

    // Recovery errors (8000-8999)
    NoActiveMembers = 8000,
    RecoveryProposalNotFound = 8001,
    OldMemberNotFound = 8002,
    NewMemberAlreadyExists = 8003,
    InvalidRecoveryAddress = 8004,
    RecoveryConsensusNotMet = 8005,
    RecoveryWindowExpired = 8006,

    // Yield and reward errors (9000-9999)
    YieldBalanceNotFound = 9000,
    HarvestNotAvailable = 9001,
    InvalidYieldAllocation = 9002,
    YieldCalculationError = 9003,
    BatchHarvestInProgress = 9004,
    InsufficientYield = 9005,
    YieldDistributionFailed = 9006,

    // Dispute resolution errors (10000-10999)
    DisputeNotFound = 10000,
    DisputeNotActive = 10001,
    InvalidDisputeStatus = 10002,
    JurorNotSelected = 10003,
    InvalidEvidence = 10004,
    DisputeExpired = 10005,
    InsufficientDisputeBond = 10006,

    // Security errors (11000-11999)
    ReentrancyDetected = 11000,
    InvalidSignature = 11001,
    AuthenticationFailed = 11002,
    PasskeyInvalid = 11003,
    MerkleProofInvalid = 11004,
    ContributionSecurityViolation = 11005,

    // Oracle and price errors (12000-12999)
    OracleUnavailable = 12000,
    PriceStale = 12001,
    InvalidPrice = 12002,
    PriceFeedError = 12003,
    CircuitBreakerTriggered = 12004,

    // Tax and compliance errors (13000-13999)
    TaxConfigurationNotFound = 13000,
    InvalidTaxRate = 13001,
    TaxExemptionInvalid = 13002,
    TaxWithholdingFailed = 13003,
    ComplianceCheckFailed = 13004,

    // Audit and logging errors (14000-14999)
    AuditEntryNotFound = 14000,
    LoggingFailed = 14001,
    InvalidAuditTrail = 14002,

    // Anchor integration errors (15000-15999)
    AnchorNotRegistered = 15000,
    AnchorDepositNotFound = 15001,
    InvalidAnchorConfiguration = 15002,
    AnchorTransferFailed = 15003,

    // Internal errors (17000-17999)
    InternalError = 17000,
    StorageError = 17001,
    SerializationError = 17002,
    DeserializationError = 17003,
    InvalidStorageKey = 17004,
    ContractInvariantViolation = 17005,
}

impl SoroSusuError {
    /// Get the error code as u32
    pub fn code(&self) -> u32 {
        *self as u32
    }

    /// Get human-readable message for the error
    pub fn message(&self) -> &str {
        match self {
            // General errors
            Self::Unauthorized => "Unauthorized access",
            Self::AdminNotSet => "Admin address not set",
            Self::CircleNotFound => "Circle not found",
            Self::MemberNotFound => "Member not found",
            Self::MemberNotActive => "Member is not active",
            Self::AlreadyMember => "Already a member of this circle",
            Self::CircleFull => "Circle is full",
            Self::NoMembersInCircle => "No members in circle",
            Self::InvalidInput => "Invalid input provided",
            Self::Overflow => "Arithmetic overflow",
            Self::Underflow => "Arithmetic underflow",
            Self::DivisionByZero => "Division by zero",
            Self::InvalidAddress => "Invalid address format",
            Self::ContractPaused => "Contract is paused",
            Self::EmergencyMode => "Contract in emergency mode",

            // Circle lifecycle errors
            Self::CycleNotMatured => "Cycle has not matured yet",
            Self::RoundAlreadyFinalized => "Round already finalized",
            Self::InvalidRound => "Invalid round number",
            Self::CircleCompleted => "Circle has been completed",
            Self::CircleNotStarted => "Circle has not started",
            Self::InvalidCycleDuration => "Invalid cycle duration",
            Self::MaxCycleDurationExceeded => "Cycle duration exceeds maximum limit",
            Self::LeadershipCrisis => "Leadership crisis detected",
            Self::NoRecipientSet => "No recipient set for current round",
            Self::InvalidRecipient => "Invalid recipient address",

            // Contribution errors
            Self::InvalidContributionAmount => "Invalid contribution amount",
            Self::InsufficientBalance => "Insufficient token balance",
            Self::ContributionOverflow => "Contribution amount overflow",
            Self::ZeroRounds => "Number of rounds must be greater than zero",
            Self::ContributionTransactionFailed => "Contribution transaction failed",
            Self::InvalidContributionWindow => "Invalid contribution window",
            Self::MaxContributionExceeded => "Maximum contribution exceeded",
            Self::ContributionAlreadyMade => "Contribution already made for this round",
            Self::InvalidToken => "Invalid token address",
            Self::TrustlineMissing => "Required trustline missing",

            // Member management errors
            Self::MemberIndexOutOfBounds => "Member index out of bounds",
            Self::MaxGroupSizeExceeded => "Maximum group size exceeded",
            Self::InvalidMemberStatus => "Invalid member status",
            Self::MemberAlreadyExists => "Member already exists",
            Self::InvalidShares => "Shares must be 1 or 2",
            Self::MemberSuspended => "Member is suspended",
            Self::MemberDefaulted => "Member has defaulted",
            Self::InvalidMemberIndex => "Invalid member index",
            Self::MemberNotEligible => "Member not eligible for this operation",

            // Financial errors
            Self::InvalidAmount => "Invalid amount specified",
            Self::InsufficientFunds => "Insufficient funds available",
            Self::AmountBelowMinimum => "Amount below minimum threshold",
            Self::AmountAboveMaximum => "Amount above maximum threshold",
            Self::InvalidFee => "Invalid fee specified",
            Self::FeeExceedsLimit => "Fee exceeds allowed limit",
            Self::InvalidInsuranceFee => "Invalid insurance fee",
            Self::InsuranceFeeExceedsLimit => "Insurance fee exceeds 100%",
            Self::InvalidInterestRate => "Invalid interest rate",
            Self::InterestRateExceedsLimit => "Interest rate exceeds limit",

            // Lending market errors
            Self::LendingMarketNotInitialized => "Lending market not initialized",
            Self::LendingMarketDisabled => "Lending market is disabled",
            Self::PoolNotFound => "Lending pool not found",
            Self::PoolNotActive => "Lending pool is not active",
            Self::InsufficientPoolLiquidity => "Insufficient pool liquidity",
            Self::PositionNotFound => "Lending position not found",
            Self::LoanNotActive => "Loan is not active",
            Self::PaymentExceedsBalance => "Payment exceeds remaining balance",
            Self::ScheduleNotFound => "Repayment schedule not found",
            Self::InvalidLTVRatio => "Invalid loan-to-value ratio",
            Self::MaxLTVExceeded => "Amount exceeds maximum LTV ratio",
            Self::EmergencyLoanNotFound => "Emergency loan not found",
            Self::VotingPeriodEnded => "Voting period has ended",
            Self::InsufficientVotes => "Insufficient votes for approval",

            // Governance errors
            Self::VotingSessionNotFound => "Voting session not found",
            Self::VotingNotActive => "Voting is not currently active",
            Self::AlreadyVoted => "Already voted on this proposal",
            Self::InvalidVote => "Invalid vote option",
            Self::QuorumNotMet => "Quorum not met",
            Self::ProposalNotFound => "Proposal not found",
            Self::ProposalExpired => "Proposal has expired",
            Self::InvalidProposal => "Invalid proposal data",
            Self::UnauthorizedGovernanceAction => "Unauthorized governance action",

            // Recovery errors
            Self::NoActiveMembers => "No active members for recovery",
            Self::RecoveryProposalNotFound => "No recovery proposal found",
            Self::OldMemberNotFound => "Old member not found for recovery",
            Self::NewMemberAlreadyExists => "New member address already exists",
            Self::InvalidRecoveryAddress => "Invalid recovery address",
            Self::RecoveryConsensusNotMet => "Recovery consensus not met",
            Self::RecoveryWindowExpired => "Recovery window has expired",

            // Yield and reward errors
            Self::YieldBalanceNotFound => "Yield balance not found",
            Self::HarvestNotAvailable => "Harvest not available at this time",
            Self::InvalidYieldAllocation => "Invalid yield allocation",
            Self::YieldCalculationError => "Yield calculation error",
            Self::BatchHarvestInProgress => "Batch harvest already in progress",
            Self::InsufficientYield => "Insufficient yield to distribute",
            Self::YieldDistributionFailed => "Yield distribution failed",

            // Dispute resolution errors
            Self::DisputeNotFound => "Dispute not found",
            Self::DisputeNotActive => "Dispute is not active",
            Self::InvalidDisputeStatus => "Invalid dispute status",
            Self::JurorNotSelected => "Juror not selected",
            Self::InvalidEvidence => "Invalid evidence provided",
            Self::DisputeExpired => "Dispute has expired",
            Self::InsufficientDisputeBond => "Insufficient dispute bond",

            // Security errors
            Self::ReentrancyDetected => "Reentrancy attack detected",
            Self::InvalidSignature => "Invalid signature provided",
            Self::AuthenticationFailed => "Authentication failed",
            Self::PasskeyInvalid => "Invalid passkey authentication",
            Self::MerkleProofInvalid => "Invalid Merkle proof",
            Self::ContributionSecurityViolation => "Contribution security violation",

            // Oracle and price errors
            Self::OracleUnavailable => "Price oracle unavailable",
            Self::PriceStale => "Price data is stale",
            Self::InvalidPrice => "Invalid price data",
            Self::PriceFeedError => "Price feed error",
            Self::CircuitBreakerTriggered => "Circuit breaker triggered",

            // Tax and compliance errors
            Self::TaxConfigurationNotFound => "Tax configuration not found",
            Self::InvalidTaxRate => "Invalid tax rate",
            Self::TaxExemptionInvalid => "Invalid tax exemption",
            Self::TaxWithholdingFailed => "Tax withholding failed",
            Self::ComplianceCheckFailed => "Compliance check failed",

            // Audit and logging errors
            Self::AuditEntryNotFound => "Audit entry not found",
            Self::LoggingFailed => "Logging operation failed",
            Self::InvalidAuditTrail => "Invalid audit trail",

            // Anchor integration errors
            Self::AnchorNotRegistered => "Anchor not registered",
            Self::AnchorDepositNotFound => "Anchor deposit not found",
            Self::InvalidAnchorConfiguration => "Invalid anchor configuration",
            Self::AnchorTransferFailed => "Anchor transfer failed",

            // Internal errors
            Self::InternalError => "Internal contract error",
            Self::StorageError => "Storage operation error",
            Self::SerializationError => "Data serialization error",
            Self::DeserializationError => "Data deserialization error",
            Self::InvalidStorageKey => "Invalid storage key",
            Self::ContractInvariantViolation => "Contract invariant violation",
        }
    }

    /// Get error category for frontend grouping
    pub fn category(&self) -> &str {
        match self {
            Self::Unauthorized | Self::AdminNotSet | Self::InvalidAddress | Self::AuthenticationFailed | Self::PasskeyInvalid => "authentication",
            Self::CircleNotFound | Self::MemberNotFound | Self::PoolNotFound | Self::PositionNotFound | Self::DisputeNotFound | Self::ProposalNotFound | Self::VotingSessionNotFound => "not_found",
            Self::InsufficientBalance | Self::InsufficientFunds | Self::InsufficientPoolLiquidity => "insufficient_funds",
            Self::ContractPaused | Self::EmergencyMode | Self::LendingMarketDisabled => "paused",
            Self::Overflow | Self::Underflow | Self::DivisionByZero | Self::ContributionOverflow => "arithmetic",
            _ => "general",
        }
    }
}

/// Result type alias for SoroSusu operations
pub type SoroSusuResult<T> = Result<T, SoroSusuError>;

/// Helper trait for converting common Soroban errors to SoroSusuError
pub trait IntoSoroSusuError<T> {
    fn into_sorosusu_error(self) -> SoroSusuResult<T>;
}

impl<T> IntoSoroSusuError<T> for Result<T, soroban_sdk::Error> {
    fn into_sorosusu_error(self) -> SoroSusuResult<T> {
        self.map_err(|_| SoroSusuError::InternalError)
    }
}

/// Macro for consistent error conversion from Option
#[macro_export]
macro_rules! require_some {
    ($option:expr, $error:expr) => {
        $option.ok_or_else(|| $error)?
    };
}

/// Macro for consistent validation with custom error
#[macro_export]
macro_rules! require {
    ($condition:expr, $error:expr) => {
        if !$condition {
            return Err($error);
        }
    };
}
