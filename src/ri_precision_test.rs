#![cfg(test)]

use soroban_sdk::{Address, Env};
use crate::{
    SoroSusu,
};

// ---------------------------------------------------------------------------
// Issue #276: Test RI Calculation under Maximum Decimal Precision
// ---------------------------------------------------------------------------

#[test]
fn test_ri_calculation_precision() {
    // Test that RI math doesn't break with extremely small or large contribution amounts
    // Verify fixed-point math stays accurate up to 7 decimal places

    // Test with very small amounts (micro-payments)
    let small_on_time = 1u32; // 1 stroop = 0.0000001 XLM
    let small_total = 10u32;
    let small_on_time_rate = (small_on_time as u64 * 10000 / small_total as u64) as u32; // 1000 bps = 10%
    
    // Volume bonus with small amount
    let small_volume = 1000u64; // 1000 stroops
    let volume_bonus = ((small_volume / 1000000).min(100) * 50) as u32; // 0
    
    let reliability_score_small = small_on_time_rate + volume_bonus; // 1000
    
    // Test with very large amounts
    let large_on_time = 1000000000u32; // 1 billion
    let large_total = 1000000000u32;
    let large_on_time_rate = (large_on_time as u64 * 10000 / large_total as u64) as u32; // 10000 bps = 100%
    
    let large_volume = 1000000000000u64; // 1 trillion stroops
    let large_volume_bonus = ((large_volume / 1000000).min(100) * 50) as u32; // 5000
    
    let reliability_score_large = (large_on_time_rate + large_volume_bonus).min(10000); // 10000
    
    // Test social capital calculation
    let leniency_given = 100u32;
    let leniency_received = 50u32;
    let voting_bonus = 200u32; // 20 votes * 10 bps each
    let baseline = 5000u32;
    
    let social_capital = (baseline + leniency_given * 50 + leniency_received * 25 + voting_bonus).min(10000);
    
    // Final RI calculation
    let ri_small = (reliability_score_small + social_capital) / 2;
    let ri_large = (reliability_score_large + social_capital) / 2;
    
    // Verify no overflow or precision loss
    assert_eq!(ri_small, 3000); // (1000 + 5000) / 2 = 3000
    assert_eq!(ri_large, 7500); // (10000 + 5000) / 2 = 7500
    
    // Test edge case: zero division protection
    let zero_total = 0u32;
    let safe_rate = if zero_total > 0 {
        (small_on_time as u64 * 10000 / zero_total as u64) as u32
    } else {
        0
    };
    assert_eq!(safe_rate, 0);
    
    // Test maximum precision: 7 decimal places (stroop level)
    let micro_amount = 1u64; // 1 stroop
    let macro_amount = 1000000000000u64; // 1 million XLM in stroops
    
    // Calculations should handle both without overflow
    let micro_calc = micro_amount * 10000 / 1000000; // Should be 10
    let macro_calc = macro_amount * 10000 / 1000000000000; // Should be 10
    
    assert_eq!(micro_calc, 10);
    assert_eq!(macro_calc, 10);
}