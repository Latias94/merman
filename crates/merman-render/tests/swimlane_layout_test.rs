use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::{SwimlaneDirection, SwimlaneLayout};
use std::path::PathBuf;
use std::sync::Arc;

const EPSILON: f64 = 1.0e-6;

struct FixedTextMeasurer;

impl HostTextMeasurer for FixedTextMeasurer {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let width = request.text.chars().count() as f64 * 8.0;
        Ok(Some(match request.operation {
            TextMeasurementOperation::RawBBoxWidth
            | TextMeasurementOperation::SimpleBBoxWidth
            | TextMeasurementOperation::ComputedLength => HostTextMeasurement::Length(width),
            TextMeasurementOperation::RawBBoxHeight
            | TextMeasurementOperation::SimpleBBoxHeight => HostTextMeasurement::Length(20.0),
            _ => HostTextMeasurement::Metrics(merman_render::text::TextMetrics {
                width,
                height: 20.0,
                line_count: 1,
            }),
        }))
    }
}

struct WidthAwareTextMeasurer;

impl HostTextMeasurer for WidthAwareTextMeasurer {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let line_width = |line: &str| line.chars().count() as f64 * 8.0;
        let width = request.text.lines().map(line_width).fold(0.0_f64, f64::max);
        let line_count = request.text.split('\n').count().max(1);
        Ok(Some(match request.operation {
            TextMeasurementOperation::RawBBoxWidth
            | TextMeasurementOperation::SimpleBBoxWidth
            | TextMeasurementOperation::ComputedLength => HostTextMeasurement::Length(width),
            TextMeasurementOperation::RawBBoxHeight
            | TextMeasurementOperation::SimpleBBoxHeight => HostTextMeasurement::Length(20.0),
            _ => HostTextMeasurement::Metrics(merman_render::text::TextMetrics {
                width,
                height: line_count as f64 * 20.0,
                line_count,
            }),
        }))
    }
}

fn try_layout_swimlane(source: &str) -> Result<SwimlaneLayout, String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .map_err(|error| format!("parse failed: {error}"))?
        .ok_or_else(|| "no diagram detected".to_string())?;
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.swimlane-fixed").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(
            identity,
            Arc::new(FixedTextMeasurer),
            TextMeasurementPhase::ALL,
        ),
    );
    let session = environment
        .begin_session()
        .map_err(|error| format!("render session failed: {error}"))?;
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .map_err(|error| format!("swimlane layout failed: {error}"))?;
    let projection = artifact
        .layout_json()
        .map_err(|error| format!("swimlane projection failed: {error}"))?;
    serde_json::from_value(projection["layout"]["SwimlaneDiagram"].clone())
        .map_err(|error| format!("swimlane layout projection failed: {error}"))
}

fn layout_swimlane(source: &str) -> SwimlaneLayout {
    try_layout_swimlane(source).expect("swimlane layout")
}

fn layout_swimlane_with_width_aware_measurer(source: &str) -> SwimlaneLayout {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse swimlane")
        .expect("detect swimlane");
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.swimlane-width-aware").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(
            identity,
            Arc::new(WidthAwareTextMeasurer),
            TextMeasurementPhase::ALL,
        ),
    );
    let session = environment.begin_session().expect("render session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare swimlane layout");
    let projection = artifact.layout_json().expect("swimlane projection");
    serde_json::from_value(projection["layout"]["SwimlaneDiagram"].clone())
        .expect("typed swimlane layout")
}

fn assert_orthogonal(points: &[merman_render::model::LayoutPoint]) {
    assert!(points.len() >= 2, "an edge needs at least two route points");
    for segment in points.windows(2) {
        let horizontal = (segment[0].y - segment[1].y).abs() <= EPSILON;
        let vertical = (segment[0].x - segment[1].x).abs() <= EPSILON;
        assert!(
            horizontal || vertical,
            "non-orthogonal segment: {:?} -> {:?}",
            segment[0],
            segment[1]
        );
    }
}

