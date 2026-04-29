//! Contribution Security Module
//! 
//! This module implements hardened contribution logic to prevent double-spend exploits
//! and provides secure contribution reversal/rollback mechanisms.

#![cfg_attr(not(test), no_std)]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    Address, Env, Symbol, Vec, Bytes, BytesN, Map,
    crypto::Sha256,
};

use crate::DataKey;

// --- ERROR TYPES ---

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContributionSecurityError {
    InvalidTransactionState = 600,      // Transaction state is invalid
    DoubleSpendAttempt = 601,         // Attempt to spend same contribution twice
    TransactionAlreadyCommitted = 602, // Transaction cannot be rolled back
    InsufficientRollbackBalance = 603, // Not enough funds to rollback
    InvalidMerkleProof = 604,         // Merkle proof verification failed
    ContributionNotFound = 605,        // Contribution record not found
    UnauthorizedRollback = 606,        // Not authorized to rollback transaction
}

// --- DATA STRUCTURES ---

/// Transaction state for atomic contribution operations
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum TransactionState {
    Pending,    // Transaction initiated but not committed
    Committed,  // Transaction successfully committed
    RolledBack, // Transaction was rolled back
}

/// Atomic contribution transaction record
#[contracttype]
#[derive(Clone)]
pub struct ContributionTransaction {
    /// Unique transaction ID
    pub tx_id: BytesN<32>,
    /// User making the contribution
    pub user: Address,
    /// Circle ID
    pub circle_id: u64,
    /// Amount contributed
    pub amount: u64,
    /// Number of rounds
    pub rounds: u32,
    /// Transaction state
    pub state: TransactionState,
    /// Timestamp when created
    pub created_at: u64,
    /// Timestamp when committed/rolled back
    pub finalized_at: Option<u64>,
    /// Previous contribution bitmap (for rollback)
    pub previous_bitmap: u64,
    /// Previous contribution count (for rollback)
    pub previous_count: u32,
}

/// Merkle tree leaf for contribution proof
#[contracttype]
#[derive(Clone)]
pub struct ContributionLeaf {
    /// User address
    pub user: Address,
    /// Circle ID
    pub circle_id: u64,
    /// Amount contributed
    pub amount: u64,
    /// Round number
    pub round: u32,
    /// Timestamp of contribution
    pub timestamp: u64,
    /// Contribution nonce (prevents replay)
    pub nonce: u64,
}

/// Merkle proof for contribution verification
#[contracttype]
#[derive(Clone)]
pub struct ContributionMerkleProof {
    /// Merkle root
    pub root: BytesN<32>,
    /// Leaf being proven
    pub leaf: ContributionLeaf,
    /// Proof path (sibling hashes)
    pub proof_path: Vec<BytesN<32>>,
    /// Proof indices (0 = left, 1 = right)
    pub proof_indices: Vec<bool>,
}

/// Contribution history for Merkle tree construction
#[contracttype]
#[derive(Clone)]
pub struct ContributionRecord {
    /// Contribution leaf data
    pub leaf: ContributionLeaf,
    /// Hash of the leaf
    pub leaf_hash: BytesN<32>,
    /// Whether contribution is verified
    pub verified: bool,
}

// --- STORAGE KEYS ---

#[contracttype]
#[derive(Clone)]
pub enum SecurityDataKey {
    /// Active contribution transaction
    ActiveTransaction(BytesN<32>),
    /// Transaction history
    TransactionHistory(BytesN<32>),
    /// Transaction counter
    TransactionCounter,
    /// Contribution records for Merkle tree
    ContributionRecord(u64), // Global contribution counter
    /// Contribution counter
    ContributionCounter,
    /// Circle Merkle root
    CircleMerkleRoot(u64),
    /// User contribution nonce
    UserContributionNonce(Address, u64),
}

// --- EVENTS ---

#[contracttype]
#[derive(Clone)]
pub enum SecurityEvent {
    /// Emitted when atomic transaction starts
    TransactionStarted {
        tx_id: BytesN<32>,
        user: Address,
        circle_id: u64,
        amount: u64,
    },
    /// Emitted when transaction is committed
    TransactionCommitted {
        tx_id: BytesN<32>,
        circle_id: u64,
    },
    /// Emitted when transaction is rolled back
    TransactionRolledBack {
        tx_id: BytesN<32>,
        reason: Symbol,
    },
    /// Emitted when Merkle root is updated
    MerkleRootUpdated {
        circle_id: u64,
        root: BytesN<32>,
        contribution_count: u64,
    },
}

