// --- MULTI-CHAIN REPUTATION EXPORT MODULE ---
//
// Implements cross-chain state anchoring using the Wormhole protocol.
// Allows users to request a "Reputation Export" cryptographically signed by SoroSusu.
// The core logic bundles the user's RI score, total cycles, and default history
// into a 32-byte hash that can be verified on destination chains (Ethereum, Solana, etc.).

#![no_std]

use soroban_sdk::{
    contracttype, contracterror, Address, Env, BytesN, Vec, Map, String, Symbol,
    crypto::sha256,
};

// --- CONSTANTS ---

/// Protocol fee for reputation export in stroops (0.1 XLM)
const EXPORT_PROTOCOL_FEE: i128 = 1_000_000;

/// Wormhole chain ID for Stellar
const WORMHOLE_STELLAR_CHAIN_ID: u16 = 1;

/// Wormhole chain ID for Ethereum
const WORMHOLE_ETHEREUM_CHAIN_ID: u16 = 2;

/// Wormhole chain ID for Solana
const WORMHOLE_SOLANA_CHAIN_ID: u16 = 5;

/// Minimum time between exports for the same user (24 hours in seconds)
const EXPORT_COOLDOWN_SECONDS: u64 = 86_400;

// --- STORAGE KEYS ---

#[contracttype]
#[derive(Clone)]
pub enum ExportDataKey {
    /// Export nonce per user - increments with each export
    ExportNonce(Address),
    /// Last export timestamp per user - for cooldown enforcement
    LastExportTimestamp(Address),
    /// Export record by sequence number - for auditability
    ExportRecord(u64),
    /// Global export sequence counter
    GlobalExportSequence,
    /// Export hash set - for deduplication (prevents replay attacks)
    ExportHash(BytesN<32>),
    /// Pending default investigations per user
    PendingDefaultInvestigation(Address),
    /// Wormhole bridge configuration
    WormholeConfig,
}

// --- DATA STRUCTURES ---

/// Wormhole bridge configuration
#[contracttype]
#[derive(Clone)]
pub struct WormholeConfig {
    pub wormhole_contract: Address,
    pub enabled: bool,
    pub supported_chains: Vec<u16>,
}

/// Reputation export payload - the data to be cross-chain verified
#[contracttype]
#[derive(Clone)]
pub struct ReputationExportPayload {
    pub user_address: Address,
    pub ri_score: u32,           // Reliability Index score (0-10000 bps)
    pub total_cycles: u32,       // Total cycles completed
    pub defaults_count: u32,     // Number of defaults
    pub on_time_rate_bps: u32,  // On-time payment rate in basis points
    pub volume_saved: i128,     // Total volume saved
    pub sequence_nonce: u64,    // Monotonically increasing sequence number
    pub export_timestamp: u64,   // When the export was created
    pub destination_chain: u16, // Target chain (Wormhole chain ID)
}

/// Export metadata stored in temporary storage
#[contracttype]
#[derive(Clone)]
pub struct ExportMetadata {
    pub export_id: u64,
    pub user_address: Address,
    pub payload_hash: BytesN<32>,
    export_timestamp: u64,
    pub destination_chain: u16,
    pub fee_paid: i128,
    pub wormhole_sequence: Option<u64>, // Sequence from Wormhole bridge
}

