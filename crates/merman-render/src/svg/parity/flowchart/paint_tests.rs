use super::svg_emit::test_support::{FlowchartSvgModelRequest, render_flowchart_svg_model};
use super::*;
use merman_render_core::svg_bounds::SvgPathBounds;
use merman_render_core::svg_edge_parity::{curve, svg_path_length_from_d};
use merman_render_flow::svg::{self as flow_svg, test_support::*};
use rustc_hash::FxHashMap;

const DAGRE_LINEAR_CONFIG: &str = "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n    curve: linear\n---\n";

fn with_source<T>(
    source: &str,
    tiny_svg_budget: bool,
    consume: impl FnOnce(FlowchartSvgModelRequest<'_>, &SvgExecution<'_>) -> Result<T>,
) -> T {
    with_layout(source, None, tiny_svg_budget, consume)
}

fn with_layout<T>(
    source: &str,
    layout: Option<FlowchartLayout>,
    tiny_svg_budget: bool,
    consume: impl FnOnce(FlowchartSvgModelRequest<'_>, &SvgExecution<'_>) -> Result<T>,
) -> T {
    let parsed = merman_core::MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = crate::environment::RenderEnvironment::deterministic();
    let merman_model::presentation::RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("Flowchart model")
    };
    let context = parsed.render_context().flowchart_render_context().unwrap();
    let layout = layout.unwrap_or_else(|| {
        let session = environment.begin_session().unwrap();
        let style = crate::test_support::prepare_flowchart_edge_style_plan(
            model,
            &parsed.metadata().effective_config,
            false,
            session.work_meter(),
        )
        .unwrap();
        crate::flowchart::dagre::layout(
            model,
            context,
            &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
            &parsed.metadata().effective_config,
            &session.text_measurer(crate::environment::TextMeasurementPhase::Layout),
            None,
            None,
            &style,
            std::sync::Arc::clone(session.work_meter()),
        )
        .unwrap()
    });
    let mut resources = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input();
    if tiny_svg_budget {
        resources = resources
            .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 1)
            .unwrap();
    }
    let session = environment
        .with_resource_policy(resources)
        .begin_session()
        .unwrap();
    let options = SvgRenderOptions::default();
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::unthemed_for_test(
        &options,
        &debug,
        &session,
        crate::DiagramFamilyId::FLOWCHART,
    )
    .unwrap();
    let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
    let theme = crate::flowchart::FlowchartPreparedTheme::resolve(
        &parsed.metadata().effective_config,
        None,
        None,
    );

    let config = render_config::prepare_flowchart_render_config(
        model,
        &parsed.metadata().effective_config,
        &theme.compatibility,
        layout.route_contract.is_elk_parity(),
        None,
    );
    let style = crate::test_support::prepare_flowchart_edge_style_plan(
        model,
        &parsed.metadata().effective_config,
        layout.route_contract.is_elk_parity(),
        execution.work_meter(),
    )
    .unwrap()
    .with_resolved_stroke_widths(model, &theme.compatibility, execution.work_meter())
    .unwrap()
    .with_typography_work_meter(std::sync::Arc::clone(session.work_meter()));
    style
        .bind_terminal_typography(&config.text_style, false)
        .unwrap();
    let nodes = FlowchartPreparedNodes::prepare(
        model,
        context,
        &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
        layout.node_layout_view(),
        &sidecar,
        &parsed.metadata().effective_config,
        &theme,
        &config,
        session.work_meter(),
        true,
    )
    .unwrap();
    consume(
        FlowchartSvgModelRequest {
            render_config: &config,
            prepared_nodes: &nodes,
            prepared_theme: &theme,
            layout: &layout,
            swimlane_layout: None,
            model,
            render_context: context,
            effective_config: &parsed.metadata().effective_config,
            diagram_type: &parsed.metadata().diagram_type,
            diagram_title: None,
            svg_label_sidecar: &sidecar,
            edge_style_plan: &style,
        },
        &execution,
    )
    .unwrap()
}

