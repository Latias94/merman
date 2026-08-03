use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

const MILLIS_PER_SECOND: i64 = 1_000;
const MILLIS_PER_MINUTE: i64 = 60 * MILLIS_PER_SECOND;
const MILLIS_PER_HOUR: i64 = 60 * MILLIS_PER_MINUTE;
const MILLIS_PER_DAY: i64 = 24 * MILLIS_PER_HOUR;

/// A proleptic-Gregorian calendar date with a signed 32-bit year.
///
/// Unlike Jiff's civil date, this type deliberately admits Mermaid's `10000`
/// year boundary. Conversion to an instant remains checked separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CivilDate {
    pub const fn new(year: i32, month: u32, day: u32) -> Option<Self> {
        if month < 1 || month > 12 || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some(Self {
            year,
            month: month as u8,
            day: day as u8,
        })
    }

    pub const fn year(self) -> i32 {
        self.year
    }

    pub const fn month(self) -> u32 {
        self.month as u32
    }

    pub const fn month0(self) -> u32 {
        self.month() - 1
    }

    pub const fn day(self) -> u32 {
        self.day as u32
    }

    pub const fn day0(self) -> u32 {
        self.day() - 1
    }

    pub fn weekday(self) -> Weekday {
        Weekday::from_monday_index((self.days_since_unix_epoch() + 3).rem_euclid(7) as u8)
    }

    pub fn ordinal(self) -> u32 {
        let january_first = Self::new(self.year, 1, 1).expect("January 1 is always valid");
        u32::try_from(self.days_since(january_first) + 1)
            .expect("a Gregorian year contains at most 366 days")
    }

    pub fn iso_week(self) -> IsoWeek {
        let weekday = i64::from(self.weekday().number_from_monday());
        let thursday = self
            .checked_add_days(4 - weekday)
            .expect("moving within one week cannot exceed the i32 year range");
        let week_year = thursday.year;
        let january_fourth = Self::new(week_year, 1, 4).expect("January 4 is always a valid date");
        let week_one_monday = january_fourth
            .checked_add_days(1 - i64::from(january_fourth.weekday().number_from_monday()))
            .expect("moving within one week cannot exceed the i32 year range");
        let week = ((self.days_since(week_one_monday)).div_euclid(7) + 1) as u32;
        IsoWeek {
            year: week_year,
            week,
        }
    }

    pub fn at_midnight(self) -> CivilDateTime {
        CivilDateTime {
            date: self,
            hour: 0,
            minute: 0,
            second: 0,
            millisecond: 0,
        }
    }

    pub const fn at_hms(self, hour: u32, minute: u32, second: u32) -> Option<CivilDateTime> {
        CivilDateTime::new(self, hour, minute, second, 0)
    }

    pub const fn at_hms_milli(
        self,
        hour: u32,
        minute: u32,
        second: u32,
        millisecond: u32,
    ) -> Option<CivilDateTime> {
        CivilDateTime::new(self, hour, minute, second, millisecond)
    }

    pub fn checked_add_days(self, days: i64) -> Option<Self> {
        civil_from_days(self.days_since_unix_epoch().checked_add(days)?)
    }

    pub fn checked_sub_days(self, days: i64) -> Option<Self> {
        self.checked_add_days(days.checked_neg()?)
    }

    pub fn days_since(self, earlier: Self) -> i64 {
        self.days_since_unix_epoch() - earlier.days_since_unix_epoch()
    }

    pub(crate) fn days_since_unix_epoch(self) -> i64 {
        days_from_civil(self.year, self.month(), self.day())
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.year {
            0..=9999 => write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day),
            10000.. => write!(f, "+{}-{:02}-{:02}", self.year, self.month, self.day),
            _ => write!(
                f,
                "-{:04}-{:02}-{:02}",
                self.year.unsigned_abs(),
                self.month,
                self.day
            ),
        }
    }
}

impl FromStr for CivilDate {
    type Err = ParseCivilDateError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (year_month, day) = value.rsplit_once('-').ok_or(ParseCivilDateError)?;
        let (year, month) = year_month.rsplit_once('-').ok_or(ParseCivilDateError)?;
        if month.len() != 2
            || day.len() != 2
            || !month.bytes().all(|byte| byte.is_ascii_digit())
            || !day.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ParseCivilDateError);
        }

        let year_digits = year.strip_prefix(['+', '-']).unwrap_or(year);
        let signed = year.starts_with(['+', '-']);
        if year_digits.len() < 4
            || (!signed && year_digits.len() != 4)
            || !year_digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ParseCivilDateError);
        }

        let year = year.parse().map_err(|_| ParseCivilDateError)?;
        let month = month.parse().map_err(|_| ParseCivilDateError)?;
        let day = day.parse().map_err(|_| ParseCivilDateError)?;
        Self::new(year, month, day).ok_or(ParseCivilDateError)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseCivilDateError;

impl fmt::Display for ParseCivilDateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a valid ISO civil date")
    }
}

