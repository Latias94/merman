#![cfg(feature = "diagram-flowchart")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::environment::RenderEnvironment;
use merman_render::{Error, LayoutOptions, RenderCapability, RenderCapabilityPolicy, family};

fn parse(source: &str) -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[test]
fn explicit_shoal_selection_respects_build_and_session_capability() {
    let parsed = parse("---\nconfig:\n  flowchart:\n    layout: shoal\n---\nflowchart LR\nA --> B");
    for (policy, available) in [
        (
            RenderCapabilityPolicy::unrestricted(),
            cfg!(feature = "layout-shoal"),
        ),
        (RenderCapabilityPolicy::deny_all(), false),
    ] {
        let session = RenderEnvironment::deterministic()
            .with_capability_policy(policy)
            .begin_session()
            .unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(
            plan.required_capabilities(),
            &[RenderCapability::LayoutShoal]
        );
        assert_eq!(plan.is_ready(), available);
        let result = family::prepare(&parsed, &LayoutOptions::default(), session);
        if available {
            assert_eq!(
                result
                    .unwrap()
                    .render_svg(&Default::default(), &Default::default())
                    .unwrap()
                    .required_capabilities(),
                &[RenderCapability::LayoutShoal]
            );
        } else {
            assert!(matches!(
                result,
                Err(Error::MissingCapability {
                    capability: RenderCapability::LayoutShoal,
                    ..
                })
            ));
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_paints_the_routes_and_anchors_of_each_visible_occurrence() {
    use base64::Engine as _;
    use merman_render::model::{FlowchartLayout, LayoutPoint};
    use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

    let parsed = parse(
        "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n---\nflowchart LR\nX((Circle)) L_A_B_0@-->|first| Y{Diamond}\nA -->|second| B\nlinkStyle 0 stroke:#ef4444\nlinkStyle 1 stroke:#2563eb",
    );
    let prepare = || {
        family::prepare(
            &parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap()
    };
    let artifact = prepare();
    let raw: FlowchartLayout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone())
            .unwrap();
    let geometry = prepare().edge_geometry_json().unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(svg.svg()).unwrap();
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .collect();
    assert_eq!((geometry.len(), raw.edges.len(), paths.len()), (2, 2, 2));
    assert_ne!(paths[0].attribute("id"), paths[1].attribute("id"));
    for (i, (text, stroke)) in [("first", "#ef4444"), ("second", "#2563eb")]
        .into_iter()
        .enumerate()
    {
        let edge = &raw.edges[i];
        let painted = &geometry[i];
        assert_eq!(edge.id, "L_A_B_0");
        assert_eq!(painted.id, edge.id);
        let path = paths
            .iter()
            .find(|path| path.attribute("style").unwrap_or_default().contains(stroke))
            .unwrap();
        assert_eq!(path.attribute("data-id"), Some(edge.id.as_str()));
        assert!(path.attribute("marker-end").is_some());
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap();
        let svg_points: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
        let points = painted.points.as_deref().unwrap();
        assert_eq!(svg_points.len(), points.len());
        for (point, svg_point) in points.iter().zip(svg_points) {
            assert!((point.x - svg_point.x).abs() < 1e-6 && (point.y - svg_point.y).abs() < 1e-6);
        }
        let src = raw.nodes.iter().find(|node| node.id == edge.from).unwrap();
        let dst = raw.nodes.iter().find(|node| node.id == edge.to).unwrap();
        assert!((edge.points[0].x - (src.x + src.width / 2.0)).abs() < 1e-6);
        assert!((edge.points.last().unwrap().x - (dst.x - dst.width / 2.0)).abs() < 1e-6);
        assert!(points.first().unwrap().x > src.x && points.last().unwrap().x < dst.x);
        let label = edge.label.as_ref().unwrap();
        let position = painted.label_position.as_ref().unwrap();
        assert!((position.x - label.x).abs() < 1e-6 && (position.y - label.y).abs() < 1e-6);
        let painted_label = document
            .descendants()
            .find(|node| node.is_text() && node.text() == Some(text))
            .unwrap();
        let group = painted_label
            .ancestors()
            .find(|node| node.attribute("class") == Some("edgeLabel"))
            .unwrap();
        let transform = group
            .attribute("transform")
            .unwrap()
            .strip_prefix("translate(")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let (x, y) = transform.split_once(',').unwrap();
        assert!(
            (x.trim().parse::<f64>().unwrap() - label.x).abs() < 1e-4
                && (y.trim().parse::<f64>().unwrap() - label.y).abs() < 1e-4
        );
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_keeps_the_shaft_aligned_through_the_arrowhead() {
    use kurbo::{BezPath, ParamCurve, ParamCurveArclen, Point};

    fn along(path: &BezPath, mut distance: f64) -> Point {
        for segment in path.segments() {
            let length = segment.arclen(1e-5);
            if distance <= length {
                return segment.eval(segment.inv_arclen(distance, 1e-5));
            }
            distance -= length;
        }
        panic!("expected a shaft at least one marker long");
    }

    for direction in ["TD", "BT", "LR", "RL"] {
        for curve in ["basis", "rounded", "linear", "stepAfter"] {
            let parsed = parse(&format!(
                "---\nconfig:\n  layout: shoal\n  look: neo\n  flowchart:\n    curve: {curve}\n---\nflowchart {direction}\nA([Start]) <-->|first| B[Review]\nA <--> C{{Decision}}\nB <--> Z([Publish])\nC <--> Z"
            ));
            let artifact = family::prepare(
                &parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .unwrap();
            let svg = artifact
                .render_svg(&Default::default(), &Default::default())
                .unwrap();
            let document = roxmltree::Document::parse(svg.svg()).unwrap();
            let paths: Vec<_> = document
                .descendants()
                .filter(|n| n.attribute("data-et") == Some("edge"))
                .collect();
            assert_eq!(paths.len(), 4);
            for edge in paths {
                let path = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
                for (attr, path) in [
                    ("marker-end", path.reverse_subpaths()),
                    ("marker-start", path),
                ] {
                    let id = edge
                        .attribute(attr)
                        .unwrap()
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let marker = document
                        .descendants()
                        .find(|n| n.attribute("id") == Some(id))
                        .unwrap();
                    let depth: f64 = marker.attribute("markerWidth").unwrap().parse().unwrap();
                    let tip = along(&path, 0.0);
                    let axis = (along(&path, 0.1) - tip).normalize();
                    let shaft = along(&path, depth) - tip;
                    assert!(
                        axis.dot(shaft) > 0.0 && axis.cross(shaft).abs() < 0.02,
                        "{direction} {curve} {attr} {}: shaft {shaft:?} diverges from marker axis {axis:?}",
                        edge.attribute("data-id").unwrap()
                    );
                }
            }
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_paints_labeled_self_loops_and_the_semantic_return_edge() {
    use kurbo::{BezPath, ParamCurve, Rect, Shape};
    use merman_render::model::FlowchartLayout;

    let parsed = parse(concat!(
        "---\nconfig:\n  layout: shoal\n---\n",
        include_str!("fixtures/shoal-cycle-loops.mmd")
    ));
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let geometry: FlowchartLayout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone())
            .unwrap();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = roxmltree::Document::parse(svg.svg()).unwrap();
    let mut loops = Vec::new();
    let mut saw_return = false;
    for edge in &geometry.edges {
        let path = doc
            .descendants()
            .find(|n| {
                n.attribute("data-et") == Some("edge")
                    && n.attribute("data-id") == Some(edge.id.as_str())
            })
            .unwrap();
        assert!(
            path.attribute("marker-start").is_none() && path.attribute("marker-end").is_some(),
            "{}",
            edge.id
        );
        assert!(edge.label.is_some());
        if edge.from == edge.to {
            let node = geometry.nodes.iter().find(|n| n.id == edge.from).unwrap();
            let body = Rect::from_center_size((node.x, node.y), (node.width, node.height));
            let d = path.attribute("d").unwrap();
            let bounds = BezPath::from_svg(d).unwrap().bounding_box();
            assert!(
                !body.contains_rect(bounds),
                "{}: self-loop stays inside its node",
                edge.id
            );
            loops.push(d);
        } else if edge.from == "check" && edge.to == "receive" {
            let curve = BezPath::from_svg(path.attribute("d").unwrap()).unwrap();
            let first = curve.segments().next().unwrap().start();
            let last = curve.segments().last().unwrap().end();
            let source = geometry.nodes.iter().find(|n| n.id == edge.from).unwrap();
            let target = geometry.nodes.iter().find(|n| n.id == edge.to).unwrap();
            let at_source = kurbo::Point::new(source.x, source.y);
            let at_target = kurbo::Point::new(target.x, target.y);
            assert!(
                first.distance(at_source) < first.distance(at_target)
                    && last.distance(at_target) < last.distance(at_source),
                "{}: painted shaft lost semantic direction",
                edge.id
            );
            saw_return = true;
        }
    }
    assert!(saw_return);
    assert_eq!(loops.len(), 2);
    assert_ne!(
        loops[0], loops[1],
        "repeated loops require distinct visible paths"
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn graph_curve_and_edge_overrides_reach_the_solver_and_painter() {
    use kurbo::BezPath;
    use merman_render::model::FlowchartLayout;

    let source = "---\nconfig:\n  layout: shoal\n  flowchart:\n    curve: linear\n---\nflowchart TD\nA bend@--> B\nA line@--> B\nlinkStyle default interpolate rounded\nline@{ curve: linear }";
    let parsed = parse(source);
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let layout: FlowchartLayout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone())
            .unwrap();
    assert_eq!(layout.edges.len(), 2);
    for edge in &layout.edges {
        assert!(
            edge.points
                .windows(2)
                .all(|p| p[0].x == p[1].x || p[0].y == p[1].y),
            "{}: graph default requests orthogonal route points",
            edge.id
        );
    }
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = roxmltree::Document::parse(svg.svg()).unwrap();
    let path = |id: &str| {
        let edge = doc
            .descendants()
            .find(|n| n.attribute("data-et") == Some("edge") && n.attribute("data-id") == Some(id))
            .unwrap();
        BezPath::from_svg(edge.attribute("d").unwrap()).unwrap()
    };
    assert!(
        path("bend")
            .segments()
            .any(|s| !matches!(s, kurbo::PathSeg::Line(_))),
        "rounded parallel lane must bend"
    );
    assert!(
        path("line")
            .segments()
            .all(|s| matches!(s, kurbo::PathSeg::Line(_))),
        "the individual edge requested a polyline"
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_rejects_subgraphs_until_compound_layout_is_available() {
    let parsed =
        parse("---\nconfig:\n  layout: shoal\n---\nflowchart TD\nsubgraph G\nA --> B\nend");
    let result = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    );
    assert!(matches!(
        result,
        Err(Error::UnsupportedLayoutConstruct {
            backend: "shoal",
            construct: "subgraphs"
        })
    ));
}