#[test]
fn dagre_neo_centerline_length_is_precomputed_only_for_svg_paint() {
    let source = format!(
        "{}flowchart LR\nA arrow@--> B\narrow@{{ animate: true }}\nB --> C",
        DAGRE_LINEAR_CONFIG.replace("look: classic", "look: neo"),
    );
    with_source(&source, false, |request, execution| {
        with_flowchart_paint_context(
            request.into(),
            &svg_services(execution),
            |ctx, edges, hierarchy| {
                assert_eq!(edges.len(), 2);
                let query = prepare_edge_routes(ctx, edges, hierarchy, false)?;
                let paint = prepare_edge_routes(ctx, edges, hierarchy, true)?;
                let mut measured_count = 0;
                for edge in edges {
                    let query = &query[&edge.key].geom;
                    let paint = &paint[&edge.key].geom;
                    assert_eq!(query.d, paint.d);
                    assert!(query.neo_length.is_none());
                    if ctx.edge_style_plan.animation_for(edge.key)?.is_active() {
                        assert!(paint.neo_length.is_none());
                    } else {
                        assert!(paint.neo_length.is_some());
                        assert_eq!(paint.neo_length, svg_path_length_from_d(&paint.d));
                        measured_count += 1;
                    }
                }
                assert_eq!(measured_count, 1);
                Ok(())
            },
        )
    });
}

#[test]
fn dagre_linear_typed_routes_match_parity_shafts_bounds_and_work() {
    let bounds_bits = |bounds: SvgPathBounds| {
        [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y].map(f64::to_bits)
    };
    for (body, bends) in [
        ("flowchart LR\nA[Plan] --> B[Ship] --- C[Done]", false),
        (
            "flowchart TB\nA[Left] --> Z[Join]\nB[Middle] --> Z\nC[Right] --> Z",
            true,
        ),
    ] {
        with_source(
            &format!("{DAGRE_LINEAR_CONFIG}{body}"),
            true,
            |request, execution| {
                with_flowchart_paint_context(
                    request.into(),
                    &svg_services(execution),
                    |ctx, edges, hierarchy| {
                        let expected_work = edges.len()
                            + edges
                                .iter()
                                .map(|edge| {
                                    ctx.layout_edges_by_key[&edge.key].points().len().max(2)
                                })
                                .sum::<usize>();
                        let before_parity = ctx.work_meter.used();
                        let parity = prepare_edge_routes(ctx, edges, hierarchy, false)?;
                        assert_eq!(ctx.work_meter.used() - before_parity, expected_work);

                        let before_typed = ctx.work_meter.used();
                        ctx.work_meter.charge(edges.len())?;
                        let mut scratch = FlowchartEdgeDataPointsScratch::default();
                        let mut saw_bend = false;
                        for edge in edges {
                            let route = prepare_dagre_linear_edge_route(
                                ctx,
                                edge.as_ref(),
                                hierarchy,
                                &mut scratch,
                            )?
                            .expect("Dagre linear shaft");
                            assert_eq!(
                                curve::curve_linear_path_d(&route.points),
                                parity[&edge.key].geom.d
                            );
                            assert_eq!(
                                Some(bounds_bits(route.bounds)),
                                parity[&edge.key].geom.pb.map(bounds_bits)
                            );
                            saw_bend |= route.points.windows(3).any(|points| {
                                let first_x = points[1].x - points[0].x;
                                let first_y = points[1].y - points[0].y;
                                let second_x = points[2].x - points[1].x;
                                let second_y = points[2].y - points[1].y;
                                (first_x * second_y - first_y * second_x).abs() > 0.001
                            });
                        }
                        ctx.work_meter
                            .checkpoint(merman_model::OperationPhase::Layout)?;
                        assert_eq!(saw_bend, bends);
                        assert_eq!(ctx.work_meter.used() - before_typed, expected_work);
                        assert_eq!(ctx.work_meter.projected_svg_bytes(), 0);
                        assert!(ctx.document_ids.get().is_none());
                        Ok(())
                    },
                )
            },
        );
    }
}

