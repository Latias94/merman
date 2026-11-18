#![cfg(feature = "diagram-gantt")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiOutput, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiSourceContext, AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::gantt::{GanttDiagramModel, GanttTask, GanttTerminalView};
use merman_model::runtime::RuntimePolicy;
use merman_model::{MermaidConfig, OperationControl, ParsedSemanticDiagram, SemanticDiagramModel};
use unicode_width::UnicodeWidthStr;

const DELIVERY: &str = concat!(
    "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\n",
    "Design: done,design,2026-01-01,2d\n",
    "Build: active,crit,build,after design,4d\n",
    "Review: review,2026-01-03,3d\n",
    "Release: milestone,release,2026-01-09,0d\n",
);

fn parse(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .with_runtime_policy(RuntimePolicy::deterministic())
        .parse_diagram_sync(source)
        .unwrap()
}

fn model(parsed: &ParsedSemanticDiagram) -> &GanttDiagramModel {
    let SemanticDiagramModel::Gantt(model) = &parsed.model else {
        panic!("expected Gantt model")
    };
    model
}

fn render(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    width: usize,
    overflow: OverflowPolicy,
) -> merman_ascii::Result<AsciiOutput> {
    AsciiRenderer::new(options)?.render_parsed_report(
        parsed,
        AsciiViewportPolicy::with_max_width(width).overflow(overflow),
        &OperationControl::new(),
        &RuntimePolicy::deterministic().begin_operation().unwrap(),
        AsciiResourcePolicy::default(),
    )
}

fn detached(model: &GanttDiagramModel) -> AsciiOutput {
    let config = MermaidConfig::empty_object();
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model_report(
            &AsciiDiagram::Gantt(GanttTerminalView::from_model(model, &config)),
            AsciiSourceContext {
                diagram_type: "gantt",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn scene(output: &AsciiOutput) -> &str {
    assert_eq!(
        output.projection,
        AsciiProjection::Diagrammatic,
        "{}",
        output.text
    );
    output.text.split_once("Gantt records").unwrap().0
}

fn task_row<'a>(scene: &'a str, key: &str) -> &'a str {
    scene
        .lines()
        .find(|row| row.starts_with(&format!("{key} ")) && row.contains(" | "))
        .unwrap_or_else(|| panic!("missing {key}:\n{scene}"))
}

fn plot<'a>(scene: &'a str, key: &str) -> &'a str {
    task_row(scene, key)
        .split_once(" | ")
        .unwrap()
        .1
        .trim_end_matches('|')
}

fn column(plot: &str, marker: char) -> usize {
    plot.chars().position(|ch| ch == marker).unwrap()
}

#[test]
fn common_axis_exposes_concurrency_dependencies_status_and_milestones() {
    let parsed = parse(DELIVERY);
    for width in [60, 80, 100, 120] {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let output = render(&parsed, options, width, OverflowPolicy::Error).unwrap();
            assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
            let scene = scene(&output);
            let design = plot(scene, "T1");
            let build = plot(scene, "T2");
            let review = plot(scene, "T3");
            let release = plot(scene, "T4");
            assert_eq!(column(design, '['), 0);
            assert_eq!(column(design, ']'), column(build, '['));
            assert_eq!(column(review, '['), column(build, '['));
            assert!(column(review, ']') < column(build, ']'));
            assert_eq!(column(release, 'o') + 1, release.chars().count());
            assert!(task_row(scene, "T1").starts_with("T1 D"));
            assert!(task_row(scene, "T2").starts_with("T2  A!"));
            let records: String = output.text.split_whitespace().collect();
            for field in [
                "T1[task]id=\"design\"",
                "T2[task]id=\"build\"",
                "T3[task]id=\"review\"",
                "T4[task]id=\"release\"",
                "startConstraint:{\"kind\":\"after\",\"dependencyIds\":[\"design\"]}",
                "milestone:true",
            ] {
                assert!(records.contains(field), "missing {field}:\n{}", output.text);
            }
            assert!(output.emitted_extent.width <= width);
            assert_eq!(
                support::terminal_extent(&output.text),
                (output.emitted_extent.width, output.emitted_extent.height)
            );
        }
    }
}

#[test]
fn milestone_uses_scheduled_midpoint_not_start_or_visible_end() {
    let parsed = parse(concat!(
        "gantt\ndateFormat YYYY-MM-DD\n",
        "Window: w,2026-01-01,12d\n",
        "Checkpoint: milestone,m,2026-01-05,4d\n",
    ));
    let output = render(
        &parsed,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    )
    .unwrap();
    let text = scene(&output);
    let window = plot(text, "T1");
    let point = plot(text, "T2");
    assert!(column(point, 'o').abs_diff(window.len() / 2) <= 1, "{text}");
    assert_eq!(
        model(&parsed).tasks[1].end_ms - model(&parsed).tasks[1].start_ms,
        4 * 86_400_000
    );
}