/// Pending default investigation record
#[contracttype]
#[derive(Clone)]
pub struct DefaultInvestigation {
    pub circle_id: u64,
    pub investigation_start: u64,
    pub status: InvestigationStatus,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum InvestigationStatus {
    Active,
    Resolved,
    Dismissed,
}

// --- ERRORS ---

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ExportError {
    /// User has pending default investigations
    PendingDefaultInvestigation = 501,
    /// Export cooldown period not met
    ExportCooldownNotMet = 502,
    /// Insufficient protocol fee
    InsufficientProtocolFee = 503,
    /// Wormhole bridge not configured
    WormholeNotConfigured = 504,
    /// Destination chain not supported
    UnsupportedChain = 505,
    /// Duplicate export detected (replay attack)
    DuplicateExport = 506,
    /// User has no reputation data
    NoReputationData = 507,
    /// Wormhole bridge is disabled
    WormholeDisabled = 508,
}

// --- HELPERS ---

/// Calculate the 32-byte hash of a reputation export payload
/// This hash is what gets cross-chain verified
fn calculate_payload_hash(env: &Env, payload: &ReputationExportPayload) -> BytesN<32> {
    // Serialize the payload components for hashing
    let mut hash_input = Vec::new(env);
    
    hash_input.push_back(payload.user_address.to_contract());
    hash_input.push_back(payload.ri_score.to_contract());
    hash_input.push_back(payload.total_cycles.to_contract());
    hash_input.push_back(payload.defaults_count.to_contract());
    hash_input.push_back(payload.on_time_rate_bps.to_contract());
    hash_input.push_back(payload.volume_saved.to_contract());
    hash_input.push_back(payload.sequence_nonce.to_contract());
    hash_input.push_back(payload.export_timestamp.to_contract());
    hash_input.push_back(payload.destination_chain.to_contract());
    
    sha256::sha256(&hash_input)
}

/// Check if a user has pending default investigations
fn has_pending_defaults(env: &Env, user: &Address) -> bool {
    if let Some(investigation) = env.storage().temporary().get(&ExportDataKey::PendingDefaultInvestigation(user.clone())) {
        if investigation.status == InvestigationStatus::Active {
            return true;
        }
    }
    false
}

/// Check if user is within export cooldown period
fn is_export_cooldown_active(env: &Env, user: &Address) -> bool {
    if let Some(last_export) = env.storage().temporary().get(&ExportDataKey::LastExportTimestamp(user.clone())) {
        let current_time = env.ledger().timestamp();
        let elapsed = current_time.saturating_sub(last_export);
        return elapsed < EXPORT_COOLDOWN_SECONDS;
    }
    false
}

/// Check if this export hash has already been used (deduplication)
fn is_duplicate_export(env: &Env, hash: &BytesN<32>) -> bool {
    env.storage().temporary().has(&ExportDataKey::ExportHash(*hash))
}

// --- PUBLIC FUNCTIONS ---

/// Initialize the Wormhole bridge configuration
/// 
/// # Parameters
/// - `env`: The Soroban environment
/// - `admin`: Admin address (must authorize)
/// - `wormhole_contract`: Address of the Wormhole bridge contract
/// - `supported_chains`: List of supported destination chain IDs
pub fn init_wormhole_config(
    env: &Env,
    admin: &Address,
    wormhole_contract: Address,
    supported_chains: Vec<u16>,
) {
    admin.require_auth();
    
    let config = WormholeConfig {
        wormhole_contract,
        enabled: true,
        supported_chains,
    };
    
    env.storage().temporary().set(&ExportDataKey::WormholeConfig, &config);
}

/// Export a user's reputation to a destination chain via Wormhole
/// 
/// # Parameters
/// - `env`: The Soroban environment
/// - `user`: User address requesting the export (must authorize)
/// - `destination_chain`: Target chain ID (Wormhole chain ID)
/// - `fee_paid`: Protocol fee paid in XLM
/// - `ri_score`: User's Reliability Index score
/// - `total_cycles`: Total cycles completed
/// - `defaults_count`: Number of defaults
/// - `on_time_rate_bps`: On-time payment rate in basis points
/// - `volume_saved`: Total volume saved
/// 
/// # Returns
/// The export ID and payload hash
/// 
/// # Errors
/// - `ExportError::PendingDefaultInvestigation` if user has active default investigations
/// - `ExportError::ExportCooldownNotMet` if cooldown period not elapsed
/// - `ExportError::InsufficientProtocolFee` if fee is too low
/// - `ExportError::WormholeNotConfigured` if Wormhole not initialized
/// - `ExportError::UnsupportedChain` if destination chain not supported
/// - `ExportError::DuplicateExport` if this exact export already exists
pub fn export_reputation(
    env: &Env,
    user: &Address,
    destination_chain: u16,
    fee_paid: i128,
    ri_score: u32,
    total_cycles: u32,
    defaults_count: u32,
    on_time_rate_bps: u32,
    volume_saved: i128,
) -> Result<(u64, BytesN<32>), ExportError> {
    // Check protocol fee
    if fee_paid < EXPORT_PROTOCOL_FEE {
        return Err(ExportError::InsufficientProtocolFee);
    }
    
    // Check for pending default investigations (edge case)
    if has_pending_defaults(env, user) {
        return Err(ExportError::PendingDefaultInvestigation);
    }
    
    // Check export cooldown
    if is_export_cooldown_active(env, user) {
        return Err(ExportError::ExportCooldownNotMet);
    }
    
    // Get Wormhole configuration
    let config: WormholeConfig = env.storage().temporary()
        .get(&ExportDataKey::WormholeConfig)
        .ok_or(ExportError::WormholeNotConfigured)?;
    
    if !config.enabled {
        return Err(ExportError::WormholeDisabled);
    }
    
    // Check if destination chain is supported
    if !config.supported_chains.iter().any(|&chain| chain == destination_chain) {
        return Err(ExportError::UnsupportedChain);
    }
    
    // Get and increment user's export nonce
    let nonce_key = ExportDataKey::ExportNonce(user.clone());
    let nonce: u64 = env.storage().temporary().get(&nonce_key).unwrap_or(0);
    env.storage().temporary().set(&nonce_key, &(nonce + 1));
    
    // Get and increment global export sequence
    let seq_key = ExportDataKey::GlobalExportSequence;
    let global_sequence: u64 = env.storage().temporary().get(&seq_key).unwrap_or(0);
    let export_id = global_sequence + 1;
    env.storage().temporary().set(&seq_key, &export_id);
    
    // Create the export payload
    let payload = ReputationExportPayload {
        user_address: user.clone(),
        ri_score,
        total_cycles,
        defaults_count,
        on_time_rate_bps,
        volume_saved,
        sequence_nonce: nonce,
        export_timestamp: env.ledger().timestamp(),
        destination_chain,
    };
    
    // Calculate the payload hash
    let payload_hash = calculate_payload_hash(env, &payload);
    
    // Check for duplicate export (deduplication to prevent replay attacks)
    if is_duplicate_export(env, &payload_hash) {
        return Err(ExportError::DuplicateExport);
    }
    
    // Store the export hash for deduplication
    env.storage().temporary().set(&ExportDataKey::ExportHash(payload_hash), &true);
    
    // Store export metadata in temporary storage
    let metadata = ExportMetadata {
        export_id,
        user_address: user.clone(),
        payload_hash,
        export_timestamp: env.ledger().timestamp(),
        destination_chain,
        fee_paid,
        wormhole_sequence: None, // Will be set when Wormhole confirms
    };
    
    env.storage().temporary().set(&ExportDataKey::ExportRecord(export_id), &metadata);
    
    // Update last export timestamp for cooldown
    env.storage().temporary().set(
        &ExportDataKey::LastExportTimestamp(user.clone()),
        &env.ledger().timestamp()
    );
    
    // In a real implementation, this would call the Wormhole bridge contract
    // to emit the cross-chain message. For now, we emit an event.
    env.events().publish(
        (Symbol::short("REPUTATION_EXPORT"), user.clone()),
        (export_id, payload_hash, destination_chain)
    );
    
    Ok((export_id, payload_hash))
}

/// Record a pending default investigation for a user
/// This prevents reputation export during investigation
pub fn record_default_investigation(
    env: &Env,
    user: Address,
    circle_id: u64,
) {
    let investigation = DefaultInvestigation {
        circle_id,
        investigation_start: env.ledger().timestamp(),
        status: InvestigationStatus::Active,
    };
    
    env.storage().temporary().set(
        &ExportDataKey::PendingDefaultInvestigation(user),
        &investigation
    );
}

/// Resolve a default investigation, allowing exports again
pub fn resolve_default_investigation(
    env: &Env,
    user: Address,
    status: InvestigationStatus,
) {
    if let Some(mut investigation) = env.storage().temporary()
        .get(&ExportDataKey::PendingDefaultInvestigation(user.clone()))
    {
        investigation.status = status;
        env.storage().temporary().set(
            &ExportDataKey::PendingDefaultInvestigation(user),
            &investigation
        );
    }
}

/// Get export metadata by export ID
pub fn get_export_metadata(env: &Env, export_id: u64) -> Option<ExportMetadata> {
    env.storage().temporary().get(&ExportDataKey::ExportRecord(export_id))
}

/// Get user's export nonce
pub fn get_export_nonce(env: &Env, user: Address) -> u64 {
    env.storage().temporary()
        .get(&ExportDataKey::ExportNonce(user))
        .unwrap_or(0)
}

/// Check if a user can export (no pending investigations, cooldown met)
pub fn can_export(env: &Env, user: Address) -> bool {
    !has_pending_defaults(env, &user) && !is_export_cooldown_active(env, &user)
}

/// Get the Wormhole configuration
pub fn get_wormhole_config(env: &Env) -> Option<WormholeConfig> {
    env.storage().temporary().get(&ExportDataKey::WormholeConfig)
}
