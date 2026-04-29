//! # Vault Balance Invariant Helpers
//!
//! Issue #339 — High-Frequency Concurrent Payout Security Hardening
//!
//! This module provides pure, deterministic invariant-checkers used by the
//! concurrent-payout fuzz suite.  All functions are `#[cfg(test)]`-gated so
//! they have zero impact on the production WASM binary.
//!
//! ## Invariants proved by this module
//!
//! | ID | Invariant |
//! |----|-----------|
//! | V1 | `total_payout ≤ vault_balance` at every ledger |
//! | V2 | A second `finalize_round` call on the same circle MUST fail (double-payout block) |
//! | V3 | Per-circle storage entry count stays within Soroban's 64-entry instance limit |
//! | V4 | `last_payout_ledger` is committed **before** any token transfer (atomic ordering) |

#[cfg(test)]
pub mod vault_balance_invariant {
    // ── Soroban network constants ─────────────────────────────────────────────

    /// Maximum number of entries that may live in Soroban's *instance* storage
    /// per contract invocation without hitting the ledger-footprint budget.
    /// Soroban Protocol-21 limit: 64 read/write entries per transaction.
    pub const SOROBAN_MAX_INSTANCE_ENTRIES: u32 = 64;

    /// Minimum XLM balance required to keep a contract account alive (base reserve).
    /// 2 XLM in stroops.
    pub const SOROBAN_BASE_RESERVE_STROOPS: i128 = 20_000_000;

    /// Maximum realistic member count per circle before storage footprint exceeds
    /// safe limits for a single `finalize_round` call.
    pub const MAX_SAFE_MEMBER_COUNT: u32 = 50;

    // ── V1: Payout amount bounded by vault balance ────────────────────────────

    /// Returns `true` iff the proposed payout does not exceed the vault balance.
    ///
    /// # Invariant V1
    /// `payout_amount ≤ vault_balance` must hold at every ledger for the
    /// protocol to remain solvent.  This function is the single canonical
    /// implementation of that check — both fuzz tests and the contract runtime
    /// should use this identical arithmetic.
    #[inline]
    pub fn check_payout_within_balance(vault_balance: i128, payout_amount: i128) -> bool {
        // Negative balances are structurally impossible in a well-formed vault;
        // treat them as a hard violation so fuzz tests catch implementation bugs.
        if vault_balance < 0 || payout_amount < 0 {
            return false;
        }
        payout_amount <= vault_balance
    }

    /// Compute the *maximum* legally-dispersable amount given a vault balance,
    /// accounting for the minimum base-reserve that must remain locked.
    #[inline]
    pub fn max_dispersable(vault_balance: i128) -> i128 {
        vault_balance.saturating_sub(SOROBAN_BASE_RESERVE_STROOPS).max(0)
    }

    // ── V2: Double-payout structural block ────────────────────────────────────

    /// Returns `true` iff a payout at `current_ledger` would be a **duplicate**
    /// of a payout already recorded at `last_payout_ledger`.
    ///
    /// # Invariant V2
    /// `last_payout_ledger` is committed atomically *before* any token transfer.
    /// Any caller observing the same ledger as `last_payout_ledger` is in a
    /// race condition and must be rejected.
    #[inline]
    pub fn is_double_payout(last_payout_ledger: u64, current_ledger: u64) -> bool {
        last_payout_ledger == current_ledger
    }

    /// Returns `true` iff the state transition is safe (no double-payout risk).
    #[inline]
    pub fn check_no_double_payout(last_payout_ledger: u64, current_ledger: u64) -> bool {
        !is_double_payout(last_payout_ledger, current_ledger)
    }

    // ── V3: Soroban storage limit compliance ──────────────────────────────────