fn contains_node(
    lane: &merman_render::model::SwimlaneLaneLayout,
    node: &merman_render::model::SwimlaneNodeLayout,
) -> bool {
    node.x - node.width / 2.0 >= lane.x - lane.width / 2.0 - EPSILON
        && node.x + node.width / 2.0 <= lane.x + lane.width / 2.0 + EPSILON
        && node.y - node.height / 2.0 >= lane.y - lane.height / 2.0 - EPSILON
        && node.y + node.height / 2.0 <= lane.y + lane.height / 2.0 + EPSILON
}

fn point_on_polyline(point: (f64, f64), points: &[merman_render::model::LayoutPoint]) -> bool {
    points.windows(2).any(|segment| {
        let min_x = segment[0].x.min(segment[1].x) - EPSILON;
        let max_x = segment[0].x.max(segment[1].x) + EPSILON;
        let min_y = segment[0].y.min(segment[1].y) - EPSILON;
        let max_y = segment[0].y.max(segment[1].y) + EPSILON;
        let collinear = if (segment[0].x - segment[1].x).abs() <= EPSILON {
            (point.0 - segment[0].x).abs() <= EPSILON
        } else {
            (point.1 - segment[0].y).abs() <= EPSILON
        };
        collinear && point.0 >= min_x && point.0 <= max_x && point.1 >= min_y && point.1 <= max_y
    })
}

fn node_by_id<'a>(
    layout: &'a SwimlaneLayout,
    id: &str,
) -> &'a merman_render::model::SwimlaneNodeLayout {
    layout
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("node {id}"))
}

#[test]
fn basic_lr_uses_lane_owned_sugiyama_and_orthogonal_routes() {
    let layout = layout_swimlane(
        r#"swimlane-beta LR
subgraph Customer
  request[Request service]
  receive[Receive update]
end
subgraph Support
  triage[Triage request]
  answer[Send answer]
end
subgraph Engineering
  investigate[Investigate issue]
  fix[Prepare fix]
end
request --> triage
triage -->|Known issue| answer
triage -->|Needs code change| investigate
investigate --> fix --> answer
answer --> receive
"#,
    );

    assert_eq!(layout.direction, SwimlaneDirection::Lr);
    assert_eq!(
        layout
            .lanes
            .iter()
            .filter(|lane| lane.parent_id.is_none())
            .count(),
        3
    );

    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("node {id}"))
    };
    assert_eq!(node("request").top_lane_id.as_deref(), Some("Customer"));
    assert_eq!(node("triage").top_lane_id.as_deref(), Some("Support"));
    assert_eq!(
        node("investigate").top_lane_id.as_deref(),
        Some("Engineering")
    );
    assert!(node("request").x < node("receive").x);
    assert!(node("triage").x < node("answer").x);
    assert!(node("investigate").x < node("fix").x);

    assert_eq!(layout.edges.len(), 6);
    for edge in &layout.edges {
        assert_orthogonal(&edge.points);
    }
}

#[test]
fn loose_nodes_are_owned_by_the_synthetic_default_lane() {
    let layout = layout_swimlane(
        r#"swimlane-beta TB
A[Start] --> B[Finish]
"#,
    );

    assert_eq!(layout.direction, SwimlaneDirection::Tb);
    assert_eq!(layout.lanes.len(), 1);
    let lane = &layout.lanes[0];
    assert_eq!(lane.id, "__swimlane_default__");
    assert!(lane.parent_id.is_none());
    let title_rect = lane.title_rect.as_ref().expect("default lane title band");
    assert!((title_rect.bottom - title_rect.top - 21.0).abs() <= EPSILON);

    let a = layout.nodes.iter().find(|node| node.id == "A").expect("A");
    let b = layout.nodes.iter().find(|node| node.id == "B").expect("B");
    for node in [a, b] {
        assert_eq!(node.parent_id.as_deref(), Some("__swimlane_default__"));
        assert_eq!(node.top_lane_id.as_deref(), Some("__swimlane_default__"));
        assert!(contains_node(lane, node));
    }
    assert!(a.y < b.y);
    assert_orthogonal(&layout.edges[0].points);
}