#[test]
fn dagre_linear_typed_route_clips_rectangles_fixes_corners_and_offsets_marker() {
    let mut layout: FlowchartLayout = serde_json::from_value(serde_json::json!({
        "nodes": [
            {"id":"A", "x":0, "y":0, "width":20, "height":20, "is_cluster":false},
            {"id":"B", "x":60, "y":40, "width":20, "height":20, "is_cluster":false}
        ],
        "edges": [
            {"id":"audit", "from":"A", "to":"B", "points":[
                {"x":0,"y":0}, {"x":30,"y":0}, {"x":30,"y":40}, {"x":60,"y":40}
            ]}
        ],
        "clusters": []
    }))
    .unwrap();
    layout.edge_owners = crate::flowchart::FlowchartEdgeOwners::from_semantic_indices([0]);
    with_layout(
        &format!("{DAGRE_LINEAR_CONFIG}flowchart LR\nA[Left] audit@--> B[Right]"),
        Some(layout),
        true,
        |request, execution| {
            with_flowchart_paint_context(
                request.into(),
                &svg_services(execution),
                |ctx, edges, hierarchy| {
                    let parity = prepare_edge_routes(ctx, edges, hierarchy, false)?;
                    let edge = edges[0].as_ref();
                    let geom = &parity[&edge.key].geom;
                    let mut scratch = FlowchartEdgeDataPointsScratch::default();
                    let route =
                        prepare_dagre_linear_edge_route(ctx, edge, hierarchy, &mut scratch)?
                            .expect("Dagre linear shaft");
                    assert_eq!(geom.data_points.len(), 4);
                    assert_eq!((geom.data_points[0].x, geom.data_points[3].x), (10.0, 50.0));
                    assert_eq!(route.points.len(), 8);
                    let expected = "M10,0L25,0L27.828,2.172L30,5L30,35L32.172,37.828L35,40L46,40";
                    assert_eq!(geom.d, expected);
                    assert_eq!(curve::curve_linear_path_d(&route.points), expected);
                    assert_eq!(
                        (
                            route.bounds.min_x,
                            route.bounds.min_y,
                            route.bounds.max_x,
                            route.bounds.max_y
                        ),
                        (10.0, 0.0, 46.0, 40.0)
                    );
                    Ok(())
                },
            )
        },
    );
}

#[test]
fn dagre_linear_typed_route_keeps_layout_cancellation_and_refuses_labeled_edges() {
    with_source(
        &format!("{DAGRE_LINEAR_CONFIG}flowchart LR\nA -->|label| B"),
        true,
        |request, execution| {
            with_flowchart_paint_context(
                request.into(),
                &svg_services(execution),
                |ctx, edges, hierarchy| {
                    let edge = edges[0].as_ref();
                    let mut scratch = FlowchartEdgeDataPointsScratch::default();
                    assert!(
                        prepare_edge_routes(ctx, edges, hierarchy, false)?.contains_key(&edge.key)
                    );
                    assert!(
                        prepare_dagre_linear_edge_route(ctx, edge, hierarchy, &mut scratch)?
                            .is_none()
                    );
                    let before_cancel = ctx.work_meter.used();
                    ctx.work_meter.control().cancel();
                    assert!(matches!(
                        prepare_dagre_linear_edge_route(ctx, edge, hierarchy, &mut scratch),
                        Err(crate::Error::Cancelled(error))
                            if error.phase == merman_model::OperationPhase::Layout
                    ));
                    assert_eq!(ctx.work_meter.used(), before_cancel);
                    Ok(())
                },
            )
        },
    );
}

