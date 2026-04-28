use soroban_sdk::{Address, Env, Symbol, Vec, testutils::{Address as TestAddress, Logs}};
use crate::{
    SoroSusu, SoroSusuTrait, DataKey, AnchorInfo, AnchorStatus, AnchorDeposit, 
    DepositStatus, AnchorDepositConfig, UserBankPreference, PayoutMethod
};

#[test]
fn test_anchor_registration() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let anchor = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Register anchor
    let supported_tokens = Vec::new(&env);
    supported_tokens.push_back(token.clone());
    
    SoroSusu::register_anchor(
        env.clone(),
        admin.clone(),
        anchor.clone(),
        Symbol::short("TestAnchor"),
        Symbol::short("SEP-24"),
        true,
        supported_tokens,
        1000, // max_deposit_amount
        5000, // daily_deposit_limit
    );
    
    // Verify anchor registration
    let anchor_info = SoroSusu::get_anchor_info(env.clone(), anchor.clone());
    assert_eq!(anchor_info.address, anchor);
    assert_eq!(anchor_info.name, Symbol::short("TestAnchor"));
    assert_eq!(anchor_info.sep_version, Symbol::short("SEP-24"));
    assert_eq!(anchor_info.status, AnchorStatus::Active);
    assert!(anchor_info.kyc_required);
    
    // Verify anchor is in registry
    let registered_anchors = SoroSusu::get_registered_anchors(env.clone());
    assert!(registered_anchors.contains(&anchor));
}

#[test]
fn test_anchor_deposit_for_user() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let anchor = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Register anchor
    let mut supported_tokens = Vec::new(&env);
    supported_tokens.push_back(token.clone());
    
    SoroSusu::register_anchor(
        env.clone(),
        admin.clone(),
        anchor.clone(),
        Symbol::short("TestAnchor"),
        Symbol::short("SEP-24"),
        true,
        supported_tokens,
        1000,
        5000,
    );
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100, // contribution_amount
        5,   // max_members
        token.clone(),
        604800, // cycle_duration (1 week)
        100,    // insurance_fee_bps (1%)
        nft_contract,
        0,       // organizer_fee_bps
    );
    
    // Join circle
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    
    // Perform anchor deposit
    SoroSusu::deposit_for_user(
        env.clone(),
        anchor.clone(),
        user.clone(),
        circle_id,
        100, // amount
        token.clone(),
        Symbol::short("FIAT_REF_123"),
    );
    
    // Verify deposit was processed
    let member_key = DataKey::Member(user.clone());
    let member = env.storage().instance().get::<DataKey, crate::Member>(&member_key).unwrap();
    assert_eq!(member.contribution_count, 1);
}

#[test]
fn test_sbt_system_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let sbt_contract = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Initialize SBT system
    SoroSusu::initialize_sbt_system(env.clone(), admin.clone(), sbt_contract.clone());
    
    // Verify default milestone was created
    let milestone = SoroSusu::get_reputation_milestone(env.clone(), 1);
    assert_eq!(milestone.id, 1);
    assert_eq!(milestone.name, Symbol::short("Reliable_Saver"));
    assert_eq!(milestone.required_cycles, 5);
    assert_eq!(milestone.min_reputation_score, 80);
    assert!(milestone.is_active);
}

#[test]
fn test_sbt_credential_issuance() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let sbt_contract = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Initialize SBT system
    SoroSusu::initialize_sbt_system(env.clone(), admin.clone(), sbt_contract.clone());
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100, // contribution_amount
        5,   // max_members
        token.clone(),
        604800, // cycle_duration (1 week)
        100,    // insurance_fee_bps (1%)
        nft_contract,
        0,       // organizer_fee_bps
    );
    
    // Join circle and make contributions to meet milestone requirements
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    
    // Simulate 5 contributions (this would normally happen through regular deposits)
    for _ in 0..5 {
        SoroSusu::deposit(env.clone(), user.clone(), circle_id, 100);
    }
    
    // Issue SBT credential
    SoroSusu::issue_sbt_credential(
        env.clone(),
        admin.clone(),
        user.clone(),
        1, // milestone_id
    );
    
    // Verify SBT was issued
    let user_sbt = SoroSusu::get_user_sbt(env.clone(), user.clone()).unwrap();
    assert_eq!(user_sbt.owner, user);
    assert_eq!(user_sbt.milestone_id, 1);
    assert_eq!(user_sbt.status, SbtStatus::Active);
    assert!(user_sbt.cycles_completed >= 5);
}

