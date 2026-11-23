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