// --- CONTRIBUTION SECURITY TRAIT ---

pub trait ContributionSecurityTrait {
    /// Start an atomic contribution transaction
    fn start_contribution_transaction(
        env: Env,
        user: Address,
        circle_id: u64,
        amount: u64,
        rounds: u32,
    ) -> Result<BytesN<32>, ContributionSecurityError>;

    /// Commit an atomic contribution transaction
    fn commit_contribution_transaction(
        env: Env,
        tx_id: BytesN<32>,
    ) -> Result<(), ContributionSecurityError>;

    /// Rollback an atomic contribution transaction
    fn rollback_contribution_transaction(
        env: Env,
        tx_id: BytesN<32>,
        reason: Symbol,
    ) -> Result<(), ContributionSecurityError>;

    /// Get transaction state
    fn get_transaction_state(env: Env, tx_id: BytesN<32>) -> Option<TransactionState>;

    /// Generate Merkle proof for contribution verification
    fn generate_contribution_proof(
        env: Env,
        user: Address,
        circle_id: u64,
        round: u32,
    ) -> Result<ContributionMerkleProof, ContributionSecurityError>;

    /// Verify contribution Merkle proof
    fn verify_contribution_proof(
        env: Env,
        proof: ContributionMerkleProof,
    ) -> Result<bool, ContributionSecurityError>;

    /// Get current Merkle root for a circle
    fn get_circle_merkle_root(env: Env, circle_id: u64) -> Option<BytesN<32>>;
}

// --- IMPLEMENTATION ---

#[contract]
pub struct ContributionSecurity;

#[contractimpl]
impl ContributionSecurityTrait for ContributionSecurity {
    fn start_contribution_transaction(
        env: Env,
        user: Address,
        circle_id: u64,
        amount: u64,
        rounds: u32,
    ) -> Result<BytesN<32>, ContributionSecurityError> {
        user.require_auth();

        // Generate unique transaction ID
        let tx_counter: u64 = env.storage().instance()
            .get(&SecurityDataKey::TransactionCounter)
            .unwrap_or(0);
        let tx_id = Self::generate_transaction_id(&env, &user, circle_id, tx_counter);

        // Check for existing pending transaction
        if env.storage().instance().has(&SecurityDataKey::ActiveTransaction(tx_id.clone())) {
            return Err(ContributionSecurityError::DoubleSpendAttempt);
        }

        // Get current circle state for rollback
        let circle: crate::CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(circle_id))
            .ok_or(ContributionSecurityError::ContributionNotFound)?;

        let member_key = DataKey::Member(user.clone());
        let member: crate::Member = env.storage().instance()
            .get(&member_key)
            .ok_or(ContributionSecurityError::ContributionNotFound)?;

        // Create transaction record
        let transaction = ContributionTransaction {
            tx_id: tx_id.clone(),
            user: user.clone(),
            circle_id,
            amount,
            rounds,
            state: TransactionState::Pending,
            created_at: env.ledger().timestamp(),
            finalized_at: None,
            previous_bitmap: circle.contribution_bitmap,
            previous_count: member.contribution_count,
        };

        // Store transaction
        env.storage().instance().set(
            &SecurityDataKey::ActiveTransaction(tx_id.clone()),
            &transaction,
        );
        env.storage().instance().set(
            &SecurityDataKey::TransactionHistory(tx_id.clone()),
            &transaction,
        );
        env.storage().instance().set(&SecurityDataKey::TransactionCounter, &(tx_counter + 1));

        // Emit event
        env.events().publish(
            (Symbol::short("tx_started"),),
            SecurityEvent::TransactionStarted {
                tx_id: tx_id.clone(),
                user,
                circle_id,
                amount,
            },
        );

