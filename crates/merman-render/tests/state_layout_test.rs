use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use merman_model::presentation::{
    AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
};
use merman_model::resources::InputResourcePolicy;
use merman_model::{MermaidConfig, OperationControl, OperationPhase};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::StateDiagramLayout;
use merman_render::resources::RenderResourcePolicy;
use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle, WrapMode};
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

fn layout_state_from_text_with_engine(engine: MermaidEngine, text: &str) -> StateDiagramLayout {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::default(), session).expect("typed State layout");
    let projection = artifact.layout_json().expect("State layout projection");
    serde_json::from_value(projection["layout"]["StateDiagramV2"].clone()).expect("State layout")
}

fn layout_state_from_text_with_options(
    text: &str,
    parse_options: ParseOptions,
) -> StateDiagramLayout {
    layout_state_from_text_with_options_and_policy(
        text,
        parse_options,
        RenderResourcePolicy::interactive(),
    )
}

fn layout_state_from_text_with_options_and_policy(
    text: &str,
    parse_options: ParseOptions,
    resource_policy: RenderResourcePolicy,
) -> StateDiagramLayout {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, parse_options)
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::default(), session).expect("typed State layout");
    let projection = artifact.layout_json().expect("State layout projection");
    serde_json::from_value(projection["layout"]["StateDiagramV2"].clone()).expect("State layout")
}

fn state_presentation_from_nodes(nodes: serde_json::Value) -> PresentationDiagram {
    let config = MermaidConfig::from_value(serde_json::json!({"layout": "dagre"}));
    let nodes: Vec<StateDiagramRenderNode> = serde_json::from_value(nodes).unwrap();
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "stateDiagram".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
            theme_recipe: None,
        },
        RenderSemanticModel::State(StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes,
            ..Default::default()
        }),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("construct State presentation")
    .expect("admit State presentation data")
}

#[test]
fn state_direct_model_rejects_parent_cycles_after_layout_id_mapping() {
    for nodes in [
        serde_json::json!([
            {"id": "A", "domId": "shared", "isGroup": true, "shape": "rect"},
            {"id": "B", "domId": "shared", "parentId": "A", "shape": "rect"}
        ]),
        serde_json::json!([
            {"id": "A", "domId": "x", "parentId": "B", "shape": "rect"},
            {"id": "B", "domId": "y", "isGroup": true, "shape": "rect"},
            {"id": "C", "domId": "y", "parentId": "A", "shape": "rect"}
        ]),
    ] {
        let presentation = state_presentation_from_nodes(nodes);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = family::prepare(&presentation, &LayoutOptions::default(), session)
            .err()
            .expect("acyclic semantic IDs must not hide a cyclic layout forest");
        let merman_render::Error::InvalidModel { message } = error else {
            panic!("expected InvalidModel, got {error:?}");
        };
        assert!(message.contains("state layout parent cycle"), "{message}");
    }
}