#[test]
fn excludes_change_visible_bar_end_without_compressing_dependency_time() {
    let parsed = parse(concat!(
        "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\n",
        "Friday: friday,2019-02-01,1d\n",
        "Monday: monday,after friday,1d\n",
    ));
    let tasks = &model(&parsed).tasks;
    assert_eq!(tasks[1].start_ms, tasks[0].end_ms);
    assert!(tasks[0].render_end_ms.unwrap() < tasks[0].end_ms);
    let output = render(
        &parsed,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    )
    .unwrap();
    let text = scene(&output);
    assert!(
        column(plot(text, "T1"), ']') < column(plot(text, "T2"), '['),
        "{text}"
    );
    let records: String = output.text.split_whitespace().collect();
    assert!(records.contains("renderEndTime:1549065600000"));
    assert!(records.contains("endTime:1549238400000"));
    assert!(records.contains("excludes:[\"weekends\"]"));
}

#[test]
fn inclusive_end_dates_are_consumed_once_and_duration_rules_remain_exact() {
    let parsed = parse(concat!(
        "gantt\ndateFormat YYYY-MM-DD\ninclusiveEndDates\n",
        "Fixed: fixed,2026-01-01,2026-01-02\n",
        "Duration: duration,2026-01-01,2d\n",
    ));
    assert_eq!(
        model(&parsed).tasks[0].end_ms,
        model(&parsed).tasks[1].end_ms
    );
    let output = render(
        &parsed,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    )
    .unwrap();
    assert_eq!(plot(scene(&output), "T1"), plot(scene(&output), "T2"));
}

#[test]
fn non_utc_operation_axis_and_utc_records_preserve_the_same_instants() {
    let runtime = RuntimePolicy::deterministic()
        .with_local_time_zone(merman_model::time::LocalTimeZone::fixed(8 * 60).unwrap());
    let parsed = SemanticParser::new()
        .with_runtime_policy(runtime.clone())
        .parse_diagram_sync("gantt\ndateFormat YYYY-MM-DD\nWork: w,2026-01-01,1d\n")
        .unwrap();
    let output = AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_parsed_report(
            &parsed,
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Error),
            &OperationControl::new(),
            &runtime.begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
    assert!(scene(&output).contains("From 2026-01-01 +08:00 to 2026-01-02 +08:00"));
    let records: String = output.text.split_whitespace().collect();
    assert!(records.contains("T1interval(UTC):2025-12-31T16:00:00.000->2026-01-01T16:00:00.000"));
    for value in [
        format!("startTime:{}", model(&parsed).tasks[0].start_ms),
        format!("endTime:{}", model(&parsed).tasks[0].end_ms),
    ] {
        assert!(records.contains(&value));
    }
}

#[test]
fn repeated_sections_and_reverse_time_order_keep_source_identity() {
    let parsed = parse(concat!(
        "gantt\ndateFormat YYYY-MM-DD\nsection A\n",
        "Late: late,2026-01-09,1d\nsection B\n",
        "Early: early,2026-01-01,1d\nsection A\n",
        "Middle: middle,2026-01-05,1d\n",
    ));
    let output = render(
        &parsed,
        AsciiRenderOptions::ascii(),
        80,
        OverflowPolicy::Error,
    )
    .unwrap();
    let text = scene(&output);
    assert!(text.find("Section 1: \"A\"").unwrap() < text.find("T1 ").unwrap());
    assert!(text.find("T1 ").unwrap() < text.find("Section 2: \"B\"").unwrap());
    assert!(text.find("T2 ").unwrap() < text.find("Section 3: \"A\"").unwrap());
    assert!(column(plot(text, "T2"), '[') < column(plot(text, "T3"), '['));
    assert!(column(plot(text, "T3"), '[') < column(plot(text, "T1"), '['));
}

#[test]
fn unsafe_long_and_cjk_labels_cannot_move_the_time_axis() {
    let data = model(&parse(DELIVERY)).clone();
    for label in [
        "接收请求 · Ω",
        "👩‍💻 审查",
        "A very long label that needs exact records",
        "forged\n| row\u{1b}[2J",
    ] {
        let mut owned = data.clone();
        owned.tasks[0].task = label.to_string();
        let config = MermaidConfig::empty_object();
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let options = AsciiRenderOptions::unicode().with_terminal_width_profile(profile);
            let output = AsciiRenderer::new(options)
                .unwrap()
                .render_model_report(
                    &AsciiDiagram::Gantt(GanttTerminalView::from_model(&owned, &config)),
                    AsciiSourceContext {
                        diagram_type: "gantt",
                        config: &config,
                        semantic_config: &config,
                        title: None,
                    },
                    AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Error),
                    &OperationControl::new(),
                    &RuntimePolicy::deterministic().begin_operation().unwrap(),
                    AsciiResourcePolicy::default(),
                )
                .unwrap();
            let text = scene(&output);
            for key in ["T1", "T2", "T3", "T4"] {
                let (left, _) = task_row(text, key).split_once(" | ").unwrap();
                let width = if profile == TerminalWidthProfile::Cjk {
                    UnicodeWidthStr::width_cjk(left)
                } else {
                    UnicodeWidthStr::width(left)
                };
                assert_eq!(width, 26, "{text}");
            }
            assert!(!output.text.contains('\u{1b}'));
            assert_eq!(
                support::terminal_extent_with_profile(&output.text, profile),
                (output.emitted_extent.width, output.emitted_extent.height)
            );
        }
    }
}