#[test]
fn explicit_default_lane_is_reused_without_losing_its_title() {
    let layout = layout_swimlane(
        r#"swimlane-beta TB
subgraph __swimlane_default__[Named lane]
  A[Inside]
end
B[Loose]
"#,
    );

    assert_eq!(layout.lanes.len(), 1);
    let lane = &layout.lanes[0];
    assert_eq!(lane.id, "__swimlane_default__");
    assert_eq!(lane.title, "Named lane");
    for id in ["A", "B"] {
        let node = node_by_id(&layout, id);
        assert_eq!(node.parent_id.as_deref(), Some("__swimlane_default__"));
        assert_eq!(node.top_lane_id.as_deref(), Some("__swimlane_default__"));
        assert!(contains_node(lane, node));
    }
}

#[test]
fn swimlane_layout_measures_render_sources_without_leaking_them_into_public_labels() {
    let layout = layout_swimlane(
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
subgraph Lane["&nbsp;Lane&nbsp;"]
  A["&nbsp;Node&nbsp;"]
end
A -->|"&nbsp;Edge&nbsp;"| B
"#,
    );
    let source_width = |text: &str| text.chars().count() as f64 * 8.0;

    let lane = layout
        .lanes
        .iter()
        .find(|lane| lane.id == "Lane")
        .expect("Lane");
    assert_eq!(lane.title, "\u{00a0}Lane\u{00a0}");
    assert_eq!(lane.title_label_width, source_width("&nbsp;Lane&nbsp;"));

    let node = node_by_id(&layout, "A");
    assert_eq!(node.label, "\u{00a0}Node\u{00a0}");
    assert_eq!(node.label_width, source_width("&nbsp;Node&nbsp;"));

    let edge_label = layout
        .nodes
        .iter()
        .find(|node| node.is_edge_label)
        .expect("edge label node");
    assert_eq!(edge_label.label, "\u{00a0}Edge\u{00a0}");
    assert_eq!(edge_label.label_width, source_width("&nbsp;Edge&nbsp;"));
}

#[test]
fn swimlane_title_metrics_follow_the_flowchart_local_html_labels_setting() {
    let html = layout_swimlane(
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: true
---
swimlane-beta LR
subgraph Lane["&nbsp;Lane&nbsp;"]
  A[Node]
end
"#,
    );
    let svg = layout_swimlane(
        r#"---
config:
  htmlLabels: true
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
subgraph Lane["&nbsp;Lane&nbsp;"]
  A[Node]
end
"#,
    );

    assert_eq!(html.lanes[0].title_label_width, 6.0 * 8.0);
    assert_eq!(svg.lanes[0].title_label_width, 16.0 * 8.0);
}

#[test]
fn swimlane_edge_label_metrics_follow_label_rect_root_html_labels() {
    let layout = layout_swimlane(
        r#"---
config:
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
subgraph Lane
  A[Node]
end
A -->|"&nbsp;Edge&nbsp;"| B
"#,
    );
    let edge_label = layout
        .nodes
        .iter()
        .find(|node| node.is_edge_label)
        .expect("edge label node");

    assert_eq!(edge_label.label, "\u{00a0}Edge\u{00a0}");
    assert_eq!(edge_label.label_width, 6.0 * 8.0);
}

#[test]
fn edge_curves_preserve_explicit_default_and_config_values() {
    let layout = layout_swimlane(
        r#"swimlane-beta TB
A e0@--> B
B e1@--> C
linkStyle default interpolate linear
e1@{ curve: stepAfter }
"#,
    );
    let curve = |id: &str| {
        layout
            .edges
            .iter()
            .find(|edge| edge.id == id)
            .unwrap_or_else(|| panic!("edge {id}"))
            .curve
            .as_str()
    };
    assert_eq!(curve("e0"), "linear");
    assert_eq!(curve("e1"), "stepAfter");

    let configured = layout_swimlane(
        r#"---
config:
  flowchart:
    curve: cardinal
---
swimlane-beta TB
A --> B
"#,
    );
    assert_eq!(configured.edges[0].curve, "cardinal");

    let implicit = layout_swimlane("swimlane-beta TB\nA --> B\n");
    assert_eq!(implicit.edges[0].curve, "rounded");
}

