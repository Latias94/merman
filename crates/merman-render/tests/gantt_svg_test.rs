use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, RenderTimeSnapshot, TextMeasurementOperation,
    TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::gantt::layout_gantt_diagram_typed;
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
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    layout_gantt_from_text_with_measurer(text, container_width, &measurer)
}

fn layout_gantt_from_text_with_measurer(
    text: &str,
    container_width: f64,
    measurer: &dyn TextMeasurer,
) -> GanttDiagramLayout {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse ok")
    .expect("diagram detected");
    let RenderSemanticModel::Gantt(model) = &parsed.model else {
        panic!("expected Gantt render model");
    };

    layout_gantt_diagram_typed(
        model,
        parsed.meta.effective_config.as_value(),
        measurer,
        container_width,
    )
    .expect("layout ok")
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
    let session = RenderEnvironment::parity()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
        .begin_session()
        .expect("render session");
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    let layout = layout_gantt_from_text_with_measurer(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d",
        1_184.0,
        &measurer,
    );

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(layout.tasks[0].label.width, 200.0);
    let report = session.text_measurement_report();
    assert_eq!(report.entries().len(), 1);
    assert_eq!(
        report.entries()[0].provenance().operation,
        TextMeasurementOperation::RawBBoxWidth
    );
    assert_eq!(
        report.entries()[0].provenance().phase,
        TextMeasurementPhase::SvgBBox
    );
}

#[test]
fn gantt_label_placement_uses_the_resolved_container_edges() {
    let measurer = RawBBoxProbeMeasurer {
        calls: Arc::new(AtomicUsize::new(0)),
        width: 200.0,
    };
    let layout = layout_gantt_from_text_with_measurer(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nFull range: full, 2024-01-01, 10d\nStart label: start, 2024-01-01, 1d\nEnd label: end, 2024-01-10, 1d",
        1_184.0,
        &measurer,
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

fn render_gantt_svg_from_text(text: &str) -> String {
    let session = RenderEnvironment::parity()
        .with_time_snapshot(
            RenderTimeSnapshot::from_unix_millis(1_704_067_200_000, 0)
                .expect("valid fixed UTC instant"),
        )
        .begin_session()
        .expect("begin render session");
    let parsed = futures::executor::block_on(
        Engine::new().parse_diagram_for_render_model(text, ParseOptions::default()),
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
        svg.contains(r#"viewBox="0 0 420 "#)
            && svg.contains(r#"style="max-width: 420px; background-color: white;""#),
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
