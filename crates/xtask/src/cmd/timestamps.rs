use merman_core::runtime::{OperationContext, RuntimePolicy};
use merman_core::time::{LocalTimeZone, OffsetDateTime, UtcOffset};

pub(crate) fn current_utc_rfc3339_seconds() -> String {
    let operation = current_operation_context();
    format_utc_rfc3339_seconds(OffsetDateTime::from_unix_millis(
        operation.unix_millis(),
        UtcOffset::UTC,
    ))
}

pub(crate) fn current_local_report_timestamp_milliseconds() -> String {
    let operation = current_operation_context();
    let datetime = operation
        .local_time_zone()
        .at_instant(operation.unix_millis())
        .expect("an operation instant must resolve in its captured local time zone");
    format_local_report_timestamp_milliseconds(datetime)
}

pub(crate) fn unix_millis_to_utc_iso(milliseconds: i64) -> String {
    OffsetDateTime::from_unix_millis(milliseconds, UtcOffset::UTC)
        .utc_datetime()
        .to_string()
}

fn current_operation_context() -> OperationContext {
    let local_time_zone = LocalTimeZone::try_system().unwrap_or_else(|_| LocalTimeZone::utc());
    RuntimePolicy::deterministic()
        .try_with_system_clock()
        .expect("xtask requires the compiled system clock adapter")
        .with_local_time_zone(local_time_zone)
        .begin_operation()
        .expect("the system clock must fit Merman's millisecond time domain")
}

fn format_utc_rfc3339_seconds(timestamp: OffsetDateTime) -> String {
    let civil = timestamp.utc_datetime();
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        civil.date(),
        civil.hour(),
        civil.minute(),
        civil.second()
    )
}

fn format_local_report_timestamp_milliseconds(timestamp: OffsetDateTime) -> String {
    let civil = timestamp.local_datetime();
    let offset_seconds = timestamp.offset().seconds();
    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let absolute = offset_seconds.unsigned_abs();
    format!(
        "{}T{:02}:{:02}:{:02}.{:03}{sign}{:02}{:02}",
        civil.date(),
        civil.hour(),
        civil.minute(),
        civil.second(),
        civil.millisecond(),
        absolute / 3_600,
        absolute % 3_600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::time::CivilDateTime;

    #[test]
    fn current_time_formats_match_the_previous_report_contracts() {
        let utc = OffsetDateTime::from_local(
            CivilDateTime::new("2026-08-03".parse().unwrap(), 4, 5, 6, 789).unwrap(),
            UtcOffset::UTC,
        )
        .unwrap();
        let local = utc.to_offset(UtcOffset::from_minutes(8 * 60).unwrap());

        assert_eq!(format_utc_rfc3339_seconds(utc), "2026-08-03T04:05:06Z");
        assert_eq!(
            format_local_report_timestamp_milliseconds(local),
            "2026-08-03T12:05:06.789+0800"
        );
    }

    #[test]
    fn unix_milliseconds_use_utc_and_euclidean_subseconds() {
        assert_eq!(
            unix_millis_to_utc_iso(947_638_923_004),
            "2000-01-12T01:02:03.004"
        );
        assert_eq!(unix_millis_to_utc_iso(-1), "1969-12-31T23:59:59.999");
    }
}
