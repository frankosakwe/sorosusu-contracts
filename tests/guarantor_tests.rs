use soroban_sdk::token::TokenClient;
use sorosusu_contracts::{SoroSusuClient, SoroSusuTrait, GuarantorStatus, VoucherStatus};

#[cfg(test)]
mod tests {
    use soroban_sdk::{Env, Address};
    use sorosusu_contracts::{SoroSusuClient, SoroSusuTrait, GuarantorStatus, VoucherStatus};

    #[test]
    fn test_guarantor_registration() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let guarantor = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        // Initialize contract
        client.init(&admin);
        
        // Register guarantor with initial collateral
        let initial_collateral = 1000;
        
        // Mock token transfer for initial collateral
        let token_client = TokenClient::new(&env, &token);
        token_client.mint(&guarantor, &initial_collateral);
        
        client.register_guarantor(&guarantor, &initial_collateral);
        
        // Verify guarantor info
        let guarantor_info = client.get_guarantor_info(&guarantor);
        assert_eq!(guarantor_info.address, guarantor);
        assert_eq!(guarantor_info.reputation_score, 100); // MIN_GUARANTOR_REPUTATION
        assert_eq!(guarantor_info.status, GuarantorStatus::Active);
        assert_eq!(guarantor_info.vault_balance, initial_collateral);
        