        Ok(tx_id)
    }

    fn commit_contribution_transaction(
        env: Env,
        tx_id: BytesN<32>,
    ) -> Result<(), ContributionSecurityError> {
        let mut transaction: ContributionTransaction = env.storage().instance()
            .get(&SecurityDataKey::ActiveTransaction(tx_id.clone()))
            .ok_or(ContributionSecurityError::ContributionNotFound)?;

        if transaction.state != TransactionState::Pending {
            return Err(ContributionSecurityError::InvalidTransactionState);
        }

        // Update transaction state
        transaction.state = TransactionState::Committed;
        transaction.finalized_at = Some(env.ledger().timestamp());

        // Update storage
        env.storage().instance().set(
            &SecurityDataKey::ActiveTransaction(tx_id.clone()),
            &transaction,
        );
        env.storage().instance().set(
            &SecurityDataKey::TransactionHistory(tx_id.clone()),
            &transaction,
        );

        // Create contribution record for Merkle tree
        let contribution_counter: u64 = env.storage().instance()
            .get(&SecurityDataKey::ContributionCounter)
            .unwrap_or(0);

        let nonce_key = SecurityDataKey::UserContributionNonce(transaction.user.clone(), transaction.circle_id);
        let nonce: u64 = env.storage().instance().get(&nonce_key).unwrap_or(0);

        let leaf = ContributionLeaf {
            user: transaction.user.clone(),
            circle_id: transaction.circle_id,
            amount: transaction.amount,
            round: 0, // Current round (simplified)
            timestamp: transaction.created_at,
            nonce,
        };

        let leaf_hash = Self::hash_contribution_leaf(&env, &leaf);
        let record = ContributionRecord {
            leaf: leaf.clone(),
            leaf_hash,
            verified: true,
        };

        // Store contribution record
        env.storage().instance().set(
            &SecurityDataKey::ContributionRecord(contribution_counter),
            &record,
        );
        env.storage().instance().set(&SecurityDataKey::ContributionCounter, &(contribution_counter + 1));
        env.storage().instance().set(&nonce_key, &(nonce + 1));

        // Update Merkle root
        Self::update_merkle_root(&env, transaction.circle_id)?;

        // Remove from active transactions
        env.storage().instance().remove(&SecurityDataKey::ActiveTransaction(tx_id.clone()));

        // Emit event
        env.events().publish(
            (Symbol::short("tx_committed"),),
            SecurityEvent::TransactionCommitted {
                tx_id,
                circle_id: transaction.circle_id,
            },
        );

        Ok(())
    }

    fn rollback_contribution_transaction(
        env: Env,
        tx_id: BytesN<32>,
        reason: Symbol,
    ) -> Result<(), ContributionSecurityError> {
        let mut transaction: ContributionTransaction = env.storage().instance()
            .get(&SecurityDataKey::ActiveTransaction(tx_id.clone()))
            .ok_or(ContributionSecurityError::ContributionNotFound)?;

        if transaction.state != TransactionState::Pending {
            return Err(ContributionSecurityError::TransactionAlreadyCommitted);
        }

        // Restore previous state
        let mut circle: crate::CircleInfo = env.storage().instance()
            .get(&DataKey::Circle(transaction.circle_id))
            .ok_or(ContributionSecurityError::ContributionNotFound)?;

        let member_key = DataKey::Member(transaction.user.clone());
        let mut member: crate::Member = env.storage().instance()
            .get(&member_key)
            .ok_or(ContributionSecurityError::ContributionNotFound)?;

        // Rollback circle state
        circle.contribution_bitmap = transaction.previous_bitmap;
        member.contribution_count = transaction.previous_count;

        // Update storage
        env.storage().instance().set(&DataKey::Circle(transaction.circle_id), &circle);
        env.storage().instance().set(&member_key, &member);

        // Update transaction state
        transaction.state = TransactionState::RolledBack;
        transaction.finalized_at = Some(env.ledger().timestamp());

        env.storage().instance().set(
            &SecurityDataKey::TransactionHistory(tx_id.clone()),
            &transaction,
        );

        // Remove from active transactions
        env.storage().instance().remove(&SecurityDataKey::ActiveTransaction(tx_id.clone()));

        // Refund tokens back to user
        let token_client = soroban_sdk::token::Client::new(&env, &circle.token);
        let refund_amount = transaction.amount.checked_mul(transaction.rounds as u64)
            .ok_or(ContributionSecurityError::InsufficientRollbackBalance)?;
        
        token_client.transfer(
            &env.current_contract_address(),
            &transaction.user,
            &(refund_amount as i128),
        );

        // Emit event
        env.events().publish(
            (Symbol::short("tx_rolled_back"),),
            SecurityEvent::TransactionRolledBack {
                tx_id,
                reason,
            },
        );

        Ok(())
    }

    fn get_transaction_state(env: Env, tx_id: BytesN<32>) -> Option<TransactionState> {
        if let Some(transaction) = env.storage().instance()
            .get::<SecurityDataKey, ContributionTransaction>(&SecurityDataKey::ActiveTransaction(tx_id)) {
            Some(transaction.state)
        } else if let Some(transaction) = env.storage().instance()
            .get::<SecurityDataKey, ContributionTransaction>(&SecurityDataKey::TransactionHistory(tx_id)) {
            Some(transaction.state)
        } else {
            None
        }
    }

    fn generate_contribution_proof(
        env: Env,
        user: Address,
        circle_id: u64,
        round: u32,
    ) -> Result<ContributionMerkleProof, ContributionSecurityError> {
        // Find the contribution record
        let contribution_counter: u64 = env.storage().instance()
            .get(&SecurityDataKey::ContributionCounter)
            .unwrap_or(0);

        let mut target_record: Option<ContributionRecord> = None;
        let mut target_index: Option<u64> = None;

        for i in 0..contribution_counter {
            if let Some(record) = env.storage().instance()
                .get::<SecurityDataKey, ContributionRecord>(&SecurityDataKey::ContributionRecord(i)) {
                if record.leaf.user == user && 
                   record.leaf.circle_id == circle_id && 
                   record.leaf.round == round {
                    target_record = Some(record);
                    target_index = Some(i);
                    break;
                }
            }
        }

        let record = target_record.ok_or(ContributionSecurityError::ContributionNotFound)?;
        let index = target_index.unwrap();

        // Generate Merkle proof
        let merkle_root = Self::get_circle_merkle_root(env.clone(), circle_id)
            .ok_or(ContributionSecurityError::ContributionNotFound)?;

        let (proof_path, proof_indices) = Self::generate_merkle_proof_path(
            &env,
            circle_id,
            index,
            contribution_counter,
        );

        let proof = ContributionMerkleProof {
            root: merkle_root,
            leaf: record.leaf,
            proof_path,
            proof_indices,
        };

        Ok(proof)
    }

    fn verify_contribution_proof(
        env: Env,
        proof: ContributionMerkleProof,
    ) -> Result<bool, ContributionSecurityError> {
        let leaf_hash = Self::hash_contribution_leaf(&env, &proof.leaf);
        let computed_root = Self::compute_merkle_root_from_proof(
            &env,
            &leaf_hash,
            &proof.proof_path,
            &proof.proof_indices,
        );

        Ok(computed_root == proof.root)
    }

    fn get_circle_merkle_root(env: Env, circle_id: u64) -> Option<BytesN<32>> {
        env.storage().instance()
            .get(&SecurityDataKey::CircleMerkleRoot(circle_id))
    }
}

