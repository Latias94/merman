use base64::Engine as _;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{LayoutPoint, MindmapDiagramLayout};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

const SOURCE: &str =
    "mindmap\n  root((Root))\n    left[Left branch]\n      leaf[Leaf]\n    right[Right branch]\n";

fn render(layout: &str) -> (MindmapDiagramLayout, String) {
    let engine =
        Engine::new().with_site_config(MermaidConfig::from_value(json!({ "layout": layout })));
    let parsed = engine
        .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
        .expect("parse Mindmap")
        .expect("detect Mindmap");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare registered Mindmap layout");
    let projection = artifact.layout_json().expect("layout JSON");
    let layout = serde_json::from_value(projection["layout"]["MindmapDiagram"].clone())
        .expect("Mindmap layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Mindmap SVG")
        .svg()
        .to_owned();
    (layout, svg)
}

fn svg_points(svg: &str, id: &str) -> Vec<LayoutPoint> {
    let document = roxmltree::Document::parse(svg).expect("SVG XML");
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| {
            node.attribute("data-id") == Some(id) && node.attribute("data-edge") == Some("true")
        })
        .collect();
    let path = paths.into_iter().next().expect("painted edge");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(path.attribute("data-points").expect("points"))
        .expect("base64");
    serde_json::from_slice(&bytes).expect("point JSON")
}

#[test]
fn mindmap_registered_dagre_and_tidy_keep_their_own_routes() {
    let (dagre, _) = render("dagre");
    let (tidy, svg) = render("tidy-tree");
    assert_eq!(dagre.nodes.len(), 4);
    assert_eq!(dagre.edges.len(), 3);
    for edge in &dagre.edges {
        let start = dagre
            .nodes
            .iter()
            .find(|node| node.id == edge.from)
            .unwrap();
        let end = dagre.nodes.iter().find(|node| node.id == edge.to).unwrap();
        assert!(end.y > start.y, "Mindmap Dagre direction is TB");
    }
    assert!(
        dagre
            .nodes
            .iter()
            .zip(&tidy.nodes)
            .any(|(a, b)| (a.x - b.x).abs() > 1.0 || (a.y - b.y).abs() > 1.0)
    );
    for edge in &tidy.edges {
        let points = svg_points(&svg, &edge.id);
        assert_eq!(points.len(), edge.points.len());
        for (painted, positioned) in points.iter().zip(&edge.points) {
            assert!((painted.x - positioned.x).abs() < 1e-9);
            assert!((painted.y - positioned.y).abs() < 1e-9);
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn mindmap_registered_elk_preserves_labels_and_clips_circle_routes() {
    let (layout, svg) = render("elk");
    let root = layout
        .nodes
        .iter()
        .find(|node| node.id == "0")
        .expect("root");
    assert_eq!(root.width, root.height);
    // label_width is an internal layout metric and is intentionally skipped from the public
    // projection; verify the measured label reached the rendered SVG instead.
    let document = roxmltree::Document::parse(&svg).expect("SVG XML");
    let root_label = document
        .descendants()
        .find(|node| {
            node.attribute("id")
                .is_some_and(|id| id.ends_with("-node_0"))
        })
        .and_then(|node| {
            node.descendants()
                .find(|child| child.has_tag_name("foreignObject"))
        })
        .expect("root measured label");
    assert!(
        root_label
            .attribute("width")
            .and_then(|value| value.parse::<f64>().ok())
            .is_some_and(|width| width > 0.0)
    );
    let mut routed_bend = false;
    for edge in &layout.edges {
        let start = layout
            .nodes
            .iter()
            .find(|node| node.id == edge.from)
            .unwrap();
        let end = layout.nodes.iter().find(|node| node.id == edge.to).unwrap();
        assert!(end.y > start.y, "Mindmap ELK direction is DOWN");
        let painted = svg_points(&svg, &edge.id);
        assert!(painted.len() >= 2);
        assert!(
            painted
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
        routed_bend |= painted.windows(3).any(|p| {
            let a = (p[1].x - p[0].x, p[1].y - p[0].y);
            let b = (p[2].x - p[1].x, p[2].y - p[1].y);
            (a.0 * b.1 - a.1 * b.0).abs() > 1e-3
        });
        if edge.from == root.id {
            let point = &painted[0];
            let radius = root.width / 2.0;
            assert!(
                ((point.x - root.x).hypot(point.y - root.y) - radius).abs() < 0.1,
                "circular attachment must follow the shape, not COSE's 15px center offset"
            );
        }
    }
    assert!(routed_bend, "SVG must retain the provider's bends");
}

#[test]
fn mindmap_missing_loader_uses_available_family_fallback() {
    let (fallback, _) = render("not-registered");
    let (expected, _) = render(if cfg!(feature = "layout-cytoscape") {
        "cose-bilkent"
    } else {
        "dagre"
    });
    assert_eq!(
        serde_json::to_value(fallback).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    #[cfg(not(feature = "layout-elk"))]
    {
        let (elk_request, _) = render("elk");
        assert_eq!(
            serde_json::to_value(elk_request).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
}
