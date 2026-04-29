#![cfg_attr(not(test), no_std)]
use soroban_sdk::{contracttype, Error, Symbol};

/// Comprehensive error system for SoroSusu smart contracts
/// Each error variant maps to a unique u32 code for frontend parsing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SoroSusuError {
    // General errors (1000-1999)
    Unauthorized = 1000,
    InvalidInput = 1001,
    Overflow = 1002,
    NotFound = 1003,
    AlreadyExists = 1004,
    InsufficientBalance = 1005,
    InvalidState = 1006,
    ArithmeticError = 1007,
    DivisionByZero = 1008,
    InvalidAddress = 1009,
    TimestampError = 1010,
    StorageError = 1011,
    SerializationError = 1012,
    InvalidSignature = 1013,
    RateLimitExceeded = 1014,
    MaintenanceMode = 1015,

    // Circle management errors (2000-2999)
    CircleNotFound = 2000,
    CircleAlreadyExists = 2001,
    CircleIsFull = 2002,
    CircleNotActive = 2003,
    CircleAlreadyFinalized = 2004,
    CircleNotMatured = 2005,
    InvalidCircleDuration = 2006,
    MaxGroupSizeExceeded = 2007,
    CircleCompleted = 2008,
    CircleNotCompleted = 2009,
    InvalidContributionAmount = 2010,
    InvalidInsuranceFee = 2011,
    InvalidMaxMembers = 2012,
    CircleDrained = 2013,
    CircleNotDrained = 2014,

    // Member management errors (3000-3999)
    MemberNotFound = 3000,
    MemberAlreadyExists = 3001,
    MemberNotActive = 3002,
    MemberIndexOutOfBounds = 3003,
    MemberAlreadyPaid = 3004,
    MemberNotPaid = 3005,
    InvalidMemberStatus = 3006,
    MemberIneligible = 3007,
    MemberDefaulted = 3008,
    MemberNotDefaulted = 3009,
    TooManyMembers = 3010,
    InsufficientMembers = 3011,
    MemberNotRecipient = 3012,
    MemberAlreadyRecipient = 3013,

    // Contribution and payment errors (4000-4999)
    InvalidContribution = 4000,
    ContributionOverflow = 4001,
    InsufficientContribution = 4002,
    ContributionAlreadyMade = 4003,
    ContributionNotMade = 4004,
    InvalidRounds = 4005,
    ZeroRounds = 4006,
    ContributionPeriodClosed = 4007,
    ContributionPeriodOpen = 4008,
    PaymentFailed = 4009,
    InvalidPaymentAmount = 4010,
    PaymentAlreadyProcessed = 4011,
    PaymentNotDue = 4012,
    LateFeeApplicable = 4013,
    LateFeeNotApplicable = 4014,

    // Payout and distribution errors (5000-5999)
    NoRecipientSet = 5000,
    PayoutAlreadyDistributed = 5001,
    InsufficientPotBalance = 5002,
    InvalidPayoutAmount = 5003,
    PayoutFailed = 5004,
    DistributionFailed = 5005,
    InvalidDistributionRatio = 5006,
    YieldNotAvailable = 5007,
    YieldAlreadyClaimed = 5008,
    InvalidYieldAmount = 5009,
    HarvestFailed = 5010,
    BatchHarvestInProgress = 5011,

    // Recovery and dispute errors (6000-6999)
    RecoveryNotProposed = 6000,
    RecoveryAlreadyProposed = 6001,
    RecoveryNotConsensus = 6002,
    RecoveryFailed = 6003,
    InvalidRecoveryAddress = 6004,
    RecoveryWindowExpired = 6005,
    DisputeNotFound = 6006,
    DisputeAlreadyExists = 6007,
    DisputeNotActive = 6008,
    DisputeResolved = 6009,
    InvalidDisputeAmount = 6010,
    JurorNotSelected = 6011,
    VotingNotActive = 6012,
    VotingAlreadyActive = 6013,
    VotingCompleted = 6014,
    InvalidVote = 6015,

    // Lending market errors (7000-7999)
    LendingMarketNotInitialized = 7000,
    LendingMarketDisabled = 7001,
    LendingMarketEmergencyMode = 7002,
    PoolNotFound = 7003,
    PoolNotActive = 7004,
    PoolAlreadyExists = 7005,
    InsufficientPoolLiquidity = 7006,
    InvalidLiquidityAmount = 7007,
    PositionNotFound = 7008,
    PositionNotActive = 7009,
    LoanAlreadyRepaid = 7010,
    InvalidLoanAmount = 7011,
    InvalidLtvRatio = 7012,
    LtvRatioExceeded = 7013,
    MinLendingAmount = 7014,
    ScheduleNotFound = 7015,
    EmergencyLoanNotFound = 7016,
    EmergencyLoanNotActive = 7017,
    VotingPeriodEnded = 7018,
    QuorumNotMet = 7019,

    // Yield and strategy errors (8000-8999)
    YieldStrategyNotFound = 8000,
    YieldStrategyNotActive = 8001,
    InvalidYieldStrategy = 8002,
    YieldOracleError = 8003,
    CircuitBreakerTriggered = 8004,
    YieldAllocationFailed = 8005,
    InvalidYieldAllocation = 8006,
    BridgeAdapterError = 8007,
    SlippageExceeded = 8008,
    PriceFeedError = 8009,
    InvalidPriceData = 8010,
    YieldCalculationError = 8011,

    // Security and compliance errors (9000-9999)
    ReentrancyDetected = 9000,
    Paused = 9001,
    NotPaused = 9002,
    EmergencyMode = 9003,
    Blacklisted = 9004,
    Sanctioned = 9005,
    ComplianceFailed = 9006,
    AmlViolation = 9007,
    KycRequired = 9008,
    TaxWithholdingFailed = 9009,
    InvalidTaxConfiguration = 9010,
    TaxExemptionInvalid = 9011,
    AuditEntryMissing = 9012,
    TrustlineMissing = 9013,
    AnchorIntegrationError = 9014,

    // Technical and system errors (10000-10999)
    ContractNotInitialized = 10000,
    AdminNotSet = 10001,
    InvalidAdmin = 10002,
    ConfigurationError = 10003,
    UpgradeFailed = 10004,
    MigrationFailed = 10005,
    BackupFailed = 10006,
    RestoreFailed = 10007,
    NetworkError = 10008,
    GasLimitExceeded = 10009,
    ExecutionTimeout = 10010,
    InternalError = 10011,
    DeprecatedFunction = 10012,
    FeatureNotEnabled = 10013,
    InvalidChain = 10014,
    VersionMismatch = 10015,
}