// --- INTERNAL HELPER FUNCTIONS ---

impl ContributionSecurity {
    fn generate_transaction_id(env: &Env, user: &Address, circle_id: u64, counter: u64) -> BytesN<32> {
        let mut hasher = Sha256::new(env);
        hasher.update(user.to_string().as_bytes());
        hasher.update(&circle_id.to_le_bytes());
        hasher.update(&counter.to_le_bytes());
        hasher.update(&env.ledger().timestamp().to_le_bytes());
        hasher.finalize()
    }

    fn hash_contribution_leaf(env: &Env, leaf: &ContributionLeaf) -> BytesN<32> {
        let mut hasher = Sha256::new(env);
        hasher.update(leaf.user.to_string().as_bytes());
        hasher.update(&leaf.circle_id.to_le_bytes());
        hasher.update(&leaf.amount.to_le_bytes());
        hasher.update(&leaf.round.to_le_bytes());
        hasher.update(&leaf.timestamp.to_le_bytes());
        hasher.update(&leaf.nonce.to_le_bytes());
        hasher.finalize()
    }

    fn update_merkle_root(env: &Env, circle_id: u64) -> Result<(), ContributionSecurityError> {
        let contribution_counter: u64 = env.storage().instance()
            .get(&SecurityDataKey::ContributionCounter)
            .unwrap_or(0);

        if contribution_counter == 0 {
            return Ok(()); // No contributions yet
        }

        // Collect all contribution hashes for this circle
        let mut circle_hashes: Vec<BytesN<32>> = Vec::new(env);
        for i in 0..contribution_counter {
            if let Some(record) = env.storage().instance()
                .get::<SecurityDataKey, ContributionRecord>(&SecurityDataKey::ContributionRecord(i)) {
                if record.leaf.circle_id == circle_id && record.verified {
                    circle_hashes.push_back(record.leaf_hash);
                }
            }
        }

        if circle_hashes.is_empty() {
            return Ok(()); // No contributions for this circle
        }

        // Build Merkle tree and get root
        let merkle_root = Self::build_merkle_root(env, &circle_hashes);

        // Store the root
        env.storage().instance()
            .set(&SecurityDataKey::CircleMerkleRoot(circle_id), &merkle_root);

        // Emit event
        env.events().publish(
            (Symbol::short("merkle_updated"),),
            SecurityEvent::MerkleRootUpdated {
                circle_id,
                root: merkle_root,
                contribution_count: circle_hashes.len() as u64,
            },
        );

        Ok(())
    }

