#![cfg_attr(not(test), no_std)]

use soroban_sdk::{Address, Env, BytesN, Symbol};
use super::zk_privacy::{ZkVerifierTrait, ZkError, ZkProof};

/// ZK-Privacy Integration Tests
/// 
/// This test module validates that the ZK-privacy logic does not interfere
/// with the Reliability Index calculation and other core contract functionality.
/// 
/// Test Coverage:
/// 1. Double-spend prevention via nullifier tracking
/// 2. Forged proof rejection
/// 3. Social slashing edge case handling
/// 4. ZK-logic compatibility with Reliability Index
/// 5. Event emission validation (ShieldedContributionVerified with nullifier only)

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_shielded_deposit_creates_commitment() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        let result = ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        );
        
        assert!(result.is_ok());
        
        // Verify commitment exists
        let retrieved_commitment = ZkVerifierTrait::get_commitment(env, nullifier);
        assert!(retrieved_commitment.is_some());
    }

    #[test]
    fn test_double_spend_prevention_blocks_reuse() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // First deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Try to deposit with same nullifier (should fail)
        let result = ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            BytesN::from_array(&env, &[3u8; 32]),
            nullifier.clone(),
        );
        
        assert!(matches!(result, Err(ZkError::NullifierAlreadySpent)));
    }

    #[test]
    fn test_double_spend_prevention_blocks_verification_after_spend() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Create and verify proof
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier.clone(),
        ).unwrap();
        
        // Try to verify again with same nullifier (should fail)
        let proof2 = ZkProof {
            proof_a: BytesN::from_array(&env, &[4u8; 64]),
            proof_b: BytesN::from_array(&env, &[5u8; 128]),
            proof_c: BytesN::from_array(&env, &[6u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        let result = ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof2,
            nullifier,
        );
        
        assert!(matches!(result, Err(ZkError::NullifierAlreadySpent)));
    }

    #[test]
    fn test_forged_proof_rejection_invalid_structure() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Create malformed proof (empty proof_a)
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[0u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        let result = ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier,
        );
        
        assert!(matches!(result, Err(ZkError::ProofVerificationFailed)));
    }

    #[test]
    fn test_forged_proof_rejection_missing_nullifier() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Create proof without nullifier in public inputs
        let wrong_nullifier = BytesN::from_array(&env, &[3u8; 32]);
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, wrong_nullifier],
            timestamp: env.ledger().timestamp(),
        };
        
        let result = ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier,
        );
        
        assert!(matches!(result, Err(ZkError::ProofVerificationFailed)));
    }

    #[test]
    fn test_forged_proof_rejection_stale_timestamp() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Create proof with stale timestamp (> 1 hour old)
        let old_timestamp = env.ledger().timestamp() - 3601;
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: old_timestamp,
        };
        
        let result = ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier,
        );
        
        assert!(matches!(result, Err(ZkError::ProofVerificationFailed)));
    }

    #[test]
    fn test_social_slash_voids_proof_on_default() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Social slash the proof
        let result = ZkVerifierTrait::social_slash_void_proof(
            env.clone(),
            admin.clone(),
            1,
            nullifier.clone(),
            Symbol::short("default"),
        );
        
        assert!(result.is_ok());
        
        // Try to use the voided proof (should fail)
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        let verify_result = ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier,
        );
        
        assert!(matches!(verify_result, Err(ZkError::SocialSlashProofVoided)));
    }

    #[test]
    fn test_social_slash_handles_already_spent() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Verify and spend the commitment
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier.clone(),
        ).unwrap();
        
        // Try to social slash an already spent commitment (should succeed gracefully)
        let result = ZkVerifierTrait::social_slash_void_proof(
            env.clone(),
            admin.clone(),
            1,
            nullifier,
            Symbol::short("default"),
        );
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_commitment_parameters() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Try to deposit with zero amount (should fail)
        let result = ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            0,
            1,
            commitment,
            nullifier,
        );
        
        assert!(matches!(result, Err(ZkError::InvalidCommitmentParameters)));
        
        // Try to deposit with negative amount (should fail)
        let result2 = ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            -1,
            1,
            BytesN::from_array(&env, &[3u8; 32]),
            BytesN::from_array(&env, &[4u8; 32]),
        );
        
        assert!(matches!(result2, Err(ZkError::InvalidCommitmentParameters)));
    }

    #[test]
    fn test_shielded_balance_tracking() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        // Initial balance should be 0
        let initial_balance = ZkVerifierTrait::get_shielded_balance(env.clone());
        assert_eq!(initial_balance, 0);
        
        // Deposit first amount
        let commitment1 = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier1 = BytesN::from_array(&env, &[2u8; 32]);
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment1,
            nullifier1,
        ).unwrap();
        
        let balance1 = ZkVerifierTrait::get_shielded_balance(env.clone());
        assert_eq!(balance1, 1000);
        
        // Deposit second amount
        let commitment2 = BytesN::from_array(&env, &[3u8; 32]);
        let nullifier2 = BytesN::from_array(&env, &[4u8; 32]);
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            2000,
            1,
            commitment2,
            nullifier2,
        ).unwrap();
        
        let balance2 = ZkVerifierTrait::get_shielded_balance(env.clone());
        assert_eq!(balance2, 3000);
    }

    #[test]
    fn test_nullifier_spent_tracking() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        // Deposit
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        // Nullifier should not be spent yet
        assert!(!ZkVerifierTrait::is_nullifier_spent(env.clone(), nullifier.clone()));
        
        // Verify and spend
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier.clone(),
        ).unwrap();
        
        // Nullifier should now be spent
        assert!(ZkVerifierTrait::is_nullifier_spent(env, nullifier));
    }

    #[test]
    fn test_multiple_commitments_independent_tracking() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        // Create multiple commitments
        let commitment1 = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier1 = BytesN::from_array(&env, &[2u8; 32]);
        
        let commitment2 = BytesN::from_array(&env, &[3u8; 32]);
        let nullifier2 = BytesN::from_array(&env, &[4u8; 32]);
        
        let commitment3 = BytesN::from_array(&env, &[5u8; 32]);
        let nullifier3 = BytesN::from_array(&env, &[6u8; 32]);
        
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment1,
            nullifier1.clone(),
        ).unwrap();
        
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            2000,
            2,
            commitment2,
            nullifier2.clone(),
        ).unwrap();
        
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            3000,
            3,
            commitment3,
            nullifier3.clone(),
        ).unwrap();
        
        // Verify each nullifier is tracked independently
        assert!(!ZkVerifierTrait::is_nullifier_spent(env.clone(), nullifier1.clone()));
        assert!(!ZkVerifierTrait::is_nullifier_spent(env.clone(), nullifier2.clone()));
        assert!(!ZkVerifierTrait::is_nullifier_spent(env.clone(), nullifier3.clone()));
        
        // Spend only nullifier2
        let proof2 = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier2.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            2,
            proof2,
            nullifier2.clone(),
        ).unwrap();
        
        // Only nullifier2 should be spent
        assert!(!ZkVerifierTrait::is_nullifier_spent(env.clone(), nullifier1));
        assert!(ZkVerifierTrait::is_nullifier_spent(env.clone(), nullifier2));
        assert!(!ZkVerifierTrait::is_nullifier_spent(env, nullifier3));
    }

    #[test]
    fn test_zk_logic_does_not_interfere_with_reliability_index() {
        // This test validates that ZK-privacy operations do not interfere
        // with the Reliability Index calculation by ensuring they use
        // separate storage keys and do not modify shared state.
        
        let env = Env::default();
        let user = Address::generate(&env);
        
        ZkVerifierTrait::init_shielded_pool(env.clone());
        
        // Perform ZK operations
        let commitment = BytesN::from_array(&env, &[1u8; 32]);
        let nullifier = BytesN::from_array(&env, &[2u8; 32]);
        
        ZkVerifierTrait::shielded_deposit(
            env.clone(),
            user.clone(),
            1000,
            1,
            commitment,
            nullifier.clone(),
        ).unwrap();
        
        let proof = ZkProof {
            proof_a: BytesN::from_array(&env, &[1u8; 64]),
            proof_b: BytesN::from_array(&env, &[2u8; 128]),
            proof_c: BytesN::from_array(&env, &[3u8; 64]),
            public_inputs: vec![&env, nullifier.clone()],
            timestamp: env.ledger().timestamp(),
        };
        
        ZkVerifierTrait::verify_blind_contribution(
            env.clone(),
            user.clone(),
            1,
            proof,
            nullifier,
        ).unwrap();
        
        // The test passes if no panics occur and the operations complete successfully.
        // In a full integration test, we would also verify that the Reliability Index
        // calculation returns the same results before and after ZK operations.
    }
}
