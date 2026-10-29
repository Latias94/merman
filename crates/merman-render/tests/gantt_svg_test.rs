use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::GanttDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn layout_gantt_from_text(text: &str) -> GanttDiagramLayout {
    layout_gantt_from_text_at_container_width(text, LayoutOptions::default().container_width)
}

fn layout_gantt_from_text_at_container_width(
    text: &str,
    container_width: f64,
) -> GanttDiagramLayout {
    let environment = RenderEnvironment::deterministic();
    layout_gantt_from_text_with_environment(text, container_width, &environment)
}

fn layout_gantt_from_text_with_environment(
    text: &str,
    container_width: f64,
    environment: &RenderEnvironment,
) -> GanttDiagramLayout {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let mut options = LayoutOptions::default();
    options.container_width = container_width;
    let session = environment.begin_session().expect("render session");
    let artifact = family::prepare(parsed, &options, session).expect("layout ok");
    let projection = artifact.layout_json().expect("Gantt layout projection");
    serde_json::from_value(projection["layout"]["GanttDiagram"].clone()).expect("Gantt layout")
}

#[test]
fn gantt_layout_uses_the_operation_container_width_unless_config_overrides_it() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: 2024-01-01, 1d";
    let narrow = layout_gantt_from_text_at_container_width(source, 640.0);
    let wide = layout_gantt_from_text_at_container_width(source, 960.0);

    assert_eq!(narrow.width, 640.0);
    assert_eq!(wide.width, 960.0);

    let configured = layout_gantt_from_text_at_container_width(
        "---\nconfig:\n  gantt:\n    useWidth: 420\n---\ngantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: 2024-01-01, 1d",
        960.0,
    );
    assert_eq!(configured.width, 420.0);
}

struct RawBBoxProbeMeasurer {
    calls: Arc<AtomicUsize>,
    width: f64,
}

impl TextMeasurer for RawBBoxProbeMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        panic!("Gantt task labels must use the raw SVG text bbox operation")
    }

    fn measure_svg_raw_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.width
    }
}

#[test]
fn gantt_task_labels_route_through_raw_svg_bbox_measurement() {
    let calls = Arc::new(AtomicUsize::new(0));
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-raw-bbox").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Arc::clone(&calls),
            width: 200.0,
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let layout = layout_gantt_from_text_with_environment(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d",
        1_184.0,
        &environment,
    );

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(layout.tasks[0].label.width, 200.0);
}

#[test]
fn gantt_label_placement_uses_the_resolved_container_edges() {
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-raw-bbox-placement").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Arc::new(AtomicUsize::new(0)),
            width: 200.0,
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let layout = layout_gantt_from_text_with_environment(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nFull range: full, 2024-01-01, 10d\nStart label: start, 2024-01-01, 1d\nEnd label: end, 2024-01-10, 1d",
        1_184.0,
        &environment,
    );
    let start = layout
        .tasks
        .iter()
        .find(|task| task.id == "start")
        .expect("start task");
    let end = layout
        .tasks
        .iter()
        .find(|task| task.id == "end")
        .expect("end task");

    assert!(start.label.class.contains("taskTextOutsideRight"));
    assert!(start.label.x > start.bar.x + start.bar.width);
    assert!(end.label.class.contains("taskTextOutsideLeft"));
    assert_eq!(end.label.x, end.bar.x - 5.0);
}

#[test]
fn gantt_layout_stops_at_the_maximum_utc_date_without_panicking() {
    // The raw model boundary case lives beside the private layout entry point.
}

fn render_gantt_svg_from_text(text: &str) -> String {
    render_gantt_svg_from_text_with_engine(Engine::new(), text)
}

fn render_gantt_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_core::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .begin_session()
        .expect("begin render session");
    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse ok")
    .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render svg")
        .svg()
        .to_owned()
}

