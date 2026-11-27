#![cfg(any(feature = "diagram-flowchart", feature = "diagram-agentflow"))]

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

fn assert_shoal_capability(source: &str) {
    let parsed = parse(source);
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

#[cfg(feature = "diagram-flowchart")]
#[test]
fn explicit_shoal_selection_respects_build_and_session_capability() {
    assert_shoal_capability(
        "---\nconfig:\n  flowchart:\n    layout: shoal\n---\nflowchart LR\nsubgraph G\nA --> B\nend",
    );
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn agentflow_explicit_shoal_selection_respects_build_and_session_capability() {
    assert_shoal_capability("---\nconfig:\n  layout: shoal\n---\nagentflow-beta LR\nA --> B");
    let parsed = parse("agentflow-beta LR\nA --> B");
    let session = RenderEnvironment::deterministic()
        .with_capability_policy(RenderCapabilityPolicy::deny_all())
        .begin_session()
        .unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    assert!(
        !plan
            .required_capabilities()
            .contains(&RenderCapability::LayoutShoal)
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn fit_width_admission_follows_the_selected_flowchart_backend() {
    use merman_render::LayoutAdaptation;

    let options = LayoutOptions::default()
        .with_adaptation(LayoutAdaptation::FitWidth)
        .with_container_size(240.0, 600.0);
    for (layout, expected) in [
        (
            "shoal",
            &[LayoutAdaptation::None, LayoutAdaptation::FitWidth][..],
        ),
        ("dagre", &[LayoutAdaptation::None][..]),
        ("elk", &[LayoutAdaptation::None][..]),
    ] {
        let source = format!("---\nconfig:\n  layout: {layout}\n---\nflowchart TB\nA --> B");
        let parsed = parse(&source);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.supported_layout_adaptations(), expected);
        match (layout, cfg!(feature = "layout-shoal")) {
            ("shoal", true) => {
                let result = family::prepare(&parsed, &options, session).unwrap();
                let width = result.layout_adaptation().unwrap();
                assert_eq!(width.requested_width(), 240.0);
                assert!(!width.overflowed());
                assert!(width.actual_width() > 0.0);
            }
            ("shoal", false) => {
                assert!(matches!(
                    family::prepare(&parsed, &options, session),
                    Err(Error::MissingCapability {
                        capability: RenderCapability::LayoutShoal,
                        ..
                    })
                ));
            }
            _ => assert!(matches!(
                family::prepare(&parsed, &options, session),
                Err(Error::UnsupportedLayoutAdaptation { .. })
            )),
        }
    }
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn shoal_preview_reuses_semantic_node_order_after_an_edit() {
    use merman_render::ShoalLayoutFrame;
    use std::sync::Arc;

    let render = |diagram: &str, options: LayoutOptions| {
        let parsed = parse(&format!("---\nconfig:\n  layout: shoal\n---\n{diagram}"));
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &options, session).unwrap();
        let json = artifact.layout_json().unwrap().to_string();
        let svg = artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        (svg, json)
    };
    let first = "flowchart TB\nA --> C\nA --> B";
    let (plain, plain_json) = render(first, LayoutOptions::default());
    let (captured, captured_json) = render(
        first,
        LayoutOptions::default().with_shoal_layout_frame(None),
    );
    assert!(plain.shoal_layout_frame().is_none());
    assert_eq!(plain.svg(), captured.svg());
    assert_eq!(plain_json, captured_json);
    let frame = captured.shoal_layout_frame().unwrap();
    let bytes = serde_json::to_vec(frame.as_ref()).unwrap();
    let frame: ShoalLayoutFrame = serde_json::from_slice(&bytes).unwrap();
    let nodes = |frame: &ShoalLayoutFrame| serde_json::to_value(frame).unwrap()["nodes"].clone();
    let edited = "flowchart TB\nA --> B[Relabeled]\nA --> C\nA --> D[New]";
    let (stateless, _) = render(
        edited,
        LayoutOptions::default().with_shoal_layout_frame(None),
    );
    let (stabilized, _) = render(
        edited,
        LayoutOptions::default().with_shoal_layout_frame(Some(Arc::new(frame.clone()))),
    );
    let before = nodes(&frame);
    let after = nodes(stabilized.shoal_layout_frame().unwrap().as_ref());
    let stateless = nodes(stateless.shoal_layout_frame().unwrap().as_ref());
    let cross_key = |frame: &serde_json::Value, id: &str| frame[id]["cross_key"].as_f64().unwrap();
    assert_eq!(before["A"]["rank"], 0);
    assert_eq!(before["B"]["rank"], 1);
    assert_eq!(after["D"]["rank"], 1);
    assert!(cross_key(&before, "C") < cross_key(&before, "B"));
    assert!(cross_key(&stateless, "B") < cross_key(&stateless, "C"));
    assert!(cross_key(&after, "C") < cross_key(&after, "B"));
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn shoal_preview_uses_local_ranks_and_discards_hints_after_a_parent_or_direction_change() {
    let render = |group, direction, declarations, options| {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n---\nflowchart TB\nsubgraph {group}\n  direction {direction}\n{declarations}\nend"
        );
        let parsed = parse(&source);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        family::prepare(&parsed, options, session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap()
    };
    let capture = LayoutOptions::default().with_shoal_layout_frame(None);
    let previous = render("G", "LR", "  A --> C\n  A --> B", &capture)
        .shoal_layout_frame()
        .unwrap();
    let measured = serde_json::to_value(previous.as_ref()).unwrap();
    let parent = measured["nodes"]["B"]["parent"].as_u64().unwrap() as usize;
    assert_eq!(measured["groups"][parent], "G");
    assert_eq!(measured["nodes"]["B"]["rank"], 1);
    assert_eq!(measured["nodes"]["B"]["direction"], "right");
    let reuse = LayoutOptions::default().with_shoal_layout_frame(Some(previous));
    for (group, direction) in [("H", "LR"), ("G", "RL")] {
        let edited = "  A --> B[Relabeled]\n  A --> C\n  A --> D[New]";
        let stateless = render(group, direction, edited, &capture);
        let hinted = render(group, direction, edited, &reuse);
        assert_eq!(hinted.svg(), stateless.svg());
        assert_eq!(hinted.shoal_layout_frame(), stateless.shoal_layout_frame());
    }
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
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

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
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

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn hand_drawn_shoal_preserves_both_shafts_and_their_interior_strokes() {
    use base64::Engine as _;
    use kurbo::{BezPath, ParamCurve, Point};
    use merman_render::model::LayoutPoint;

    let parsed = parse(
        "---\nconfig:\n  layout: shoal\n  look: handDrawn\n  handDrawnSeed: 42\n  flowchart:\n    rankSpacing: 160\n---\nflowchart LR\nA <--> B",
    );
    let prepare = || {
        family::prepare(
            &parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap()
    };
    let query = prepare().edge_geometry_json().unwrap();
    let svg = prepare()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = roxmltree::Document::parse(svg.svg()).unwrap();
    let edge = doc
        .descendants()
        .find(|node| node.attribute("data-et") == Some("edge"))
        .unwrap();
    assert_eq!(edge.attribute("data-look"), Some("handDrawn"));
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(edge.attribute("data-points").unwrap())
        .unwrap();
    let contacts: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
    let points = query[0].points.as_ref().unwrap();
    assert_eq!(contacts.len(), points.len());
    for (svg, query) in contacts.iter().zip(points) {
        assert!((svg.x - query.x).abs() < 1e-6 && (svg.y - query.y).abs() < 1e-6);
    }

    let ink = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
    let strokes: Vec<_> = ink.segments().collect();
    assert_eq!(ink.subpaths().count(), strokes.len());
    assert!(strokes.len() >= 6 && strokes.len().is_multiple_of(2));
    let source = points.first().unwrap();
    let target = points.last().unwrap();
    let marker_width = |attribute| {
        let url = edge.attribute(attribute).unwrap();
        let id = url
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        doc.descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap()
            .attribute("markerWidth")
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    for pass in [0, 1] {
        let segments: Vec<_> = strokes.iter().skip(pass).step_by(2).collect();
        assert!(
            segments
                .windows(2)
                .all(|pair| pair[0].end() == pair[1].start())
        );
        let first = segments[0];
        let last = segments.last().unwrap();
        assert!(first.start().distance(Point::new(source.x + 4., source.y)) < 1e-6);
        assert!(last.end().distance(Point::new(target.x - 4., target.y)) < 1e-6);
        for (shaft, depth) in [
            (first, marker_width("marker-start")),
            (*last, marker_width("marker-end")),
        ] {
            assert!(shaft.end().x - shaft.start().x >= depth);
            for sample in [0., 0.25, 0.5, 0.75, 1.] {
                assert!((shaft.eval(sample).y - source.y).abs() < 1e-6);
            }
        }
        assert!(
            segments
                .iter()
                .any(|segment| (segment.eval(0.5).y - source.y).abs() > 1e-6)
        );
    }
    let (passes, _) = strokes.as_chunks::<2>();
    assert!(
        passes
            .iter()
            .any(|pair| (pair[0].eval(0.5).y - pair[1].eval(0.5).y).abs() > 1e-6)
    );
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn hand_drawn_shoal_paints_every_audit_shape_and_cylinder_variant() {
    let (_, svg) = render(
        r#"---
config:
  layout: shoal
  look: handDrawn
  handDrawnSeed: 7
---
flowchart LR
  A([Stadium]) --> B{Diamond}
  B --> C[(Cylinder)]
  B --> D{{Hexagon}}
  C --> E[/Parallelogram/]
  D --> F[[Subroutine]]
  E --> G(((Double Circle)))
  F --> G
  G --> H@{ shape: h-cyl, label: "Horizontal cylinder" }
  G --> I@{ shape: lin-cyl, label: "Lined cylinder" }
  style I fill:#ABCDEF,stroke:#123456,stroke-width:2px,stroke-dasharray:5 3
"#,
        "FlowchartV2",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for id in ["A", "B", "C", "D", "E", "F", "G", "H", "I"] {
        let node = document
            .descendants()
            .find(|node| {
                node.attribute("data-et") == Some("node") && node.attribute("data-id") == Some(id)
            })
            .unwrap();
        assert!(
            node.attribute("class")
                .unwrap()
                .split_whitespace()
                .any(|class| class == "rough-node")
        );
        let paths: Vec<_> = node
            .descendants()
            .filter(|child| child.has_tag_name("path"))
            .collect();
        assert!(!paths.is_empty(), "missing shape geometry for {id}");
        for path in &paths {
            for property in ["fill", "stroke"] {
                assert!(
                    path.attribute(property)
                        .is_some_and(|value| !value.is_empty()),
                    "shape {id} lacks explicit {property} on a hand-drawn path"
                );
            }
        }
        if id == "I" {
            assert!(
                paths
                    .iter()
                    .any(|path| path.attribute("stroke") == Some("#ABCDEF"))
            );
            assert!(paths.iter().any(|path| {
                path.attribute("stroke") == Some("#123456")
                    && path.attribute("stroke-width") == Some("2")
                    && path.attribute("stroke-dasharray") == Some("5 3")
            }));
        }
    }
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
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

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
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
fn render(source: &str, variant: &str) -> (merman_render::model::FlowchartLayout, String) {
    let parsed = parse(source);
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let layout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"][variant].clone()).unwrap();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    assert_eq!(
        svg.required_capabilities(),
        &[RenderCapability::LayoutShoal]
    );
    (layout, svg.svg().to_owned())
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn shoal_docks_both_labeled_decision_branches_on_their_side_vertices() {
    let (layout, svg) = render(
        "---\nconfig:\n  layout: shoal\n  look: neo\n  htmlLabels: false\n  flowchart:\n    curve: rounded\n---\nflowchart TB\nA[Start] --> B{Condition?}\nB yes@-->|Yes| C[Approve]\nB no@-->|No| D[Reject]",
        "FlowchartV2",
    );
    assert_docked_decision_branches(&layout, &svg);
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn shoal_docks_labeled_branches_on_the_native_wide_diamond() {
    use merman_render::model::FlowchartLayout;
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    let parsed = parse(
        "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n  flowchart:\n    curve: rounded\n---\nflowchart TB\nA[Start] --> B{Condition?}\nB yes@-->|Yes| C[Execute]\nB no@-->|No| D[End]",
    );
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"paper"}"#, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let snapshot = artifact.layout_json().unwrap();
    let layout: FlowchartLayout =
        serde_json::from_value(snapshot["layout"]["FlowchartV2"].clone()).unwrap();
    let diamond = layout.nodes.iter().find(|node| node.id == "B").unwrap();
    assert!((diamond.width / diamond.height - 1.6).abs() < 1e-6);
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    assert_docked_decision_branches(&layout, svg.svg());
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
fn assert_docked_decision_branches(layout: &merman_render::model::FlowchartLayout, svg: &str) {
    use kurbo::{BezPath, PathSeg, Rect};

    let decision = layout.nodes.iter().find(|node| node.id == "B").unwrap();
    let document = roxmltree::Document::parse(svg).unwrap();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    for (id, side) in [("yes", -1.0), ("no", 1.0)] {
        let edge = layout.edges.iter().find(|edge| edge.id == id).unwrap();
        let [start, bend, end] = edge.points.as_slice() else {
            panic!("{id} does not have a single elbow: {:?}", edge.points);
        };
        assert!(near(start.x, decision.x + side * decision.width / 2.0));
        assert!(near(start.y, decision.y) && near(bend.y, start.y));
        assert!(side * (bend.x - start.x) > 0.0);
        assert!(near(bend.x, end.x) && end.y > bend.y);

        let element = document
            .descendants()
            .find(|node| {
                node.attribute("data-et") == Some("edge") && node.attribute("data-id") == Some(id)
            })
            .unwrap();
        assert!(element.attribute("marker-end").is_some());
        let path = BezPath::from_svg(element.attribute("d").unwrap()).unwrap();
        let label = edge.label.as_ref().unwrap();
        assert!(
            path.segments().any(|segment| {
                let PathSeg::Line(line) = segment else {
                    return false;
                };
                (near(line.p0.x, label.x)
                    && near(line.p1.x, label.x)
                    && (line.p0.y.min(line.p1.y)..=line.p0.y.max(line.p1.y)).contains(&label.y))
                    || (near(line.p0.y, label.y)
                        && near(line.p1.y, label.y)
                        && (line.p0.x.min(line.p1.x)..=line.p0.x.max(line.p1.x)).contains(&label.x))
            }),
            "{id} label must lie on its painted shaft"
        );
        let label = Rect::from_center_size((label.x, label.y), (label.width, label.height));
        assert!(layout.nodes.iter().all(|node| {
            if node.id == decision.id {
                let dx = node.x - node.x.clamp(label.x0, label.x1);
                let dy = node.y - node.y.clamp(label.y0, label.y1);
                return dx.abs() / (node.width * 0.5) + dy.abs() / (node.height * 0.5) >= 1.0;
            }
            let node = Rect::from_center_size((node.x, node.y), (node.width, node.height));
            label.intersect(node).area() == 0.0
        }));
    }
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn compound_flowcharts_keep_named_groups_members_and_original_edges() {
    use kurbo::Rect;
    for (source, counts) in [
        (
            include_str!(
                "../../../fixtures/flowchart/stress_flowchart_nested_subgraphs_titles_001.mmd"
            ),
            (7, 3, 7),
        ),
        (
            include_str!(
                "../../../fixtures/flowchart/stress_flowchart_subgraph_cross_edges_between_siblings_048.mmd"
            ),
            (4, 2, 4),
        ),
        (
            include_str!(
                "../../../fixtures/flowchart/stress_flowchart_subgraph_edges_to_subgraph_and_internal_edges_041.mmd"
            ),
            (5, 2, 4),
        ),
    ] {
        let (layout, svg) = render(
            &format!("---\nconfig:\n  layout: shoal\n---\n{source}"),
            "FlowchartV2",
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let count = |kind| {
            doc.descendants()
                .filter(|n| n.attribute("data-et") == Some(kind))
                .count()
        };
        assert_eq!((count("node"), count("cluster"), count("edge")), counts);
        let frame = |id: &str| {
            let group = layout.clusters.iter().find(|group| group.id == id).unwrap();
            Rect::from_center_size((group.x, group.y), (group.width, group.height))
        };
        for (id, group) in [
            ("s", "A1"),
            ("b1", "A1"),
            ("d1", "A2"),
            ("l1", "left"),
            ("r2", "right"),
            ("b", "sg_to"),
            ("c", "sg_inherit"),
        ] {
            if let Some(node) = layout.nodes.iter().find(|node| node.id == id) {
                assert!(
                    frame(group).contains_rect(Rect::from_center_size(
                        (node.x, node.y),
                        (node.width, node.height)
                    )),
                    "{group}/{id}"
                );
            }
        }
        if layout.clusters.iter().any(|g| g.id == "A") {
            assert!(frame("A").contains_rect(frame("A1")) && frame("A").contains_rect(frame("A2")));
        }
        if layout.clusters.iter().any(|g| g.id == "sg_to") {
            let edge = layout
                .edges
                .iter()
                .find(|e| e.from == "outside" && e.to == "sg_to")
                .unwrap();
            let port = edge.points.last().unwrap();
            let group = frame("sg_to");
            let error = [
                port.x - group.x0,
                port.x - group.x1,
                port.y - group.y0,
                port.y - group.y1,
            ]
            .into_iter()
            .map(f64::abs)
            .fold(f64::INFINITY, f64::min);
            assert!(error < 1e-6, "group contact at {port:?}, frame {group:?}");
        }
    }
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn positioned_titles_and_local_directions_reach_painted_group_frames() {
    for (vertical, horizontal) in [
        ("top", "left"),
        ("top", "center"),
        ("top", "right"),
        ("bottom", "left"),
        ("bottom", "center"),
        ("bottom", "right"),
    ] {
        let (layout, svg) = render(
            &format!(
                "---\nconfig:\n  layout: shoal\n  htmlLabels: true\n  flowchart:\n    subGraphTitlePosition: {vertical}-{horizontal}\n---\nflowchart TB\nsubgraph A[Title]\ndirection LR\nstart[First node] --> endNode[Last node]\nend\nsubgraph B[Second]\ndirection RL\nfirst --> second\nend\nB@{{ titlePosition: bottom-right }}\nA --> B"
            ),
            "FlowchartV2",
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        for (id, y_side, x_side, direction) in [
            ("A", vertical, horizontal, "LR"),
            ("B", "bottom", "right", "RL"),
        ] {
            let group = layout.clusters.iter().find(|g| g.id == id).unwrap();
            assert_eq!(group.effective_dir, direction);
            let title = &group.title_label;
            let element = doc
                .descendants()
                .find(|n| {
                    n.attribute("data-et") == Some("cluster") && n.attribute("data-id") == Some(id)
                })
                .unwrap();
            let label = element
                .children()
                .find(|n| n.attribute("class") == Some("cluster-label"))
                .unwrap();
            let (left, top) = label
                .attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split_once(',')
                .unwrap();
            assert!(
                (left.trim().parse::<f64>().unwrap() + title.width / 2.0 - title.x).abs() < 1e-4
            );
            assert!(
                (top.trim().parse::<f64>().unwrap() + title.height / 2.0 - title.y).abs() < 1e-4
            );
            assert!(if y_side == "top" {
                title.y < group.y
            } else {
                title.y > group.y
            });
            match x_side {
                "left" => assert!(title.x < group.x),
                "right" => assert!(title.x > group.x),
                _ => assert!((title.x - group.x).abs() < 1e-4),
            }
        }
        let x = |id: &str| layout.nodes.iter().find(|n| n.id == id).unwrap().x;
        assert!(x("start") < x("endNode") && x("first") > x("second"));
    }
}

#[cfg(feature = "layout-shoal")]
fn assert_struck_title_halo(keyword: &str, group_keyword: &str) {
    use merman_render::svg::SvgPipeline;
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    for html_labels in [false, true] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n  htmlLabels: {html_labels}\n---\n\
             {keyword} TB\n\
             {group_keyword} Parent[Parent]\n\
             {group_keyword} G[A long title with no room beside the entering route]\n\
             A[Short]\nend\nend\n\
             Outside trunk@--> A\n\
             style Parent fill:#FF0000\nstyle G fill:#0000FF80\n"
        );
        let parsed = parse(&source);
        let theme = ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"paper","form":{"elevation":"flat"}}"#,
                None,
            )
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let check = |svg: &str, contains_html: bool| {
            let doc = roxmltree::Document::parse(svg).unwrap();
            let edge = doc
                .descendants()
                .find(|node| {
                    node.attribute("data-et") == Some("edge")
                        && node.attribute("data-id") == Some("trunk")
                })
                .unwrap();
            let titles: Vec<_> = doc
                .descendants()
                .filter(|node| node.attribute("data-merman-title-struck") == Some("true"))
                .collect();
            assert_eq!(titles.len(), 1);
            let title = titles[0];
            let mut groups = doc.descendants().filter(|node| {
                node.attribute("data-et") == Some("cluster")
                    && node.attribute("data-id") == Some("G")
            });
            let group = groups.next().unwrap();
            assert!(groups.next().is_none());
            let group_id = group.attribute("id").unwrap();
            assert_eq!(
                title.attribute("data-merman-cluster-title-for"),
                Some(group_id)
            );
            assert_eq!(
                doc.descendants()
                    .filter(|n| n.attribute("id") == Some(group_id))
                    .count(),
                1
            );
            let outline = group
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            assert!(outline.range().start < edge.range().start);
            assert!(edge.range().start < title.range().start);
            assert_eq!(title.attribute("paint-order"), Some("stroke"));
            assert_eq!(title.attribute("stroke"), Some("#7F0080"));
            assert_eq!(title.attribute("stroke-width"), Some("5"));
            assert_eq!(
                title
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                contains_html,
            );
            if contains_html {
                let span = title
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("span") && node.attribute("class") == Some("nodeLabel")
                    })
                    .expect("struck HTML title span");
                let style = span.attribute("style").unwrap_or_default();
                assert!(
                    style.contains("-webkit-text-stroke:5px #7F0080"),
                    "struck HTML title halo must keep raw hex color instead of entity-decoding `#`: {style}"
                );
                assert!(!style.contains("&7F0080"));
            }
            assert!(title.descendants().any(|node| node.has_tag_name("text")));
        };
        check(rendered.svg(), html_labels);
        let finalized = rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap();
        assert!(
            !finalized.svg().as_str().contains("<switch"),
            "resvg_safe should unwrap struck-title switch fallback"
        );
        check(finalized.svg().as_str(), false);
    }
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn flowchart_shoal_dark_struck_title_preserves_authored_color_in_resvg_safe() {
    use merman_render::svg::SvgPipeline;
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    let source = "\
---
config:
  layout: shoal
---
flowchart TD
  req([Pull Request]) --> gate{CI Gate}
  subgraph ci[Verification]
    direction TB
    unit[Unit Tests]
    lint[Static Checks]
  end
  subgraph prod[Production Deploy]
    rollout[Canary Rollout]
  end
  gate -->|Fast Path| unit
  gate -->|Policy| lint
  unit --> rollout
  lint --> rollout
  style prod fill:#1e3a5f,stroke:#60a5fa,color:#ffffff
";
    let parsed = parse(source);
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let finalized = rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap();
    let svg = finalized.svg().as_str();
    assert!(!svg.contains("<switch"), "resvg_safe must unwrap <switch>");
    let doc = roxmltree::Document::parse(svg).unwrap();
    let title = doc
        .descendants()
        .find(|node| node.attribute("data-merman-title-struck") == Some("true"))
        .expect("struck dark cluster title");
    let text = title
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .expect("native SVG <text> fallback");
    let style = text.attribute("style").unwrap_or_default();
    assert!(
        style.contains("fill:#ffffff !important"),
        "dark struck cluster title fallback must carry authored fill: {style}"
    );
}

#[cfg(feature = "layout-shoal")]
fn assert_compatibility_struck_title_fill(
    keyword: &str,
    group_keyword: &str,
    variant: &str,
    expected_fill: &str,
) {
    let source = format!(
        "---\nconfig:\n  layout: shoal\n  theme: redux-color\n  look: neo\n  htmlLabels: false\n---\n\
         {keyword} TB\n\
         {group_keyword} Parent[Parent]\n\
         {group_keyword} G[A long title with no room beside the entering route]\n\
         A[Short]\nend\nend\n\
         Outside trunk@--> A\n",
    );
    let (_, svg) = render(&source, variant);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let title = document
        .descendants()
        .find(|node| node.attribute("data-merman-title-struck") == Some("true"))
        .unwrap();
    assert_eq!(title.attribute("stroke"), Some(expected_fill));
    assert_eq!(title.attribute("stroke-width"), Some("5"));

    let group_id = title.attribute("data-merman-cluster-title-for").unwrap();
    let group = document
        .descendants()
        .find(|node| node.attribute("id") == Some(group_id))
        .unwrap();
    let rect = group
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    assert!(
        rect.attribute("style")
            .unwrap()
            .split(';')
            .filter_map(|decl| decl.split_once(':'))
            .any(|(property, value)| property == "fill" && value == expected_fill)
    );
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn flowchart_shoal_struck_title_matches_the_compatibility_cluster_fill() {
    assert_compatibility_struck_title_fill("flowchart", "subgraph", "FlowchartV2", "#F9F9FB");
}

#[cfg(all(feature = "diagram-agentflow", feature = "layout-shoal"))]
#[test]
fn agentflow_shoal_struck_title_matches_the_palette_cluster_fill() {
    assert_compatibility_struck_title_fill("agentflow-beta", "flow", "AgentflowDiagram", "#EEF2FF");
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn flowchart_shoal_struck_titles_keep_their_surface_halo_in_static_svg() {
    assert_struck_title_halo("flowchart", "subgraph");
}

#[cfg(all(feature = "diagram-agentflow", feature = "layout-shoal"))]
#[test]
fn agentflow_shoal_struck_titles_keep_their_surface_halo_in_static_svg() {
    assert_struck_title_halo("agentflow-beta", "flow");
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn own_member_markers_point_into_the_group_from_both_semantic_ends() {
    use kurbo::{BezPath, ParamCurve, Rect};
    let (layout, svg) = render(
        "---\nconfig:\n  layout: shoal\n---\nflowchart TB\nsubgraph G[Group]\nA --> B\nend\nG forward@<-->|assign| A\nB returnEdge@--> G",
        "FlowchartV2",
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let group = &layout.clusters[0];
    let inside = Rect::from_center_size((group.x, group.y), (group.width, group.height));
    for (id, marker, reverse) in [
        ("forward", "marker-start", false),
        ("returnEdge", "marker-end", true),
    ] {
        let edge = doc
            .descendants()
            .find(|n| n.attribute("data-et") == Some("edge") && n.attribute("data-id") == Some(id))
            .unwrap();
        assert!(edge.attribute(marker).is_some());
        let path = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
        let near_group = if reverse {
            path.reverse_subpaths()
        } else {
            path
        };
        let first = near_group.segments().next().unwrap();
        assert!(
            inside.contains(first.eval(0.9)),
            "{id} has an outward marker shaft"
        );
    }
    assert!(
        layout
            .edges
            .iter()
            .find(|e| e.id == "forward")
            .unwrap()
            .label
            .is_some()
    );
}

#[cfg(all(feature = "diagram-agentflow", feature = "layout-shoal"))]
#[test]
fn collapsed_agentflow_redirects_edges_and_keeps_semantic_markers() {
    let source = include_str!("../../../fixtures/agentflow/local_nested_collapse_routes.mmd")
        .replace("layout: dagre", "layout: shoal");
    let (layout, svg) = render(
        &format!("{source}\nmanual[Manual]@{{ shape: refdoc }}\ntask cite@-.- manual"),
        "AgentflowDiagram",
    );
    assert!(!layout.nodes.iter().any(|node| node.id == "task"));
    assert_eq!(layout.clusters.len(), 1);
    assert_eq!(layout.clusters[0].id, "outer");
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let collapsed = doc
        .descendants()
        .find(|n| n.attribute("data-et") == Some("node") && n.attribute("data-id") == Some("inner"))
        .unwrap();
    assert!(collapsed.descendants().any(|n| {
        n.attribute("class")
            .is_some_and(|s| s.split_whitespace().any(|c| c == "flow-collapsed"))
    }));
    assert_eq!(layout.edges.len(), 4);
    for (id, from, to) in [
        ("enter", "start", "inner"),
        ("exit", "inner", "api"),
        ("loop", "inner", "inner"),
        ("cite", "inner", "manual"),
    ] {
        let edge = layout.edges.iter().find(|e| e.id == id).unwrap();
        assert_eq!((edge.from.as_str(), edge.to.as_str()), (from, to));
        let path = doc
            .descendants()
            .find(|n| n.attribute("data-et") == Some("edge") && n.attribute("data-id") == Some(id))
            .unwrap();
        assert_eq!(path.attribute("marker-end").is_some(), id != "cite");
        if id == "cite" {
            assert!(path.attribute("marker-start").is_none());
            assert!(
                path.attribute("class")
                    .unwrap()
                    .contains("edge-pattern-dotted")
            );
        }
    }
}

#[cfg(all(feature = "diagram-agentflow", feature = "layout-shoal"))]
#[test]
fn agentflow_shoal_consumes_its_family_spacing() {
    let extents = |spacing| {
        let (layout, _) = render(
            &format!(
                "---\nconfig:\n  layout: shoal\n  flowchart:\n    nodeSpacing: 17\n    rankSpacing: 17\n  agentflow:\n    nodeSpacing: {spacing}\n    rankSpacing: {spacing}\n---\nagentflow-beta TB\nA --> B\nA --> C"
            ),
            "AgentflowDiagram",
        );
        let node = |id: &str| layout.nodes.iter().find(|n| n.id == id).unwrap();
        (
            (node("B").x - node("C").x).abs(),
            (node("A").y - node("B").y).abs(),
        )
    };
    let small = extents(35);
    let large = extents(135);
    assert!(
        large.0 > small.0 && large.1 > small.1,
        "{small:?} -> {large:?}"
    );
}

#[cfg(feature = "layout-shoal")]
fn render_studio_shoal(source: &str) -> String {
    use merman_render::theme::{NativeTheme, ThemeCompiler};
    let parsed = parse(source);
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap()
        .svg()
        .to_owned()
}

#[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
#[test]
fn flowchart_shoal_dark_container_adapts_edge_label_ink_and_multi_loops_stay_smooth() {
    let dark_svg = render_studio_shoal(
        "---\nconfig:\n  layout: shoal\n---\n\
flowchart TD\n\
  subgraph DarkBox[Dark Container]\n\
    A[Step 1] -->|transition| B[Step 2]\n\
  end\n\
  style DarkBox fill:#1e293b,stroke:#475569,color:#f8fafc\n",
    );
    assert!(
        dark_svg.contains(r#"<span class="edgeLabel" style="color:#FFFFFF !important;">"#),
        "edge label inside dark subgraph must switch to white ink: {dark_svg}"
    );

    let loop_svg = render_studio_shoal(
        "---\nconfig:\n  layout: shoal\n---\n\
flowchart TD\n\
  A[Start] --> B[Process]\n\
  B --> B\n\
  B -->|retry| B\n\
  B -->|backoff| B\n\
  B --> C[Done]\n",
    );
    let doc = roxmltree::Document::parse(&loop_svg).unwrap();
    let mut loop_paths = 0usize;
    for path in doc.descendants().filter(|n| {
        n.has_tag_name("path")
            && n.attribute("data-et") == Some("edge")
            && n.attribute("data-id")
                .is_some_and(|id| id.starts_with("L_B_B_"))
    }) {
        let d = path.attribute("d").unwrap();
        assert!(
            d.matches('C').count() >= 2,
            "self-loop {} must curve both corners without 1-ULP collapse or sharp return: {d}",
            path.attribute("data-id").unwrap()
        );
        loop_paths += 1;
    }
    assert_eq!(loop_paths, 3);
}

#[cfg(all(feature = "diagram-class", feature = "layout-shoal"))]
#[test]
fn class_shoal_namespace_edge_labels_use_container_surface_backplate() {
    let svg = render_studio_shoal(
        "---\nconfig:\n  layout: shoal\n---\n\
classDiagram\n\
  namespace Core {\n\
    class Repository\n\
    class AbstractRepo\n\
  }\n\
  Repository <|.. AbstractRepo : implements\n",
    );
    assert!(
        svg.contains("background-color:#EFF4FA !important;"),
        "Class edge label inside namespace Core must sample container fill (#EFF4FA) instead of canvas white"
    );
}

#[cfg(all(feature = "diagram-c4", feature = "layout-shoal"))]
#[test]
fn c4_shoal_wide_edge_labels_stay_near_routes_not_canvas_bottom() {
    let svg = render_studio_shoal(
        "---\nconfig:\n  layout: shoal\n---\n\
C4Context\n\
  title System Context\n\
  Person(customer, \"Customer\", \"Bank customer\")\n\
  Enterprise_Boundary(b0, \"Bank\") {\n\
    System(banking, \"Internet Banking\", \"Core app\")\n\
    System(mail, \"E-mail\", \"Exchange\")\n\
  }\n\
  Rel(customer, banking, \"Views balances using\", \"HTTPS\")\n\
  Rel(banking, mail, \"Sends mail via\", \"SMTP\")\n\
  Rel(mail, customer, \"Sends alerts to\", \"SMTP\")\n",
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let label_ys: Vec<f64> = doc
        .descendants()
        .filter(|n| {
            n.has_tag_name("g")
                && n.attribute("data-id")
                    .is_some_and(|id| id.starts_with("c4-rel-"))
        })
        .map(|n| {
            let tf = n.attribute("transform").unwrap();
            let (_, y) = tf
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split_once(',')
                .unwrap();
            y.trim().parse::<f64>().unwrap()
        })
        .collect();
    assert_eq!(label_ys.len(), 3);
    for y in label_ys {
        assert!(
            y < 420.0,
            "C4 edge label detached to bottom of canvas at y={y}"
        );
    }
}

#[cfg(all(feature = "diagram-architecture", feature = "layout-shoal"))]
#[test]
fn architecture_shoal_parallel_entries_keep_straight_border_alignment() {
    let svg = render_studio_shoal(
        "---\nconfig:\n  layout: shoal\n---\n\
architecture-beta\n\
  group cloud(cloud)[Production Cloud Platform]\n\
  group edge(internet)[Edge & Ingress] in cloud\n\
  group core(server)[Core Services] in cloud\n\
  group data(database)[Data Tier] in cloud\n\
  service cdn(internet)[Global CDN Distribution] in edge\n\
  service waf(server)[Web Application Firewall] in edge\n\
  service gw(server)[API Gateway] in core\n\
  service auth(server)[Auth & Identity Service] in core\n\
  service billing(server)[Billing Microservice] in core\n\
  service db(database)[Primary PostgreSQL Cluster] in data\n\
  service cache(disk)[Redis Session Cache] in data\n\
  junction hub in core\n\
  cdn:R --> L:waf\n\
  waf:B --> T:gw\n\
  gw:R <--> L:hub\n\
  hub:T --> B:auth\n\
  hub:R --> L:billing\n\
  hub:B -- T:db\n\
  auth:R --> L:cache\n\
  billing:B --> T:db\n\
  db:L <--> R:cache\n",
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let hub_db = doc
        .descendants()
        .find(|n| {
            n.has_tag_name("path") && n.attribute("id").is_some_and(|id| id.contains("L_hub_db"))
        })
        .expect("hub -> db edge path");
    let d = hub_db.attribute("d").unwrap();
    assert!(
        !d.contains('A'),
        "hub:B -- T:db should drop straight across group borders without an S-jog: {d}"
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_flowchart_calibrated_widths_and_hand_drawn_cylinders_and_class_alignment() {
    let render_any = |source: &str| -> (merman_render::model::FlowchartLayout, String) {
        let parsed = parse(source);
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap();
        let layout = serde_json::from_value(
            artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone(),
        )
        .unwrap();
        let svg = artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        (layout, svg.svg().to_owned())
    };

    // B1: Shoal Flowchart uses calibrated deterministic widths (wider on identifiers above
    // Flowchart's 152px minimum box width than Compatibility Dagre).
    let label_source = "flowchart LR\nA[customer_account_service] --> B[Observability]";
    let (shoal_layout, _) = render(
        &format!("---\nconfig:\n  layout: shoal\n---\n{label_source}"),
        "FlowchartV2",
    );
    let (dagre_layout, _) = render_any(&format!(
        "---\nconfig:\n  layout: dagre\n---\n{label_source}"
    ));
    let shoal_w = shoal_layout
        .nodes
        .iter()
        .find(|n| n.id == "A")
        .unwrap()
        .width;
    let dagre_w = dagre_layout
        .nodes
        .iter()
        .find(|n| n.id == "A")
        .unwrap()
        .width;
    assert!(
        shoal_w > dagre_w,
        "Shoal calibrated Flowchart width ({shoal_w}) must exceed Compatibility Dagre width ({dagre_w})"
    );

    // B2: Shoal handDrawn cylinder with classDef + htmlLabels:false preserves fill & stroke !important
    // and scopes SVG classDef path rules to `.node`/`.cluster` so `.rough-node` hatches stay clean.
    let (_, shoal_cyl_svg) = render(
        "---\nconfig:\n  layout: shoal\n  look: handDrawn\n  htmlLabels: false\n---\n\
flowchart TB\n\
  db[(Primary DB)]:::accent\n\
  classDef accent fill:#dbeafe,stroke:#1d4ed8,stroke-width:2px",
        "FlowchartV2",
    );
    assert!(
        shoal_cyl_svg.contains("fill:none !important;stroke:#1d4ed8 !important"),
        "handDrawn Shoal cylinder interior rim must guard against classDef fill override"
    );
    assert!(
        shoal_cyl_svg.contains(".node.accent path,")
            && shoal_cyl_svg.contains(".cluster.accent path{"),
        "handDrawn SVG classDef rules must scope shape selectors away from .rough-node"
    );

    // M2: Non-Shoal (Dagre) handDrawn cylinder emits explicit fill/stroke attributes on body path.
    let (_, dagre_cyl_svg) = render_any(
        "---\nconfig:\n  layout: dagre\n  look: handDrawn\n---\n\
flowchart TB\n\
  db[(Primary DB)]",
    );
    let doc = roxmltree::Document::parse(&dagre_cyl_svg).unwrap();
    let db_node = doc
        .descendants()
        .find(|n| n.attribute("data-id") == Some("db"))
        .expect("db node");
    let has_styled_body = db_node.descendants().any(|n| {
        n.has_tag_name("path") && n.attribute("fill").is_some() && n.attribute("stroke").is_some()
    });
    assert!(
        has_styled_body,
        "non-Shoal handDrawn cylinder must emit explicit fill and stroke attributes"
    );

    // M5: Shoal Class member rows include `text-align: left;` inside foreignObject.
    #[cfg(feature = "diagram-class")]
    {
        let class_svg = render_studio_shoal(
            "---\nconfig:\n  layout: shoal\n---\n\
classDiagram\n\
  class Ledger {\n\
    +id: String\n\
    +commit() bool\n\
  }",
        );
        assert!(
            class_svg.contains("text-align: left;"),
            "Shoal Class member foreignObject divs must include explicit left alignment"
        );
    }
}
