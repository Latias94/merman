use super::*;
use merman_model::OperationControl;
use merman_model::runtime::RuntimePolicy;
use merman_model::time::LocalTimeZone;

fn runtime() -> OperationContext {
    runtime_in(LocalTimeZone::utc())
}

fn runtime_in(zone: LocalTimeZone) -> OperationContext {
    RuntimePolicy::deterministic()
        .with_local_time_zone(zone)
        .begin_operation()
        .unwrap()
}

fn large_gantt_db(task_count: usize) -> GanttDb {
    let mut db = GanttDb::default();
    db.clear();
    db.set_date_format("YYYY-MM-DD");
    db.add_section("Delivery");
    for index in 0..task_count {
        let id = format!("task-{index}");
        let fields = [id.as_str(), "2026-01-01", "1d"];
        let task_info = db.parse_task_info(&fields);
        db.add_task(&format!("Task {index}"), &fields.join(","), task_info);
    }
    db
}

fn local_ms(y: i32, m0: u32, d: u32, h: u32, min: u32, s: u32, runtime: &OperationContext) -> i64 {
    let local = CivilDate::new(y, m0 + 1, d)
        .and_then(|date| date.at_hms(h, min, s))
        .expect("valid test civil datetime");
    runtime
        .local_time_zone()
        .resolve_local(local)
        .expect("test datetime is supported by the active timezone")
        .timestamp_millis()
}