    /// Returns `true` iff `entry_count` fits within Soroban's instance-storage
    /// footprint budget for a single transaction.
    ///
    /// # Invariant V3
    /// Bulk-withdrawal operations must not read/write more than
    /// `SOROBAN_MAX_INSTANCE_ENTRIES` storage entries per call.  Exceeding this
    /// causes the transaction to be rejected by the network with
    /// `ExceededWorkLimit`.
    #[inline]
    pub fn check_soroban_storage_limit(entry_count: u32) -> bool {
        entry_count <= SOROBAN_MAX_INSTANCE_ENTRIES
    }

    /// Estimate the number of instance-storage entries touched by a
    /// `finalize_round` call for a group with `member_count` members.
    ///
    /// Breakdown (conservative upper bound):
    /// - 1  × `CircleCount`
    /// - 1  × `Circle(id)` (read + write)
    /// - N  × `Deposit(id, member)` reads (one per member)
    /// - 1  × `NonReentrant` guard (read + write)
    /// - 1  × `IsPaused` read
    ///
    /// Total = N + 4.
    #[inline]
    pub fn estimate_finalize_storage_entries(member_count: u32) -> u32 {
        member_count.saturating_add(4)
    }

    // ── V4: Atomic state commit ordering ─────────────────────────────────────