impl std::error::Error for ParseCivilDateError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDateTime {
    date: CivilDate,
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
}

impl CivilDateTime {
    pub const fn new(
        date: CivilDate,
        hour: u32,
        minute: u32,
        second: u32,
        millisecond: u32,
    ) -> Option<Self> {
        if hour > 23 || minute > 59 || second > 59 || millisecond > 999 {
            return None;
        }
        Some(Self {
            date,
            hour: hour as u8,
            minute: minute as u8,
            second: second as u8,
            millisecond: millisecond as u16,
        })
    }

    pub const fn date(self) -> CivilDate {
        self.date
    }

    pub const fn year(self) -> i32 {
        self.date.year()
    }

    pub const fn month(self) -> u32 {
        self.date.month()
    }

    pub const fn month0(self) -> u32 {
        self.date.month0()
    }

    pub const fn day(self) -> u32 {
        self.date.day()
    }

    pub const fn day0(self) -> u32 {
        self.date.day0()
    }

    pub fn weekday(self) -> Weekday {
        self.date.weekday()
    }

    pub const fn hour(self) -> u32 {
        self.hour as u32
    }

    pub const fn minute(self) -> u32 {
        self.minute as u32
    }

    pub const fn second(self) -> u32 {
        self.second as u32
    }

    pub const fn millisecond(self) -> u32 {
        self.millisecond as u32
    }

    pub fn checked_add_millis(self, milliseconds: i64) -> Option<Self> {
        Self::from_naive_unix_millis(self.naive_unix_millis()?.checked_add(milliseconds)?)
    }

    pub fn checked_add_days(self, days: i64) -> Option<Self> {
        Some(Self {
            date: self.date.checked_add_days(days)?,
            ..self
        })
    }

    pub fn checked_add_months(self, months: i64) -> Option<Self> {
        let month_index = i64::from(self.year())
            .checked_mul(12)?
            .checked_add(i64::from(self.month0()))?
            .checked_add(months)?;
        let year = i32::try_from(month_index.div_euclid(12)).ok()?;
        let month = u32::try_from(month_index.rem_euclid(12)).ok()? + 1;
        let day = self.day().min(days_in_month(year, month));
        Some(Self {
            date: CivilDate::new(year, month, day)?,
            ..self
        })
    }

    pub fn checked_add_years(self, years: i64) -> Option<Self> {
        let years = i32::try_from(years).ok()?;
        let year = self.year().checked_add(years)?;
        let day = self.day().min(days_in_month(year, self.month()));
        Some(Self {
            date: CivilDate::new(year, self.month(), day)?,
            ..self
        })
    }

    pub fn signed_duration_millis_since(self, earlier: Self) -> Option<i64> {
        self.naive_unix_millis()?
            .checked_sub(earlier.naive_unix_millis()?)
    }

    pub(crate) fn naive_unix_millis(self) -> Option<i64> {
        self.date
            .days_since_unix_epoch()
            .checked_mul(MILLIS_PER_DAY)?
            .checked_add(i64::from(self.hour) * MILLIS_PER_HOUR)?
            .checked_add(i64::from(self.minute) * MILLIS_PER_MINUTE)?
            .checked_add(i64::from(self.second) * MILLIS_PER_SECOND)?
            .checked_add(i64::from(self.millisecond))
    }

    pub(crate) fn from_naive_unix_millis(milliseconds: i64) -> Option<Self> {
        Self::from_wide_naive_unix_millis(i128::from(milliseconds))
    }

    fn from_wide_naive_unix_millis(milliseconds: i128) -> Option<Self> {
        let day = milliseconds.div_euclid(i128::from(MILLIS_PER_DAY));
        let within_day = milliseconds.rem_euclid(i128::from(MILLIS_PER_DAY));
        let date = civil_from_days(day.try_into().ok()?)?;
        let hour = within_day / i128::from(MILLIS_PER_HOUR);
        let within_hour = within_day % i128::from(MILLIS_PER_HOUR);
        let minute = within_hour / i128::from(MILLIS_PER_MINUTE);
        let within_minute = within_hour % i128::from(MILLIS_PER_MINUTE);
        let second = within_minute / i128::from(MILLIS_PER_SECOND);
        let millisecond = within_minute % i128::from(MILLIS_PER_SECOND);
        Self::new(
            date,
            hour.try_into().ok()?,
            minute.try_into().ok()?,
            second.try_into().ok()?,
            millisecond.try_into().ok()?,
        )
    }
}

impl fmt::Display for CivilDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}T{:02}:{:02}:{:02}.{:03}",
            self.date, self.hour, self.minute, self.second, self.millisecond
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UtcOffset {
    seconds: i32,
}

impl UtcOffset {
    pub const UTC: Self = Self { seconds: 0 };

    pub const fn from_minutes(minutes: i32) -> Option<Self> {
        if minutes < -1439 || minutes > 1439 {
            return None;
        }
        Some(Self {
            seconds: minutes * 60,
        })
    }

