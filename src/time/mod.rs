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
/// ```
///   div 4  | div 100 | div 400 |  leap?  | example
/// ---------+---------+---------+---------+---------
///     F    |    F    |    F    |    F    |  2019
///     T    |    F    |    F    |    T    |  2020
///     -    |    T    |    F    |    F    |  1900
///     -    |    -    |    T    |    T    |  2000
/// ```
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

/// Compute the weekday from an epoch day count.
///
/// Uses Howard Hinnant's weekday algorithm.
/// Convention: 0=Sunday, 1=Monday, 2=Tuesday, 3=Wednesday, 4=Thursday, 5=Friday, 6=Saturday
/// Verification: weekday_from_days(0) == 4 (1970-01-01 is Thursday)
fn weekday_from_days(z: i64) -> u8 {
    if z >= -4 {
        ((z + 4) % 7) as u8
    } else {
        ((z + 5) % 7 + 6) as u8
    }
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

    /// Format the date/time as an IMF-fixdate string per RFC 9110 Section 5.6.7.
    ///
    /// Example output: "Sun, 06 Nov 1994 08:49:37 GMT"
    /// The output is always exactly 29 characters long.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_imf_fixdate(&self) -> String {
        const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const MONTH_NAMES: [&str; 13] = [
            "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let dow = weekday_from_days(self.epoch_days as i64);
        let secs_today = self.epoch_seconds % 86_400;
        let hour = secs_today / 3_600;
        let minute = (secs_today % 3_600) / 60;
        let second = secs_today % 60;
        let month_num: u8 = self.month.into();
        format!(
            "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
            DAY_NAMES[dow as usize],
            self.day,
            MONTH_NAMES[month_num as usize],
            self.year,
            hour,
            minute,
            second
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_epoch_zero() {
        // 1970-01-01 is Thursday, index 4 in 0=Sunday convention
        assert_eq!(weekday_from_days(0), 4);
    }

    #[test]
    fn weekday_2025_dec_01() {
        // 2025-12-01 is Monday, index 1 in 0=Sunday convention.
        // Epoch day = 20_423 (verified: date(2025,12,1) - date(1970,1,1) == 20423 days).
        // Note: the plan specified 20_440, but that is 2025-12-18 (Thursday). Fixed to 20_423.
        assert_eq!(weekday_from_days(20423), 1);
    }

    #[test]
    fn imf_fixdate_format_length() {
        assert_eq!(DateTime::now().unwrap().to_imf_fixdate().len(), 29);
    }

    #[test]
    fn imf_fixdate_ends_with_gmt() {
        assert!(DateTime::now().unwrap().to_imf_fixdate().ends_with(" GMT"));
    }

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
