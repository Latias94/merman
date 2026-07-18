#![cfg(all(feature = "render", feature = "core-host", unix))]

use std::{process::Command, sync::Arc};

use merman::{
    Engine, ParseOptions, RenderSemanticModel,
    render::{HeadlessRenderer, RenderClock, RenderEnvironment},
};

const CHILD_PROCESS: &str = "MERMAN_DST_TEST_CHILD";
const PROVENANCE_CHILD: &str = "MERMAN_TZ_PROVENANCE_CHILD";
const SOURCE: &str = "gantt\ndateFormat YYYY-MM-DD\nsection Winter\nTask: task, 2026-01-15, 1d";
const GAP_SOURCE: &str =
    "gantt\ndateFormat YYYY-MM-DD HH:mm\nsection Gap\nTask: task, 2026-03-08 02:30, 1h";

struct JulyNewYorkClock;

impl RenderClock for JulyNewYorkClock {
    fn unix_millis_and_offset(&self) -> (i64, i32) {
        // SystemTimeZone derives the coherent offset from this instant.
        (1_784_390_400_000, 0)
    }
}

struct WinterMountainClock;

impl RenderClock for WinterMountainClock {
    fn unix_millis_and_offset(&self) -> (i64, i32) {
        (1_768_478_400_000, 0)
    }
}

#[test]
fn system_timezone_resolves_target_date_dst_in_parse_and_layout() {
    if let Some(time_zone) = std::env::var_os(PROVENANCE_CHILD) {
        emit_timezone_provenance(&time_zone.to_string_lossy());
        return;
    }
    if std::env::var_os(CHILD_PROCESS).is_some() {
        assert_new_york_winter_semantics();
        return;
    }

    let status = Command::new(std::env::current_exe().expect("current test executable"))
        .arg("--exact")
        .arg("system_timezone_resolves_target_date_dst_in_parse_and_layout")
        .arg("--nocapture")
        .env(CHILD_PROCESS, "1")
        .env("TZ", "America/New_York")
        .status()
        .expect("spawn isolated timezone test");
    assert!(status.success(), "isolated timezone test failed: {status}");
}

fn assert_new_york_winter_semantics() {
    let winter_midnight = merman::render::RenderTimeSnapshot::from_system_local_midnight(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid winter date"),
    )
    .expect("resolve New York winter midnight");
    assert_eq!(winter_midnight.unix_ms(), 1_768_453_200_000);
    assert_eq!(winter_midnight.local_offset_minutes(), -5 * 60);

    let environment =
        RenderEnvironment::host().with_system_time_zone_clock(Arc::new(JulyNewYorkClock));
    let renderer = HeadlessRenderer::new().with_environment(environment);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
        .expect("parse Gantt")
        .expect("detect Gantt");
    let RenderSemanticModel::Gantt(model) = parsed.model else {
        panic!("expected Gantt semantic model");
    };
    assert_eq!(model.tasks[0].start_ms, 1_768_453_200_000);

    let gap = Engine::new()
        .parse_diagram_for_render_model_sync(GAP_SOURCE, ParseOptions::strict())
        .expect("parse DST gap Gantt")
        .expect("detect DST gap Gantt");
    let RenderSemanticModel::Gantt(gap_model) = gap.model else {
        panic!("expected Gantt semantic model");
    };
    assert_eq!(
        gap_model.tasks[0].start_ms, 1_772_955_000_000,
        "02:30 in the New York spring gap must normalize to 03:30 EDT"
    );

    let layout = renderer
        .layout_json_sync(SOURCE)
        .expect("layout Gantt")
        .expect("detect Gantt layout");
    assert_eq!(
        layout.pointer("/layout/GanttDiagram/tasks/0/start_ms"),
        Some(&serde_json::json!(1_768_453_200_000_i64))
    );

    let gap_layout = renderer
        .layout_json_sync(GAP_SOURCE)
        .expect("layout DST gap Gantt")
        .expect("detect DST gap Gantt layout");
    assert_eq!(
        gap_layout.pointer("/layout/GanttDiagram/tasks/0/start_ms"),
        Some(&serde_json::json!(1_772_955_000_000_i64))
    );

    assert_timezone_rules_are_captured_and_reported();
}

fn assert_timezone_rules_are_captured_and_reported() {
    let denver = capture_timezone_provenance("America/Denver");
    let phoenix = capture_timezone_provenance("America/Phoenix");

    assert_eq!(denver.0, phoenix.0, "winter snapshots must be identical");
    assert_ne!(denver.1, phoenix.1, "rule reports must be distinguishable");
    assert_eq!(denver.2, -6 * 60 * 60);
    assert_eq!(phoenix.2, -7 * 60 * 60);
}

fn capture_timezone_provenance(time_zone: &str) -> (String, String, i32) {
    let output = Command::new(std::env::current_exe().expect("current test executable"))
        .arg("--exact")
        .arg("system_timezone_resolves_target_date_dst_in_parse_and_layout")
        .arg("--nocapture")
        .env(PROVENANCE_CHILD, time_zone)
        .env("TZ", time_zone)
        .output()
        .expect("spawn isolated timezone provenance test");
    assert!(
        output.status.success(),
        "timezone provenance child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| line.starts_with("MERMAN_TIME_ZONE_EVIDENCE|"))
        .unwrap_or_else(|| panic!("missing timezone evidence in child output: {stdout}"));
    let mut fields = line.split('|');
    assert_eq!(fields.next(), Some("MERMAN_TIME_ZONE_EVIDENCE"));
    assert_eq!(fields.next(), Some(time_zone));
    let snapshot = fields.next().expect("snapshot evidence").to_string();
    let rules = fields.next().expect("rules evidence").to_string();
    let summer_offset = fields
        .next()
        .expect("summer offset evidence")
        .parse()
        .expect("numeric summer offset");
    (snapshot, rules, summer_offset)
}

fn emit_timezone_provenance(expected_identifier: &str) {
    let environment =
        RenderEnvironment::host().with_system_time_zone_clock(Arc::new(WinterMountainClock));
    let session = environment.begin_session().expect("capture timezone rules");
    let report = session.report();
    assert_eq!(report.local_time_zone().identifier(), expected_identifier);
    let summer = chrono::NaiveDate::from_ymd_opt(2026, 7, 15)
        .expect("valid summer date")
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight");
    let summer_offset = session
        .local_time_zone()
        .datetime_from_naive_local(summer)
        .expect("summer midnight")
        .offset()
        .local_minus_utc();
    println!(
        "MERMAN_TIME_ZONE_EVIDENCE|{}|{}:{}:{}|{}|{}",
        report.local_time_zone().identifier(),
        report.time().unix_ms(),
        report.time().local_date(),
        report.time().local_offset_minutes(),
        report
            .local_time_zone()
            .rules_sha256()
            .expect("system rules digest"),
        summer_offset
    );
}
