use merman_core::{Engine, MermaidConfig, ParseOptions, RenderSemanticModel};
use merman_render::environment::{RenderEnvironment, TextMeasurementPhase};
use merman_render::model::StateDiagramLayout;
use merman_render::state::layout_state_diagram_typed;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn rect_from_node(n: &merman_render::model::LayoutNode) -> (f64, f64, f64, f64) {
    let hw = n.width / 2.0;
    let hh = n.height / 2.0;
    (n.x - hw, n.y - hh, n.x + hw, n.y + hh)
}

fn rect_from_cluster(c: &merman_render::model::LayoutCluster) -> (f64, f64, f64, f64) {
    let hw = c.width / 2.0;
    let hh = c.height / 2.0;
    (c.x - hw, c.y - hh, c.x + hw, c.y + hh)
}

fn rect_contains(outer: (f64, f64, f64, f64), inner: (f64, f64, f64, f64), eps: f64) -> bool {
    let (omin_x, omin_y, omax_x, omax_y) = outer;
    let (imin_x, imin_y, imax_x, imax_y) = inner;
    imin_x + eps >= omin_x
        && imax_x <= omax_x + eps
        && imin_y + eps >= omin_y
        && imax_y <= omax_y + eps
}

fn deep_state_composite_chain(depth: usize) -> String {
    let mut input = String::from("stateDiagram-v2\n");
    for level in 0..depth {
        input.push_str(&format!("state S{level} {{\n"));
    }
    input.push_str("Leaf\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

fn layout_state_from_text(text: &str) -> StateDiagramLayout {
    layout_state_from_text_with_options(text, ParseOptions::default())
}

fn layout_state_from_text_with_engine(engine: Engine, text: &str) -> StateDiagramLayout {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let RenderSemanticModel::State(model) = parsed.model() else {
        panic!("expected State render model");
    };
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    layout_state_diagram_typed(
        model,
        parsed.metadata().effective_config.as_value(),
        &measurer,
    )
    .expect("typed State layout")
}

fn layout_state_from_text_with_options(
    text: &str,
    parse_options: ParseOptions,
) -> StateDiagramLayout {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, parse_options)
        .expect("parse ok")
        .expect("diagram detected");
    let RenderSemanticModel::State(model) = parsed.model() else {
        panic!("expected State render model");
    };
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    layout_state_diagram_typed(
        model,
        parsed.metadata().effective_config.as_value(),
        &measurer,
    )
    .expect("typed State layout")
}

#[test]
fn state_parse_for_render_model_handles_deep_composite_chain() {
    const DEPTH: usize = 1500;
    let text = deep_state_composite_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");

    assert_eq!(parsed.metadata().diagram_type, "stateDiagram");
}

#[test]
fn state_layout_handles_deep_composite_chain() {
    const DEPTH: usize = 512;
    let text = deep_state_composite_chain(DEPTH);

    let layout = layout_state_from_text_with_options(&text, ParseOptions::strict());

    assert!(layout.clusters.iter().any(|cluster| cluster.id == "S0"));
    assert!(layout.nodes.iter().any(|node| node.id == "Leaf"));
}

#[test]
fn state_layout_produces_positions_and_routes() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_state_from_text(&text);

    assert!(layout.nodes.len() >= 3);
    assert!(layout.edges.len() >= 3);

    for n in &layout.nodes {
        assert!(n.width.is_finite() && n.width > 0.0);
        assert!(n.height.is_finite() && n.height > 0.0);
        assert!(n.x.is_finite() && n.y.is_finite());
    }

    for e in &layout.edges {
        assert!(
            e.points.len() >= 2,
            "edge {} should have at least two points",
            e.id
        );
        for p in &e.points {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}

#[test]
fn state_start_and_end_use_source_defined_nominal_diameter() {
    let text = "stateDiagram-v2\n[*] --> A\nA --> [*]\n";
    let layout = layout_state_from_text(text);

    let mut by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        by_id.insert(n.id.as_str(), (n.width, n.height));
    }

    let (sw, sh) = by_id["root_start"];
    let (ew, eh) = by_id["root_end"];

    // Mermaid 11.16 `stateStart.ts` and `stateEnd.ts` both normalize their source geometry to a
    // 14px diameter. Browser-specific RoughJS path `getBBox()` floats are a bounded SVG residual,
    // not a stable layout constant.
    const STATE_MARKER_DIAMETER_PX: f64 = 14.0;

    assert!(
        (sw - STATE_MARKER_DIAMETER_PX).abs() < 1e-6
            && (sh - STATE_MARKER_DIAMETER_PX).abs() < 1e-6
    );
    assert!(
        (ew - STATE_MARKER_DIAMETER_PX).abs() < 1e-6
            && (eh - STATE_MARKER_DIAMETER_PX).abs() < 1e-6
    );
}

#[test]
fn state_layout_root_html_labels_override_deprecated_flowchart_html_labels() {
    let root_false = layout_state_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );
    let root_true = layout_state_from_text(
        r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );

    let false_label = root_false
        .edges
        .iter()
        .find_map(|edge| edge.label.as_ref())
        .expect("root=false edge label");
    let true_label = root_true
        .edges
        .iter()
        .find_map(|edge| edge.label.as_ref())
        .expect("root=true edge label");

    assert!(
        false_label.width > true_label.width
            && (false_label.height - true_label.height).abs() > 1e-6,
        "root htmlLabels=false should select SVG-like label metrics over deprecated flowchart=true: false={false_label:?}, true={true_label:?}"
    );
}

