#![no_std]

use soroban_sdk::{Env};

// ---------------------------
// DATE STRUCT (SAFE MODEL)
// ---------------------------
#[derive(Clone, Debug, PartialEq)]
pub struct Date {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

// ---------------------------
// CORE: SAFE DEADLINE CALCULATION
// ---------------------------
pub fn add_days(mut date: Date, mut days: u32) -> Date {
    while days > 0 {
        let dim = days_in_month(date.year, date.month);

        if date.day < dim {
            date.day += 1;
        } else {
            date.day = 1;
            if date.month < 12 {
                date.month += 1;
            } else {
                date.month = 1;
                date.year += 1;
            }
        }

        days -= 1;
    }

    date
}

// ---------------------------
// DAYS IN MONTH (LEAP SAFE)
// ---------------------------
pub fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => panic!("Invalid month"),
    }
}

// ---------------------------
// LEAP YEAR CHECK
// ---------------------------
pub fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// ---------------------------
// DEADLINE FUNCTION
// ---------------------------
pub fn calculate_deadline(start: Date, contribution_interval_days: u32) -> Date {
    add_days(start, contribution_interval_days)
}

// ---------------------------
// UNIT TESTS
// ---------------------------
#[cfg(test)]
mod test {
    use super::*;

    // ---------------------------
    // FEB 29 (LEAP YEAR)
    // ---------------------------
    #[test]
    fn test_feb_29_leap_year() {
        let start = Date { year: 2024, month: 2, day: 28 };
        let result = calculate_deadline(start, 1);

        assert_eq!(result, Date { year: 2024, month: 2, day: 29 });
    }

    // ---------------------------
    // FEB NON-LEAP
    // ---------------------------
    #[test]
    fn test_feb_non_leap() {
        let start = Date { year: 2023, month: 2, day: 28 };
        let result = calculate_deadline(start, 1);

        assert_eq!(result, Date { year: 2023, month: 3, day: 1 });
    }

    // ---------------------------
    // MONTH OVERFLOW (31st → next month)
    // ---------------------------
    #[test]
    fn test_31st_overflow() {
        let start = Date { year: 2025, month: 1, day: 31 };
        let result = calculate_deadline(start, 1);

        assert_eq!(result, Date { year: 2025, month: 2, day: 1 });
    }

    // ---------------------------
    // APRIL (30 DAYS)
    // ---------------------------
    #[test]
    fn test_30_day_month() {
        let start = Date { year: 2025, month: 4, day: 30 };
        let result = calculate_deadline(start, 1);

        assert_eq!(result, Date { year: 2025, month: 5, day: 1 });
    }

    // ---------------------------
    // MULTI-DAY CROSS MONTH
    // ---------------------------
    #[test]
    fn test_multi_day_cross_month() {
        let start = Date { year: 2025, month: 1, day: 30 };
        let result = calculate_deadline(start, 5);

        assert_eq!(result, Date { year: 2025, month: 2, day: 4 });
    }

    // ---------------------------
    // YEAR ROLLOVER
    // ---------------------------
    #[test]
    fn test_year_rollover() {
        let start = Date { year: 2025, month: 12, day: 31 };
        let result = calculate_deadline(start, 1);

        assert_eq!(result, Date { year: 2026, month: 1, day: 1 });
    }

    // ---------------------------
    // LONG RANGE TEST (STRESS)
    // ---------------------------
    #[test]
    fn test_large_interval() {
        let start = Date { year: 2020, month: 1, day: 1 };
        let result = calculate_deadline(start, 365);

        assert_eq!(result.year, 2020 + 1); // handles leap internally
    }

    // ---------------------------
    // DST SAFE TEST (NO EFFECT)
    // ---------------------------
    #[test]
    fn test_dst_irrelevance() {
        // Blockchain timestamps are UTC → DST should NOT affect logic
        let start = Date { year: 2025, month: 3, day: 30 }; // DST region date
        let result = calculate_deadline(start, 1);

        assert_eq!(result, Date { year: 2025, month: 3, day: 31 });
    }
}