#[test]
fn geometry_frontier_preserves_the_actual_rough_paths_after_nested_prefixes() {
    for seed in [-1_i64, 4_294_967_296, 7] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n  look: handDrawn\n  handDrawnSeed: {seed}\n---\nflowchart LR\nsubgraph Group\n direction TB\n A((Circle)) -->|inner| B[End]\nend\nX -->|outer| Y\nY --> Group"
        );
        let svg = with_source(&source, false, |request, execution| {
            render_flowchart_svg_model(request, execution).map(|svg| svg.as_str().to_owned())
        });
        let document = crate::xml_dom::Document::parse(&svg).unwrap();
        let actual: FxHashMap<_, _> = document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-points").is_some())
            .map(|node| {
                (
                    node.attribute("data-id").unwrap().to_owned(),
                    node.attribute("d").unwrap().to_owned(),
                )
            })
            .collect();
        with_source(&source, true, |request, execution| {
            with_flowchart_paint_context(
                request.into(),
                &svg_services(execution),
                |ctx, edges, hierarchy| {
                    let mut routes = prepare_edge_routes(ctx, edges, hierarchy, false)?;
                    let labels = FlowchartEdgeLabels::prepare(ctx, hierarchy, &routes)?;
                    assert!(prepare_query_rough_paths(
                        ctx,
                        hierarchy,
                        &mut routes,
                        &labels
                    )?);
                    let mut compared = 0;
                    for edge in edges {
                        let Some(rough) = routes
                            .get(&edge.key)
                            .and_then(|entry| entry.geom.emitted_d_for_label.as_ref())
                        else {
                            continue;
                        };
                        assert_eq!(
                            rough, &actual[&edge.edge.id],
                            "seed {seed}, edge {}",
                            edge.edge.id
                        );
                        compared += 1;
                    }
                    assert!(compared >= 2);
                    assert_eq!(ctx.work_meter.projected_svg_bytes(), 0);
                    assert!(ctx.document_ids.get().is_none());
                    Ok(())
                },
            )
        });
    }
}

#[test]
fn geometry_without_positioned_labels_does_not_prepare_rough_paths() {
    let source = "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n  look: handDrawn\n  handDrawnSeed: -1\n---\nflowchart LR\nA --> B";
    with_source(source, true, |request, execution| {
        with_flowchart_paint_context(
            request.into(),
            &svg_services(execution),
            |ctx, edges, hierarchy| {
                let mut routes = prepare_edge_routes(ctx, edges, hierarchy, false)?;
                let labels = FlowchartEdgeLabels::prepare(ctx, hierarchy, &routes)?;
                let before = ctx.work_meter.used();
                assert!(!prepare_query_rough_paths(
                    ctx,
                    hierarchy,
                    &mut routes,
                    &labels
                )?);
                assert_eq!(before, ctx.work_meter.used());
                assert!(
                    routes
                        .values()
                        .all(|entry| entry.geom.emitted_d_for_label.is_none())
                );
                Ok(())
            },
        )
    });
}

#[test]
fn canonical_route_failure_is_returned_by_the_svg_renderer() {
    use crate::graph_layout::{
        AxisRoutes, RouteContract, RouteTopology,
        axis_clearance::AxisClearance,
        contour::ShapePortProfile,
        ir::{Axis, Endpoint, KnotKind, Side},
        route_paint::ObstacleOwner,
    };
    let mut layout: FlowchartLayout = serde_json::from_value(serde_json::json!({
        "nodes": [
            {"id":"A", "x":-20, "y":0, "width":40, "height":20, "is_cluster":false},
            {"id":"B", "x":160, "y":60, "width":40, "height":20, "is_cluster":false}
        ],
        "edges": [
            {"id":"audit", "from":"A", "to":"B", "points":[
                {"x":0,"y":0}, {"x":20,"y":0}, {"x":120,"y":60}, {"x":140,"y":60}
            ]}
        ],
        "clusters": []
    }))
    .unwrap();
    layout.edge_owners = crate::flowchart::FlowchartEdgeOwners::from_semantic_indices([0]);
    layout.route_contract = RouteContract::CanonicalAxis(
        AxisRoutes {
            clearance: AxisClearance::new(vec![(
                kurbo::Rect::new(18., -2., 22., 2.),
                ObstacleOwner::GroupTitle(0),
            )]),
            group_parents: Vec::new(),
            struck_group_titles: Default::default(),
            border_stitches: Vec::new(),
            nodes: vec![ShapePortProfile::rectangle(40., 20.); 2],
            routes: vec![Some(RouteTopology {
                endpoints: (Endpoint::Node(0), Endpoint::Node(1)),
                ports: (Side::Right, Side::Left),
                knots: vec![(Axis::Horizontal, KnotKind::Free); 4],
                corridor_clearance: 0.0,
            })],
            #[cfg(feature = "layout-shoal")]
            svg_edges: None,
        }
        .into(),
    );
    with_layout(
        "flowchart LR\nA audit@--> B",
        Some(layout),
        false,
        |request, execution| {
            let error = render_flowchart_svg_model(request, execution)
                .err()
                .unwrap();
            assert!(matches!(
                error,
                Error::InvalidModel { message } if message.contains("`audit`") &&
                    message.contains("no clear path between fixed route points")
            ));
            Ok(())
        },
    );
}