    fn build_merkle_root(env: &Env, hashes: &Vec<BytesN<32>>) -> BytesN<32> {
        if hashes.len() == 1 {
            return hashes.get_unchecked(0).clone();
        }

        let mut current_level = hashes.clone();
        
        while current_level.len() > 1 {
            let mut next_level: Vec<BytesN<32>> = Vec::new(env);
            
            for i in (0..current_level.len()).step_by(2) {
                let left = current_level.get(i).unwrap();
                let right = if i + 1 < current_level.len() {
                    current_level.get(i + 1).unwrap()
                } else {
                    left // Duplicate odd leaf
                };

                let mut hasher = Sha256::new(env);
                hasher.update(left.as_slice());
                hasher.update(right.as_slice());
                let combined = hasher.finalize();
                
                next_level.push_back(combined);
            }
            
            current_level = next_level;
        }

        current_level.get_unchecked(0).clone()
    }

    fn generate_merkle_proof_path(
        env: &Env,
        circle_id: u64,
        leaf_index: u64,
        total_contributions: u64,
    ) -> (Vec<BytesN<32>>, Vec<bool>) {
        let mut proof_path: Vec<BytesN<32>> = Vec::new(env);
        let mut proof_indices: Vec<bool> = Vec::new(env);

        // Collect all verified contributions for this circle
        let mut circle_hashes: Vec<BytesN<32>> = Vec::new(env);
        for i in 0..total_contributions {
            if let Some(record) = env.storage().instance()
                .get::<SecurityDataKey, ContributionRecord>(&SecurityDataKey::ContributionRecord(i)) {
                if record.leaf.circle_id == circle_id && record.verified {
                    circle_hashes.push_back(record.leaf_hash);
                }
            }
        }

        let mut current_level = circle_hashes;
        let mut current_index = leaf_index;

        while current_level.len() > 1 {
            let is_right = current_index % 2 == 1;
            let sibling_index = if is_right { current_index - 1 } else { current_index + 1 };

            if sibling_index < current_level.len() {
                proof_path.push_back(current_level.get(sibling_index).unwrap().clone());
                proof_indices.push_back(is_right);
            }

            current_index /= 2;

            // Build next level
            let mut next_level: Vec<BytesN<32>> = Vec::new(env);
            for i in (0..current_level.len()).step_by(2) {
                let left = current_level.get(i).unwrap();
                let right = if i + 1 < current_level.len() {
                    current_level.get(i + 1).unwrap()
                } else {
                    left
                };

                let mut hasher = Sha256::new(env);
                hasher.update(left.as_slice());
                hasher.update(right.as_slice());
                let combined = hasher.finalize();
                
                next_level.push_back(combined);
            }
            current_level = next_level;
        }

        (proof_path, proof_indices)
    }

    fn compute_merkle_root_from_proof(
        env: &Env,
        leaf_hash: &BytesN<32>,
        proof_path: &Vec<BytesN<32>>,
        proof_indices: &Vec<bool>,
    ) -> BytesN<32> {
        let mut current_hash = leaf_hash.clone();

        for (i, sibling_hash) in proof_path.iter().enumerate() {
            let is_right = proof_indices.get(i).unwrap_or(&false);

            let mut hasher = Sha256::new(env);
            if *is_right {
                hasher.update(sibling_hash.as_slice());
                hasher.update(current_hash.as_slice());
            } else {
                hasher.update(current_hash.as_slice());
                hasher.update(sibling_hash.as_slice());
            }
            current_hash = hasher.finalize();
        }

        current_hash
    }
}
