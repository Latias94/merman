#![cfg(all(
    feature = "layout-shoal",
    any(
        feature = "diagram-agentflow",
        feature = "diagram-architecture",
        feature = "diagram-block",
        feature = "diagram-c4",
        feature = "diagram-class",
        feature = "diagram-er",
        feature = "diagram-flowchart",
        feature = "diagram-requirement",
        feature = "diagram-state",
        feature = "diagram-swimlane",
        feature = "diagram-usecase"
    )
))]

use kurbo::{BezPath, ParamCurve, PathSeg};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::theme::{NativeTheme, ThemeCompiler, ThemePortabilityRequirement};
use merman_render::{LayoutOptions, RenderCapability, family};
use merman_theme_contract::v2::Form;

struct Edge {
    id: String,
    points: String,
    path: BezPath,
}

fn render(diagram: &str, style: &str, config: &str) -> Vec<Edge> {
    let source =
        format!("---\nconfig:\n  layout: shoal\n  htmlLabels: false\n{config}---\n{diagram}");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap_or_else(|error| panic!("{diagram}: {error}"))
        .unwrap()
        .into_presentation();
    let defaults = Form::default();
    let theme = serde_json::json!({
        "schema_version": 2, "extends": "studio",
        "form": {
            "edge_style": style, "elevation": "flat", "marker_style": "classic",
            "radius": defaults.radius, "container_radius": defaults.container_radius
        }
    });
    let theme = ThemeCompiler::new()
        .compile_json(theme.to_string().as_bytes(), None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let prepared = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap_or_else(|error| panic!("{diagram} ({style}): {error}"));
    let rendered = prepared
        .render_svg(&Default::default(), &Default::default())
        .unwrap_or_else(|error| panic!("{diagram} ({style}): {error}"));
    assert!(
        rendered
            .required_capabilities()
            .contains(&RenderCapability::LayoutShoal)
    );
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    let mut edges = doc
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-et") == Some("edge"))
        .map(|node| Edge {
            id: node.attribute("data-id").unwrap().to_owned(),
            points: node.attribute("data-points").unwrap().to_owned(),
            path: BezPath::from_svg(node.attribute("d").unwrap()).unwrap(),
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    edges
}

fn axis_aligned(segment: PathSeg) -> bool {
    let delta = segment.end() - segment.start();
    delta.x.abs() < 1e-6 || delta.y.abs() < 1e-6
}

fn assert_linear(edges: &[Edge], orthogonal: bool, context: &str) {
    for edge in edges {
        assert!(
            edge.path.segments().next().is_some(),
            "{context}: {}",
            edge.id
        );
        assert!(
            edge.path.segments().all(|segment| {
                matches!(segment, PathSeg::Line(_)) && (!orthogonal || axis_aligned(segment))
            }),
            "{context}: {} {}",
            edge.id,
            edge.path.to_svg()
        );
    }
}

fn assert_styles(diagram: &str, config: &str, count: usize) -> (Vec<Edge>, Vec<Edge>, Vec<Edge>) {
    let curved = render(diagram, "curved", config);
    let orthogonal = render(diagram, "orthogonal", config);
    let straight = render(diagram, "straight", config);
    let sketch = render(diagram, "sketch", config);
    assert_eq!(orthogonal.len(), count, "{diagram}");
    let ids = |edges: &[Edge]| edges.iter().map(|edge| edge.id.clone()).collect::<Vec<_>>();
    let orthogonal_ids = ids(&orthogonal);
    for style in [&curved, &straight, &sketch] {
        assert_eq!(ids(style), orthogonal_ids, "{diagram}");
    }
    assert!(
        curved
            .iter()
            .flat_map(|edge| edge.path.segments())
            .any(|seg| !matches!(seg, PathSeg::Line(_))),
        "{diagram}: curved shafts"
    );
    assert!(
        curved
            .iter()
            .zip(&orthogonal)
            .any(|(a, b)| a.points != b.points)
            || straight
                .iter()
                .zip(&orthogonal)
                .any(|(a, b)| a.points != b.points),
        "{diagram}: routing must affect layout geometry (curved: {}, orthogonal: {}, straight: {})",
        curved[0].path.to_svg(),
        orthogonal[0].path.to_svg(),
        straight[0].path.to_svg(),
    );
    assert!(
        straight
            .iter()
            .zip(&sketch)
            .all(|(a, b)| a.points == b.points),
        "{diagram}: sketch must use the same route knots as straight"
    );
    for edge in &sketch {
        assert!(
            edge.path
                .segments()
                .any(|segment| matches!(segment, PathSeg::Cubic(_))),
            "{diagram}: sketch stroke missing on {}",
            edge.id,
        );
    }
    assert_linear(&orthogonal, true, diagram);
    assert_linear(&straight, false, diagram);
    (curved, orthogonal, straight)
}

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-requirement",
    feature = "diagram-state",
    feature = "diagram-usecase"
))]
#[test]
fn directed_families_receive_theme_routing_and_paint() {
    #[cfg(feature = "diagram-agentflow")]
    let agentflow = include_str!("fixtures/elk/flowchart_crossings.mmd").replacen(
        "flowchart",
        "agentflow-beta",
        1,
    );
    let diagrams = [
        #[cfg(feature = "diagram-flowchart")]
        include_str!("fixtures/elk/flowchart_crossings.mmd"),
        #[cfg(feature = "diagram-agentflow")]
        agentflow.as_str(),
        #[cfg(feature = "diagram-class")]
        include_str!("fixtures/elk/class_crossings.mmd"),
        #[cfg(feature = "diagram-er")]
        include_str!("fixtures/elk/er_crossings.mmd"),
        #[cfg(feature = "diagram-requirement")]
        include_str!("fixtures/elk/requirement_crossings.mmd"),
        #[cfg(feature = "diagram-state")]
        include_str!("fixtures/elk/state_crossings.mmd"),
        #[cfg(feature = "diagram-usecase")]
        include_str!("fixtures/elk/usecase_crossings.mmd"),
    ];
    for diagram in diagrams {
        let (curved, orthogonal, straight) = assert_styles(diagram, "", 9);
        for (style, routes) in [("orthogonal", orthogonal), ("straight", straight)] {
            assert!(
                curved.iter().zip(routes).any(|(a, b)| a.points != b.points),
                "{diagram}: {style} routing must change the layout knots"
            );
        }
    }
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_uses_theme_and_authored_default_for_route_projection() {
    let diagram = "swimlane-beta TB\nsubgraph Lane1\nA\nB\nend\nsubgraph Lane2\nC\nD\nend\nC inbound@--> B\nA cross@--> D\nA local@--> B\nD return@--> A\n";
    let config = "  swimlane:\n    lineHops: false\n";
    let (_, _, straight) = assert_styles(diagram, config, 4);
    let overridden = render(
        diagram,
        "orthogonal",
        &format!("{config}  flowchart:\n    curve: linear\n"),
    );
    for (explicit, themed) in overridden.iter().zip(&straight) {
        assert_eq!(explicit.points, themed.points, "{}", explicit.id);
        assert_eq!(explicit.path, themed.path, "{}", explicit.id);
    }
}

#[cfg(any(
    feature = "diagram-architecture",
    feature = "diagram-block",
    feature = "diagram-c4"
))]
#[test]
fn fixed_grid_themes_allow_clear_diagonals_between_port_leads() {
    let diagrams = [
        #[cfg(feature = "diagram-architecture")]
        (
            "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R --> T:b\n",
            1,
        ),
        #[cfg(feature = "diagram-block")]
        (
            "block\ncolumns 2\nA[\"A\"] space\nspace B[\"B\"]\nA --> B\n",
            1,
        ),
        #[cfg(feature = "diagram-c4")]
        (
            "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nSystem(c, \"C\")\nRel(b, c, \"\")\n",
            1,
        ),
    ];
    for (diagram, count) in diagrams {
        let (_, _, straight) = assert_styles(diagram, "", count);
        assert!(
            straight
                .iter()
                .flat_map(|edge| edge.path.segments())
                .any(|segment| !axis_aligned(segment)),
            "{diagram}: straight shafts should use the clear diagonal"
        );
    }
}