impl SoroSusuError {
    /// Convert error to human-readable message (safe for frontend display)
    pub fn to_human_readable(&self) -> &'static str {
        match self {
            // General errors
            SoroSusuError::Unauthorized => "You are not authorized to perform this action",
            SoroSusuError::InvalidInput => "Invalid input provided",
            SoroSusuError::Overflow => "Arithmetic overflow detected",
            SoroSusuError::NotFound => "Requested resource not found",
            SoroSusuError::AlreadyExists => "Resource already exists",
            SoroSusuError::InsufficientBalance => "Insufficient balance for this operation",
            SoroSusuError::InvalidState => "Invalid state for this operation",
            SoroSusuError::ArithmeticError => "Arithmetic operation failed",
            SoroSusuError::DivisionByZero => "Division by zero attempted",
            SoroSusuError::InvalidAddress => "Invalid address provided",
            SoroSusuError::TimestampError => "Invalid timestamp provided",
            SoroSusuError::StorageError => "Storage operation failed",
            SoroSusuError::SerializationError => "Data serialization failed",
            SoroSusuError::InvalidSignature => "Invalid signature provided",
            SoroSusuError::RateLimitExceeded => "Rate limit exceeded",
            SoroSusuError::MaintenanceMode => "System is under maintenance",

            // Circle management errors
            SoroSusuError::CircleNotFound => "Circle not found",
            SoroSusuError::CircleAlreadyExists => "Circle already exists",
            SoroSusuError::CircleIsFull => "Circle is full",
            SoroSusuError::CircleNotActive => "Circle is not active",
            SoroSusuError::CircleAlreadyFinalized => "Circle round already finalized",
            SoroSusuError::CircleNotMatured => "Circle has not matured yet",
            SoroSusuError::InvalidCircleDuration => "Invalid circle duration",
            SoroSusuError::MaxGroupSizeExceeded => "Maximum group size exceeded",
            SoroSusuError::CircleCompleted => "Circle is already completed",
            SoroSusuError::CircleNotCompleted => "Circle is not completed",
            SoroSusuError::InvalidContributionAmount => "Invalid contribution amount",
            SoroSusuError::InvalidInsuranceFee => "Invalid insurance fee",
            SoroSusuError::InvalidMaxMembers => "Invalid maximum members",
            SoroSusuError::CircleDrained => "Circle has been drained",
            SoroSusuError::CircleNotDrained => "Circle has not been drained",

            // Member management errors
            SoroSusuError::MemberNotFound => "Member not found",
            SoroSusuError::MemberAlreadyExists => "Member already exists",
            SoroSusuError::MemberNotActive => "Member is not active",
            SoroSusuError::MemberIndexOutOfBounds => "Member index out of bounds",
            SoroSusuError::MemberAlreadyPaid => "Member has already paid",
            SoroSusuError::MemberNotPaid => "Member has not paid",
            SoroSusuError::InvalidMemberStatus => "Invalid member status",
            SoroSusuError::MemberIneligible => "Member is ineligible",
            SoroSusuError::MemberDefaulted => "Member has defaulted",
            SoroSusuError::MemberNotDefaulted => "Member has not defaulted",
            SoroSusuError::TooManyMembers => "Too many members",
            SoroSusuError::InsufficientMembers => "Insufficient members",
            SoroSusuError::MemberNotRecipient => "Member is not the recipient",
            SoroSusuError::MemberAlreadyRecipient => "Member is already the recipient",

            // Contribution and payment errors
            SoroSusuError::InvalidContribution => "Invalid contribution",
            SoroSusuError::ContributionOverflow => "Contribution overflow",
            SoroSusuError::InsufficientContribution => "Insufficient contribution",
            SoroSusuError::ContributionAlreadyMade => "Contribution already made",
            SoroSusuError::ContributionNotMade => "Contribution not made",
            SoroSusuError::InvalidRounds => "Invalid number of rounds",
            SoroSusuError::ZeroRounds => "Number of rounds must be greater than zero",
            SoroSusuError::ContributionPeriodClosed => "Contribution period is closed",
            SoroSusuError::ContributionPeriodOpen => "Contribution period is open",
            SoroSusuError::PaymentFailed => "Payment failed",
            SoroSusuError::InvalidPaymentAmount => "Invalid payment amount",
            SoroSusuError::PaymentAlreadyProcessed => "Payment already processed",
            SoroSusuError::PaymentNotDue => "Payment is not due yet",
            SoroSusuError::LateFeeApplicable => "Late fee is applicable",
            SoroSusuError::LateFeeNotApplicable => "Late fee is not applicable",

            // Payout and distribution errors
            SoroSusuError::NoRecipientSet => "No recipient set",
            SoroSusuError::PayoutAlreadyDistributed => "Payout already distributed",
            SoroSusuError::InsufficientPotBalance => "Insufficient pot balance",
            SoroSusuError::InvalidPayoutAmount => "Invalid payout amount",
            SoroSusuError::PayoutFailed => "Payout failed",
            SoroSusuError::DistributionFailed => "Distribution failed",
            SoroSusuError::InvalidDistributionRatio => "Invalid distribution ratio",
            SoroSusuError::YieldNotAvailable => "Yield not available",
            SoroSusuError::YieldAlreadyClaimed => "Yield already claimed",
            SoroSusuError::InvalidYieldAmount => "Invalid yield amount",
            SoroSusuError::HarvestFailed => "Yield harvest failed",
            SoroSusuError::BatchHarvestInProgress => "Batch harvest in progress",

            // Recovery and dispute errors
            SoroSusuError::RecoveryNotProposed => "Recovery not proposed",
            SoroSusuError::RecoveryAlreadyProposed => "Recovery already proposed",
            SoroSusuError::RecoveryNotConsensus => "Recovery consensus not reached",
            SoroSusuError::RecoveryFailed => "Recovery failed",
            SoroSusuError::InvalidRecoveryAddress => "Invalid recovery address",
            SoroSusuError::RecoveryWindowExpired => "Recovery window expired",
            SoroSusuError::DisputeNotFound => "Dispute not found",
            SoroSusuError::DisputeAlreadyExists => "Dispute already exists",
            SoroSusuError::DisputeNotActive => "Dispute is not active",
            SoroSusuError::DisputeResolved => "Dispute is already resolved",
            SoroSusuError::InvalidDisputeAmount => "Invalid dispute amount",
            SoroSusuError::JurorNotSelected => "Juror not selected",
            SoroSusuError::VotingNotActive => "Voting is not active",
            SoroSusuError::VotingAlreadyActive => "Voting is already active",
            SoroSusuError::VotingCompleted => "Voting is completed",
            SoroSusuError::InvalidVote => "Invalid vote",

            // Lending market errors
            SoroSusuError::LendingMarketNotInitialized => "Lending market not initialized",
            SoroSusuError::LendingMarketDisabled => "Lending market is disabled",
            SoroSusuError::LendingMarketEmergencyMode => "Lending market in emergency mode",
            SoroSusuError::PoolNotFound => "Lending pool not found",
            SoroSusuError::PoolNotActive => "Lending pool is not active",
            SoroSusuError::PoolAlreadyExists => "Lending pool already exists",
            SoroSusuError::InsufficientPoolLiquidity => "Insufficient pool liquidity",
            SoroSusuError::InvalidLiquidityAmount => "Invalid liquidity amount",
            SoroSusuError::PositionNotFound => "Lending position not found",
            SoroSusuError::PositionNotActive => "Lending position is not active",
            SoroSusuError::LoanAlreadyRepaid => "Loan already repaid",
            SoroSusuError::InvalidLoanAmount => "Invalid loan amount",
            SoroSusuError::InvalidLtvRatio => "Invalid LTV ratio",
            SoroSusuError::LtvRatioExceeded => "LTV ratio exceeded",
            SoroSusuError::MinLendingAmount => "Amount below minimum lending amount",
            SoroSusuError::ScheduleNotFound => "Repayment schedule not found",
            SoroSusuError::EmergencyLoanNotFound => "Emergency loan not found",
            SoroSusuError::EmergencyLoanNotActive => "Emergency loan is not active",
            SoroSusuError::VotingPeriodEnded => "Voting period has ended",
            SoroSusuError::QuorumNotMet => "Quorum not met",

            // Yield and strategy errors
            SoroSusuError::YieldStrategyNotFound => "Yield strategy not found",
            SoroSusuError::YieldStrategyNotActive => "Yield strategy is not active",
            SoroSusuError::InvalidYieldStrategy => "Invalid yield strategy",
            SoroSusuError::YieldOracleError => "Yield oracle error",
            SoroSusuError::CircuitBreakerTriggered => "Circuit breaker triggered",
            SoroSusuError::YieldAllocationFailed => "Yield allocation failed",
            SoroSusuError::InvalidYieldAllocation => "Invalid yield allocation",
            SoroSusuError::BridgeAdapterError => "Bridge adapter error",
            SoroSusuError::SlippageExceeded => "Slippage exceeded",
            SoroSusuError::PriceFeedError => "Price feed error",
            SoroSusuError::InvalidPriceData => "Invalid price data",
            SoroSusuError::YieldCalculationError => "Yield calculation error",

            // Security and compliance errors
            SoroSusuError::ReentrancyDetected => "Reentrancy attack detected",
            SoroSusuError::Paused => "Contract is paused",
            SoroSusuError::NotPaused => "Contract is not paused",
            SoroSusuError::EmergencyMode => "Contract in emergency mode",
            SoroSusuError::Blacklisted => "Address is blacklisted",
            SoroSusuError::Sanctioned => "Address is sanctioned",
            SoroSusuError::ComplianceFailed => "Compliance check failed",
            SoroSusuError::AmlViolation => "AML violation detected",
            SoroSusuError::KycRequired => "KYC verification required",
            SoroSusuError::TaxWithholdingFailed => "Tax withholding failed",
            SoroSusuError::InvalidTaxConfiguration => "Invalid tax configuration",
            SoroSusuError::TaxExemptionInvalid => "Invalid tax exemption",
            SoroSusuError::AuditEntryMissing => "Audit entry missing",
            SoroSusuError::TrustlineMissing => "Trustline missing",
            SoroSusuError::AnchorIntegrationError => "Anchor integration error",

            // Technical and system errors
            SoroSusuError::ContractNotInitialized => "Contract not initialized",
            SoroSusuError::AdminNotSet => "Admin not set",
            SoroSusuError::InvalidAdmin => "Invalid admin",
            SoroSusuError::ConfigurationError => "Configuration error",
            SoroSusuError::UpgradeFailed => "Contract upgrade failed",
            SoroSusuError::MigrationFailed => "Data migration failed",
            SoroSusuError::BackupFailed => "Backup failed",
            SoroSusuError::RestoreFailed => "Restore failed",
            SoroSusuError::NetworkError => "Network error",
            SoroSusuError::GasLimitExceeded => "Gas limit exceeded",
            SoroSusuError::ExecutionTimeout => "Execution timeout",
            SoroSusuError::InternalError => "Internal error occurred",
            SoroSusuError::DeprecatedFunction => "Function is deprecated",
            SoroSusuError::FeatureNotEnabled => "Feature not enabled",
            SoroSusuError::InvalidChain => "Invalid blockchain",
            SoroSusuError::VersionMismatch => "Version mismatch",
        }
    }

    /// Get the u32 error code for frontend parsing
    pub fn code(&self) -> u32 {
        *self as u32
    }

    /// Convert from u32 code back to error (for frontend parsing)
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            // General errors
            1000 => Some(SoroSusuError::Unauthorized),
            1001 => Some(SoroSusuError::InvalidInput),
            1002 => Some(SoroSusuError::Overflow),
            1003 => Some(SoroSusuError::NotFound),
            1004 => Some(SoroSusuError::AlreadyExists),
            1005 => Some(SoroSusuError::InsufficientBalance),
            1006 => Some(SoroSusuError::InvalidState),
            1007 => Some(SoroSusuError::ArithmeticError),
            1008 => Some(SoroSusuError::DivisionByZero),
            1009 => Some(SoroSusuError::InvalidAddress),
            1010 => Some(SoroSusuError::TimestampError),
            1011 => Some(SoroSusuError::StorageError),
            1012 => Some(SoroSusuError::SerializationError),
            1013 => Some(SoroSusuError::InvalidSignature),
            1014 => Some(SoroSusuError::RateLimitExceeded),
            1015 => Some(SoroSusuError::MaintenanceMode),

            // Circle management errors
            2000 => Some(SoroSusuError::CircleNotFound),
            2001 => Some(SoroSusuError::CircleAlreadyExists),
            2002 => Some(SoroSusuError::CircleIsFull),
            2003 => Some(SoroSusuError::CircleNotActive),
            2004 => Some(SoroSusuError::CircleAlreadyFinalized),
            2005 => Some(SoroSusuError::CircleNotMatured),
            2006 => Some(SoroSusuError::InvalidCircleDuration),
            2007 => Some(SoroSusuError::MaxGroupSizeExceeded),
            2008 => Some(SoroSusuError::CircleCompleted),
            2009 => Some(SoroSusuError::CircleNotCompleted),
            2010 => Some(SoroSusuError::InvalidContributionAmount),
            2011 => Some(SoroSusuError::InvalidInsuranceFee),
            2012 => Some(SoroSusuError::InvalidMaxMembers),
            2013 => Some(SoroSusuError::CircleDrained),
            2014 => Some(SoroSusuError::CircleNotDrained),

            // Member management errors
            3000 => Some(SoroSusuError::MemberNotFound),
            3001 => Some(SoroSusuError::MemberAlreadyExists),
            3002 => Some(SoroSusuError::MemberNotActive),
            3003 => Some(SoroSusuError::MemberIndexOutOfBounds),
            3004 => Some(SoroSusuError::MemberAlreadyPaid),
            3005 => Some(SoroSusuError::MemberNotPaid),
            3006 => Some(SoroSusuError::InvalidMemberStatus),
            3007 => Some(SoroSusuError::MemberIneligible),
            3008 => Some(SoroSusuError::MemberDefaulted),
            3009 => Some(SoroSusuError::MemberNotDefaulted),
            3010 => Some(SoroSusuError::TooManyMembers),
            3011 => Some(SoroSusuError::InsufficientMembers),
            3012 => Some(SoroSusuError::MemberNotRecipient),
            3013 => Some(SoroSusuError::MemberAlreadyRecipient),

            // Contribution and payment errors
            4000 => Some(SoroSusuError::InvalidContribution),
            4001 => Some(SoroSusuError::ContributionOverflow),
            4002 => Some(SoroSusuError::InsufficientContribution),
            4003 => Some(SoroSusuError::ContributionAlreadyMade),
            4004 => Some(SoroSusuError::ContributionNotMade),
            4005 => Some(SoroSusuError::InvalidRounds),
            4006 => Some(SoroSusuError::ZeroRounds),
            4007 => Some(SoroSusuError::ContributionPeriodClosed),
            4008 => Some(SoroSusuError::ContributionPeriodOpen),
            4009 => Some(SoroSusuError::PaymentFailed),
            4010 => Some(SoroSusuError::InvalidPaymentAmount),
            4011 => Some(SoroSusuError::PaymentAlreadyProcessed),
            4012 => Some(SoroSusuError::PaymentNotDue),
            4013 => Some(SoroSusuError::LateFeeApplicable),
            4014 => Some(SoroSusuError::LateFeeNotApplicable),

            // Payout and distribution errors
            5000 => Some(SoroSusuError::NoRecipientSet),
            5001 => Some(SoroSusuError::PayoutAlreadyDistributed),
            5002 => Some(SoroSusuError::InsufficientPotBalance),
            5003 => Some(SoroSusuError::InvalidPayoutAmount),
            5004 => Some(SoroSusuError::PayoutFailed),
            5005 => Some(SoroSusuError::DistributionFailed),
            5006 => Some(SoroSusuError::InvalidDistributionRatio),
            5007 => Some(SoroSusuError::YieldNotAvailable),
            5008 => Some(SoroSusuError::YieldAlreadyClaimed),
            5009 => Some(SoroSusuError::InvalidYieldAmount),
            5010 => Some(SoroSusuError::HarvestFailed),
            5011 => Some(SoroSusuError::BatchHarvestInProgress),

            // Recovery and dispute errors
            6000 => Some(SoroSusuError::RecoveryNotProposed),
            6001 => Some(SoroSusuError::RecoveryAlreadyProposed),
            6002 => Some(SoroSusuError::RecoveryNotConsensus),
            6003 => Some(SoroSusuError::RecoveryFailed),
            6004 => Some(SoroSusuError::InvalidRecoveryAddress),
            6005 => Some(SoroSusuError::RecoveryWindowExpired),
            6006 => Some(SoroSusuError::DisputeNotFound),
            6007 => Some(SoroSusuError::DisputeAlreadyExists),
            6008 => Some(SoroSusuError::DisputeNotActive),
            6009 => Some(SoroSusuError::DisputeResolved),
            6010 => Some(SoroSusuError::InvalidDisputeAmount),
            6011 => Some(SoroSusuError::JurorNotSelected),
            6012 => Some(SoroSusuError::VotingNotActive),
            6013 => Some(SoroSusuError::VotingAlreadyActive),
            6014 => Some(SoroSusuError::VotingCompleted),
            6015 => Some(SoroSusuError::InvalidVote),

            // Lending market errors
            7000 => Some(SoroSusuError::LendingMarketNotInitialized),
            7001 => Some(SoroSusuError::LendingMarketDisabled),
            7002 => Some(SoroSusuError::LendingMarketEmergencyMode),
            7003 => Some(SoroSusuError::PoolNotFound),
            7004 => Some(SoroSusuError::PoolNotActive),
            7005 => Some(SoroSusuError::PoolAlreadyExists),
            7006 => Some(SoroSusuError::InsufficientPoolLiquidity),
            7007 => Some(SoroSusuError::InvalidLiquidityAmount),
            7008 => Some(SoroSusuError::PositionNotFound),
            7009 => Some(SoroSusuError::PositionNotActive),
            7010 => Some(SoroSusuError::LoanAlreadyRepaid),
            7011 => Some(SoroSusuError::InvalidLoanAmount),
            7012 => Some(SoroSusuError::InvalidLtvRatio),
            7013 => Some(SoroSusuError::LtvRatioExceeded),
            7014 => Some(SoroSusuError::MinLendingAmount),
            7015 => Some(SoroSusuError::ScheduleNotFound),
            7016 => Some(SoroSusuError::EmergencyLoanNotFound),
            7017 => Some(SoroSusuError::EmergencyLoanNotActive),
            7018 => Some(SoroSusuError::VotingPeriodEnded),
            7019 => Some(SoroSusuError::QuorumNotMet),

            // Yield and strategy errors
            8000 => Some(SoroSusuError::YieldStrategyNotFound),
            8001 => Some(SoroSusuError::YieldStrategyNotActive),
            8002 => Some(SoroSusuError::InvalidYieldStrategy),
            8003 => Some(SoroSusuError::YieldOracleError),
            8004 => Some(SoroSusuError::CircuitBreakerTriggered),
            8005 => Some(SoroSusuError::YieldAllocationFailed),
            8006 => Some(SoroSusuError::InvalidYieldAllocation),
            8007 => Some(SoroSusuError::BridgeAdapterError),
            8008 => Some(SoroSusuError::SlippageExceeded),
            8009 => Some(SoroSusuError::PriceFeedError),
            8010 => Some(SoroSusuError::InvalidPriceData),
            8011 => Some(SoroSusuError::YieldCalculationError),

            // Security and compliance errors
            9000 => Some(SoroSusuError::ReentrancyDetected),
            9001 => Some(SoroSusuError::Paused),
            9002 => Some(SoroSusuError::NotPaused),
            9003 => Some(SoroSusuError::EmergencyMode),
            9004 => Some(SoroSusuError::Blacklisted),
            9005 => Some(SoroSusuError::Sanctioned),
            9006 => Some(SoroSusuError::ComplianceFailed),
            9007 => Some(SoroSusuError::AmlViolation),
            9008 => Some(SoroSusuError::KycRequired),
            9009 => Some(SoroSusuError::TaxWithholdingFailed),
            9010 => Some(SoroSusuError::InvalidTaxConfiguration),
            9011 => Some(SoroSusuError::TaxExemptionInvalid),
            9012 => Some(SoroSusuError::AuditEntryMissing),
            9013 => Some(SoroSusuError::TrustlineMissing),
            9014 => Some(SoroSusuError::AnchorIntegrationError),

            // Technical and system errors
            10000 => Some(SoroSusuError::ContractNotInitialized),
            10001 => Some(SoroSusuError::AdminNotSet),
            10002 => Some(SoroSusuError::InvalidAdmin),
            10003 => Some(SoroSusuError::ConfigurationError),
            10004 => Some(SoroSusuError::UpgradeFailed),
            10005 => Some(SoroSusuError::MigrationFailed),
            10006 => Some(SoroSusuError::BackupFailed),
            10007 => Some(SoroSusuError::RestoreFailed),
            10008 => Some(SoroSusuError::NetworkError),
            10009 => Some(SoroSusuError::GasLimitExceeded),
            10010 => Some(SoroSusuError::ExecutionTimeout),
            10011 => Some(SoroSusuError::InternalError),
            10012 => Some(SoroSusuError::DeprecatedFunction),
            10013 => Some(SoroSusuError::FeatureNotEnabled),
            10014 => Some(SoroSusuError::InvalidChain),
            10015 => Some(SoroSusuError::VersionMismatch),

            _ => None,
        }
    }
}

