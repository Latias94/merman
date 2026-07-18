use chrono::{DateTime, FixedOffset, NaiveDateTime, Offset, TimeZone};

#[cfg(feature = "host-clock")]
use chrono::{Datelike, Timelike};

#[cfg(feature = "host-clock")]
use sha2::{Digest, Sha256};
#[cfg(feature = "host-clock")]
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

/// Stable evidence for the local-time rules used by an engine or render operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTimeZoneProvenance {
    source: LocalTimeZoneSource,
    identifier: String,
    rules_sha256: Option<String>,
}

impl LocalTimeZoneProvenance {
    pub const fn source(&self) -> LocalTimeZoneSource {
        self.source
    }

    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// SHA-256 of the complete offset-transition behavior supported by the resolver.
    pub fn rules_sha256(&self) -> Option<&str> {
        self.rules_sha256.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTimeZoneSource {
    FixedOffset,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocalTimeZoneError {
    #[error("local UTC offset must be between -1439 and 1439 minutes, got {0}")]
    InvalidFixedOffset(i32),
}

/// An immutable local-time resolver.
///
/// Materialized system resolvers own the complete time-zone rule set rather than the offset sampled
/// at "now": another target date may be on the other side of a daylight-saving transition. The
/// default engine carries a lazy ambient resolver so diagrams that do not use local time never pay
/// for host time-zone discovery. Both the captured rules and their lazily computed provenance are
/// shared by every clone of a resolver.
#[derive(Debug, Clone)]
pub struct LocalTimeZone {
    data: Arc<LocalTimeZoneData>,
}

#[derive(Debug)]
struct LocalTimeZoneData {
    inner: LocalTimeZoneInner,
    provenance: OnceLock<LocalTimeZoneProvenance>,
}

#[derive(Debug)]
enum LocalTimeZoneInner {
    Fixed(FixedOffset),
    #[cfg(feature = "host-clock")]
    System(OnceLock<jiff::tz::TimeZone>),
}

impl LocalTimeZone {
    pub fn fixed(offset_minutes: i32) -> Result<Self, LocalTimeZoneError> {
        if !(-1439..=1439).contains(&offset_minutes) {
            return Err(LocalTimeZoneError::InvalidFixedOffset(offset_minutes));
        }
        let offset = FixedOffset::east_opt(offset_minutes * 60)
            .ok_or(LocalTimeZoneError::InvalidFixedOffset(offset_minutes))?;
        Ok(Self {
            data: Arc::new(LocalTimeZoneData {
                inner: LocalTimeZoneInner::Fixed(offset),
                provenance: OnceLock::from(LocalTimeZoneProvenance {
                    source: LocalTimeZoneSource::FixedOffset,
                    identifier: format_fixed_offset(offset_minutes),
                    rules_sha256: None,
                }),
            }),
        })
    }

    pub fn utc() -> Self {
        Self::fixed(0).expect("UTC is a valid fixed offset")
    }

    /// Captures the current system time-zone rules immediately.
    ///
    /// Render operations use this constructor at their environment boundary so parsing, layout,
    /// and provenance all observe the same immutable rule set.
    #[cfg(feature = "host-clock")]
    pub fn system() -> Self {
        thread_local! {
            static CACHE: RefCell<Option<LocalTimeZone>> = const { RefCell::new(None) };
        }

        let time_zone = jiff::tz::TimeZone::system();
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(cached) = cache.as_ref()
                && cached.materialized_system_time_zone() == Some(&time_zone)
            {
                return cached.clone();
            }
            let captured = Self {
                data: Arc::new(LocalTimeZoneData {
                    inner: LocalTimeZoneInner::System(OnceLock::from(time_zone)),
                    provenance: OnceLock::new(),
                }),
            };
            *cache = Some(captured.clone());
            captured
        })
    }

    pub(crate) fn ambient() -> Self {
        #[cfg(feature = "host-clock")]
        {
            // Default parsing must not discover host resources until a diagram asks for local time.
            Self {
                data: Arc::new(LocalTimeZoneData {
                    inner: LocalTimeZoneInner::System(OnceLock::new()),
                    provenance: OnceLock::new(),
                }),
            }
        }
        #[cfg(not(feature = "host-clock"))]
        {
            Self::utc()
        }
    }

    pub fn provenance(&self) -> &LocalTimeZoneProvenance {
        self.data.provenance.get_or_init(|| match &self.data.inner {
            LocalTimeZoneInner::Fixed(offset) => LocalTimeZoneProvenance {
                source: LocalTimeZoneSource::FixedOffset,
                identifier: format_fixed_offset(offset.local_minus_utc() / 60),
                rules_sha256: None,
            },
            #[cfg(feature = "host-clock")]
            LocalTimeZoneInner::System(time_zone) => {
                system_provenance(time_zone.get_or_init(jiff::tz::TimeZone::system))
            }
        })
    }

    pub fn is_system(&self) -> bool {
        #[cfg(feature = "host-clock")]
        {
            matches!(self.data.inner, LocalTimeZoneInner::System(_))
        }
        #[cfg(not(feature = "host-clock"))]
        {
            false
        }
    }

    pub fn fixed_offset_minutes(&self) -> Option<i32> {
        match &self.data.inner {
            LocalTimeZoneInner::Fixed(offset) => Some(offset.local_minus_utc() / 60),
            #[cfg(feature = "host-clock")]
            LocalTimeZoneInner::System(_) => None,
        }
    }

    pub fn datetime_from_naive_local(&self, naive: NaiveDateTime) -> Option<DateTime<FixedOffset>> {
        match &self.data.inner {
            LocalTimeZoneInner::Fixed(offset) => offset.from_local_datetime(&naive).single(),
            #[cfg(feature = "host-clock")]
            LocalTimeZoneInner::System(time_zone) => {
                let time_zone = time_zone.get_or_init(jiff::tz::TimeZone::system);
                let civil = jiff::civil::DateTime::new(
                    naive.year().try_into().ok()?,
                    naive.month().try_into().ok()?,
                    naive.day().try_into().ok()?,
                    naive.hour().try_into().ok()?,
                    naive.minute().try_into().ok()?,
                    naive.second().try_into().ok()?,
                    naive.nanosecond().try_into().ok()?,
                )
                .ok()?;
                let timestamp = time_zone.to_timestamp(civil).ok()?;
                datetime_from_jiff_timestamp(timestamp, time_zone.to_offset(timestamp).seconds())
            }
        }
    }

    pub fn datetime_to_local_fixed(
        &self,
        datetime: DateTime<FixedOffset>,
    ) -> Option<DateTime<FixedOffset>> {
        match &self.data.inner {
            LocalTimeZoneInner::Fixed(offset) => Some(datetime.with_timezone(offset)),
            #[cfg(feature = "host-clock")]
            LocalTimeZoneInner::System(time_zone) => {
                let time_zone = time_zone.get_or_init(jiff::tz::TimeZone::system);
                let timestamp = jiff::Timestamp::new(
                    datetime.timestamp(),
                    datetime.timestamp_subsec_nanos().try_into().ok()?,
                )
                .ok()?;
                datetime_from_jiff_timestamp(timestamp, time_zone.to_offset(timestamp).seconds())
            }
        }
    }

    #[cfg(feature = "host-clock")]
    fn materialized_system_time_zone(&self) -> Option<&jiff::tz::TimeZone> {
        match &self.data.inner {
            LocalTimeZoneInner::Fixed(_) => None,
            LocalTimeZoneInner::System(time_zone) => time_zone.get(),
        }
    }
}

impl PartialEq for LocalTimeZone {
    fn eq(&self, other: &Self) -> bool {
        match (&self.data.inner, &other.data.inner) {
            (LocalTimeZoneInner::Fixed(left), LocalTimeZoneInner::Fixed(right)) => left == right,
            #[cfg(feature = "host-clock")]
            (LocalTimeZoneInner::System(left), LocalTimeZoneInner::System(right)) => {
                left.get_or_init(jiff::tz::TimeZone::system)
                    == right.get_or_init(jiff::tz::TimeZone::system)
            }
            #[cfg(feature = "host-clock")]
            _ => false,
        }
    }
}

impl Eq for LocalTimeZone {}

fn format_fixed_offset(offset_minutes: i32) -> String {
    let sign = if offset_minutes < 0 { '-' } else { '+' };
    let absolute = offset_minutes.unsigned_abs();
    format!("{sign}{:02}:{:02}", absolute / 60, absolute % 60)
}

#[cfg(feature = "host-clock")]
fn datetime_from_jiff_timestamp(
    timestamp: jiff::Timestamp,
    offset_seconds: i32,
) -> Option<DateTime<FixedOffset>> {
    let total_nanos = timestamp.as_nanosecond();
    let seconds: i64 = total_nanos.div_euclid(1_000_000_000).try_into().ok()?;
    let nanoseconds: u32 = total_nanos.rem_euclid(1_000_000_000).try_into().ok()?;
    let utc = DateTime::from_timestamp(seconds, nanoseconds)?;
    let offset = FixedOffset::east_opt(offset_seconds)?;
    Some(utc.with_timezone(&offset))
}

#[cfg(feature = "host-clock")]
fn system_provenance(time_zone: &jiff::tz::TimeZone) -> LocalTimeZoneProvenance {
    let identifier = time_zone.iana_name().map(str::to_owned).unwrap_or_else(|| {
        if time_zone.is_unknown() {
            "Etc/Unknown".to_string()
        } else {
            format!("{time_zone:?}")
        }
    });

    let mut digest = Sha256::new();
    digest.update(b"merman-local-time-zone-offset-rules-v1\0");
    let minimum = jiff::Timestamp::MIN;
    digest.update(time_zone.to_offset(minimum).seconds().to_be_bytes());
    for transition in time_zone.following(minimum) {
        let timestamp = transition.timestamp();
        digest.update(timestamp.as_second().to_be_bytes());
        digest.update(timestamp.subsec_nanosecond().to_be_bytes());
        digest.update(transition.offset().seconds().to_be_bytes());
    }
    let hash = digest.finalize();
    let mut rules_sha256 = String::with_capacity(hash.len() * 2);
    use std::fmt::Write as _;
    for byte in hash {
        write!(rules_sha256, "{byte:02x}").expect("writing to a String cannot fail");
    }

    LocalTimeZoneProvenance {
        source: LocalTimeZoneSource::System,
        identifier,
        rules_sha256: Some(rules_sha256),
    }
}

/// Runs a closure with one immutable local-time resolver installed for this thread.
pub fn with_local_time_zone<R>(time_zone: &LocalTimeZone, f: impl FnOnce() -> R) -> R {
    crate::runtime::with_local_time_zone(time_zone, f)
}

/// Interprets a local `NaiveDateTime` as an absolute instant in the active local timezone.
///
/// The resolver installed by [`with_local_time_zone`] is used. Without an explicit context, a
/// system resolver is captured when `host-clock` is enabled, and UTC is used otherwise.
pub fn datetime_from_naive_local(naive: NaiveDateTime) -> Option<DateTime<FixedOffset>> {
    crate::runtime::datetime_from_naive_local(naive)
}

/// Maps an absolute instant to the active local timezone (as a `FixedOffset`).
///
/// The resolver installed by [`with_local_time_zone`] is used. Without an explicit context, a
/// system resolver is captured when `host-clock` is enabled, and UTC is used otherwise.
pub fn datetime_to_local_fixed(dt: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    crate::runtime::datetime_to_local_fixed(dt)
}

/// Returns the `NaiveDateTime` for an absolute instant under the active local-time semantics.
pub fn datetime_to_naive_local(dt: DateTime<FixedOffset>) -> NaiveDateTime {
    crate::runtime::datetime_to_naive_local(dt)
}

/// Returns the UTC fixed offset without fallible construction.
pub fn utc_fixed_offset() -> FixedOffset {
    chrono::Utc.fix()
}

#[cfg(all(test, feature = "host-clock"))]
mod host_clock_tests {
    use super::*;

    #[test]
    fn ambient_resolver_construction_uses_bounded_stack() {
        let handle = std::thread::Builder::new()
            .name("merman-core-ambient-time-zone-small-stack".to_string())
            .stack_size(256 * 1024)
            .spawn(|| std::hint::black_box(LocalTimeZone::ambient()))
            .expect("spawn ambient time-zone construction test");

        handle
            .join()
            .expect("ambient time-zone construction should not overflow a 256 KiB stack");
    }
}