#[test]
fn edge_labels_are_layout_waypoints_and_anchor_to_the_original_edge() {
    let layout = layout_swimlane(
        r#"swimlane-beta TB
subgraph Team
  A[Draft]
  B[Ship]
end
A -->|approval| B
"#,
    );

    let a = layout.nodes.iter().find(|node| node.id == "A").expect("A");
    let b = layout.nodes.iter().find(|node| node.id == "B").expect("B");
    let label = layout
        .nodes
        .iter()
        .find(|node| node.is_edge_label)
        .expect("edge label node");
    assert_eq!(label.label, "approval");
    assert_eq!(label.shape, "labelRect");
    assert!((label.width - 64.0).abs() <= EPSILON);
    assert!((label.height - 20.0).abs() <= EPSILON);
    assert!((label.label_width - 64.0).abs() <= EPSILON);
    assert!(a.layer < label.layer && label.layer < b.layer);

    assert_eq!(layout.edges.len(), 1, "virtual edges must not render");
    let edge = &layout.edges[0];
    assert_eq!(edge.label_node_id.as_deref(), Some(label.id.as_str()));
    assert!(point_on_polyline((label.x, label.y), &edge.points));
    assert_orthogonal(&edge.points);
}

#[test]
fn edge_label_nodes_use_plain_text_semantics_after_swimlane_conversion() {
    let layout = layout_swimlane(
        r#"swimlane-beta LR
A -->|`This is **bold**`| B
"#,
    );

    let label = layout
        .nodes
        .iter()
        .find(|node| node.is_edge_label)
        .expect("edge label node");
    assert_eq!(label.label_type, "text");
    // The public layout projection retains the semantic source spelling. Rendering uses the
    // parser-owned label source and the converted labelRect's plain-text label type.
    assert_eq!(label.label, "`This is **bold**`");
}

#[test]
fn edge_label_nodes_use_configured_flowchart_wrapping_width() {
    fn label_for(width: usize) -> merman_render::model::SwimlaneNodeLayout {
        let source = format!(
            r#"---
config:
  htmlLabels: false
  flowchart:
    wrappingWidth: {width}
---
swimlane-beta LR
A -->|one two three four five six seven eight nine ten| B
"#
        );
        layout_swimlane_with_width_aware_measurer(&source)
            .nodes
            .into_iter()
            .find(|node| node.is_edge_label)
            .expect("edge label node")
    }

    let narrow = label_for(80);
    let wide = label_for(240);
    assert!(
        narrow.label_height > wide.label_height,
        "80px must wrap into more rows than 240px: narrow={narrow:?}, wide={wide:?}"
    );
    assert!(
        narrow.label_width <= 80.0 && wide.label_width <= 240.0,
        "label metrics must be bounded by the configured width: narrow={narrow:?}, wide={wide:?}"
    );
}

#[test]
fn cycles_reverse_only_the_layout_constraint_not_the_rendered_edge() {
    let layout = layout_swimlane(
        r#"swimlane-beta TB
subgraph Lane
  A --> B
  B --> C
  C --> A
end
"#,
    );

    assert_eq!(layout.edges.len(), 3);
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.reversed_for_layout)
            .count(),
        1
    );
    let endpoints: std::collections::HashSet<_> = layout
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();
    assert!(endpoints.contains(&("A", "B")));
    assert!(endpoints.contains(&("B", "C")));
    assert!(endpoints.contains(&("C", "A")));
    for edge in &layout.edges {
        assert_orthogonal(&edge.points);
    }
}