#[test]
fn state_direct_model_rejects_generated_helper_parent_collision() {
    let presentation = state_presentation_from_nodes(serde_json::json!([
        {"id": "P", "domId": "A---A---1", "isGroup": true, "shape": "rect"},
        {"id": "A", "domId": "state-A", "parentId": "P", "shape": "rect"}
    ]));
    let (metadata, mut model, context) = presentation.into_parts();
    let RenderSemanticModel::State(state) = &mut model else {
        panic!("expected State model");
    };
    state.edges.push(StateDiagramRenderEdge {
        id: "loop".to_string(),
        start: "A".to_string(),
        end: "A".to_string(),
        ..Default::default()
    });
    let presentation = PresentationDiagram::try_from_frozen(
        metadata,
        model,
        context,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let error = family::prepare(&presentation, &LayoutOptions::default(), session)
        .err()
        .expect("generated helper IDs must also belong to an acyclic layout forest");
    let merman_render::Error::InvalidModel { message } = error else {
        panic!("expected InvalidModel, got {error:?}");
    };
    assert!(message.contains("state layout parent cycle"), "{message}");
}

#[test]
fn state_direct_model_preserves_note_group_merges_and_authored_labels() {
    let presentation = state_presentation_from_nodes(serde_json::json!([
        {"id": "Outer", "domId": "outer", "isGroup": true, "shape": "rect"},
        {"id": "A----parent", "domId": "first-group", "isGroup": true,
         "shape": "noteGroup", "parentId": "Outer"},
        {"id": "A----parent-2", "domId": "last-group", "isGroup": true,
         "shape": "noteGroup", "parentId": "Outer"},
        {"id": "A", "domId": "state-A", "shape": "rect", "parentId": "A----parent",
         "label": "Repeated label"},
        {"id": "note-1", "domId": "note-1", "shape": "note", "parentId": "A----parent",
         "label": "Repeated label"},
        {"id": "note-2", "domId": "note-2", "shape": "note", "parentId": "A----parent-2",
         "label": "Repeated label"}
    ]));
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&presentation, &LayoutOptions::default(), session)
        .expect("intentional note-group aliases must remain valid");
    let projection = artifact.layout_json().unwrap();
    let layout: StateDiagramLayout =
        serde_json::from_value(projection["layout"]["StateDiagramV2"].clone()).unwrap();
    let group = layout
        .clusters
        .iter()
        .find(|group| group.id == "A----parent-2")
        .expect("last note group is the physical cluster");
    assert!(
        !layout
            .clusters
            .iter()
            .any(|group| group.id == "A----parent")
    );
    for id in ["A", "note-1", "note-2"] {
        let node = layout.nodes.iter().find(|node| node.id == id).unwrap();
        assert!(rect_contains(
            rect_from_cluster(group),
            rect_from_node(node),
            1e-6
        ));
    }
    let RenderSemanticModel::State(model) = presentation.model() else {
        panic!("expected State model");
    };
    for node in model.nodes.iter().filter(|node| !node.is_group) {
        assert_eq!(node.label.as_ref().unwrap(), "Repeated label");
    }
}

#[test]
fn state_direct_model_parent_admission_preserves_operation_terminals() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
        TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use std::sync::Arc;

    struct CancellingHost;
    impl HostTextMeasurer for CancellingHost {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(None)
        }

        fn measure_with_control(
            &self,
            _request: HostTextMeasurementRequest<'_>,
            control: &OperationControl,
        ) -> HostMeasurementResult {
            control.cancel();
            Ok(None)
        }
    }

    let presentation = state_presentation_from_nodes(serde_json::json!([
        {"id": "A", "domId": "shared", "isGroup": true, "shape": "rect"},
        {"id": "B", "domId": "shared", "parentId": "A", "shape": "rect"}
    ]));
    let control = OperationControl::new();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.state-parent-cancellation").unwrap(),
        "v1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            Arc::new(CancellingHost),
            TextMeasurementPhase::ALL,
        ))
        .begin_session_with_control(control.clone())
        .unwrap();
    assert!(
        matches!(
            family::prepare(&presentation, &LayoutOptions::default(), session),
            Err(merman_render::Error::Cancelled(_)),
        ),
        "cancellation during node label measurement must precede the cycle error"
    );

    for cancel_first in [true, false] {
        let control = OperationControl::new();
        let policy = RenderResourcePolicy::interactive()
            .with_limit(merman_render::ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session_with_control(control.clone())
            .unwrap();
        if cancel_first {
            control.cancel();
        }
        let error = family::prepare(&presentation, &LayoutOptions::default(), session)
            .err()
            .expect("operation terminal must precede State structural rejection");
        if cancel_first {
            assert!(matches!(error, merman_render::Error::Cancelled(_)));
        } else {
            let merman_render::Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected adapter resource rejection, got {error:?}");
            };
            assert_eq!(limit.limit, "max_layout_work_units");
            assert!(limit.actual > limit.max);
        }
        let terminal = control
            .terminal_checkpoint_at(OperationPhase::Layout)
            .unwrap_err();
        control.cancel();
        assert_eq!(
            control
                .terminal_checkpoint_at(OperationPhase::Layout)
                .unwrap_err(),
            terminal,
            "later cancellation must preserve the first operation terminal",
        );
    }
}

