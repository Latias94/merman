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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
    inputs: Arc<Mutex<Vec<(String, TextStyle)>>>,
}

impl TextMeasurer for RawBBoxProbeMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        panic!("Gantt task labels must use the raw SVG text bbox operation")
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inputs
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
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
            inputs: Default::default(),
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
fn gantt_raw_label_whitespace_preserves_width_threshold_placement() {
    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let style = TextStyle {
        font_size: 11.0,
        ..Default::default()
    };
    let normalized_width = measurer.measure("Alpha Beta", &style).width;
    let uncollapsed_width = measurer.measure("Alpha     Beta", &style).width;
    assert!(uncollapsed_width > normalized_width);
    // One-day task in a ten-day domain: put its bar between the old and correct widths.
    let bar_width = (normalized_width + uncollapsed_width) / 2.0;
    let container_width = 150.0 + 10.0 * bar_width;
    let source = |label: &str| {
        format!(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\n{label}: target, 2024-01-01, 1d\nHorizon: horizon, 2024-01-01, 10d\n"
        )
    };
    let plain = layout_gantt_from_text_at_container_width(&source("Alpha Beta"), container_width);
    let spaced =
        layout_gantt_from_text_at_container_width(&source("Alpha     Beta"), container_width);
    let plain = plain.tasks.iter().find(|task| task.id == "target").unwrap();
    let spaced = spaced
        .tasks
        .iter()
        .find(|task| task.id == "target")
        .unwrap();
    assert!(plain.label.width < plain.bar.width);
    assert!(
        uncollapsed_width > plain.bar.width,
        "the previous measurement crossed this threshold"
    );
    assert_eq!(plain.label.width, spaced.label.width);
    assert_eq!(plain.label.x, spaced.label.x);
    assert_eq!(plain.label.class, spaced.label.class);
    assert!(
        spaced
            .label
            .class
            .split_whitespace()
            .any(|token| token == "taskText")
    );
    assert_eq!(
        spaced.label.text, "Alpha     Beta",
        "layout retains authored text for final source projection"
    );

    // Custom raw-DOM measurement stays authoritative and receives uncollapsed source text.
    let inputs = Arc::new(Mutex::new(Vec::new()));
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-raw-whitespace").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Default::default(),
            width: uncollapsed_width,
            inputs: Arc::clone(&inputs),
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let host = layout_gantt_from_text_with_environment(
        &source("Alpha     Beta"),
        container_width,
        &environment,
    );
    assert!(
        inputs
            .lock()
            .unwrap()
            .iter()
            .any(|(text, _)| text == "Alpha     Beta")
    );
    let task = host.tasks.iter().find(|task| task.id == "target").unwrap();
    assert_eq!(task.label.width, uncollapsed_width);
    assert!(
        task.label
            .class
            .split_whitespace()
            .any(|token| token == "taskTextOutsideRight")
    );
}

#[test]
fn gantt_source_bbox_measurements_restore_threshold_sensitive_label_placement() {
    // Browser getBBox measurements are injected through the existing host measurement route.
    // These fixture observations are evidence for the placement formula, not correction factors
    // in the production deterministic profile. Only the selected task is compared: the probe
    // deliberately returns the same width for other tasks in each fixture.
    for (fixture, task_id, source_width, expected_bar_width, expected_x) in [
        (
            "upstream_cypress_gantt_spec_example_001",
            "task5",
            123.78125,
            121.0,
            992.0,
        ),
        (
            "upstream_cypress_theme_spec_should_render_a_gantt_diagram_006",
            "task2",
            135.375,
            129.0,
            209.0,
        ),
    ] {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/gantt")
                .join(format!("{fixture}.mmd")),
        )
        .expect("Gantt source fixture");
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.gantt-source-raw-bbox").unwrap(),
                "v1",
            )
            .unwrap(),
            Arc::new(RawBBoxProbeMeasurer {
                calls: Default::default(),
                width: source_width,
                inputs: Default::default(),
            }),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let layout = layout_gantt_from_text_with_environment(&source, 1_184.0, &environment);
        let task = layout.tasks.iter().find(|task| task.id == task_id).unwrap();
        assert_eq!(task.label.width, source_width, "{fixture}");
        assert_eq!(task.bar.width, expected_bar_width, "{fixture}");
        assert!(source_width > task.bar.width, "{fixture}");
        assert!(
            task.label
                .class
                .split_whitespace()
                .any(|class| class == "taskTextOutsideRight"),
            "{fixture}: {}",
            task.label.class,
        );
        assert_eq!(task.label.x, expected_x, "{fixture}");
    }
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
            inputs: Default::default(),
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
fn gantt_reversed_task_interval_keeps_the_source_signed_overflow_check() {
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-reversed-interval").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Default::default(),
            width: 200.0,
            inputs: Default::default(),
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let layout = layout_gantt_from_text_with_environment(
        "gantt\ndateFormat YYYY-MM-DD\nFull: full, 2024-01-01, 2024-01-11\nReversed: reversed, 2024-01-09, 2024-01-02\n",
        1184.0,
        &environment,
    );
    let reversed = layout
        .tasks
        .iter()
        .find(|task| task.id == "reversed")
        .unwrap();
    assert_eq!(reversed.bar.width, 0.0);
    assert!(reversed.label.class.contains("taskTextOutsideRight"));
    let full = layout.tasks.iter().find(|task| task.id == "full").unwrap();
    let end_x = full.bar.x + (full.bar.width / 10.0).round();
    assert!((reversed.label.x - end_x - 5.0).abs() < 1e-9);
}