    pub const fn from_seconds(seconds: i32) -> Option<Self> {
        if seconds <= -86_400 || seconds >= 86_400 {
            return None;
        }
        Some(Self { seconds })
    }

    pub const fn seconds(self) -> i32 {
        self.seconds
    }

    pub const fn minutes(self) -> i32 {
        self.seconds / 60
    }
}

impl fmt::Display for UtcOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.seconds < 0 { '-' } else { '+' };
        let absolute = self.seconds.unsigned_abs();
        write!(
            f,
            "{sign}{:02}:{:02}",
            absolute / 3600,
            (absolute % 3600) / 60
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OffsetDateTime {
    unix_millis: i64,
    offset: UtcOffset,
}

impl OffsetDateTime {
    pub const fn from_unix_millis(unix_millis: i64, offset: UtcOffset) -> Self {
        Self {
            unix_millis,
            offset,
        }
    }

    pub fn from_local(local: CivilDateTime, offset: UtcOffset) -> Option<Self> {
        let unix_millis = local
            .naive_unix_millis()?
            .checked_sub(i64::from(offset.seconds) * MILLIS_PER_SECOND)?;
        Some(Self {
            unix_millis,
            offset,
        })
    }

    pub const fn timestamp_millis(self) -> i64 {
        self.unix_millis
    }

    pub const fn timestamp_seconds(self) -> i64 {
        self.unix_millis.div_euclid(MILLIS_PER_SECOND)
    }

    pub const fn timestamp_subsec_millis(self) -> u32 {
        self.unix_millis.rem_euclid(MILLIS_PER_SECOND) as u32
    }

    pub const fn offset(self) -> UtcOffset {
        self.offset
    }

    pub fn local_datetime(self) -> CivilDateTime {
        let local_millis = i128::from(self.unix_millis)
            + i128::from(self.offset.seconds) * i128::from(MILLIS_PER_SECOND);
        CivilDateTime::from_wide_naive_unix_millis(local_millis)
            .expect("an i64 millisecond instant plus a UTC offset fits an i32 civil year")
    }

    pub fn utc_datetime(self) -> CivilDateTime {
        CivilDateTime::from_naive_unix_millis(self.unix_millis)
            .expect("an i64 millisecond instant fits an i32 civil year")
    }

    pub const fn to_offset(self, offset: UtcOffset) -> Self {
        Self { offset, ..self }
    }

    pub fn checked_add_millis(self, milliseconds: i64) -> Option<Self> {
        Some(Self {
            unix_millis: self.unix_millis.checked_add(milliseconds)?,
            ..self
        })
    }
}

impl PartialEq for OffsetDateTime {
    fn eq(&self, other: &Self) -> bool {
        self.unix_millis == other.unix_millis
    }
}

impl Eq for OffsetDateTime {}

impl PartialOrd for OffsetDateTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OffsetDateTime {
    fn cmp(&self, other: &Self) -> Ordering {
        self.unix_millis.cmp(&other.unix_millis)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    const fn from_monday_index(index: u8) -> Self {
        match index {
            0 => Self::Monday,
            1 => Self::Tuesday,
            2 => Self::Wednesday,
            3 => Self::Thursday,
            4 => Self::Friday,
            5 => Self::Saturday,
            _ => Self::Sunday,
        }
    }

    pub const fn number_from_monday(self) -> u32 {
        match self {
            Self::Monday => 1,
            Self::Tuesday => 2,
            Self::Wednesday => 3,
            Self::Thursday => 4,
            Self::Friday => 5,
            Self::Saturday => 6,
            Self::Sunday => 7,
        }
    }

    pub const fn number_from_sunday(self) -> u32 {
        match self {
            Self::Sunday => 1,
            Self::Monday => 2,
            Self::Tuesday => 3,
            Self::Wednesday => 4,
            Self::Thursday => 5,
            Self::Friday => 6,
            Self::Saturday => 7,
        }
    }

    pub const fn full_name(self) -> &'static str {
        match self {
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
            Self::Sunday => "Sunday",
        }
    }

    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Monday => "Mon",
            Self::Tuesday => "Tue",
            Self::Wednesday => "Wed",
            Self::Thursday => "Thu",
            Self::Friday => "Fri",
            Self::Saturday => "Sat",
            Self::Sunday => "Sun",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoWeek {
    year: i32,
    week: u32,
}

impl IsoWeek {
    pub const fn year(self) -> i32 {
        self.year
    }

    pub const fn week(self) -> u32 {
        self.week
    }
}

pub const fn is_leap_year(year: i32) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

pub const fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut year = i64::from(year);
    if month <= 2 {
        year -= 1;
    }
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_prime = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> Option<CivilDate> {
    let days = days.checked_add(719_468)?;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    CivilDate::new(
        year.try_into().ok()?,
        month.try_into().ok()?,
        day.try_into().ok()?,
    )
}
