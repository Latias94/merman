use chrono::{FixedOffset, NaiveDate, NaiveDateTime, TimeZone};
use std::cell::Cell;

thread_local! {
    static FIXED_TODAY_LOCAL: Cell<Option<NaiveDate>> = const { Cell::new(None) };
    static FIXED_LOCAL_OFFSET_MINUTES: Cell<Option<i32>> = const { Cell::new(None) };
}

#[cfg(feature = "host-timing")]
pub(crate) type TimingDuration = web_time::Duration;

#[cfg(not(feature = "host-timing"))]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TimingDuration;

#[cfg(feature = "host-timing")]
pub(crate) type TimingInstant = web_time::Instant;

#[cfg(not(feature = "host-timing"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct TimingInstant;

pub(crate) fn timing_start(enabled: bool) -> Option<TimingInstant> {
    #[cfg(feature = "host-timing")]
    {
        enabled.then(timing_now)
    }

    #[cfg(not(feature = "host-timing"))]
    {
        let _ = enabled;
        None
    }
}

#[cfg(feature = "host-timing")]
fn timing_now() -> TimingInstant {
    web_time::Instant::now()
}

#[cfg(feature = "host-timing")]
pub(crate) fn timing_elapsed(start: TimingInstant) -> TimingDuration {
    start.elapsed()
}

#[cfg(not(feature = "host-timing"))]
pub(crate) fn timing_elapsed(_start: TimingInstant) -> TimingDuration {
    TimingDuration
}

pub(crate) fn timing_zero_duration() -> TimingDuration {
    TimingDuration::default()
}

pub(crate) fn with_fixed_today_local<R>(today: Option<NaiveDate>, f: impl FnOnce() -> R) -> R {
    FIXED_TODAY_LOCAL.with(|cell| {
        let prev = cell.replace(today);
        let out = f();
        cell.set(prev);
        out
    })
}

pub(crate) fn with_fixed_local_offset_minutes<R>(
    offset_minutes: Option<i32>,
    f: impl FnOnce() -> R,
) -> R {
    FIXED_LOCAL_OFFSET_MINUTES.with(|cell| {
        let prev = cell.replace(offset_minutes);
        let out = f();
        cell.set(prev);
        out
    })
}

pub(crate) fn today_naive_local() -> NaiveDate {
    FIXED_TODAY_LOCAL
        .with(|cell| cell.get())
        .unwrap_or_else(default_today_naive_local)
}

pub(crate) fn datetime_from_naive_local(naive: NaiveDateTime) -> chrono::DateTime<FixedOffset> {
    if let Some(mins) = FIXED_LOCAL_OFFSET_MINUTES.with(|cell| cell.get()) {
        let offset = FixedOffset::east_opt(mins.saturating_mul(60))
            .unwrap_or_else(crate::time::utc_fixed_offset);
        return offset
            .from_local_datetime(&naive)
            .single()
            .unwrap_or_else(|| {
                chrono::DateTime::<FixedOffset>::from_naive_utc_and_offset(naive, offset)
            });
    }

    #[cfg(not(feature = "host-clock"))]
    {
        return chrono::DateTime::<FixedOffset>::from_naive_utc_and_offset(
            naive,
            crate::time::utc_fixed_offset(),
        );
    }

    #[cfg(feature = "host-clock")]
    match chrono::Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt.fixed_offset(),
        chrono::LocalResult::Ambiguous(a, _b) => a.fixed_offset(),
        chrono::LocalResult::None => chrono::DateTime::<FixedOffset>::from_naive_utc_and_offset(
            naive,
            crate::time::utc_fixed_offset(),
        ),
    }
}

pub(crate) fn datetime_to_local_fixed(
    dt: chrono::DateTime<FixedOffset>,
) -> chrono::DateTime<FixedOffset> {
    if let Some(mins) = FIXED_LOCAL_OFFSET_MINUTES.with(|cell| cell.get()) {
        let offset = FixedOffset::east_opt(mins.saturating_mul(60))
            .unwrap_or_else(crate::time::utc_fixed_offset);
        return dt.with_timezone(&offset);
    }

    #[cfg(not(feature = "host-clock"))]
    {
        return dt.with_timezone(&crate::time::utc_fixed_offset());
    }

    #[cfg(feature = "host-clock")]
    dt.with_timezone(&chrono::Local).fixed_offset()
}

pub(crate) fn datetime_to_naive_local(dt: chrono::DateTime<FixedOffset>) -> NaiveDateTime {
    datetime_to_local_fixed(dt).naive_local()
}

pub(crate) fn generated_id_hex(len: usize, counter: u64, domain_salt: u64) -> String {
    #[cfg(feature = "host-random")]
    {
        let _ = (counter, domain_salt);
        let hex = uuid::Uuid::new_v4().simple().to_string();
        hex.chars().take(len).collect()
    }

    #[cfg(not(feature = "host-random"))]
    deterministic_id_hex(len, counter, domain_salt)
}

#[cfg(not(feature = "host-random"))]
fn deterministic_id_hex(len: usize, counter: u64, domain_salt: u64) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(len);
    let mut state = counter ^ domain_salt;
    while out.len() < len {
        state = splitmix64(state);
        for shift in (0..16).rev() {
            if out.len() == len {
                break;
            }
            let idx = ((state >> (shift * 4)) & 0xF) as usize;
            out.push(HEX[idx] as char);
        }
    }
    out
}

#[cfg(not(feature = "host-random"))]
fn splitmix64(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn default_today_naive_local() -> NaiveDate {
    #[cfg(feature = "host-clock")]
    {
        chrono::Local::now().date_naive()
    }

    #[cfg(not(feature = "host-clock"))]
    {
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or(NaiveDate::MIN)
    }
}

#[cfg(all(test, not(feature = "host-clock")))]
mod no_host_clock_tests {
    use super::*;

    #[test]
    fn default_today_is_deterministic_without_host_clock() {
        assert_eq!(
            today_naive_local(),
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
        );
    }

    #[test]
    fn local_datetime_uses_utc_without_host_clock() {
        let naive = NaiveDate::from_ymd_opt(2026, 2, 15)
            .unwrap()
            .and_hms_opt(3, 4, 5)
            .unwrap();
        let dt = datetime_from_naive_local(naive);

        assert_eq!(dt.offset(), &crate::time::utc_fixed_offset());
        assert_eq!(datetime_to_naive_local(dt), naive);
    }
}

#[cfg(all(test, not(feature = "host-random")))]
mod no_host_random_tests {
    use super::*;

    #[test]
    fn generated_id_hex_is_deterministic_without_host_random() {
        assert_eq!(
            generated_id_hex(12, 7, 0x626C_6F63_6B),
            generated_id_hex(12, 7, 0x626C_6F63_6B)
        );
        assert_ne!(
            generated_id_hex(12, 7, 0x626C_6F63_6B),
            generated_id_hex(12, 8, 0x626C_6F63_6B)
        );
    }
}

#[cfg(all(test, not(feature = "host-timing")))]
mod no_host_timing_tests {
    use super::*;

    #[test]
    fn timing_start_is_disabled_without_host_timing() {
        assert!(timing_start(true).is_none());
    }
}