#[test]
fn canonical_route_contract_owns_ports_and_labels_even_with_a_different_config() {
    // The solver rows are deliberately reversed. Both semantic edges have the same id and
    // endpoints, but only the first is bidirectional. Neither label is at a path midpoint.
    let mut layout: FlowchartLayout = serde_json::from_value(serde_json::json!({
        "nodes": [
            {"id":"A", "x":10, "y":20, "width":48, "height":36, "is_cluster":false},
            {"id":"B", "x":130, "y":110, "width":40, "height":30, "is_cluster":false}
        ],
        "edges": [
            {"id":"L_A_B_2", "from":"A", "to":"B", "points":[
                {"x":10,"y":2}, {"x":10,"y":-40},
                {"x":130,"y":-40}, {"x":130,"y":95}
            ], "label":{"x":94.5,"y":36.25,"width":32,"height":16}},
            {"id":"L_A_B_2", "from":"A", "to":"B", "points":[
                {"x":34,"y":20}, {"x":64,"y":20}, {"x":80,"y":110}, {"x":110,"y":110}
            ], "label":{"x":41.25,"y":-7.5,"width":48,"height":16}}
        ],
        "clusters": []
    }))
    .unwrap();
    layout.edge_owners = crate::flowchart::FlowchartEdgeOwners::from_semantic_indices([1, 0]);

    use crate::graph_layout::contour::ShapePortProfile;
    use crate::graph_layout::{
        AxisRoutes, RouteContract, RouteTopology,
        ir::{Axis, Endpoint, KnotKind, Side},
    };
    layout.route_contract = RouteContract::CanonicalAxis(
        AxisRoutes {
            clearance: Default::default(),
            group_parents: Vec::new(),
            struck_group_titles: Default::default(),
            border_stitches: Vec::new(),
            nodes: vec![
                ShapePortProfile::diamond(48.0, 36.0),
                ShapePortProfile::rectangle(40.0, 30.0),
            ],
            routes: vec![
                Some(RouteTopology {
                    endpoints: (Endpoint::Node(0), Endpoint::Node(1)),
                    ports: (Side::Right, Side::Left),
                    knots: vec![(Axis::Horizontal, KnotKind::Free); 4],
                    corridor_clearance: 0.0,
                }),
                Some(RouteTopology {
                    endpoints: (Endpoint::Node(0), Endpoint::Node(1)),
                    ports: (Side::Top, Side::Top),
                    knots: [
                        Axis::Vertical,
                        Axis::Vertical,
                        Axis::Horizontal,
                        Axis::Vertical,
                    ]
                    .map(|axis| (axis, KnotKind::Free))
                    .to_vec(),
                    corridor_clearance: 0.0,
                }),
            ],
            #[cfg(feature = "layout-shoal")]
            svg_edges: None,
        }
        .into(),
    );

    for (html, look) in [(false, "classic"), (true, "handDrawn")] {
        let source = format!(
            "---
config:
  layout: dagre
  htmlLabels: {html}
  look: {look}
  handDrawnSeed: 7
  flowchart:
curve: linear
---
flowchart LR
A{{A}} L_A_B_2@<-->|direct| B[B]
A -->|bent| B"
        );
        with_layout(
            &source,
            Some(layout.clone()),
            false,
            |request, execution| {
                let geometry = flow_svg::edge_geometry(request.into(), &svg_services(execution))?;
                let svg = render_flowchart_svg_model(request, execution)?;
                let document = crate::xml_dom::Document::parse(svg.as_str()).unwrap();
                let paths: Vec<_> = document
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("path") && node.attribute("data-points").is_some()
                    })
                    .collect();
                let labels: Vec<_> = document
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel")
                    })
                    .collect();
                assert_eq!(geometry.len(), 2);
                assert_eq!(paths.len(), 2);
                assert_eq!(labels.len(), 2);
                assert_ne!(paths[0].attribute("id"), paths[1].attribute("id"));

                let direct = [(34.0, 20.0), (64.0, 20.0), (80.0, 110.0), (110.0, 110.0)];
                let bent = [(10.0, 2.0), (10.0, -40.0), (130.0, -40.0), (130.0, 95.0)];
                for (i, (points, (x, y), text)) in [
                    (direct.as_slice(), (41.25, -7.5), "direct"),
                    (bent.as_slice(), (94.5, 36.25), "bent"),
                ]
                .into_iter()
                .enumerate()
                {
                    let path = paths[i];
                    let query = &geometry[i];
                    assert_eq!(query.id, "L_A_B_2");
                    assert_eq!(path.attribute("data-id"), Some(query.id.as_str()));
                    let svg_points = decode_path_points(path);
                    for projected in [svg_points.as_slice(), query.points.as_deref().unwrap()] {
                        assert_eq!(
                            projected.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
                            points
                        );
                    }
                    let label = query.label_position.as_ref().unwrap();
                    assert_eq!((label.x, label.y), (x, y));
                    assert_eq!(
                        labels[i].attribute("transform").unwrap(),
                        format!("translate({x},{y})")
                    );
                    assert!(
                        labels[i]
                            .descendants()
                            .any(|node| node.is_text() && node.text() == Some(text))
                    );
                }
                assert!(
                    paths[0]
                        .attribute("marker-start")
                        .unwrap()
                        .contains("pointStart")
                );
                assert!(
                    paths[0]
                        .attribute("marker-end")
                        .unwrap()
                        .contains("pointEnd")
                );
                assert!(paths[1].attribute("marker-start").is_none());
                if look == "classic" {
                    // Four units along each terminal tangent, after the diamond and rect contacts.
                    let path = kurbo::BezPath::from_svg(paths[0].attribute("d").unwrap()).unwrap();
                    use kurbo::ParamCurve;
                    assert_eq!(
                        path.segments().next().unwrap().start(),
                        kurbo::Point::new(38.0, 20.0)
                    );
                    assert_eq!(
                        path.segments().last().unwrap().end(),
                        kurbo::Point::new(106.0, 110.0)
                    );
                }
                Ok(())
            },
        );
    }
}