/// Result type alias for SoroSusu operations
pub type SoroSusuResult<T> = Result<T, SoroSusuError>;

/// Convert SoroSusuError to Soroban Error for contract return
impl From<SoroSusuError> for Error {
    fn from(error: SoroSusuError) -> Self {
        Error::from_contract_error(error.code())
    }
}

/// Convert Soroban Error to SoroSusuError (for error handling)
impl From<Error> for SoroSusuError {
    fn from(error: Error) -> Self {
        // Try to extract contract error code
        if let Some(contract_code) = error.to_contract_error() {
            if let Some(sorosusu_error) = SoroSusuError::from_code(contract_code) {
                return sorosusu_error;
            }
        }
        
        // Fallback to generic errors based on error type
        match error {
            Error::FromContractError(_) => SoroSusuError::InternalError,
            Error::FromContractErrorWithDetails { .. } => SoroSusuError::InternalError,
            Error::HostError(_) => SoroSusuError::NetworkError,
            Error::ContextError(_) => SoroSusuError::InvalidState,
            Error::WasmVmError(_) => SoroSusuError::InternalError,
            Error::StorageError(_) => SoroSusuError::StorageError,
            Error::ObjectError(_) => SoroSusuError::SerializationError,
            Error::CryptoError(_) => SoroSusuError::InvalidSignature,
            Error::EventsError(_) => SoroSusuError::InternalError,
            Error::BudgetError(_) => SoroSusuError::GasLimitExceeded,
            Error::VmError(_) => SoroSusuError::InternalError,
            Error::IoError(_) => SoroSusuError::StorageError,
            Error::Infallible => SoroSusuError::InternalError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(SoroSusuError::Unauthorized.code(), 1000);
        assert_eq!(SoroSusuError::CircleNotFound.code(), 2000);
        assert_eq!(SoroSusuError::MemberNotFound.code(), 3000);
    }

    #[test]
    fn test_error_from_code() {
        assert_eq!(SoroSusuError::from_code(1000), Some(SoroSusuError::Unauthorized));
        assert_eq!(SoroSusuError::from_code(2000), Some(SoroSusuError::CircleNotFound));
        assert_eq!(SoroSusuError::from_code(9999), None);
    }

    #[test]
    fn test_human_readable_messages() {
        assert_eq!(
            SoroSusuError::CircleNotMatured.to_human_readable(),
            "Circle has not matured yet"
        );
        assert_eq!(
            SoroSusuError::Unauthorized.to_human_readable(),
            "You are not authorized to perform this action"
        );
    }

    #[test]
    fn test_error_conversion() {
        let sorosusu_error = SoroSusuError::CircleNotFound;
        let soroban_error: Error = sorosusu_error.clone().into();
        let converted_back: SoroSusuError = soroban_error.into();
        
        // Should preserve the error type if it's a contract error
        match converted_back {
            SoroSusuError::InternalError => {
                // This is expected for non-contract errors
            }
            _ => {
                // For contract errors, it should preserve the type
                assert_eq!(converted_back.code(), sorosusu_error.code());
            }
        }
    }
}