const GANTT_HOST_STATE_SOURCE: &str = r#"gantt
dateFormat YYYY-MM-DD
todayMarker off
section Delivery
Default: default-task, 2024-01-01, 100d
Active: active, active-task, 2024-01-01, 100d
Done: done, done-task, 2024-01-01, 100d
Critical: crit, crit-task, 2024-01-01, 100d
Active critical: active, crit, active-crit-task, 2024-01-01, 100d
Done critical: done, crit, done-crit-task, 2024-01-01, 100d
A long outside done label: done, outside-done-task, 2024-01-01, 1d
A long outside done critical label: done, crit, outside-done-crit-task, 2024-01-01, 1d
Vertical: vert, vertical-task, 2024-02-01, 0d
click default-task href "https://example.com"
"#;

fn render_gantt_host_preset(
    source: &str,
    preset: merman_render::presentation::HostThemePreset,
    engine: Engine,
) -> String {
    use merman_render::presentation::{HostTheme, Presentation};
    let engine = Presentation::new()
        .with_theme(HostTheme::from_preset(preset))
        .resolve()
        .materialize_engine(engine);
    render_gantt_svg_from_text_with_engine(engine, source)
}

fn gantt_rule_property<'a>(css: &'a str, selector: &str, property: &str) -> &'a str {
    css.split('}')
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(selectors, _)| selectors.split(',').any(|value| value.trim() == selector))
        .flat_map(|(_, declarations)| declarations.split(';'))
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim())
        .next_back()
        .unwrap_or_else(|| panic!("missing {property} on {selector}"))
}

fn gantt_terminal_class<'a>(document: &'a roxmltree::Document<'_>, id: &str) -> &'a str {
    document
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .and_then(|node| node.attribute("class"))
        .unwrap_or_else(|| panic!("missing terminal {id}"))
}

#[test]
fn gantt_host_presets_preserve_native_task_state_palette() {
    use merman_render::presentation::HostThemePreset;
    for (preset, default, active, done, critical_border, text) in [
        (
            HostThemePreset::EditorLight,
            "#f8fafc",
            "#f1f5f9",
            "#e2e8f0",
            "#dc2626",
            "#0f172a",
        ),
        (
            HostThemePreset::EditorDark,
            "#111827",
            "#334155",
            "#1f2937",
            "#f87171",
            "#e5e7eb",
        ),
    ] {
        let svg = render_gantt_host_preset(GANTT_HOST_STATE_SOURCE, preset, Engine::new());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        for (id, state, fill, label_state) in [
            ("default-task", "task0", default, "taskText0"),
            ("active-task", "active0", active, "activeText0"),
            ("done-task", "done0", done, "doneText0"),
            ("crit-task", "crit0", done, "critText0"),
            ("active-crit-task", "activeCrit0", active, "activeCritText0"),
            ("done-crit-task", "doneCrit0", done, "doneCritText0"),
        ] {
            assert!(
                gantt_terminal_class(&document, &format!("gantt-config-{id}"))
                    .split_ascii_whitespace()
                    .any(|class| class == state),
                "{preset:?} {id}"
            );
            assert_eq!(
                gantt_rule_property(css, &format!("#gantt-config .{state}"), "fill"),
                fill,
                "{preset:?} {id}"
            );
            assert!(
                gantt_terminal_class(&document, &format!("gantt-config-{id}-text"))
                    .split_ascii_whitespace()
                    .any(|class| class == label_state),
                "{preset:?} {id} label"
            );
        }
        for state in ["crit0", "activeCrit0", "doneCrit0"] {
            assert_eq!(
                gantt_rule_property(css, &format!("#gantt-config .{state}"), "stroke"),
                critical_border
            );
        }
        assert_eq!(
            gantt_rule_property(css, "#gantt-config .taskText0", "fill"),
            text
        );
        for label in [
            "activeText0",
            "doneText0",
            "activeCritText0",
            "doneCritText0",
        ] {
            assert_eq!(
                gantt_rule_property(css, &format!("#gantt-config .{label}"), "fill"),
                format!("{text}!important")
            );
        }
    }
}