#[test]
fn state_layout_preserves_html_min_content_width_for_long_labels() {
    let layout = layout_state_from_text(
        r#"stateDiagram-v2
direction RL
[*] --> ThisIsAReallyLongStateIdentifierWithNumbers123
ThisIsAReallyLongStateIdentifierWithNumbers123 --> Done : another-long-label-with-a-veryveryverylongwordthatforceswrapping
Done --> [*]
"#,
    );

    let long_node = layout
        .nodes
        .iter()
        .find(|node| node.id == "ThisIsAReallyLongStateIdentifierWithNumbers123")
        .expect("long state node");
    let long_edge = layout
        .edges
        .iter()
        .find(|edge| edge.from == "ThisIsAReallyLongStateIdentifierWithNumbers123")
        .and_then(|edge| edge.label.as_ref())
        .expect("long transition label");

    assert!(
        long_node.width > 300.0,
        "an unbreakable HTML label must expand beyond max-width: {long_node:?}"
    );
    assert!(
        long_edge.width > 250.0,
        "an unbreakable transition segment must expand the HTML table: {long_edge:?}"
    );
}

#[test]
fn state_layout_reflows_at_the_expanded_html_min_content_width() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("stress_state_font_size_precedence_071.mmd");
    let text = std::fs::read_to_string(path).expect("font-size precedence fixture");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "secure": [
            "secure",
            "securityLevel",
            "startOnLoad",
            "maxTextSize",
            "suppressErrorRendering",
            "maxEdges"
        ]
    })));
    let layout = layout_state_from_text_with_engine(engine, &text);
    let node = layout
        .nodes
        .iter()
        .find(|node| node.id == "A")
        .expect("state A");

    assert!(
        node.width > 160.0,
        "min-content must be allowed to widen the configured 120px table: {node:?}"
    );
    assert_eq!(
        node.height, 232.0,
        "the widened table must be reflowed at its actual min-content width"
    );
}

#[test]
fn state_layout_measures_note_markup_as_rendered_html() {
    let layout = layout_state_from_text(
        r#"stateDiagram-v2
A
note right of A
  <a href='https://mermaid.js.org/' target='_blank'><code>note about mermaid</code></a><br/>
  <img src=x onerror=alert(1)>
end note
"#,
    );
    let note = layout
        .nodes
        .iter()
        .find(|node| node.id.contains("----note-"))
        .expect("rendered note");

    assert!(
        (190.0..=220.0).contains(&note.width),
        "markup must contribute rendered content, not literal tag text: {note:?}"
    );
    assert!(
        note.height <= 120.0,
        "HTML tags must not be counted as wrapped text lines: {note:?}"
    );
}

#[test]
fn state_layout_note_groups_contain_notes() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("upstream_stateDiagram_v2_note_statements_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_state_from_text(&text);

    let mut node_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        node_by_id.insert(n.id.as_str(), n);
    }
    let mut cluster_by_id = std::collections::HashMap::new();
    for c in &layout.clusters {
        cluster_by_id.insert(c.id.as_str(), c);
    }

    let parent = cluster_by_id["Active----parent"];
    let note = node_by_id["Active----note-2"];

    assert!(
        rect_contains(rect_from_cluster(parent), rect_from_node(note), 1e-6),
        "note should be inside its noteGroup cluster"
    );
}

