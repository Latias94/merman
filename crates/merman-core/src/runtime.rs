use chrono::{FixedOffset, NaiveDate, NaiveDateTime};
use std::cell::{Cell, RefCell};

thread_local! {
    static FIXED_TODAY_LOCAL: Cell<Option<NaiveDate>> = const { Cell::new(None) };
    static LOCAL_TIME_ZONE: RefCell<Option<crate::time::LocalTimeZone>> = const { RefCell::new(None) };
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

#[cfg(feature = "host-timing")]
pub(crate) fn timing_zero_duration() -> TimingDuration {
    TimingDuration::default()
}

#[cfg(not(feature = "host-timing"))]
pub(crate) fn timing_zero_duration() -> TimingDuration {
    TimingDuration
}

pub(crate) fn with_fixed_today_local<R>(today: Option<NaiveDate>, f: impl FnOnce() -> R) -> R {
    FIXED_TODAY_LOCAL.with(|cell| {
        let previous = cell.replace(today);
        struct Restore<'a> {
            cell: &'a Cell<Option<NaiveDate>>,
            previous: Option<NaiveDate>,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.cell.set(self.previous);
            }
        }
        let _restore = Restore { cell, previous };
        f()
    })
}

pub(crate) fn with_local_time_zone<R>(
    time_zone: &crate::time::LocalTimeZone,
    f: impl FnOnce() -> R,
) -> R {
    LOCAL_TIME_ZONE.with(|cell| {
        let previous = cell.replace(Some(time_zone.clone()));
        struct Restore<'a> {
            cell: &'a RefCell<Option<crate::time::LocalTimeZone>>,
            previous: Option<crate::time::LocalTimeZone>,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.cell.replace(self.previous.take());
            }
        }
        let _restore = Restore { cell, previous };
        f()
    })
}

pub(crate) fn today_naive_local() -> NaiveDate {
    FIXED_TODAY_LOCAL
        .with(|cell| cell.get())
        .unwrap_or_else(default_today_naive_local)
}

pub(crate) fn datetime_from_naive_local(
    naive: NaiveDateTime,
) -> Option<chrono::DateTime<FixedOffset>> {
    active_local_time_zone().datetime_from_naive_local(naive)
}

pub(crate) fn datetime_to_local_fixed(
    dt: chrono::DateTime<FixedOffset>,
) -> chrono::DateTime<FixedOffset> {
    active_local_time_zone()
        .datetime_to_local_fixed(dt)
        .unwrap_or(dt)
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
        let now = chrono::Utc::now().fixed_offset();
        active_local_time_zone()
            .datetime_to_local_fixed(now)
            .map(|local| local.date_naive())
            .unwrap_or_else(|| now.date_naive())
    }

    #[cfg(not(feature = "host-clock"))]
    {
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or(NaiveDate::MIN)
    }
}

fn active_local_time_zone() -> crate::time::LocalTimeZone {
    LOCAL_TIME_ZONE
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(crate::time::LocalTimeZone::ambient)
}

#[cfg(test)]
mod time_context_tests {
    use super::*;

    #[test]
    fn fixed_today_context_restores_after_panic() {
        let outer = NaiveDate::from_ymd_opt(2026, 7, 18).expect("valid date");
        let inner = NaiveDate::from_ymd_opt(2030, 1, 2).expect("valid date");

        with_fixed_today_local(Some(outer), || {
            let panic = std::panic::catch_unwind(|| {
                with_fixed_today_local(Some(inner), || panic!("test panic"));
            });
            assert!(panic.is_err());
            assert_eq!(today_naive_local(), outer);
        });
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
        let dt = datetime_from_naive_local(naive).expect("UTC supports this datetime");

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
            generated_id_hex(12, 7, 0x0062_6C6F_636B),
            generated_id_hex(12, 7, 0x0062_6C6F_636B)
        );
        assert_ne!(
            generated_id_hex(12, 7, 0x0062_6C6F_636B),
            generated_id_hex(12, 8, 0x0062_6C6F_636B)
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