#[test]
fn gantt_host_outside_clickable_and_vertical_text_keep_native_css_precedence() {
    use merman_render::presentation::HostThemePreset;
    let svg = render_gantt_host_preset(
        GANTT_HOST_STATE_SOURCE,
        HostThemePreset::EditorDark,
        Engine::new(),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    for (id, state) in [
        ("outside-done-task", "doneText0"),
        ("outside-done-crit-task", "doneCritText0"),
    ] {
        let class = gantt_terminal_class(&document, &format!("gantt-config-{id}-text"));
        assert!(class.split_ascii_whitespace().any(|class| class == state));
        assert!(
            class
                .split_ascii_whitespace()
                .any(|class| class == "taskTextOutsideRight")
        );
        assert_eq!(
            gantt_rule_property(
                css,
                &format!("#gantt-config .{state}.taskTextOutsideRight"),
                "fill"
            ),
            "#cbd5e1!important"
        );
    }
    assert!(
        gantt_terminal_class(&document, "gantt-config-default-task-text")
            .split_ascii_whitespace()
            .any(|class| class == "clickable")
    );
    assert_eq!(
        gantt_rule_property(css, "#gantt-config .taskText.clickable", "fill"),
        "#94a3b8!important"
    );
    assert!(
        gantt_terminal_class(&document, "gantt-config-vertical-task-text")
            .split_ascii_whitespace()
            .any(|class| class == "vertText")
    );
    assert_eq!(
        gantt_rule_property(css, "#gantt-config .vertText", "fill"),
        "#fbbf24!important"
    );
}

#[test]
fn gantt_host_preserves_explicit_task_and_foreground_variables() {
    use merman_render::presentation::HostThemePreset;
    let variables = serde_json::json!({
        "taskBkgColor": "#111111", "activeTaskBkgColor": "#222222",
        "doneTaskBkgColor": "#333333", "critBkgColor": "#444444",
        "critBorderColor": "#555555", "taskTextColor": "#aaaaaa",
        "taskTextDarkColor": "#bbbbbb", "taskTextOutsideColor": "#cccccc",
        "taskTextClickableColor": "#dddddd", "vertLineColor": "#eeeeee",
    });
    for site_owned in [false, true] {
        let (source, engine) = if site_owned {
            (
                GANTT_HOST_STATE_SOURCE.to_string(),
                Engine::new().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({"themeVariables": variables}),
                )),
            )
        } else {
            (
                format!(
                    "---\nconfig: {}\n---\n{GANTT_HOST_STATE_SOURCE}",
                    serde_json::json!({"themeVariables": variables})
                ),
                Engine::new(),
            )
        };
        let svg = render_gantt_host_preset(&source, HostThemePreset::EditorDark, engine);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        for (selector, property, expected) in [
            ("task0", "fill", "#111111"),
            ("active0", "fill", "#222222"),
            ("activeCrit0", "fill", "#222222"),
            ("done0", "fill", "#333333"),
            ("doneCrit0", "fill", "#333333"),
            ("crit0", "fill", "#444444"),
            ("crit0", "stroke", "#555555"),
            ("activeCrit0", "stroke", "#555555"),
            ("doneCrit0", "stroke", "#555555"),
            ("taskText0", "fill", "#aaaaaa"),
            ("activeText0", "fill", "#bbbbbb!important"),
            ("doneText0", "fill", "#bbbbbb!important"),
            ("activeCritText0", "fill", "#bbbbbb!important"),
            ("doneCritText0", "fill", "#bbbbbb!important"),
            (
                "doneText0.taskTextOutsideRight",
                "fill",
                "#cccccc!important",
            ),
            (
                "doneCritText0.taskTextOutsideRight",
                "fill",
                "#cccccc!important",
            ),
            ("taskText.clickable", "fill", "#dddddd!important"),
            ("vertText", "fill", "#eeeeee!important"),
        ] {
            assert_eq!(
                gantt_rule_property(css, &format!("#gantt-config .{selector}"), property),
                expected,
                "site_owned={site_owned} {selector}"
            );
        }
    }
}