#[test]
fn gantt_layout_stops_at_the_maximum_utc_date_without_panicking() {
    // The raw model boundary case lives beside the private layout entry point.
}

#[test]
fn gantt_layout_measurement_uses_the_rendered_font_and_preserves_nonbreaking_spaces() {
    for (config, expected_font) in [
        (
            serde_json::json!({"fontFamily": "Arial", "themeVariables": {"fontFamily": "Verdana"}, "gantt": {"fontFamily": "Courier"}}),
            "Verdana",
        ),
        (
            serde_json::json!({"fontFamily": "Arial", "gantt": {"fontFamily": "Courier"}}),
            "Arial",
        ),
    ] {
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.gantt-measurement-input").unwrap(),
                "v1",
            )
            .unwrap(),
            Arc::new(RawBBoxProbeMeasurer {
                calls: Default::default(),
                width: 200.0,
                inputs: Arc::clone(&inputs),
            }),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let source = "gantt\ndateFormat YYYY-MM-DD\nTask\u{a0} :task, 2024-01-01, 1d\n";
        let parsed = Engine::new()
            .with_site_config(MermaidConfig::from_value(config))
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = family::prepare(
            parsed,
            &LayoutOptions::default(),
            environment.begin_session().unwrap(),
        )
        .unwrap();
        let layout: GanttDiagramLayout = serde_json::from_value(
            artifact.layout_json().unwrap()["layout"]["GanttDiagram"].clone(),
        )
        .unwrap();
        let inputs = inputs.lock().unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(
            inputs[0].0, "Task\u{a0}",
            "only SVG-collapsible trailing spaces may be removed"
        );
        assert_eq!(inputs[0].1.font_family.as_deref(), Some(expected_font));
        assert_eq!(layout.tasks[0].label.width, 200.0);
    }
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
    let title_texts = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid Gantt SVG XML");
        document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_whitespace().any(|token| token == "titleText")
                    })
            })
            .filter_map(|node| node.text().map(str::to_owned))
            .collect::<Vec<_>>()
    };
    assert!(
        title_texts(&frontmatter_svg)
            .iter()
            .any(|title| title == "Frontmatter schedule"),
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
    let body_titles = title_texts(&body_svg);
    assert!(body_titles.iter().any(|title| title == "Body schedule"));
    assert!(
        !body_titles
            .iter()
            .any(|title| title == "Frontmatter schedule")
    );
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

    let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node
                .attribute("class")
                .is_some_and(|class| class.split_whitespace().any(|token| token == "titleText"))
            && node.text() == Some(" ")
    }));
    assert!(!svg.contains("Frontmatter schedule"));
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
    topPadding: 70
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
        svg.contains(r#"viewBox="0 0 420 "#)
            && svg.contains(r#"style="max-width: 420px; background-color: white;""#),
        "frontmatter gantt.useWidth should set rendered SVG width: {svg}"
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
    let class_has = |node: roxmltree::Node<'_, '_>, token: &str| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|value| value == token))
    };
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && class_has(*node, "grid"))
            .count(),
        2,
        "frontmatter gantt.topAxis should render both top and bottom axes: {svg}"
    );
    let height: f64 = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .nth(3)
        .unwrap()
        .parse()
        .unwrap();
    let axis_y = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && class_has(*node, "grid"))
        .map(|node| {
            node.attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split(',')
                .nth(1)
                .unwrap()
                .trim()
                .parse::<f64>()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        axis_y,
        [height - 50.0, 70.0],
        "Mermaid fixes the bottom inset at 50, independently of topPadding"
    );
    for section in ["section0", "section1"] {
        assert!(
            document.descendants().any(|node| {
                node.has_tag_name("rect")
                    && class_has(node, section)
                    && node.attribute("width") == Some("415")
                    && node.attribute("height") == Some("24")
            }),
            "frontmatter Gantt row {section} should reflect configured width: {svg}"
        );
    }
    for id in ["gantt-config-a1", "gantt-config-b1-text"] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(id)),
            "configured Gantt SVG should expose scoped DOM id {id}: {svg}"
        );
    }
    for section in ["sectionTitle0", "sectionTitle1"] {
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("text") && class_has(node, section)),
            "configured Gantt SVG should expose section title class {section}: {svg}"
        );
    }
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
