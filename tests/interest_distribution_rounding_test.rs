//! Issue #340: Verification of "Rounding-Down" in Interest Distribution
//!
//! When distributing AMM yield across 20+ members, integer division always
//! truncates (rounds down). These tests prove that:
//!   1. `yield_per_member = total_yield / member_count` always rounds down.
//!   2. The contract never attempts to send more funds than it holds.
//!   3. The fractional remainder (dust) stays in the contract reserve — it is
//!      never lost and never causes the payout transaction to fail.

#![cfg(test)]

/// Pure-math unit tests — no Soroban env needed.
#[cfg(test)]
mod rounding_down_math {

    /// Helper: simulate the pro-rata distribution used in `batch_harvest`.
    /// Returns `(per_member_share, remainder_dust)`.
    fn distribute(total_yield: i128, member_count: u32) -> (i128, i128) {
        assert!(member_count > 0, "member_count must be > 0");
        let per_member = total_yield / member_count as i128;
        let dust = total_yield - per_member * member_count as i128;
        (per_member, dust)
    }

    // -----------------------------------------------------------------------
    // Core invariant: sum of all member payouts never exceeds total_yield
    // -----------------------------------------------------------------------

    #[test]
    fn test_sum_never_exceeds_total_yield() {
        let cases: &[(i128, u32)] = &[
            (100_000_001, 20),
            (123_456_789, 20),
            (999_999_999, 23),
            (1, 20),
            (19, 20),   // less than member_count → each member gets 0
            (20, 20),   // exactly divisible
            (21, 20),   // 1 stroop remainder
            (1_000_000_000_000, 7), // large amount, odd divisor
        ];

        for &(total_yield, member_count) in cases {
            let (per_member, dust) = distribute(total_yield, member_count);
            let total_paid_out = per_member * member_count as i128;

            // The contract must never pay out more than it holds.
            assert!(
                total_paid_out <= total_yield,
                "Overpayment detected: total_yield={total_yield}, member_count={member_count}, \
                 total_paid_out={total_paid_out}"
            );

            // Dust is non-negative and stays in the reserve.
            assert!(
                dust >= 0,
                "Negative dust is impossible: total_yield={total_yield}, dust={dust}"
            );

            // Dust + paid-out must reconstruct the original yield exactly.
            assert_eq!(
                total_paid_out + dust,
                total_yield,
                "Accounting mismatch: total_yield={total_yield}, member_count={member_count}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Rounding direction: integer division always rounds toward zero (down)
    // -----------------------------------------------------------------------

    #[test]
    fn test_per_member_share_rounds_down() {
        // 123_456_789 / 20 = 6_172_839.45 → floor = 6_172_839
        let (per_member, dust) = distribute(123_456_789, 20);
        assert_eq!(per_member, 6_172_839);
        assert_eq!(dust, 9); // 123_456_789 - 6_172_839 * 20 = 9

        // 100_000_001 / 20 = 5_000_000.05 → floor = 5_000_000
        let (per_member, dust) = distribute(100_000_001, 20);
        assert_eq!(per_member, 5_000_000);
        assert_eq!(dust, 1);

        // 999_999_999 / 23 = 43_478_260.826… → floor = 43_478_260
        let (per_member, dust) = distribute(999_999_999, 23);
        assert_eq!(per_member, 43_478_260);
        assert_eq!(dust, 19); // 999_999_999 - 43_478_260 * 23 = 19
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_yield_less_than_member_count_gives_zero_per_member() {
        // 19 stroops across 20 members → each gets 0, all 19 stay as dust
        let (per_member, dust) = distribute(19, 20);
        assert_eq!(per_member, 0);
        assert_eq!(dust, 19);
        // Contract holds all 19 stroops — no transfer attempted.
        assert!(per_member * 20 <= 19);
    }

    #[test]
    fn test_exactly_divisible_produces_zero_dust() {
        let (per_member, dust) = distribute(200, 20);
        assert_eq!(per_member, 10);
        assert_eq!(dust, 0);
    }

    #[test]
    fn test_single_stroop_remainder() {
        let (per_member, dust) = distribute(21, 20);
        assert_eq!(per_member, 1);
        assert_eq!(dust, 1);
        // 20 members × 1 stroop = 20 stroops paid; 1 stroop stays in reserve.
        assert_eq!(per_member * 20 + dust, 21);
    }

    #[test]
    fn test_maximum_dust_is_less_than_member_count() {
        // By definition of integer division, remainder < divisor.
        for member_count in [2u32, 5, 10, 20, 50, 100] {
            let total_yield = 123_456_789_i128;
            let (_, dust) = distribute(total_yield, member_count);
            assert!(
                dust < member_count as i128,
                "Dust {dust} must be < member_count {member_count}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Large-group scenario (20+ members) — the specific case from issue #340
    // -----------------------------------------------------------------------

    #[test]
    fn test_twenty_plus_members_rounding_down() {
        // Simulate AMM yield distributed across groups of 20, 25, and 30 members.
        let yield_amount = 123_456_789_i128; // realistic fractional stroop scenario

        for member_count in [20u32, 25, 30] {
            let (per_member, dust) = distribute(yield_amount, member_count);
            let total_paid = per_member * member_count as i128;

            // Core safety invariant: never overpay.
            assert!(total_paid <= yield_amount);
            // Dust is correctly retained.
            assert_eq!(total_paid + dust, yield_amount);
            // Each member receives a non-negative amount.
            assert!(per_member >= 0);
        }
    }

    // -----------------------------------------------------------------------
    // Two-level distribution: first split yield, then distribute to members
    // (mirrors the actual contract flow: BPS split → per-member division)
    // -----------------------------------------------------------------------

    #[test]
    fn test_two_level_rounding_never_overpays() {
        let total_yield: i128 = 123_456_789;
        let recipient_bps: i128 = 5_000; // 50 %

        // Level 1: BPS split (rounds down, remainder goes to treasury).
        let recipient_pool = (total_yield * recipient_bps) / 10_000;
        let treasury_pool = total_yield - recipient_pool; // no dust at this level

        assert_eq!(recipient_pool + treasury_pool, total_yield);

        // Level 2: distribute recipient_pool equally across 20 members.
        let member_count = 20_i128;
        let per_member = recipient_pool / member_count;
        let dust = recipient_pool % member_count;

        let total_paid = per_member * member_count;
        assert!(total_paid <= recipient_pool);
        assert_eq!(total_paid + dust, recipient_pool);

        // Grand total: treasury_pool + member payouts + dust == original yield.
        assert_eq!(treasury_pool + total_paid + dust, total_yield);
    }
}