#[test]
fn all_four_directions_are_mirrored_and_deterministic() {
    const BODY: &str = r#"
subgraph Customer
  request[Request service]
  receive[Receive update]
end
subgraph Support
  triage[Triage request]
  answer[Send answer]
end
subgraph Engineering
  investigate[Investigate issue]
  fix[Prepare fix]
end
request --> triage
triage -->|Known issue| answer
triage -->|Needs code change| investigate
investigate --> fix --> answer
answer --> receive
"#;

    for (header, expected_direction) in [
        ("TB", SwimlaneDirection::Tb),
        ("BT", SwimlaneDirection::Bt),
        ("LR", SwimlaneDirection::Lr),
        ("RL", SwimlaneDirection::Rl),
    ] {
        let source = format!("swimlane-beta {header}{BODY}");
        let layout = layout_swimlane(&source);
        assert_eq!(layout.direction, expected_direction);

        let investigate = node_by_id(&layout, "investigate");
        let fix = node_by_id(&layout, "fix");
        match expected_direction {
            SwimlaneDirection::Tb => assert!(investigate.y < fix.y),
            SwimlaneDirection::Bt => assert!(investigate.y > fix.y),
            SwimlaneDirection::Lr => assert!(investigate.x < fix.x),
            SwimlaneDirection::Rl => assert!(investigate.x > fix.x),
        }
        for edge in &layout.edges {
            assert_orthogonal(&edge.points);
        }

        let expected = serde_json::to_value(&layout).expect("serialize Swimlane layout");
        for _ in 0..8 {
            let repeated = serde_json::to_value(layout_swimlane(&source))
                .expect("serialize repeated Swimlane layout");
            assert_eq!(repeated, expected, "{header} layout must be deterministic");
        }
    }
}

#[test]
fn upstream_swimlane_ddlt_corpus_has_finite_orthogonal_layouts() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures/swimlane/_upstream_ddlt");
    let mut fixtures: Vec<_> = std::fs::read_dir(&corpus)
        .expect("read Swimlane DDLT corpus")
        .map(|entry| entry.expect("read corpus entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mmd"))
        .collect();
    fixtures.sort();
    assert_eq!(
        fixtures.len(),
        30,
        "the Mermaid 11.16 DDLT sweep has 30 files"
    );

    let mut failures = Vec::new();
    for fixture in fixtures {
        let source = match std::fs::read_to_string(&fixture) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!("{}: read failed: {error}", fixture.display()));
                continue;
            }
        };
        let layout = match std::panic::catch_unwind(|| try_layout_swimlane(&source)) {
            Ok(Ok(layout)) => layout,
            Ok(Err(error)) => {
                failures.push(format!("{}: {error}", fixture.display()));
                continue;
            }
            Err(_) => {
                failures.push(format!("{}: layout panicked", fixture.display()));
                continue;
            }
        };

        if layout.nodes.is_empty() {
            failures.push(format!("{}: layout has no nodes", fixture.display()));
        }
        if layout.lanes.is_empty() {
            failures.push(format!("{}: layout has no lanes", fixture.display()));
        }

        let finite = |values: &[f64]| values.iter().all(|value| value.is_finite());
        for node in &layout.nodes {
            if !finite(&[
                node.x,
                node.y,
                node.width,
                node.height,
                node.label_width,
                node.label_height,
            ]) || node.width <= 0.0
                || node.height <= 0.0
            {
                failures.push(format!(
                    "{}: node {} has invalid geometry",
                    fixture.display(),
                    node.id
                ));
            }
        }
        for lane in &layout.lanes {
            if !finite(&[
                lane.x,
                lane.y,
                lane.width,
                lane.height,
                lane.padding,
                lane.title_label_width,
                lane.title_label_height,
            ]) || lane.width <= 0.0
                || lane.height <= 0.0
            {
                failures.push(format!(
                    "{}: lane {} has invalid geometry",
                    fixture.display(),
                    lane.id
                ));
            }
        }
        for edge in &layout.edges {
            if edge
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
            {
                failures.push(format!(
                    "{}: edge {} has a non-finite point",
                    fixture.display(),
                    edge.id
                ));
                continue;
            }
            if edge.points.len() < 2
                || edge.points.windows(2).any(|segment| {
                    (segment[0].x - segment[1].x).abs() > EPSILON
                        && (segment[0].y - segment[1].y).abs() > EPSILON
                })
            {
                failures.push(format!(
                    "{}: edge {} is not orthogonal",
                    fixture.display(),
                    edge.id
                ));
            }
        }

        let Some(bounds) = &layout.bounds else {
            failures.push(format!("{}: layout has no bounds", fixture.display()));
            continue;
        };
        if !finite(&[bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y])
            || bounds.max_x <= bounds.min_x
            || bounds.max_y <= bounds.min_y
        {
            failures.push(format!("{}: layout has invalid bounds", fixture.display()));
        }
    }

    assert!(
        failures.is_empty(),
        "Swimlane DDLT corpus failures:\n{}",
        failures.join("\n")
    );
}