fn utc_ms(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> i64 {
    let utc = CivilDate::new(y, m, d)
        .and_then(|date| date.at_hms(h, min, s))
        .and_then(|civil| OffsetDateTime::from_local(civil, UtcOffset::UTC))
        .expect("valid test UTC datetime");
    utc.timestamp_millis()
}

#[test]
fn gantt_task_finalization_observes_cancellation_inside_large_task_sets() {
    let runtime = runtime();
    let mut db = large_gantt_db(512);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(db.finalize_tasks_controlled(&runtime, &control).is_err());
}

#[cfg(feature = "presentation")]
#[test]
fn gantt_model_projection_observes_cancellation_inside_large_task_sets() {
    let runtime = runtime();
    let mut db = large_gantt_db(512);
    db.finalize_tasks_controlled(&runtime, &OperationControl::new())
        .unwrap()
        .unwrap();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(super::parse::gantt_db_to_render_model_controlled(db, &runtime, &control).is_err());
}

#[test]
fn gantt_js_date_fallback_year_bounds_match_upstream_guardrail() {
    let runtime = runtime();
    let dt = parse_js_date_fallback("10000", &runtime).unwrap();
    assert_eq!(dt.year(), 10000);

    let err = parse_js_date_fallback("10001", &runtime).unwrap_err();
    assert!(err.message.contains("Invalid date:10001"));
}

#[test]
fn gantt_js_date_fallback_keeps_ecmascript_expanded_years_distinct_from_civil_dates() {
    let runtime = runtime_in(LocalTimeZone::fixed(8 * 60).unwrap());
    for invalid in [
        "-0001-01-01",
        "+0001-01-01",
        "+10000-01-01",
        "-10000-01-01",
        "+010001-01-01",
        "-000000-01-01",
        "-000000-01-01T00:00:00",
        "-000000-01-01T00:00:00Z",
        "-000000-01-01T00:00:00+08:00",
    ] {
        assert!(
            parse_js_date_fallback(invalid, &runtime).is_err(),
            "{invalid}"
        );
    }

    assert_eq!(
        parse_js_date_fallback("+002024-01-01", &runtime)
            .unwrap()
            .timestamp_millis(),
        utc_ms(2024, 1, 1, 0, 0, 0)
    );
    assert_eq!(
        parse_js_date_fallback("-000001-01-01", &runtime)
            .unwrap()
            .year(),
        -1
    );
    assert_eq!(
        parse_js_date_fallback("+010000-01-01", &runtime)
            .unwrap()
            .year(),
        10_000
    );
    assert_eq!(
        parse_js_date_fallback("-010000-01-01", &runtime)
            .unwrap()
            .year(),
        -10_000
    );

    assert_eq!(
        parse_js_date_fallback("+002024-01-01T00:00:00", &runtime)
            .unwrap()
            .timestamp_millis(),
        local_ms(2024, 0, 1, 0, 0, 0, &runtime)
    );
    assert_eq!(
        parse_js_date_fallback("+002024-01-01T00:00:00Z", &runtime)
            .unwrap()
            .timestamp_millis(),
        utc_ms(2024, 1, 1, 0, 0, 0)
    );
}

#[cfg(feature = "system-timezone")]
#[test]
fn gantt_js_date_fallback_supports_upper_guardrail_in_system_timezone() {
    let runtime = runtime_in(LocalTimeZone::try_system().expect("system time-zone adapter"));
    let dt = parse_js_date_fallback("10000", &runtime).unwrap();
    assert_eq!(dt.year(), 10000);
}

#[test]
fn gantt_js_date_fallback_parses_mdy_hm_strings_like_v8() {
    let runtime = runtime();
    // Mermaid's gantt parser falls back to `new Date(str)`; in V8, the string
    // `08-08-09-01:00` parses as local time `2009-08-08 01:00`.
    let dt = parse_js_date_fallback("08-08-09-01:00", &runtime).unwrap();
    assert_eq!(dt.year(), 2009);
    assert_eq!(dt.month(), 8);
    assert_eq!(dt.day(), 8);
    assert_eq!(dt.hour(), 1);
    assert_eq!(dt.minute(), 0);
}

#[test]
fn gantt_js_date_fallback_rejects_invalid_calendar_dates() {
    let runtime = runtime();
    let err = parse_js_date_fallback("2019-02-30", &runtime).unwrap_err();
    assert!(err.message.contains("Invalid date:2019-02-30"));
}

#[test]
fn gantt_parse_duration_matches_upstream_examples() {
    assert_eq!(parse_duration("1d"), (1.0, "d".to_string()));
    assert!(parse_duration("1f").0.is_nan());
    assert_eq!(parse_duration("0.1s"), (0.1, "s".to_string()));
    assert_eq!(parse_duration("1ms"), (1.0, "ms".to_string()));
    assert_eq!(parse_duration(" 2m "), (2.0, "m".to_string()));
    assert_eq!(parse_duration("01M"), (1.0, "M".to_string()));

    for invalid in [".1s", "1.s", "1MS", "1msx", "1 ms", ""] {
        let (value, unit) = parse_duration(invalid);
        assert!(value.is_nan(), "expected invalid duration for {invalid:?}");
        assert_eq!(unit, "ms");
    }
}

#[test]
fn gantt_duration_arithmetic_rejects_unrepresentable_ranges_without_panicking() {
    let runtime = runtime();
    let base =
        local_from_civil(CivilDate::new(2026, 1, 1).unwrap().at_midnight(), &runtime).unwrap();
    let max = OffsetDateTime::from_unix_millis(i64::MAX, UtcOffset::UTC);

    assert!(add_days_local(base, i64::MIN, &runtime).is_none());
    assert!(add_months_local(base, i64::MAX, &runtime).is_none());
    assert!(add_months_local(base, i64::MIN, &runtime).is_none());
    assert!(add_years_local(base, i64::MAX, &runtime).is_none());
    assert!(add_years_local(base, i64::MIN, &runtime).is_none());
    let db = GanttDb::default();
    assert_eq!(
        get_end_date(&db, max, "x", "1ms", false, &runtime).unwrap(),
        Some(max)
    );
    assert_eq!(
        get_end_date(
            &db,
            base,
            "x",
            "1000000000000000000000000000000M",
            false,
            &runtime
        )
        .unwrap(),
        Some(base)
    );
    assert_eq!(
        get_end_date(
            &db,
            base,
            "x",
            "1000000000000000000000000000000y",
            false,
            &runtime
        )
        .unwrap(),
        Some(base)
    );
}

#[test]
fn gantt_fractional_calendar_setters_reject_an_invalid_dayjs_month_end() {
    let runtime = runtime();
    let db = GanttDb::default();
    for (year, month, duration) in [(275760, 8, "1.5M"), (275759, 9, "1.5y")] {
        let start = local_from_civil(
            CivilDate::new(year, month, 13).unwrap().at_midnight(),
            &runtime,
        )
        .unwrap();
        let end = get_end_date(&db, start, "x", duration, false, &runtime)
            .unwrap()
            .unwrap();
        assert_eq!(end, start, "{duration}");
    }
}

#[test]
fn gantt_javascript_time_clip_accepts_only_the_inclusive_endpoints() {
    for milliseconds in [-JS_MAX_TIME_MILLIS, JS_MAX_TIME_MILLIS] {
        let datetime = js_date_from_unix_millis(milliseconds)
            .expect("JavaScript TimeClip endpoint must remain representable");
        assert_eq!(datetime.timestamp_millis(), milliseconds);
    }

    assert!(js_date_from_unix_millis(-JS_MAX_TIME_MILLIS - 1).is_none());
    assert!(js_date_from_unix_millis(JS_MAX_TIME_MILLIS + 1).is_none());
}

#[test]
fn dayjs_strict_parses_month_names_and_offsets() {
    let runtime = runtime();
    let dt = parse_dayjs_like_strict("YYYY-MMM-DD", "2013-Jan-02", &runtime).unwrap();
    assert_eq!(
        dt.timestamp_millis(),
        local_ms(2013, 0, 2, 0, 0, 0, &runtime)
    );

    let dt = parse_dayjs_like_strict(
        "YYYY-MM-DDTHH:mm:ssZ",
        "2013-01-01T00:00:00+00:00",
        &runtime,
    )
    .unwrap();
    assert_eq!(dt.timestamp_millis(), utc_ms(2013, 1, 1, 0, 0, 0));

    let dt = parse_dayjs_like_strict(
        "YYYY-MM-DDTHH:mm:ssZ",
        "2013-01-01T00:00:00+08:00",
        &runtime,
    )
    .unwrap();
    assert_eq!(dt.timestamp_millis(), utc_ms(2012, 12, 31, 16, 0, 0));

    let dt = parse_dayjs_like_strict(
        "YYYY-MM-DDTHH:mm:ssZZ",
        "2013-01-01T00:00:00+0800",
        &runtime,
    )
    .unwrap();
    assert_eq!(dt.timestamp_millis(), utc_ms(2012, 12, 31, 16, 0, 0));

    let dt =
        parse_dayjs_like_strict("YYYY-MM-DDTHH:mm:ssZ", "2013-01-01T00:00:00Z", &runtime).unwrap();
    assert_eq!(dt.timestamp_millis(), utc_ms(2013, 1, 1, 0, 0, 0));
}

#[test]
fn gantt_date_parsers_reject_utf8_at_byte_cutoffs_without_panicking() {
    let runtime = runtime();
    assert!(parse_dayjs_like_strict("YYYY", "你好", &runtime).is_none());
    assert!(parse_js_date_fallback("2024-01-01T00:你", &runtime).is_err());
}

#[test]
fn dayjs_strict_rejects_rather_than_panics_on_multibyte_input() {
    let runtime = runtime();
    // `parse_int_exact` used to slice at a byte offset without checking char
    // boundaries, panicking on multi-byte (e.g. Japanese) input.
    assert!(parse_dayjs_like_strict("YYYY-MM-DD", "日本語のテキスト", &runtime).is_none());
    assert!(parse_dayjs_like_strict("MM-DD", "日本-24", &runtime).is_none());
    assert!(parse_dayjs_like_strict("YYYY-MMM-DD", "２０１３-Jan-０２", &runtime).is_none());
}

#[test]
fn js_date_fallback_rejects_rather_than_panics_on_multibyte_timezone_suffix() {
    let runtime = runtime();
    // Same byte-boundary bug as above, reached via the timezone-offset tail
    // of `parse_js_date_fallback`.
    assert!(parse_js_date_fallback("2013-01-01T00:00:00+日本語", &runtime).is_err());
    assert!(parse_js_date_fallback("2013-01-01T00:00+タ", &runtime).is_err());
}

#[test]
fn gantt_js_date_fallback_parses_timezone_offsets_with_or_without_colon() {
    let runtime = runtime();
    let dt = parse_js_date_fallback("2013-01-01T00:00:00+0800", &runtime).unwrap();
    assert_eq!(dt.timestamp_millis(), utc_ms(2012, 12, 31, 16, 0, 0));

    let dt = parse_js_date_fallback("2013-01-01T00:00:00+08:00", &runtime).unwrap();
    assert_eq!(dt.timestamp_millis(), utc_ms(2012, 12, 31, 16, 0, 0));

    let dt = parse_js_date_fallback("2013-01-01T00:00:00Z", &runtime).unwrap();
    assert_eq!(dt.timestamp_millis(), utc_ms(2013, 1, 1, 0, 0, 0));
}

#[test]
fn gantt_js_date_fallback_parses_slash_dates_as_local() {
    let runtime = runtime();
    let dt = parse_js_date_fallback("2013/01/01", &runtime).unwrap();
    assert_eq!(
        dt.timestamp_millis(),
        local_ms(2013, 0, 1, 0, 0, 0, &runtime)
    );

    let dt = parse_js_date_fallback("2013/01/01 00:00:00", &runtime).unwrap();
    assert_eq!(
        dt.timestamp_millis(),
        local_ms(2013, 0, 1, 0, 0, 0, &runtime)
    );
}

#[test]
fn gantt_relative_ref_ids_preserve_source_regex_space_backtracking() {
    assert_eq!(relative_ref_ids("after id_1-2", "after"), Some("id_1-2"));
    assert_eq!(relative_ref_ids("after  #", "after"), Some(" "));
    assert_eq!(relative_ref_ids("after \t #", "after"), Some(" "));
    assert_eq!(relative_ref_ids("after #", "after"), None);
    assert_eq!(relative_ref_ids("After id1", "after"), None);
}

#[test]
fn dayjs_unix_seconds_floor_negative_milliseconds() {
    let runtime = runtime();
    for (milliseconds, expected) in [
        (-1_001, "-2"),
        (-1_000, "-1"),
        (-999, "-1"),
        (-1, "-1"),
        (0, "0"),
        (999, "0"),
        (1_000, "1"),
    ] {
        let datetime = OffsetDateTime::from_unix_millis(milliseconds, UtcOffset::UTC);
        assert_eq!(format_dayjs_like(datetime, "X", &runtime), expected);
    }
}