#[test]
fn test_sbt_revocation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let sbt_contract = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Initialize SBT system
    SoroSusu::initialize_sbt_system(env.clone(), admin.clone(), sbt_contract.clone());
    
    // Create and join circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100,
        5,
        token.clone(),
        604800,
        100,
        nft_contract,
        0,
    );
    
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    
    // Make contributions and issue SBT
    for _ in 0..5 {
        SoroSusu::deposit(env.clone(), user.clone(), circle_id, 100);
    }
    
    SoroSusu::issue_sbt_credential(env.clone(), admin.clone(), user.clone(), 1);
    
    // Revoke SBT
    SoroSusu::revoke_sbt_credential(
        env.clone(),
        admin.clone(),
        user.clone(),
        Symbol::short("Default_detected"),
    );
    
    // Verify SBT was revoked
    let user_sbt = SoroSusu::get_user_sbt(env.clone(), user.clone()).unwrap();
    assert_eq!(user_sbt.status, SbtStatus::Revoked);
}

#[test]
fn test_anchor_deposit_limits() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let anchor = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Register anchor with low deposit limit
    let mut supported_tokens = Vec::new(&env);
    supported_tokens.push_back(token.clone());
    
    SoroSusu::register_anchor(
        env.clone(),
        admin.clone(),
        anchor.clone(),
        Symbol::short("TestAnchor"),
        Symbol::short("SEP-24"),
        true,
        supported_tokens,
        500, // max_deposit_amount (low limit)
        5000,
    );
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100,
        5,
        token.clone(),
        604800,
        100,
        nft_contract,
        0,
    );
    
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    
    // Try to deposit more than the limit - should fail
    let result = env.try_invoke_contract::<SoroSusuTrait>(
        &env.current_contract_address(),
        &SoroSusuTrait::deposit_for_user,
        (
            anchor.clone(),
            user.clone(),
            circle_id,
            1000, // amount exceeding limit
            token.clone(),
            Symbol::short("FIAT_REF_123"),
        ),
    );
    
    assert!(result.is_err());
}

#[test]
fn test_payout_preference_setting() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let anchor = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100,
        5,
        token.clone(),
        604800,
        true,
        1,
        86400,
        100,
    );
    
    // Set up anchor config
    let anchor_config = AnchorDepositConfig {
        preferred_anchor: anchor.clone(),
        bank_account_hash: 12345,
        mobile_money_provider: Symbol::short(&env, "M-Pesa"),
        mobile_number_hash: 67890,
        fiat_currency: Symbol::short(&env, "KES"),
        auto_convert: true,
    };
    
    // Set payout preference to Direct-to-Bank
    SoroSusu::set_payout_preference(
        env.clone(),
        user.clone(),
        circle_id,
        PayoutMethod::DirectToBank,
        Some(anchor_config.clone()),
    );
    
    // Verify preference was set
    let preference = SoroSusu::get_payout_preference(env.clone(), user.clone(), circle_id);
    assert_eq!(preference.payout_method, PayoutMethod::DirectToBank);
    assert!(preference.anchor_config.is_some());
    
    let config = preference.anchor_config.unwrap();
    assert_eq!(config.preferred_anchor, anchor);
    assert_eq!(config.bank_account_hash, 12345);
    assert_eq!(config.mobile_money_provider, Symbol::short(&env, "M-Pesa"));
}

#[test]
fn test_direct_to_bank_payout() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let anchor = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Register anchor
    let mut supported_tokens = Vec::new(&env);
    supported_tokens.push_back(token.clone());
    
    SoroSusu::register_anchor(
        env.clone(),
        admin.clone(),
        anchor.clone(),
        Symbol::short(&env, "TestAnchor"),
        Symbol::short(&env, "SEP-24"),
        false, // No KYC required for test
        supported_tokens,
        10000,
        50000,
    );
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100,
        5,
        token.clone(),
        604800,
        true,
        1,
        86400,
        100,
    );
    
    // Join circle and make deposits
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    SoroSusu::deposit(env.clone(), user.clone(), circle_id, 100);
    
    // Set payout preference to Direct-to-Bank
    let anchor_config = AnchorDepositConfig {
        preferred_anchor: anchor.clone(),
        bank_account_hash: 12345,
        mobile_money_provider: Symbol::short(&env, "M-Pesa"),
        mobile_number_hash: 67890,
        fiat_currency: Symbol::short(&env, "KES"),
        auto_convert: true,
    };
    
    SoroSusu::set_payout_preference(
        env.clone(),
        user.clone(),
        circle_id,
        PayoutMethod::DirectToBank,
        Some(anchor_config),
    );
    
    // Process payout (this should route to anchor)
    SoroSusu::payout(env.clone(), admin.clone(), circle_id);
    
    // Verify anchor deposit was created
    // Note: In a real test, you'd check the anchor received the funds
    // For now, we just verify the function doesn't panic
}