#[test]
fn gantt_section_titles_share_mermaid_break_rows_and_vertical_alignment() {
    let source = include_str!("../../../fixtures/gantt/multiline_section_title_br_tags.mmd");
    let layout = layout_gantt_from_text(source);
    let section = &layout.section_titles[0];
    let expected = ["Line1", "Line2", "Line3", "Line4", "Line5"];
    assert_eq!(section.lines, expected);
    assert_eq!(section.dy_em, -2.0);

    let svg = render_gantt_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("sectionTitle sectionTitle0"))
        .expect("section title");
    assert_eq!(title.attribute("dy"), Some("-2em"));
    let rows: Vec<_> = title
        .children()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(
        rows.iter()
            .map(|node| node.text().unwrap_or_default())
            .collect::<Vec<_>>(),
        expected
    );
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.attribute("dy"), (index > 0).then_some("1em"));
    }
}

#[test]
fn gantt_task_text_height_follows_final_svg_security_sanitization() {
    let source = r#"gantt
dateFormat YYYY-MM-DD
section Delivery
Task: task, 2024-01-01, 1d
"#;
    let strict = render_gantt_svg_from_text(source);
    let loose = render_gantt_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        source,
    );

    let task_text_heights = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid Gantt SVG XML");
        document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("id").is_some())
            .filter_map(|node| node.attribute("text-height").map(str::to_owned))
            .collect::<Vec<_>>()
    };

    assert!(task_text_heights(&strict).is_empty(), "{strict}");
    assert_eq!(task_text_heights(&loose), vec!["20"], "{loose}");
}