    /// Simulate the atomic commit sequence and return `true` iff
    /// `last_payout_ledger` was written before the token transfer.
    ///
    /// In Soroban, all storage writes within a single contract invocation are
    /// applied atomically at the end of the execution frame.  This function
    /// models the *logical ordering* that the contract must maintain:
    ///
    /// 1. Read current circle state
    /// 2. Assert `!is_round_finalized`
    /// 3. Write `is_round_finalized = true` and `last_payout_ledger`
    /// 4. Invoke token transfer
    ///
    /// If step 3 is missing or swapped with step 4 the contract is vulnerable
    /// to a re-entrancy / double-payout exploit.
    pub fn simulate_atomic_commit(
        is_round_finalized_before: bool,
        state_written_before_transfer: bool,
    ) -> Result<(), &'static str> {
        if is_round_finalized_before {
            return Err("Double-payout: round already finalized");
        }
        if !state_written_before_transfer {
            return Err("Invariant violation: state commit must precede token transfer");
        }
        Ok(())
    }

    // ── Congestion simulation helpers ─────────────────────────────────────────

    /// Model Stellar network congestion by producing a burst of ledger sequence
    /// numbers with variable gaps.  Returns a `Vec` of `(ledger_sequence,
    /// timestamp)` pairs simulating a 100-ledger window with random skips.
    pub fn generate_congested_ledger_sequence(
        base_ledger: u64,
        base_timestamp: u64,
        count: usize,
        skip_factor: u64, // 1 = no congestion, 5 = heavy congestion
    ) -> std::vec::Vec<(u64, u64)> {
        let mut seq = std::vec::Vec::with_capacity(count);
        let mut ledger = base_ledger;
        let mut ts = base_timestamp;
        for i in 0..count {
            // Under congestion, ledgers close faster but skip sequence numbers
            // as validators time-out on some slots.
            let ledger_gap = if i % 7 == 0 { skip_factor * 2 } else { 1 };
            let time_gap = if i % 7 == 0 { 5 } else { 6 }; // seconds per ledger
            ledger += ledger_gap;
            ts += time_gap;
            seq.push((ledger, ts));
        }
        seq
    }

    // ── Fuzz failure logger ───────────────────────────────────────────────────

    /// A lightweight record of a fuzz iteration that violated an invariant.
    #[derive(Debug, Clone)]
    pub struct FuzzFailureRecord {
        pub iteration: u64,
        pub vault_balance: i128,
        pub payout_amount: i128,
        pub last_payout_ledger: u64,
        pub current_ledger: u64,
        pub storage_entries: u32,
        pub violated_invariant: &'static str,
    }

    impl FuzzFailureRecord {
        /// Format the record as a human-readable log line suitable for
        /// inclusion in CI artefacts and bug reports.
        pub fn to_log_line(&self) -> std::string::String {
            format!(
                "[FUZZ FAILURE] iter={} invariant=\"{}\" \
                 vault_balance={} payout={} \
                 last_payout_ledger={} current_ledger={} \
                 storage_entries={}",
                self.iteration,
                self.violated_invariant,
                self.vault_balance,
                self.payout_amount,
                self.last_payout_ledger,
                self.current_ledger,
                self.storage_entries,
            )
        }
    }

    // ── Unit tests for the helpers themselves ─────────────────────────────────

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_check_payout_within_balance_normal() {
            assert!(check_payout_within_balance(1_000_000, 500_000));
            assert!(check_payout_within_balance(1_000_000, 1_000_000));
            assert!(!check_payout_within_balance(1_000_000, 1_000_001));
        }

        #[test]
        fn test_check_payout_negative_values_rejected() {
            assert!(!check_payout_within_balance(-1, 100));
            assert!(!check_payout_within_balance(100, -1));
        }

        #[test]
        fn test_max_dispersable_respects_reserve() {
            let balance = SOROBAN_BASE_RESERVE_STROOPS + 5_000_000;
            assert_eq!(max_dispersable(balance), 5_000_000);
        }

        #[test]
        fn test_max_dispersable_cannot_go_negative() {
            assert_eq!(max_dispersable(0), 0);
            assert_eq!(max_dispersable(SOROBAN_BASE_RESERVE_STROOPS - 1), 0);
        }

        #[test]
        fn test_double_payout_detection() {
            assert!(is_double_payout(42, 42));
            assert!(!is_double_payout(41, 42));
            assert!(!is_double_payout(0, 1));
        }

        #[test]
        fn test_storage_limit_boundary() {
            assert!(check_soroban_storage_limit(SOROBAN_MAX_INSTANCE_ENTRIES));
            assert!(!check_soroban_storage_limit(SOROBAN_MAX_INSTANCE_ENTRIES + 1));
        }

        #[test]
        fn test_estimate_storage_entries_worst_case_50_members() {
            let entries = estimate_finalize_storage_entries(50);
            assert!(
                check_soroban_storage_limit(entries),
                "50 members should stay within storage limit, got {} entries",
                entries
            );
        }

        #[test]
        fn test_atomic_commit_success() {
            let result = simulate_atomic_commit(false, true);
            assert!(result.is_ok());
        }

        #[test]
        fn test_atomic_commit_double_payout_blocked() {
            let result = simulate_atomic_commit(true, true);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), "Double-payout: round already finalized");
        }

        #[test]
        fn test_atomic_commit_wrong_ordering_blocked() {
            let result = simulate_atomic_commit(false, false);
            assert!(result.is_err());
        }

        #[test]
        fn test_congested_ledger_sequence_length() {
            let seq = generate_congested_ledger_sequence(1_000_000, 1_700_000_000, 100, 3);
            assert_eq!(seq.len(), 100);
        }

        #[test]
        fn test_congested_ledger_sequence_monotonic() {
            let seq = generate_congested_ledger_sequence(1_000_000, 1_700_000_000, 50, 5);
            for window in seq.windows(2) {
                assert!(
                    window[1].0 > window[0].0,
                    "Ledger sequence must be strictly increasing"
                );
                assert!(
                    window[1].1 > window[0].1,
                    "Timestamp must be strictly increasing"
                );
            }
        }

        #[test]
        fn test_fuzz_failure_record_log_line() {
            let rec = FuzzFailureRecord {
                iteration: 12345,
                vault_balance: 1_000_000,
                payout_amount: 1_000_001,
                last_payout_ledger: 0,
                current_ledger: 1,
                storage_entries: 20,
                violated_invariant: "V1",
            };
            let line = rec.to_log_line();
            assert!(line.contains("FUZZ FAILURE"));
            assert!(line.contains("12345"));
            assert!(line.contains("V1"));
        }
    }
}