#[test]
fn state_parse_for_render_model_handles_deep_composite_chain() {
    const DEPTH: usize = 1500;
    let text = deep_state_composite_chain(DEPTH);

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();

    assert_eq!(parsed.metadata().diagram_type, "stateDiagram");
}

#[test]
fn state_layout_handles_deep_composite_chain() {
    const DEPTH: usize = 512;
    let text = deep_state_composite_chain(DEPTH);

    let layout = layout_state_from_text_with_options_and_policy(
        &text,
        ParseOptions::strict(),
        RenderResourcePolicy::unbounded_for_trusted_input(),
    );

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
    let label_size = |root: bool, flowchart: bool| {
        let layout = layout_state_from_text(&format!(
            "%%{{init: {config}}}%%\nstateDiagram-v2\nA --> B: owns\n",
            config = serde_json::json!({"htmlLabels":root, "flowchart":{"htmlLabels":flowchart}})
        ));
        let label = layout
            .edges
            .iter()
            .find_map(|edge| edge.label.as_ref())
            .expect("State edge label");
        (label.width, label.height)
    };
    let svg_control = label_size(false, false);
    let html_control = label_size(true, true);

    assert_ne!(
        svg_control, html_control,
        "the controls must distinguish SVG and HTML label geometry"
    );
    assert_eq!(
        label_size(false, true),
        svg_control,
        "root htmlLabels=false must override deprecated flowchart=true"
    );
    assert_eq!(
        label_size(true, false),
        html_control,
        "root htmlLabels=true must override deprecated flowchart=false"
    );
}

#[test]
fn state_layout_preserves_html_min_content_width_for_long_labels() {
    let layout = layout_state_from_text(
        r#"%%{init: {"htmlLabels": true, "state": {"wrappingWidth": 200}}}%%
stateDiagram-v2
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
        long_node.width > 200.0,
        "an unbreakable HTML label must expand beyond the configured 200px wrapping width: {long_node:?}"
    );
    assert!(
        long_edge.width > 200.0,
        "an unbreakable transition segment must expand the HTML table beyond the configured 200px wrapping width: {long_edge:?}"
    );
}

#[test]
fn state_layout_reflows_at_the_expanded_html_min_content_width() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("stress_state_font_size_precedence_071.mmd");
    let text = std::fs::read_to_string(path).expect("font-size precedence fixture");
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "secure": [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ]
        })),
    );
    let layout = layout_state_from_text_with_engine(engine, &text);
    let node = layout
        .nodes
        .iter()
        .find(|node| node.id == "A")
        .expect("state A");

    assert!(
        node.width > 120.0,
        "min-content must be allowed to widen the configured 120px table: {node:?}"
    );
    assert_eq!(
        node.height, 232.0,
        "the widened table must be reflowed at its actual min-content width"
    );
}

#[test]
fn state_layout_measures_note_markup_as_rendered_html() {
    let literal_markup = "<a href='https://mermaid.js.org/' target='_blank'><code>note about mermaid</code></a><br/> <img src=x onerror=alert(1)>";
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
    let literal_metrics = DeterministicTextMeasurer::default().measure_wrapped(
        literal_markup,
        &TextStyle::default(),
        Some(200.0),
        WrapMode::HtmlLike,
    );

    assert!(
        note.width < literal_metrics.width + 16.0,
        "markup must contribute rendered content, not literal tag text: note={note:?}, literal={literal_metrics:?}"
    );
    assert!(
        note.height < literal_metrics.height + 16.0,
        "HTML tags must not be counted as wrapped text lines: note={note:?}, literal={literal_metrics:?}"
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

    let layout =
        layout_state_from_text(&format!("%%{{init: {{\"layout\": \"dagre\"}}}}%%\n{text}"));

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
fn state_layout_keeps_explicit_direction_composite_with_external_edge_in_parent_graph() {
    let layout = layout_state_from_text(
        r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
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
        (b.y - a.y).abs() > 1e-6,
        "an explicit nested direction must not extract a composite with external edges: A={a:?}, B={b:?}"
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
        r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
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

#[allow(dead_code)]
mod common;
