//! Utilities for handing std::time::SystemTime
//!
//! This is implemented as a facade around std::time::SystemTime mainly so that
//! we can implement a Display trait for SystemTime

use std::{
    fmt, result,
    time::{SystemTime, UNIX_EPOCH},
};

pub use error::{Error, Result};

mod error;

/// Months
#[derive(Debug, Clone, Copy)]
pub enum Month {
    January = 1,
    February = 2,
    March = 3,
    April = 4,
    May = 5,
    June = 6,
    July = 7,
    August = 8,
    September = 9,
    October = 10,
    November = 11,
    December = 12,
}

impl Month {
    pub fn next_month(&self) -> Month {
        match self {
            Month::January => Month::February,
            Month::February => Month::March,
            Month::March => Month::April,
            Month::April => Month::May,
            Month::May => Month::June,
            Month::June => Month::July,
            Month::July => Month::August,
            Month::August => Month::September,
            Month::September => Month::October,
            Month::October => Month::November,
            Month::November => Month::December,
            Month::December => Month::January,
        }
    }
}

// Implement TryFrom<u8> for Month
impl TryFrom<u8> for Month {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Month::January),
            2 => Ok(Month::February),
            3 => Ok(Month::March),
            4 => Ok(Month::April),
            5 => Ok(Month::May),
            6 => Ok(Month::June),
            7 => Ok(Month::July),
            8 => Ok(Month::August),
            9 => Ok(Month::September),
            10 => Ok(Month::October),
            11 => Ok(Month::November),
            12 => Ok(Month::December),
            _ => Err(Error::InvalidMonth),
        }
    }
}

// Implement From<Month> for u8
impl From<Month> for u8 {
    fn from(month: Month) -> Self {
        month as u8
    }
}

/// System date and time
#[derive(Debug, Clone, Copy)]
pub struct DateTime {
    epoch_seconds: u64,
    epoch_sub_nanoseconds: u32,
    epoch_days: u64,
    year: u16,
    day_of_year: u16,
    month: Month,
    day: u8,
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let month_num: u8 = self.month.into();
        write!(
            f,
            "{:04}/{:02}/{:02}, Day {}, ({} sec {} ns, day {}, from epoch)",
            self.year,
            month_num,
            self.day,
            self.day_of_year,
            self.epoch_seconds,
            self.epoch_sub_nanoseconds,
            self.epoch_days
        )
    }
}

/// Determine if a year is a leap year
///
/// Any year prior to 1582 when the Gregorian calendar was adopted is returned as 'false' since leap years did
/// not exist prior to its adoption. For years beyond 1582, the following rules are followed:
///
///   div 4  | div 100 | div 400 |  leap?  | example
/// ---------+---------+---------+---------+---------
///     F    |    F    |    F    |    F    |  2019
///     T    |    F    |    F    |    T    |  2020
///     -    |    T    |    F    |    F    |  1900
///     -    |    -    |    T    |    T    |  2000
pub fn is_leap_year<T>(year: T) -> bool
where
    T: Into<u16>,
{
    let year = year.into();
    if year < 1582 || year % 4 != 0 {
        return false;
    }
    if year % 100 != 0 {
        return true;
    }
    year % 400 == 0
}

/// Determine the number of days in a month of a given year
#[allow(dead_code)]
pub fn days_in_month<T, U>(year: T, month: U) -> u8
where
    T: Into<u16>,
    U: Into<Month>,
{
    match month.into() {
        Month::January => 31,
        Month::February => {
            if is_leap_year(year.into()) {
                29
            } else {
                28
            }
        }
        Month::March => 31,
        Month::April => 30,
        Month::May => 31,
        Month::June => 30,
        Month::July => 31,
        Month::August => 31,
        Month::September => 30,
        Month::October => 31,
        Month::November => 30,
        Month::December => 31,
    }
}

/// Convert an epoch day count to a proleptic Gregorian calendar date.
///
/// Uses Howard Hinnant's O(1) civil_from_days algorithm.
/// Source: https://howardhinnant.github.io/date_algorithms.html
///
/// Returns (year, month, day) where month is 1-based.
fn civil_from_days(epoch_days: i64) -> (i32, u8, u8) {
    let z = epoch_days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u8, d as u8)
}

impl DateTime {
    /// Retrieves the current time
    pub fn now() -> Result<DateTime> {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH)?;
        let epoch_seconds = duration.as_secs();
        let epoch_days = (epoch_seconds / 86_400) as i64;
        let (year, month_num, day) = civil_from_days(epoch_days);
        let month = Month::try_from(month_num)?;
        // Compute day_of_year using a cumulative days-before-month lookup (O(1), no loop).
        let days_before_month: [u16; 13] =
            [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let leap = is_leap_year(year as u16);
        let doy = days_before_month[month_num as usize]
            + if leap && month_num > 2 { 1 } else { 0 }
            + day as u16;
        Ok(DateTime {
            epoch_seconds,
            epoch_sub_nanoseconds: duration.subsec_nanos(),
            epoch_days: epoch_days as u64,
            year: year as u16,
            day_of_year: doy,
            month,
            day,
        })
    }

    /// The year
    #[allow(dead_code)]
    pub fn year(&self) -> u16 {
        self.year
    }

    /// The month
    #[allow(dead_code)]
    pub fn month(&self) -> Month {
        self.month
    }

    /// The day
    #[allow(dead_code)]
    pub fn day(&self) -> u8 {
        self.day
    }

    /// The day of the year
    #[allow(dead_code)]
    pub fn day_of_year(&self) -> u16 {
        self.day_of_year
    }

    /// Is it a leap year
    #[allow(dead_code)]
    pub fn is_leap_year(&self) -> bool {
        is_leap_year(self.year)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_year() {
        // not leap year - div 100 true, div 400 false
        assert!(!is_leap_year(1900u16));
        // leap year - div 400 true
        assert!(is_leap_year(2000u16));
        // not leap year - div 4 false
        assert!(!is_leap_year(2019u16));
        // leap year - div 4 true, div 100 false
        assert!(is_leap_year(2020u16));
    }

    #[test]
    fn civil_from_days_epoch_zero() {
        // 1970-01-01
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_y2k() {
        // 2000-01-01 = epoch day 10957
        assert_eq!(civil_from_days(10957), (2000, 1, 1));
    }

    #[test]
    fn civil_from_days_2002() {
        // 2002-01-01 = epoch day 11688
        assert_eq!(civil_from_days(11688), (2002, 1, 1));
    }

    #[test]
    fn datetime_now_returns_result() {
        // DateTime::now() must succeed on normal hardware; just verify it compiles and Ok
        let result = DateTime::now();
        assert!(result.is_ok());
    }
}