#[test]
fn test_anchor_deposit_limits() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let anchor = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Register anchor with low limits
    let mut supported_tokens = Vec::new(&env);
    supported_tokens.push_back(token.clone());
    
    SoroSusu::register_anchor(
        env.clone(),
        admin.clone(),
        anchor.clone(),
        Symbol::short(&env, "LowLimitAnchor"),
        Symbol::short(&env, "SEP-24"),
        false,
        supported_tokens,
        500, // max_deposit_amount
        1000, // daily_deposit_limit
    );
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100,
        5,
        token.clone(),
        604800,
        true,
        1,
        86400,
        100,
    );
    
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    
    // Try to deposit more than max limit - should fail
    let result = env.try_invoke_contract::<SoroSusuTrait>(
        &env.current_contract_address(),
        &SoroSusuTrait::deposit_for_user,
        (
            anchor.clone(),
            user.clone(),
            circle_id,
            1000, // amount exceeding max_deposit_amount
            token.clone(),
            Symbol::short(&env, "TEST_REF"),
        ),
    );
    
    assert!(result.is_err());
}

#[test]
fn test_kyc_verification_required() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let anchor = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Register anchor with KYC requirement
    let mut supported_tokens = Vec::new(&env);
    supported_tokens.push_back(token.clone());
    
    SoroSusu::register_anchor(
        env.clone(),
        admin.clone(),
        anchor.clone(),
        Symbol::short(&env, "KYCAnchor"),
        Symbol::short(&env, "SEP-24"),
        true, // KYC required
        supported_tokens,
        10000,
        50000,
    );
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100,
        5,
        token.clone(),
        604800,
        true,
        1,
        86400,
        100,
    );
    
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    
    // Try to deposit without KYC - should fail
    let result = env.try_invoke_contract::<SoroSusuTrait>(
        &env.current_contract_address(),
        &SoroSusuTrait::deposit_for_user,
        (
            anchor.clone(),
            user.clone(),
            circle_id,
            100,
            token.clone(),
            Symbol::short(&env, "TEST_REF"),
        ),
    );
    
    assert!(result.is_err());
}

#[test]
fn test_fallback_to_direct_token_payout() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Create circle
    let circle_id = SoroSusu::create_circle(
        env.clone(),
        user.clone(),
        100,
        5,
        token.clone(),
        604800,
        true,
        1,
        86400,
        100,
    );
    
    // Join circle and make deposits
    SoroSusu::join_circle(env.clone(), user.clone(), circle_id);
    SoroSusu::deposit(env.clone(), user.clone(), circle_id, 100);
    
    // Set payout preference to Direct-to-Bank but without anchor config
    // This should fallback to direct token payout
    SoroSusu::set_payout_preference(
        env.clone(),
        user.clone(),
        circle_id,
        PayoutMethod::DirectToBank,
        None, // No anchor config
    );
    
    // Process payout (should fallback to direct token)
    SoroSusu::payout(env.clone(), admin.clone(), circle_id);
    
    // Verify preference is still Direct-to-Bank but payout worked
    let preference = SoroSusu::get_payout_preference(env.clone(), user.clone(), circle_id);
    assert_eq!(preference.payout_method, PayoutMethod::DirectToBank);
    assert!(preference.anchor_config.is_none());
}

#[test]
fn test_reputation_verification() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Initialize contract
    SoroSusu::init(env.clone(), admin.clone());
    
    // Verify user reputation (should be 0 for new user)
    let (reputation_score, has_sbt) = SoroSusu::verify_user_reputation(env.clone(), user.clone());
    assert_eq!(reputation_score, 0);
    assert!(!has_sbt);
}