#[test]
fn nonspatial_domains_are_complete_text_primaries_and_narrow_charts_use_fallback() {
    for source in [
        "gantt\n",
        "gantt\ndateFormat YYYY-MM-DD\nPoint: p,2026-01-01,0d\n",
        "gantt\ndateFormat YYYY-MM-DD\nMarker: vert,v,2026-01-01,1d\n",
        "---\nconfig:\n  gantt:\n    displayMode: compact\n---\ngantt\ndateFormat YYYY-MM-DD\nWork: w,2026-01-01,2d\n",
    ] {
        let output = render(
            &parse(source),
            AsciiRenderOptions::ascii(),
            80,
            OverflowPolicy::Error,
        )
        .unwrap();
        assert_eq!(
            output.projection,
            AsciiProjection::StructuredText,
            "{}",
            output.text
        );
        assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
        assert!(!output.fallback.attempted);
        assert!(output.text.contains("gantt details:"));
        assert!(!output.text.contains("Gantt timeline"));
    }
    let parsed = parse(DELIVERY);
    let output = render(
        &parsed,
        AsciiRenderOptions::ascii(),
        30,
        OverflowPolicy::Fallback,
    )
    .unwrap();
    assert_eq!(output.projection, AsciiProjection::StructuredText);
    assert_eq!(output.outcome, AsciiOutputOutcome::Fallback);
    assert!(output.fallback.attempted);
    assert!(!output.emitted_overflowed);
    for index in 1..=4 {
        assert!(
            output
                .text
                .split_whitespace()
                .collect::<String>()
                .contains(&format!("T{index}[task]"))
        );
    }
    assert!(matches!(
        render(
            &parsed,
            AsciiRenderOptions::ascii(),
            30,
            OverflowPolicy::Error
        ),
        Err(AsciiError::WidthOverflow { .. })
    ));
}

#[test]
fn invalid_detached_schedule_facts_are_disclosed_without_fabricated_geometry() {
    let data = model(&parse(DELIVERY)).clone();
    for variant in 0..6 {
        let mut altered = data.clone();
        match variant {
            0 => altered.tasks[0].processed = false,
            1 => altered.tasks[0].order = 9,
            2 => altered.tasks[0].end_ms = altered.tasks[0].start_ms - 1,
            3 => altered.tasks[0].render_end_ms = Some(altered.tasks[0].end_ms + 1),
            4 => altered.tasks[0].render_end_ms = Some(altered.tasks[0].start_ms - 1),
            _ => {
                altered.sections = vec!["Later".into(), "Delivery".into()];
                altered.tasks[0].section_index = Some(1);
                altered.tasks[1].section = "Later".into();
                altered.tasks[1].section_index = Some(0);
                altered.tasks[2].section_index = Some(1);
                altered.tasks[3].section_index = Some(1);
            }
        }
        let output = detached(&altered);
        assert_eq!(output.projection, AsciiProjection::StructuredText);
        assert!(output.text.contains("T4 [task]"));
        assert!(!output.text.contains("Gantt timeline"));
    }
}

