#[cfg(test)]
mod tax_withholding_tests {
    use super::*;
    use soroban_sdk::{Address, Env, Symbol};

    #[test]
    fn test_tax_configuration_setup() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000, // $10 contribution
            5,    // 5 members
            token.clone(),
            604800, // 1 week cycle
            true,  // yield enabled
            1,     // risk tolerance
            86400, // grace period
            100,   // late fee bps
        );

        // Configure tax settings
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 1500, // 15% tax rate
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0, // Will be set by function
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Verify tax configuration was stored
        let stored_config = SoroSusuTrait::get_tax_configuration(env.clone(), circle_id)
            .expect("Tax configuration should be stored");
        
        assert!(stored_config.enabled);
        assert_eq!(stored_config.tax_bps, 1500);
        assert_eq!(stored_config.tax_collector_address, tax_collector);
        assert!(!stored_config.jurisdiction_exempt);
        assert!(stored_config.reporting_enabled);
        assert!(stored_config.cycle_start_timestamp > 0); // Should be set to current time
    }

    #[test]
    fn test_tax_configuration_validation() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
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
            86400,
            100,
        );

        // Test tax rate too high (over 50%)
        let invalid_tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 6000, // 60% - should fail
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, invalid_tax_config);
        });
        assert!(result.is_err(), "Should panic when tax rate exceeds 50%");
    }

    #[test]
    fn test_tax_withholding_during_payout() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());
        env.mock_all_auths();

        // Create a circle
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1000, // $10 contribution per member
            5,    // 5 members
            token.clone(),
            604800,
            true,
            1,
            86400,
            100,
        );

        // Configure tax settings (20% tax)
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 2000, // 20% tax rate
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Add members (including the creator/recipient)
        for i in 0..5 {
            let member = if i == 0 { recipient.clone() } else { Address::generate(&env) };
            SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
            
            // Make deposits for all members
            SoroSusuTrait::deposit(env.clone(), member.clone(), circle_id, 1);
        }

        // Get initial tax pool state
        let initial_pool = SoroSusuTrait::get_tax_withholding_pool(env.clone());
        assert_eq!(initial_pool.total_collected, 0);
        assert_eq!(initial_pool.pending_distribution, 0);

        // Process payout (should trigger tax withholding)
        // Total payout: 5 members * 1000 = 5000
        // Tax: 5000 * 20% = 1000
        // Net: 5000 - 1000 = 4000
        SoroSusuTrait::payout(env.clone(), admin.clone(), circle_id);

        // Verify tax was collected
        let final_pool = SoroSusuTrait::get_tax_withholding_pool(env.clone());
        assert_eq!(final_pool.total_collected, 1000);
        assert_eq!(final_pool.pending_distribution, 1000);

        // Verify financial receipt was generated
        let receipt = SoroSusuTrait::get_financial_receipt(env.clone(), circle_id, recipient.clone())
            .expect("Financial receipt should be generated");
        
        assert_eq!(receipt.gross_amount, 5000);
        assert_eq!(receipt.tax_withheld, 1000);
        assert_eq!(receipt.net_amount, 4000);
        assert_eq!(receipt.recipient_address, recipient);
        assert_eq!(receipt.circle_id, circle_id);
    }

    #[test]
    fn test_jurisdiction_exemption() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
        let exempt_user = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());
        env.mock_all_auths();

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
            86400,
            100,
        );

        // Configure tax settings
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 2000, // 20% tax
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Set user as exempt from interest withholding
        SoroSusuTrait::set_jurisdiction_exemption(env.clone(), admin.clone(), exempt_user.clone(), true);

        // Add members (including exempt user as creator)
        for i in 0..5 {
            let member = if i == 0 { exempt_user.clone() } else { Address::generate(&env) };
            SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
            SoroSusuTrait::deposit(env.clone(), member.clone(), circle_id, 1);
        }

        // Process payout - exempt user should receive full amount
        SoroSusuTrait::payout(env.clone(), admin.clone(), circle_id);

        // Verify no tax was withheld for exempt user
        let receipt = SoroSusuTrait::get_financial_receipt(env.clone(), circle_id, exempt_user.clone())
            .expect("Financial receipt should be generated");
        
        assert_eq!(receipt.gross_amount, 5000);
        assert_eq!(receipt.tax_withheld, 0); // No tax withheld due to exemption
        assert_eq!(receipt.net_amount, 5000); // Full amount received
    }

    #[test]
    fn test_tax_funds_distribution() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());
        env.mock_all_auths();

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
            86400,
            100,
        );

        // Configure tax settings
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 2000, // 20% tax
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Add members and make deposits
        for i in 0..5 {
            let member = Address::generate(&env);
            SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
            SoroSusuTrait::deposit(env.clone(), member.clone(), circle_id, 1);
        }

        // Process payout to collect tax
        SoroSusuTrait::payout(env.clone(), admin.clone(), circle_id);

        // Distribute tax funds
        let distribution_result = SoroSusuTrait::distribute_tax_funds(env.clone(), admin.clone());
        assert!(distribution_result.is_ok(), "Tax distribution should succeed");
        
        let distributed_amount = distribution_result.unwrap();
        assert_eq!(distributed_amount, 1000, "Should distribute 1000 in tax funds");

        // Verify pool state after distribution
        let final_pool = SoroSusuTrait::get_tax_withholding_pool(env.clone());
        assert_eq!(final_pool.total_collected, 1000);
        assert_eq!(final_pool.total_distributed, 1000);
        assert_eq!(final_pool.pending_distribution, 0);
        assert!(final_pool.last_distribution_timestamp > 0);
    }

    #[test]
    fn test_tax_report_generation() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
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
            86400,
            100,
        );

        // Configure tax settings with reporting enabled
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 1500, // 15% tax
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Generate tax report for a period
        let current_time = env.ledger().timestamp();
        let report_result = SoroSusuTrait::generate_tax_report(
            env.clone(),
            admin.clone(),
            circle_id,
            current_time - 86400, // 1 day ago
            current_time,         // now
        );

        assert!(report_result.is_ok(), "Tax report generation should succeed");
        
        let report_id = report_result.unwrap();
        assert!(report_id > 0, "Report ID should be positive");

        // Retrieve and verify report data
        let report = SoroSusuTrait::get_tax_report_data(env.clone(), circle_id, report_id)
            .expect("Tax report should be retrievable");
        
        assert_eq!(report.circle_id, circle_id);
        assert_eq!(report.report_id, report_id);
        assert!(report.generated_timestamp > 0);
        assert!(!report.report_cid.is_empty());
    }

    #[test]
    fn test_tax_disabled_scenario() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());
        env.mock_all_auths();

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
            86400,
            100,
        );

        // Do NOT configure tax settings (tax disabled by default)

        // Add members and make deposits
        for i in 0..5 {
            let member = Address::generate(&env);
            SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
            SoroSusuTrait::deposit(env.clone(), member.clone(), circle_id, 1);
        }

        // Process payout - should work without tax
        SoroSusuTrait::payout(env.clone(), admin.clone(), circle_id);

        // Verify no tax was collected
        let pool = SoroSusuTrait::get_tax_withholding_pool(env.clone());
        assert_eq!(pool.total_collected, 0);
        assert_eq!(pool.pending_distribution, 0);

        // Verify financial receipt shows no tax
        let receipt = SoroSusuTrait::get_financial_receipt(env.clone(), circle_id, creator.clone())
            .expect("Financial receipt should be generated");
        
        assert_eq!(receipt.gross_amount, 5000);
        assert_eq!(receipt.tax_withheld, 0);
        assert_eq!(receipt.net_amount, 5000);
    }

    #[test]
    fn test_security_prevents_rate_change_during_cycle() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());
        env.mock_all_auths();

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
            86400,
            100,
        );

        // Configure initial tax settings
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 1000, // 10% tax
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Add members and make deposits
        for i in 0..5 {
            let member = Address::generate(&env);
            SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
            SoroSusuTrait::deposit(env.clone(), member.clone(), circle_id, 1);
        }

        // Process one payout to start the cycle
        SoroSusuTrait::payout(env.clone(), admin.clone(), circle_id);

        // Try to change tax rate during active cycle - should fail
        let new_tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 3000, // 30% tax - different rate
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        let result = std::panic::catch_unwind(|| {
            SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, new_tax_config);
        });
        assert!(result.is_err(), "Should prevent tax rate changes during active cycle");
    }

    #[test]
    fn test_financial_receipt_hash_generation() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());
        env.mock_all_auths();

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
            86400,
            100,
        );

        // Configure tax settings
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 2000,
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Add members and make deposits
        for i in 0..5 {
            let member = Address::generate(&env);
            SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
            SoroSusuTrait::deposit(env.clone(), member.clone(), circle_id, 1);
        }

        // Process payout
        SoroSusuTrait::payout(env.clone(), admin.clone(), circle_id);

        // Verify financial receipt hash is not zero
        let receipt = SoroSusuTrait::get_financial_receipt(env.clone(), circle_id, creator.clone())
            .expect("Financial receipt should be generated");
        
        let zero_hash = BytesN::from_array(&env, &[0u8; 32]);
        assert_ne!(receipt.receipt_hash, zero_hash, "Receipt hash should be computed and non-zero");
    }

    #[test]
    fn test_read_only_reporting_hook() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
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
            86400,
            100,
        );

        // Configure tax settings with reporting enabled
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 1500,
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Generate a tax report
        let current_time = env.ledger().timestamp();
        let report_id = SoroSusuTrait::generate_tax_report(
            env.clone(),
            admin.clone(),
            circle_id,
            current_time - 86400,
            current_time,
        ).expect("Report generation should succeed");

        // Test read-only access (should work without auth)
        let report_data = SoroSusuTrait::get_tax_report_data(env.clone(), circle_id, report_id)
            .expect("Should be able to read report data");

        // Verify data integrity
        assert_eq!(report_data.circle_id, circle_id);
        assert_eq!(report_data.report_id, report_id);

        // Test with wrong circle ID (should return None)
        let wrong_circle_data = SoroSusuTrait::get_tax_report_data(env.clone(), circle_id + 1, report_id);
        assert!(wrong_circle_data.is_none(), "Should return None for wrong circle ID");
    }

    #[test]
    fn test_tax_withholding_math_edge_cases() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let tax_collector = Address::generate(&env);
        let token = Address::generate(&env);

        // Initialize contract
        SoroSusuTrait::init(env.clone(), admin.clone());
        env.mock_all_auths();

        // Test with very small amounts
        let circle_id = SoroSusuTrait::create_circle(
            env.clone(),
            creator.clone(),
            1, // 1 stroop - minimal amount
            2, // 2 members
            token.clone(),
            604800,
            true,
            1,
            86400,
            100,
        );

        // Configure with 1% tax rate (100 bps)
        let tax_config = TaxConfiguration {
            enabled: true,
            tax_bps: 100, // 1%
            tax_collector_address: tax_collector.clone(),
            jurisdiction_exempt: false,
            cycle_start_timestamp: 0,
            sep40_oracle_address: None,
            reporting_enabled: true,
        };

        SoroSusuTrait::configure_tax_settings(env.clone(), admin.clone(), circle_id, tax_config);

        // Add members and make deposits
        for i in 0..2 {
            let member = Address::generate(&env);
            SoroSusuTrait::join_circle(env.clone(), member.clone(), circle_id, 1, None);
            SoroSusuTrait::deposit(env.clone(), member.clone(), circle_id, 1);
        }

        // Process payout
        SoroSusuTrait::payout(env.clone(), admin.clone(), circle_id);

        // Verify math: 2 * 1 = 2 total, 1% tax = 0.02 (integer division should handle this)
        let receipt = SoroSusuTrait::get_financial_receipt(env.clone(), circle_id, creator.clone())
            .expect("Financial receipt should be generated");
        
        assert_eq!(receipt.gross_amount, 2);
        assert!(receipt.tax_withheld >= 0, "Tax withheld should be non-negative");
        assert!(receipt.net_amount >= 0, "Net amount should be non-negative");
        assert!(receipt.net_amount <= receipt.gross_amount, "Net amount should not exceed gross amount");
    }
}
