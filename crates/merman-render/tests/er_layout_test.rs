use merman_core::{Engine, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::environment::{RenderEnvironment, TextMeasurementPhase};
use merman_render::er::layout_er_diagram_typed;
use merman_render::model::ErDiagramLayout;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn layout_er_with_dagre(text: &str) -> ErDiagramLayout {
    let parsed: ParsedDiagramRender = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let RenderSemanticModel::Er(model) = &parsed.model else {
        panic!("expected ER render model");
    };
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    layout_er_diagram_typed(model, parsed.meta.effective_config.as_value(), &measurer)
        .expect("Dagre ER layout")
}

#[test]
fn er_layout_produces_positions_and_routes() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_er_with_dagre(&text);

    assert!(layout.nodes.len() >= 3);
    assert!(layout.edges.len() >= 2);

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
fn er_layout_emits_markers_and_dashes_from_rel_spec() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_relationship_aliases.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_er_with_dagre(&text);

    let mut has_marker = false;
    let mut has_dashed = false;
    for e in &layout.edges {
        if e.start_marker.is_some() || e.end_marker.is_some() {
            has_marker = true;
        }
        if e.stroke_dasharray.as_deref() == Some("8,8") {
            has_dashed = true;
        }
    }

    assert!(has_marker, "expected at least one edge to have ER markers");
    assert!(
        has_dashed,
        "expected at least one NON_IDENTIFYING relationship to be dashed"
    );
}