#[test]
fn subcell_intervals_and_full_i64_domain_keep_exact_facts() {
    let data = GanttDiagramModel {
        tasks: vec![
            GanttTask {
                id: "range".into(),
                task: "Range".into(),
                start_ms: i64::MIN,
                end_ms: i64::MAX,
                processed: true,
                ..Default::default()
            },
            GanttTask {
                id: "tiny".into(),
                task: "Tiny".into(),
                start_ms: 0,
                end_ms: 1,
                order: 1,
                processed: true,
                ..Default::default()
            },
            GanttTask {
                id: "point".into(),
                task: "Point".into(),
                start_ms: -1,
                end_ms: 0,
                order: 2,
                processed: true,
                milestone: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let output = detached(&data);
    let text = scene(&output);
    assert_eq!(column(plot(text, "T1"), '['), 0);
    assert_eq!(column(plot(text, "T1"), ']') + 1, plot(text, "T1").len());
    assert_eq!(plot(text, "T2").matches('*').count(), 1);
    assert_eq!(plot(text, "T3").matches('o').count(), 1);
    let records: String = output.text.split_whitespace().collect();
    assert!(records.contains("startTime:-9223372036854775808"));
    assert!(records.contains("endTime:9223372036854775807"));
    assert!(records.contains("startTime:0"));
    assert!(records.contains("endTime:1"));
}

#[test]
fn primary_encodings_keep_the_same_measured_geometry_and_styled_fallback_is_rejected() {
    let parsed = parse(DELIVERY);
    let mut extent = None;
    for color in [
        AsciiColorMode::Plain,
        AsciiColorMode::Ansi16,
        AsciiColorMode::Ansi256,
        AsciiColorMode::TrueColor,
        AsciiColorMode::Html,
    ] {
        let options = AsciiRenderOptions::unicode().with_color_mode(color);
        let output = render(&parsed, options, 80, OverflowPolicy::Error).unwrap();
        assert_eq!(output.projection, AsciiProjection::Diagrammatic);
        assert_eq!(
            output.encoding,
            merman_ascii::AsciiOutputEncoding::from_color_mode(color)
        );
        if let Some(expected) = extent {
            assert_eq!(output.emitted_extent, expected);
        }
        extent = Some(output.emitted_extent);
        if color != AsciiColorMode::Plain {
            assert!(render(&parsed, options, 80, OverflowPolicy::Fallback).is_err());
        }
    }
}

#[test]
fn small_typical_and_dense_schedules_keep_each_task_and_record_at_admission_widths() {
    let mut dense = String::from("gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n");
    for index in 0..24 {
        if index % 8 == 0 {
            dense.push_str(&format!("section Stage {}\n", index / 8 + 1));
        }
        dense.push_str(&format!(
            "Task {index}: t{index},2026-01-{:02},{}d\n",
            index % 12 + 1,
            index % 5 + 1,
        ));
    }
    for source in [
        "gantt\ndateFormat YYYY-MM-DD\nDesign: design,2026-01-01,1d\nBuild: build,after design,2d\n",
        DELIVERY,
        &dense,
    ] {
        let parsed = parse(source);
        let tasks = &model(&parsed).tasks;
        for width in [80, 100, 120] {
            for options in [
                AsciiRenderOptions::ascii(),
                AsciiRenderOptions::unicode(),
                AsciiRenderOptions::unicode()
                    .with_terminal_width_profile(TerminalWidthProfile::Cjk),
            ] {
                let output = render(&parsed, options, width, OverflowPolicy::Error).unwrap();
                let text = scene(&output);
                let records: String = output
                    .text
                    .split_once("Gantt records")
                    .unwrap()
                    .1
                    .split_whitespace()
                    .collect();
                assert_eq!(records.matches("[task]").count(), tasks.len());
                for (index, task) in tasks.iter().enumerate() {
                    let key = format!("T{}", index + 1);
                    assert_eq!(plot(text, &key).chars().count(), width - 30);
                    assert!(
                        plot(text, &key)
                            .chars()
                            .any(|ch| ch == '[' || ch == '*' || ch == 'o')
                    );
                    assert!(records.contains(&format!("{key}[task]id=\"{}\"", task.id)));
                    assert!(records.contains(&format!("{key}details:")));
                }
                assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
                assert_eq!(
                    support::terminal_extent_with_profile(
                        &output.text,
                        options.terminal_width_profile
                    ),
                    (output.emitted_extent.width, output.emitted_extent.height)
                );
            }
        }
    }
}

#[test]
fn deadlines_cancellation_and_explicit_work_limits_do_not_become_text_successes() {
    let parsed = parse(DELIVERY);
    let renderer = AsciiRenderer::new(AsciiRenderOptions::ascii()).unwrap();
    let control = OperationControl::new();
    control.cancel();
    let runtime = RuntimePolicy::deterministic().begin_operation().unwrap();
    assert!(matches!(
        renderer.render_parsed_report(
            &parsed,
            AsciiViewportPolicy::unrestricted(),
            &control,
            &runtime,
            AsciiResourcePolicy::default()
        ),
        Err(AsciiError::Cancelled(_))
    ));
    let deadline = OperationControl::new().with_deadline(std::time::Duration::ZERO);
    assert!(
        matches!(renderer.render_parsed_report(&parsed, AsciiViewportPolicy::unrestricted(), &deadline, &runtime, AsciiResourcePolicy::default()), Err(AsciiError::Cancelled(cancelled)) if cancelled.reason == merman_model::CancelReason::DeadlineExceeded)
    );
    let policy = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    assert!(matches!(
        renderer.render_parsed_report(
            &parsed,
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &runtime,
            policy
        ),
        Err(AsciiError::ResourceLimitExceeded(_))
    ));
}
