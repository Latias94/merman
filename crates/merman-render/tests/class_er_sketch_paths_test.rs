#![cfg(all(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "layout-dagre"
))]

use kurbo::{BezPath, PathSeg};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};

fn render_path(layout: &str, diagram: &str, edge_style: &str) -> (BezPath, String) {
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: classic\n  htmlLabels: false\n---\n{diagram}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let theme = serde_json::json!({
        "schema_version": 2,
        "extends": "studio",
        "form": { "edge_style": edge_style }
    });
    let compiled = ThemeCompiler::new()
        .compile_json(theme.to_string().as_bytes(), None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(compiled))
        .unwrap();
    let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = roxmltree::Document::parse(svg.svg()).unwrap();
    let edge = doc
        .descendants()
        .find(|node| node.attribute("data-et") == Some("edge"))
        .unwrap();
    assert!(edge.attribute("marker-start").is_some());
    assert!(edge.attribute("marker-end").is_some());
    if layout == "shoal" {
        assert!(
            !edge
                .attribute("style")
                .is_some_and(|value| value.contains(";;;"))
        );
    }
    (
        BezPath::from_svg(edge.attribute("d").unwrap()).unwrap(),
        edge.attribute("data-points").unwrap().to_owned(),
    )
}

#[test]
fn sketch_paths_curve_without_changing_marker_contacts_or_tangents() {
    let layouts = [
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
        #[cfg(feature = "layout-shoal")]
        "shoal",
    ];
    for layout in layouts {
        for diagram in [
            "classDiagram\nA <|--* B : owns\n",
            "erDiagram\nA }o--o{ B : owns\n",
        ] {
            let (line, line_points) = render_path(layout, diagram, "straight");
            let (sketch, sketch_points) = render_path(layout, diagram, "sketch");
            assert_eq!(line_points, sketch_points, "{layout}: {diagram}");
            assert!(
                line.segments()
                    .all(|segment| matches!(segment, PathSeg::Line(_)))
            );
            assert!(
                sketch
                    .segments()
                    .any(|segment| matches!(segment, PathSeg::Cubic(_)))
            );
            let first = |path: &BezPath| path.segments().next().unwrap().to_cubic();
            let last = |path: &BezPath| path.segments().last().unwrap().to_cubic();
            let check_tangent = |a: [kurbo::Vec2; 3], b: [kurbo::Vec2; 3]| {
                let tangent = |controls: [kurbo::Vec2; 3]| {
                    controls
                        .into_iter()
                        .find(|vector| vector.hypot2() > 1e-12)
                        .expect("nondegenerate terminal tangent")
                        .normalize()
                };
                assert!(
                    (tangent(a) - tangent(b)).hypot() < 1e-6,
                    "{layout}: {diagram}: {a:?} vs {b:?}"
                );
            };
            let (a, b) = (first(&line), first(&sketch));
            assert!(a.p0.distance(b.p0) < 1e-6, "{layout}: {diagram}");
            check_tangent(
                [a.p1 - a.p0, a.p2 - a.p0, a.p3 - a.p0],
                [b.p1 - b.p0, b.p2 - b.p0, b.p3 - b.p0],
            );
            let (a, b) = (last(&line), last(&sketch));
            assert!(a.p3.distance(b.p3) < 1e-6, "{layout}: {diagram}");
            check_tangent(
                [a.p3 - a.p2, a.p3 - a.p1, a.p3 - a.p0],
                [b.p3 - b.p2, b.p3 - b.p1, b.p3 - b.p0],
            );
        }
    }
}