        // Test duplicate registration
        let result = env.as_contract(&contract_id, || {
            client.try_register_guarantor(&guarantor, &500)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_guarantor_reputation_update() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let guarantor = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        // Initialize and register guarantor
        client.init(&admin);
        
        let token_client = TokenClient::new(&env, &token);
        token_client.mint(&guarantor, &1000);
        client.register_guarantor(&guarantor, &1000);
        
        // Update reputation as admin
        client.update_guarantor_reputation(&admin, &guarantor, &200);
        
        let guarantor_info = client.get_guarantor_info(&guarantor);
        assert_eq!(guarantor_info.reputation_score, 200);
        assert_eq!(guarantor_info.status, GuarantorStatus::Active);
        
        // Test reputation below minimum
        client.update_guarantor_reputation(&admin, &guarantor, &50);
        let guarantor_info = client.get_guarantor_info(&guarantor);
        assert_eq!(guarantor_info.status, GuarantorStatus::Suspended);
        
        // Test unauthorized update
        let unauthorized = Address::generate(&env);
        let result = env.as_contract(&contract_id, || {
            client.try_update_guarantor_reputation(&unauthorized, &guarantor, &150)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_voucher_creation() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let guarantor = Address::generate(&env);
        let member = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        // Initialize contract and register guarantor
        client.init(&admin);
        
        let token_client = TokenClient::new(&env, &token);
        token_client.mint(&guarantor, &2000);
        client.register_guarantor(&guarantor, &2000);
        
        // Create a high-value circle (requires collateral)
        let circle_id = client.create_circle(
            &admin,
            &1000, // contribution amount
            &5,     // max members
            &token,
            &1000,  // cycle duration
            &100,   // insurance fee bps
            &Address::generate(&env), // nft contract
        );
        
        // Join member first
        client.join_circle(&member, &circle_id, &1, &None);
        
        // Create voucher
        let vouched_amount = 1000;
        client.create_voucher(&guarantor, &member, &circle_id, &vouched_amount);
        
        // Verify voucher info
        let voucher_info = client.get_voucher_info(&guarantor, &circle_id);
        assert_eq!(voucher_info.guarantor, guarantor);
        assert_eq!(voucher_info.member, member);
        assert_eq!(voucher_info.circle_id, circle_id);
        assert_eq!(voucher_info.vouched_amount, vouched_amount);
        assert_eq!(voucher_info.status, VoucherStatus::Active);
        
        // Verify member has guarantor
        let member_guarantor = client.get_member_guarantor(&member);
        assert_eq!(member_guarantor, Some(guarantor));
        
        // Verify guarantor stats updated
        let guarantor_info = client.get_guarantor_info(&guarantor);
        assert_eq!(guarantor_info.active_vouchers_count, 1);
        assert_eq!(guarantor_info.total_vouched_amount, vouched_amount);
    }

    #[test]
    fn test_voucher_constraints() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let guarantor = Address::generate(&env);
        let member1 = Address::generate(&env);
        let member2 = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        // Initialize and register guarantor
        client.init(&admin);
        
        let token_client = TokenClient::new(&env, &token);
        token_client.mint(&guarantor, &10000);
        client.register_guarantor(&guarantor, &10000);
        
        // Create circles
        let circle_id1 = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        let circle_id2 = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        let circle_id3 = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        let circle_id4 = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        let circle_id5 = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        let circle_id6 = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        
        // Join members
        client.join_circle(&member1, &circle_id1, &1, &None);
        client.join_circle(&member2, &circle_id2, &1, &None);
        
        // Create maximum vouchers (5)
        client.create_voucher(&guarantor, &member1, &circle_id1, &1000);
        client.create_voucher(&guarantor, &member2, &circle_id2, &1000);
        
        // Try to create 6th voucher (should fail)
        let result = env.as_contract(&contract_id, || {
            client.try_create_voucher(&guarantor, &Address::generate(&env), &circle_id3, &1000)
        });
        assert!(result.is_err());
        
        // Test self-guarantee (should fail)
        let result = env.as_contract(&contract_id, || {
            client.try_create_voucher(&guarantor, &guarantor, &circle_id4, &1000)
        });
        assert!(result.is_err());
        
        // Test insufficient reputation
        client.update_guarantor_reputation(&admin, &guarantor, &50); // Below MIN_GUARANTOR_REPUTATION
        let result = env.as_contract(&contract_id, || {
            client.try_create_voucher(&guarantor, &Address::generate(&env), &circle_id5, &1000)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_voucher_claim_on_default() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let guarantor = Address::generate(&env);
        let member = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        // Initialize and register guarantor
        client.init(&admin);
        
        let token_client = TokenClient::new(&env, &token);
        token_client.mint(&guarantor, &5000);
        client.register_guarantor(&guarantor, &5000);
        
        // Create circle and setup voucher
        let circle_id = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        client.join_circle(&member, &circle_id, &1, &None);
        client.create_voucher(&guarantor, &member, &circle_id, &1000);
        
        let initial_guarantor_balance = client.get_guarantor_vault_balance(&guarantor);
        let initial_guarantor_info = client.get_guarantor_info(&guarantor);
        
        // Mark member as defaulted
        client.mark_member_defaulted(&admin, &circle_id, &member);
        
        // Verify voucher was claimed automatically
        let voucher_info = client.get_voucher_info(&guarantor, &circle_id);
        assert_eq!(voucher_info.status, VoucherStatus::Claimed);
        assert!(voucher_info.claimed_timestamp.is_some());
        
        // Verify guarantor stats updated
        let guarantor_info = client.get_guarantor_info(&guarantor);
        assert_eq!(guarantor_info.active_vouchers_count, initial_guarantor_info.active_vouchers_count - 1);
        assert_eq!(guarantor_info.claimed_vouchers, initial_guarantor_info.claimed_vouchers + 1);
        assert!(guarantor_info.vault_balance < initial_guarantor_balance);
    }

    #[test]
    fn test_guarantor_collateral_management() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let guarantor = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        // Initialize and register guarantor
        client.init(&admin);
        
        let token_client = TokenClient::new(&env, &token);
        token_client.mint(&guarantor, &3000);
        client.register_guarantor(&guarantor, &1000);
        
        let initial_balance = client.get_guarantor_vault_balance(&guarantor);
        assert_eq!(initial_balance, 1000);
        
        // Add more collateral
        client.add_guarantor_collateral(&guarantor, &500);
        assert_eq!(client.get_guarantor_vault_balance(&guarantor), 1500);
        
        // Withdraw some collateral
        client.withdraw_guarantor_collateral(&guarantor, &200);
        assert_eq!(client.get_guarantor_vault_balance(&guarantor), 1300);
        
        // Test invalid amounts
        let result = env.as_contract(&contract_id, || {
            client.try_add_guarantor_collateral(&guarantor, &0)
        });
        assert!(result.is_err());
        
        let result = env.as_contract(&contract_id, || {
            client.try_withdraw_guarantor_collateral(&guarantor, &0)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_query_functions() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let guarantor = Address::generate(&env);
        let member = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, SoroSusu);
        let client = SoroSusuClient::new(&env, &contract_id);
        
        // Initialize and register guarantor
        client.init(&admin);
        
        let token_client = TokenClient::new(&env, &token);
        token_client.mint(&guarantor, &2000);
        client.register_guarantor(&guarantor, &2000);
        
        // Create circle and voucher
        let circle_id = client.create_circle(&admin, &1000, &5, &token, &1000, &100, &Address::generate(&env));
        client.join_circle(&member, &circle_id, &1, &None);
        client.create_voucher(&guarantor, &member, &circle_id, &1000);
        
        // Test query functions
        let guarantor_info = client.get_guarantor_info(&guarantor);
        assert_eq!(guarantor_info.address, guarantor);
        
        let voucher_info = client.get_voucher_info(&guarantor, &circle_id);
        assert_eq!(voucher_info.member, member);
        
        let member_guarantor = client.get_member_guarantor(&member);
        assert_eq!(member_guarantor, Some(guarantor));
        
        let vault_balance = client.get_guarantor_vault_balance(&guarantor);
        assert!(vault_balance > 0);
        
        // Test queries for non-existent entities
        let non_existent = Address::generate(&env);
        let result = env.as_contract(&contract_id, || {
            client.try_get_guarantor_info(&non_existent)
        });
        assert!(result.is_err());
        
        let result = env.as_contract(&contract_id, || {
            client.try_get_voucher_info(&guarantor, &999)
        });
        assert!(result.is_err());
    }
}