#[test]
fn gantt_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_gantt_svg_from_text(
        r#"---
title: Frontmatter schedule
---
gantt
dateFormat YYYY-MM-DD
section Delivery
Task: 2024-01-01, 1d
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="titleText">Frontmatter schedule</text>"#),
        "frontmatter title should render when the Gantt body has none: {frontmatter_svg}"
    );

    let body_svg = render_gantt_svg_from_text(
        r#"---
title: Frontmatter schedule
---
gantt
title Body schedule
dateFormat YYYY-MM-DD
section Delivery
Task: 2024-01-01, 1d
"#,
    );
    assert!(body_svg.contains(r#"class="titleText">Body schedule</text>"#));
    assert!(!body_svg.contains(">Frontmatter schedule</text>"));
}

#[test]
fn gantt_explicit_whitespace_title_overrides_frontmatter_without_trimming() {
    let svg = render_gantt_svg_from_text(concat!(
        "---\n",
        "title: Frontmatter schedule\n",
        "---\n",
        "gantt\n",
        "title  \n",
        "dateFormat YYYY-MM-DD\n",
        "section Delivery\n",
        "Task: 2024-01-01, 1d\n",
    ));

    assert!(
        svg.contains(r#"class="titleText"> </text>"#),
        "the one remaining Jison separator must be rendered exactly: {svg}"
    );
    assert!(!svg.contains(">Frontmatter schedule</text>"));
}

#[test]
fn gantt_svg_tick_interval_uses_config_fallback_and_body_precedence_in_utc() {
    for (body_interval, expected) in [
        ("", &["2024-02-01"][..]),
        (
            "tickInterval 1day\n",
            &[
                "2024-01-30",
                "2024-01-31",
                "2024-02-01",
                "2024-02-02",
                "2024-02-03",
            ][..],
        ),
    ] {
        let source = format!(
            r#"---
config:
  gantt:
    tickInterval: 1month
    topAxis: true
---
gantt
dateFormat YYYY-MM-DD
axisFormat %Y-%m-%d
{body_interval}todayMarker off
section Delivery
Task: task, 2024-01-30, 2024-02-03
"#,
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
        let axes = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 2, "both bottom and top axes must render");
        for (index, axis) in axes.iter().enumerate() {
            let labels = axis
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| node.text().expect("axis tick label"))
                .collect::<Vec<_>>();
            assert_eq!(labels, expected, "axis {index}, body: {body_interval:?}");
        }
    }
}

#[test]
fn gantt_svg_frontmatter_config_fields_affect_visible_output() {
    let svg = render_gantt_svg_from_text(
        r#"---
displayMode: compact
config:
  gantt:
    useWidth: 420
    rightPadding: 10
    topAxis: true
    numberSectionStyles: 2
---
gantt
  title Config Frontmatter SVG Fields
  dateFormat YYYY-MM-DD
  axisFormat %Y-%m-%d
  tickInterval 1day
  todayMarker off
  section Alpha
  Task A :a1, 2024-01-01, 1d
  section Beta
  Task B :b1, 2024-01-02, 1d
"#,
    );

    assert!(
        svg.contains(r#"viewBox="0 0 420 "#) && svg.contains(r#"style="max-width: 420px;""#),
        "frontmatter gantt.useWidth should set rendered SVG width: {svg}"
    );
    assert_eq!(
        svg.matches(r#"<g class="grid" transform="translate(75, 50)""#)
            .count(),
        1,
        "frontmatter gantt.topAxis should add the top axis grid at top padding: {svg}"
    );
    assert_eq!(
        svg.matches(r#"<g class="grid" transform="translate(75, "#)
            .count(),
        2,
        "frontmatter gantt.topAxis should render both top and bottom axes: {svg}"
    );
    assert!(
        svg.contains(r#"width="415" height="24" class="section section0""#)
            && svg.contains(r#"width="415" height="24" class="section section1""#),
        "frontmatter gantt.rightPadding and numberSectionStyles should affect visible rows: {svg}"
    );
    assert!(
        svg.contains(r#"class="sectionTitle sectionTitle0""#)
            && svg.contains(r#"class="sectionTitle sectionTitle1""#)
            && svg.contains(r#"id="gantt-config-a1""#)
            && svg.contains(r#"id="gantt-config-b1-text""#),
        "configured Gantt SVG should expose section classes and scoped task DOM: {svg}"
    );
}

#[test]
fn gantt_vertical_markers_do_not_affect_standard_row_layout() {
    let layout = layout_gantt_from_text(
        r#"
gantt
dateFormat YYYY-MM-DD
section Delivery
Start marker: vert,marker-start,2024-01-01,0d
Task A: task-a,2024-01-02,1d
Middle marker: vert,marker-middle,2024-01-05,0d
Task B: task-b,2024-01-06,1d
Final marker: vert,marker-final,2024-01-10,0d
"#,
    );
    assert_eq!(layout.height, 148.0);
    assert_eq!(
        layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        vec![0, 1]
    );

    let markers = layout
        .tasks
        .iter()
        .filter(|task| task.vert)
        .collect::<Vec<_>>();
    assert_eq!(markers.len(), 3);
    assert!(markers.iter().all(|task| task.order == -1));
    assert!(markers.iter().all(|task| task.bar.height == 88.0));
    assert!(markers.iter().all(|task| task.label.y == 143.0));

    let final_marker = markers
        .iter()
        .find(|task| task.id == "marker-final")
        .expect("final marker");
    assert_eq!(
        final_marker.bar.x,
        layout.width - layout.right_padding,
        "vertical markers must remain part of the time domain"
    );
}

#[test]
fn gantt_vertical_markers_do_not_affect_compact_row_packing() {
    let layout = layout_gantt_from_text(
        r#"---
displayMode: compact
---
gantt
dateFormat YYYY-MM-DD
section Delivery
Long marker: vert,marker-long,2024-01-01,31d
Task A: task-a,2024-01-01,1d
Task B: task-b,2024-01-03,1d
"#,
    );
    assert_eq!(layout.height, 124.0);
    assert_eq!(
        layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        layout
            .tasks
            .iter()
            .map(|task| (task.id.as_str(), task.order))
            .collect::<Vec<_>>(),
        vec![("marker-long", -1), ("task-a", 0), ("task-b", 0)]
    );
}