fn decode_path_points(path: crate::xml_dom::Node<'_, '_>) -> Vec<crate::model::LayoutPoint> {
    use base64::Engine as _;
    serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap(),
    )
    .unwrap()
}

#[cfg(feature = "layout-elk")]
#[test]
fn geometry_preserves_the_elk_missing_section_path_and_label() {
    with_source(
        "flowchart LR\nA((Circle)) -->|label| B{Diamond}",
        false,
        |request, execution| {
            let mut layout = request.layout.clone();
            layout.route_contract = crate::graph_layout::RouteContract::ElkParity;
            layout.edges[0].points.clear();
            let request = FlowchartSvgModelRequest {
                layout: &layout,
                ..request
            };
            let geometry = flow_svg::edge_geometry(request.into(), &svg_services(execution))?;
            let svg = render_flowchart_svg_model(request, execution)?;
            let document = crate::xml_dom::Document::parse(svg.as_str()).unwrap();
            let path = document
                .descendants()
                .find(|node| node.attribute("data-points").is_some())
                .unwrap();
            let points = decode_path_points(path);
            let query = geometry[0]
                .points
                .as_ref()
                .expect("missing section receives endpoint geometry");
            assert_eq!(query.len(), points.len());
            for (query, actual) in query.iter().zip(points.iter()) {
                assert_eq!((query.x, query.y), (actual.x, actual.y));
            }
            let label = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("edgeLabel")
                        && node.attribute("transform").is_some()
                })
                .unwrap();
            let (x, y) = label
                .attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split_once(',')
                .unwrap();
            let position = geometry[0].label_position.as_ref().unwrap();
            assert!((position.x - x.trim().parse::<f64>().unwrap()).abs() < 1e-5);
            assert!((position.y - y.trim().parse::<f64>().unwrap()).abs() < 1e-5);
            Ok(())
        },
    );
}
