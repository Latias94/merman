use super::*;

pub(super) fn today_midnight_local(runtime: &OperationContext) -> Option<OffsetDateTime> {
    local_from_civil(runtime.today_local().at_midnight(), runtime)
}

pub(super) fn local_from_civil(
    local: CivilDateTime,
    runtime: &OperationContext,
) -> Option<OffsetDateTime> {
    runtime.local_time_zone().resolve_local(local)
}

pub(super) fn datetime_to_local(
    datetime: OffsetDateTime,
    runtime: &OperationContext,
) -> OffsetDateTime {
    runtime
        .local_time_zone()
        .at_instant(datetime.timestamp_millis())
        .unwrap_or(datetime)
}

pub(super) fn datetime_to_local_civil(
    datetime: OffsetDateTime,
    runtime: &OperationContext,
) -> CivilDateTime {
    datetime_to_local(datetime, runtime).local_datetime()
}

pub(super) fn add_days_local(
    dt: OffsetDateTime,
    days: i64,
    runtime: &OperationContext,
) -> Option<OffsetDateTime> {
    let local = datetime_to_local_civil(dt, runtime);
    local_from_civil(local.checked_add_days(days)?, runtime)
}

pub(super) fn add_months_local(
    dt: OffsetDateTime,
    months: i64,
    runtime: &OperationContext,
) -> Option<OffsetDateTime> {
    let local = datetime_to_local_civil(dt, runtime);
    local_from_civil(local.checked_add_months(months)?, runtime)
}

pub(super) fn add_years_local(
    dt: OffsetDateTime,
    years: i64,
    runtime: &OperationContext,
) -> Option<OffsetDateTime> {
    let local = datetime_to_local_civil(dt, runtime);
    local_from_civil(local.checked_add_years(years)?, runtime)
}
