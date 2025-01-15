use super::{
    constants::*,
    types::*,
    utils::*,
};

/// Representation of Calendar without timezones, has awareness of leap years, days in a given month for leap and non-leap years.
/// 
/// Typical usage
/// ```rust
/// use chrono_light::prelude::*;
/// let c = Calendar::create();
/// let now_in_ms: u64 = 1650412800000;  // represents 20/04/2022 00:00:00:000
/// let schedule = Schedule {
///     start: DateTime { year: 2020, month: 4, day: 30, hour: 0, minute: 0, second: 0, ms: 0 },
///     items: vec![(Frequency::Year, 1)],
///     end: Some(DateTime { year: 2025, month: 4, day: 30, hour: 0, minute: 0, second: 0, ms: 0 })
/// };
/// assert!(c.validate_schedule(&schedule).is_ok());
/// assert_eq!(Some(10*24*60*60*1000), c.next_occurrence_ms(&c.from_unixtime(now_in_ms), &schedule));  // triggers in 10 days
/// ```
/// 
/// Beware `c.to_unixtime()` may panic, use `c.validate()` and/or `c.to_unixtime_opt()` to guarantee safety.
pub struct Calendar {
    // values required for the lookup of the years/months, considering leap Februaries
    // - year_offset_ms, taking into account leap/non leap years. store in array with implied index starting at 1970
    // - month_offset_ms, for every year, taking into account leap februaries
    year_ms_offsets:             &'static [u64],
    leap_year_month_offsets:     &'static [u64],
    non_leap_year_month_offsets: &'static [u64],
}

impl Calendar {
    /// Constructor for the calendar.
    pub fn create() -> Self {
        Self {
            year_ms_offsets: YEAR_MS_OFFSETS,
            leap_year_month_offsets: LEAP_YEAR_MONTH_OFFSETS,
            non_leap_year_month_offsets: NON_LEAP_YEAR_MONTH_OFFSETS,
        }
    }

    /// Converts a `&DateTime` to ms from epoch. Note: may panic if invalid `DateTime` specified.
    /// ```rust
    /// # use chrono_light::prelude::*;
    /// let c = Calendar::create();
    /// assert_eq!(c.to_unixtime(&DateTime {year: 2010, month: 10, day: 10, hour: 10, minute: 10, second: 10, ms: 10}), 1286705410010);
    /// ```
    pub fn to_unixtime(&self, dt: &DateTime) -> u64 {
        let year = dt.year as usize - EPOCH_YEAR;
        let year_offset = self.year_ms_offsets[year];
        let month_offset = if LEAP_YEARS.contains(&(dt.year as u16)) {
            self.leap_year_month_offsets[dt.month.checked_sub(1).expect("failed to calc month - 1") as usize]
        } else {
            self.non_leap_year_month_offsets[dt.month.checked_sub(1).expect("failed to calc month - 1") as usize]
        };
        let day_offset = dt.day.checked_sub(1).expect("failed to calc day - 1") as u64 * MS_IN_DAY;
        let hour_offset = dt.hour as u64 * MS_IN_HOUR;
        let minute_offset = dt.minute as u64 * MS_IN_MIN;
        let second_offset = dt.second as u64 * MS_IN_SEC;
        let ms_offset = dt.ms as u64;

        year_offset + month_offset + day_offset + hour_offset + minute_offset + second_offset + ms_offset
    }

    /// Converts a `&DateTime` to ms from epoch, returning `Some()` if supplied `DateTime` was valid, `None` otherwise.
    /// ```rust
    /// # use chrono_light::prelude::*;
    /// let c = Calendar::create();
    /// assert_eq!(c.to_unixtime_res(&DateTime {year: 2010, month: 10, day: 10, hour: 10, minute: 10, second: 10, ms: 10}), Ok(1286705410010));
    /// assert_eq!(c.to_unixtime_res(&DateTime {year: 2010, month:  0, day: 10, hour: 10, minute: 10, second: 10, ms: 10}), Err(ValidationError::Invalid));
    /// assert_eq!(c.to_unixtime_res(&DateTime {year: 2010, month: 10, day:  0, hour: 10, minute: 10, second: 10, ms: 10}), Err(ValidationError::Invalid));
    /// ```
    pub fn to_unixtime_res(&self, dt: &DateTime) -> Result<u64, ValidationError> {
        self.validate_datetime(dt)?;
        Ok(self.to_unixtime(dt))
    }

    /// Converts ms from epoch to `DateTime`.
    pub fn from_unixtime(&self, ts: u64) -> DateTime {
        // find year
        let mut year = CURRENT_YEAR - EPOCH_YEAR;
        if ts > self.year_ms_offsets[year] {
            while ts > self.year_ms_offsets[year + 1] {
                year += 1;
            }
        } else {
            year -= 1;
            while ts < self.year_ms_offsets[year] {
                year -= 1;
            }
        }
        let year_offset = ts - self.year_ms_offsets[year];
        let month_offsets = if LEAP_YEARS.contains(&(year as u16 + EPOCH_YEAR as u16)) {
            &self.leap_year_month_offsets
        } else {
            &self.non_leap_year_month_offsets
        };

        let mut month = 1_usize;
        if year_offset > 0 {
            while year_offset > month_offsets[month - 1] {
                month += 1;
            }
            month -= 1;
        }

        let day_offset = year_offset - month_offsets[month - 1];
        let day = day_offset / MS_IN_DAY + 1;
        let hour = (day_offset % MS_IN_DAY) / MS_IN_HOUR;
        let minute = (day_offset % MS_IN_HOUR) / MS_IN_MIN;
        let second = (day_offset % MS_IN_MIN) / MS_IN_SEC;
        let ms = day_offset % MS_IN_SEC;

        DateTime {
            year: (year + EPOCH_YEAR) as u16,
            month: month as u8,
            day: day as u8,
            hour: hour as u8,
            minute: minute as u8,
            second: second as u8,
            ms: ms as u16
        }
    }

    /// Finds ms delta between 2 `DateTime`s.
    pub fn ms_between(&self, from: &DateTime, to: &DateTime) -> i64 {
        (self.to_unixtime(to) as i64).checked_sub(self.to_unixtime(from) as i64).expect("failed to calc ms_between")
    }

    /// Validates `DateTime` for correctness of fields, checking in respect to leap years.
    pub fn validate_datetime(&self, dt: &DateTime) -> Result<(), ValidationError> {
        // scope check
        (EPOCH_YEAR..=EPOCH_YEAR + self.year_ms_offsets.len() - 1).contains(&(dt.year as usize));
        if !(EPOCH_YEAR..=EPOCH_YEAR + self.year_ms_offsets.len() - 1).contains(&(dt.year as usize)) {
            return Err(ValidationError::OutOfScope);
        }

        // static valid check
        if !(1..=12).contains(&dt.month) || !(1..=31).contains(&dt.day) || dt.hour >= 24 || dt.minute >= 60 || dt.second >= 60 || dt.ms >= 1000 {
            return Err(ValidationError::Invalid);
        }

        // leap year check
        let is_leap_year = LEAP_YEARS.contains(&(dt.year as u16));
        if (is_leap_year && dt.day > MONTH_FOR_LEAP_YEAR[dt.month.checked_sub(1).expect("failed to calc month - 1") as usize]) ||
            (!is_leap_year && dt.day > MONTH_FOR_NON_LEAP_YEAR[dt.month.checked_sub(1).expect("failed to calc month - 1") as usize]) {
            return Err(ValidationError::Invalid);
        }
        Ok(())
    }
}