#[test]
fn state_layout_composite_and_dividers_contain_children() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("upstream_stateDiagram_v2_concurrent_state_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_state_from_text(&text);

    let mut node_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        node_by_id.insert(n.id.as_str(), n);
    }
    let mut cluster_by_id = std::collections::HashMap::new();
    for c in &layout.clusters {
        cluster_by_id.insert(c.id.as_str(), c);
    }

    let active = cluster_by_id["Active"];
    let div1 = cluster_by_id["divider-id-1"];
    let div2 = cluster_by_id["divider-id-2"];
    let div3_id = cluster_by_id
        .keys()
        .copied()
        .find(|id| id.starts_with("id-"))
        .expect("expected generated divider id (id-*)");
    let div3 = cluster_by_id[div3_id];

    let active_rect = rect_from_cluster(active);
    let div1_rect = rect_from_cluster(div1);
    let div2_rect = rect_from_cluster(div2);
    let div3_rect = rect_from_cluster(div3);

    assert!(rect_contains(active_rect, div1_rect, 1e-6));
    assert!(rect_contains(active_rect, div2_rect, 1e-6));
    assert!(rect_contains(active_rect, div3_rect, 1e-6));

    let num_lock_off = node_by_id["NumLockOff"];
    let num_lock_on = node_by_id["NumLockOn"];
    assert!(rect_contains(div1_rect, rect_from_node(num_lock_off), 1e-6));
    assert!(rect_contains(div1_rect, rect_from_node(num_lock_on), 1e-6));

    let caps_lock_off = node_by_id["CapsLockOff"];
    let caps_lock_on = node_by_id["CapsLockOn"];
    assert!(rect_contains(
        div2_rect,
        rect_from_node(caps_lock_off),
        1e-6
    ));
    assert!(rect_contains(div2_rect, rect_from_node(caps_lock_on), 1e-6));

    let scroll_lock_off = node_by_id["ScrollLockOff"];
    let scroll_lock_on = node_by_id["ScrollLockOn"];
    assert!(rect_contains(
        div3_rect,
        rect_from_node(scroll_lock_off),
        1e-6
    ));
    assert!(rect_contains(
        div3_rect,
        rect_from_node(scroll_lock_on),
        1e-6
    ));
}

#[test]
fn state_layout_exposes_one_logical_self_loop_edge() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("upstream_stateDiagram_v2_composite_self_link_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_state_from_text(&text);

    let self_loop = layout
        .edges
        .iter()
        .find(|edge| edge.from == "Active" && edge.to == "Active")
        .expect("logical Active self-loop");

    assert_eq!(self_loop.id, "edge1");
    assert_eq!(self_loop.points.len(), 4);
    assert!(self_loop.label.is_some());

    let ordinary_edge = layout
        .edges
        .iter()
        .find(|edge| edge.id == "edge0")
        .expect("ordinary composite transition");
    assert_eq!(
        ordinary_edge.points.len(),
        4,
        "the self-loop helper key must not perturb the unrelated transition topology"
    );
    assert!(
        layout
            .edges
            .iter()
            .all(|edge| !edge.id.contains("cyclic-special")),
        "cyclic-special segments are internal Dagre helpers, not public layout edges"
    );
    assert!(
        layout
            .nodes
            .iter()
            .any(|node| node.id == "Active---Active---1")
            && layout
                .nodes
                .iter()
                .any(|node| node.id == "Active---Active---2"),
        "Dagre self-loop helper nodes must remain available as layout hints"
    );
}

#[test]
fn state_safe_anchor_avoids_leaf_inside_extractable_sibling_cluster() {
    let layout = layout_state_from_text(
        r#"stateDiagram-v2
state P {
  state I {
    a
  }
  b
}
b --> x
P --> y
"#,
    );

    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing state node {id}"))
    };
    let edge = layout
        .edges
        .iter()
        .find(|edge| edge.from == "P" && edge.to == "y")
        .expect("direct edge from P to y");
    let route_control = edge.points.get(1).expect("first Dagre route control point");
    let distance = |id: &str| {
        let node = node(id);
        (route_control.x - node.x).hypot(route_control.y - node.y)
    };

    let distance_to_a = distance("a");
    let distance_to_b = distance("b");
    assert!(
        distance_to_b < distance_to_a,
        "the edge anchor must survive extraction of sibling cluster I: points={:?}, \
         a={:?}, b={:?}, distance_to_a={distance_to_a}, distance_to_b={distance_to_b}",
        edge.points,
        node("a"),
        node("b")
    );
    assert!(
        edge.from_cluster.is_some(),
        "the rebound edge must retain its source-cluster metadata"
    );
}

#[test]
fn state_layout_extracts_explicit_direction_composite_with_external_edge() {
    let layout = layout_state_from_text(
        r#"stateDiagram-v2
state Composite {
  direction LR
  A --> B: internal
}
B --> Outside: external
"#,
    );

    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing state node {id}"))
    };
    let a = node("A");
    let b = node("B");

    assert!(
        b.x > a.x && (b.y - a.y).abs() < 1e-6,
        "explicit LR direction must govern the extracted composite layout: A={a:?}, B={b:?}"
    );
    assert!(
        layout
            .edges
            .iter()
            .any(|edge| edge.from == "B" && edge.to == "Outside"),
        "the cross-boundary edge must survive explicit-direction extraction"
    );
}

#[test]
fn state_layout_keeps_implicit_direction_composite_with_external_edge_in_parent_graph() {
    let layout = layout_state_from_text(
        r#"stateDiagram-v2
direction LR
state Composite {
  A --> B: internal
}
B --> Outside: external
"#,
    );

    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing state node {id}"))
    };
    let a = node("A");
    let b = node("B");

    assert!(
        b.x > a.x && (b.y - a.y).abs() < 1e-6,
        "an implicit nested direction must not override the LR parent graph when the composite has external edges: A={a:?}, B={b:?}"
    );
}
