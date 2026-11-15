use super::*;
use crate::AsciiRenderOptions;
use crate::graph::label::GraphLabel;
use crate::graph::layout::{GridCoord, layout_graph};
use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphNodeShape, GraphNodeStyle};
use crate::graph::routing::terminal;
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_model::resources::ResourceProfile;
use std::cell::Cell;

#[test]
fn planned_route_cells_debit_before_materializing_exact_and_max_minus_one() {
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let exact_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 2)
        .expect("exact route-cell work limit should be valid");
    let mut exact_resources = crate::resource::ResourceContext::new(exact_policy);
    let mut exact_cells = PlannedRouteCells::new();
    exact_cells
        .try_push(&mut exact_resources, || route_cell(0, 0, '-'))
        .expect("first exact-budget route cell should materialize");
    exact_cells
        .try_push(&mut exact_resources, || route_cell(1, 0, '-'))
        .expect("second exact-budget route cell should materialize");
    assert_eq!(exact_cells.into_vec().len(), 2);

    let below_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .expect("max-minus-one route-cell work limit should be valid");
    let mut below_resources = crate::resource::ResourceContext::new(below_policy);
    let mut below_cells = PlannedRouteCells::new();
    below_cells
        .try_push(&mut below_resources, || route_cell(0, 0, '-'))
        .expect("first max-minus-one route cell should materialize");
    let second_materialized = Cell::new(false);
    let error = below_cells
        .try_push(&mut below_resources, || {
            second_materialized.set(true);
            route_cell(1, 0, '-')
        })
        .expect_err("second max-minus-one route cell should fail before materialization");
    assert!(!second_materialized.get());
    let crate::AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected a layout-work resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
    assert_eq!(details.actual, 2);
    assert_eq!(details.max, 1);
}

#[test]
fn marker_candidates_carry_the_contiguous_route_local_terminal_tail() {
    let plan = RoutePlan::new(
        (0..4)
            .map(|x| cell(x, 0, '-', PlannedRouteCellKind::RouteCell))
            .collect(),
        Vec::new(),
        MarkerAnchors::new(
            MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
            MarkerAnchor::new(PlannedCellId::new(3), StepDirection::Right),
        ),
    )
    .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
    .unwrap();
    let mut resources = unbounded_route_resources();

    let candidates = plan
        .marker_candidates(MarkerEndpoint::End, "flowchart", &mut resources)
        .unwrap()
        .expect("straight terminal should have marker candidates");

    assert_eq!(candidates.len(), 3);
    assert!(candidates[0].terminal_tail().is_empty());
    assert_eq!(candidates[1].terminal_tail(), &[PlannedCellId::new(3)]);
    assert_eq!(
        candidates[2].terminal_tail(),
        &[PlannedCellId::new(3), PlannedCellId::new(2)]
    );
    assert!(candidates[1].follows_terminal_predecessor(candidates[0]));
    assert!(candidates[2].follows_terminal_predecessor(candidates[1]));
}

#[test]
fn marker_candidates_reject_a_primary_bend_without_confusing_open_markers() {
    let plan = RoutePlan::new(
        vec![
            cell(0, 0, '-', PlannedRouteCellKind::RouteCell),
            cell(1, 0, '+', PlannedRouteCellKind::RouteCell),
            cell(1, 1, '|', PlannedRouteCellKind::RouteCell),
        ],
        Vec::new(),
        MarkerAnchors::new(
            MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
            MarkerAnchor::new(PlannedCellId::new(1), StepDirection::Right),
        ),
    );
    let mut resources = unbounded_route_resources();
    assert!(
        plan.marker_candidates(MarkerEndpoint::End, "flowchart", &mut resources)
            .unwrap()
            .expect("open endpoint needs no marker berth")
            .is_empty()
    );

    let plan = plan
        .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
        .unwrap();
    assert!(
        plan.marker_candidates(MarkerEndpoint::End, "flowchart", &mut resources)
            .unwrap()
            .is_none(),
        "a bend must reject the route candidate instead of silently losing its marker"
    );
}

#[test]
fn self_loop_marker_candidates_stop_before_the_terminal_corner() {
    let options = AsciiRenderOptions::ascii().with_node_padding(1);
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    graph.add_edge("a", "a");
    let layout = layout_graph(&graph, &options);
    let mut resources = unbounded_route_resources();
    let policy = resources.policy();
    let plan = terminal::plan_self_loop_candidates(
        EdgeRouteRequest {
            graph: &graph,
            graph_layout: &layout,
            edges: &graph.edges,
            from: &layout.nodes[0],
            to: &layout.nodes[0],
            edge_index: 0,
            edge: &graph.edges[0],
            charset: &charset,
        },
        None,
        &mut resources,
        AsciiExecution::for_test(&policy),
    )
    .unwrap()
    .into_iter()
    .next()
    .expect("a self-loop should have a terminal channel")
    .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
    .unwrap();

    let candidates = plan
        .marker_candidates(MarkerEndpoint::End, "flowchart", &mut resources)
        .unwrap()
        .expect("straight terminal should have marker candidates");

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].is_primary());
    assert!(candidates[0].terminal_tail().is_empty());
}

#[test]
fn parallel_routes_keep_distinct_markers_and_continuous_scene_terminals() {
    let options = AsciiRenderOptions::ascii();
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    for _ in 0..3 {
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                label: Some("parallel".into()),
                ..Default::default()
            },
        );
    }
    let edges = &graph.edges;
    let charset = GraphCharset::for_options(&options);
    let layout = layout_graph(&graph, &options);
    let scene = super::super::prepare_route_scene(&graph, &layout, edges, &charset).unwrap();
    let markers = scene
        .routes
        .iter()
        .map(|route| {
            route
                .plan
                .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                .unwrap()
                .unwrap()
                .coord
        })
        .collect::<HashSet<_>>();
    assert_eq!(markers.len(), edges.len());
    for route in &scene.routes {
        let marker = route
            .plan
            .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
            .unwrap()
            .unwrap();
        let attachment = route.plan.attachments.unwrap().end;
        let axis = match attachment.outward {
            StepDirection::Left | StepDirection::Right => {
                super::super::cell::DIR_LEFT | super::super::cell::DIR_RIGHT
            }
            StepDirection::Up | StepDirection::Down => {
                super::super::cell::DIR_UP | super::super::cell::DIR_DOWN
            }
        };
        let distance = attachment.contact.x.abs_diff(marker.coord.x)
            + attachment.contact.y.abs_diff(marker.coord.y);
        let mut coord = attachment.contact;
        for _ in 0..distance {
            coord = marker_relocation_step(
                coord,
                route.plan.marker_point_direction(MarkerEndpoint::End),
                &unbounded_route_resources(),
            )
            .unwrap()
            .unwrap();
            assert!(
                scene
                    .routes
                    .iter()
                    .any(|owner| owner.plan.active_cells().any(|(_, cell)| {
                        cell.coord == coord && cell.directions & axis == axis
                    })),
                "allocated markers must retain a continuous scene terminal into the node"
            );
        }
    }
}

#[test]
fn stroke_projection_preserves_edge_line_connector_style() {
    let ascii_options = AsciiRenderOptions::ascii();
    let ascii_charset = GraphCharset::for_options(&ascii_options);
    let ascii_plan = RoutePlan::new_without_markers_for_test(
        vec![cell(0, 0, '|', PlannedRouteCellKind::EdgeLine)],
        Vec::new(),
    )
    .try_with_stroke(GraphEdgeStroke::Thick, &ascii_charset, "flowchart")
    .expect("ASCII edge-line stroke should be representable");
    let ascii_cell = &ascii_plan.cells[0];
    assert_eq!(ascii_cell.ch, '|');
    assert_eq!(ascii_cell.stroke, GraphEdgeStroke::Thick);
    assert!(!ascii_cell.unicode);
    assert_ne!(ascii_cell.directions, 0);

    let unicode_options = AsciiRenderOptions::unicode();
    let unicode_charset = GraphCharset::for_options(&unicode_options);
    let unicode_plan = RoutePlan::new_without_markers_for_test(
        vec![cell(0, 0, '├', PlannedRouteCellKind::EdgeLine)],
        Vec::new(),
    )
    .try_with_stroke(GraphEdgeStroke::Thick, &unicode_charset, "flowchart")
    .expect("Unicode edge-line stroke should be representable");
    let unicode_cell = &unicode_plan.cells[0];
    assert_eq!(unicode_cell.ch, '┝');
    assert_eq!(unicode_cell.stroke, GraphEdgeStroke::Thick);
    assert!(unicode_cell.unicode);
    assert_ne!(unicode_cell.directions, 0);
}

#[test]
fn attachments_reject_missing_bent_and_duplicate_normal_terminal_cells() {
    let from = node("from", 0, 0, 5, 5);
    let to = node("to", 12, 0, 5, 5);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_edge("from", "to");
    let edge = &graph.edges[0];
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());
    for defect in ["missing", "bent", "duplicate"] {
        let mut resources = unbounded_route_resources();
        let mut plan = terminal_bottom_lane(
            &from,
            &to,
            edge,
            from.bottom().max(to.bottom()) + 4,
            &charset,
            &mut resources,
        );
        let anchor = plan.anchors.end;
        let primary = plan.cells[anchor.cell.index()].coord;
        let farther = plan
            .cells
            .iter()
            .position(|cell| cell.coord.x == primary.x && cell.coord.y == primary.y + 1)
            .unwrap();
        plan.anchors.end.cell = PlannedCellId(farther);
        match defect {
            "missing" => plan.cells[anchor.cell.index()].coord.x += 1,
            "bent" => plan.cells[anchor.cell.index()].ch = charset.bottom_right,
            "duplicate" => plan.cells.push(plan.cells[anchor.cell.index()]),
            _ => unreachable!(),
        }
        assert!(
            plan.resolve_node_attachments(&from, &to, &charset, &resources)
                .unwrap()
                .is_none(),
            "{defect} terminal must be rejected before scene ownership is committed"
        );
    }
}

#[test]
fn lower_bottom_lane_keeps_target_berth_adjacent_and_emits_the_complete_terminal() {
    let from = node("from", 0, 0, 5, 5);
    let to = node("to", 12, 0, 5, 5);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_edge("from", "to");
    let edge = &graph.edges[0];
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());
    let mut resources = unbounded_route_resources();
    let plan = terminal_bottom_lane(
        &from,
        &to,
        edge,
        from.bottom().max(to.bottom()) + 8,
        &charset,
        &mut resources,
    )
    .resolve_node_attachments(&from, &to, &charset, &resources)
    .unwrap()
    .unwrap();
    let berth = plan.cells[plan.anchors.end.cell.index()].coord;
    assert_eq!(
        berth,
        CanvasCoord {
            x: to.center_x(),
            y: to.bottom() + 1
        }
    );
    let lane = from.bottom().max(to.bottom()) + 8;
    for y in (to.bottom() + 1)..lane {
        assert!(plan.cells.iter().any(|cell| cell.coord
            == CanvasCoord {
                x: to.center_x(),
                y
            }
            && super::super::cell::route_char_directions(cell.ch)
                == super::super::cell::DIR_UP | super::super::cell::DIR_DOWN));
    }
}

#[test]
fn terminal_coordinate_index_admits_exact_work_before_allocation_and_preserves_failed_ledgers() {
    const INDEX_WORK: usize = 3;
    let plan = RoutePlan::new_without_markers_for_test(
        vec![
            route_cell(0, 0, '-'),
            route_cell(1, 0, '-'),
            route_cell(2, 0, '-'),
        ],
        Vec::new(),
    );
    let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let exact = ResourceContext::new(
        policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, INDEX_WORK + 1)
            .unwrap(),
    );
    exact.charge_layout_work(1).unwrap();
    assert_eq!(
        plan.terminal_coordinate_index(&exact).unwrap().len(),
        INDEX_WORK
    );
    assert_eq!(exact.layout_work_used(), INDEX_WORK + 1);

    let below = ResourceContext::new(
        policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, INDEX_WORK)
            .unwrap(),
    );
    below.charge_layout_work(1).unwrap();
    let error = below
        .transaction(|resources| plan.terminal_coordinate_index(resources))
        .unwrap_err();
    assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
            && details.actual == INDEX_WORK + 1 && details.max == INDEX_WORK
            && details.phase() == crate::AsciiResourceLimitPhase::LayoutWork
            && details.cause == crate::AsciiResourceLimitCause::Ceiling));
    assert_eq!(below.layout_work_used(), 1);

    let control = merman_model::OperationControl::new();
    let cancelled = ResourceContext::new(policy)
        .controlled(control.clone(), merman_model::OperationPhase::Layout);
    cancelled.charge_layout_work(1).unwrap();
    control.cancel();
    let error = plan.terminal_coordinate_index(&cancelled).unwrap_err();
    assert!(matches!(error, AsciiError::Cancelled(details)
        if details.phase == merman_model::OperationPhase::Layout && details.reason == merman_model::CancelReason::Requested));
    assert_eq!(cancelled.layout_work_used(), 1);
}

fn cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind,
        segment: PlannedRouteSegment::Direct,
        stroke: GraphEdgeStroke::Normal,
        directions: 0,
        unicode: false,
        paint: PlannedRoutePaint::role(match kind {
            PlannedRouteCellKind::EdgeArrow => AsciiColorRole::EdgeArrow,
            PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::RouteCell => {
                AsciiColorRole::EdgeLine
            }
        }),
    }
}

fn terminal_bottom_lane(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    lane_y: usize,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> RoutePlan {
    let policy = resources.policy();
    let path = terminal::direct_path(
        [
            CanvasCoord {
                x: from.center_x(),
                y: from.bottom() + 1,
            },
            CanvasCoord {
                x: from.center_x(),
                y: lane_y,
            },
            CanvasCoord {
                x: to.center_x(),
                y: lane_y,
            },
            CanvasCoord {
                x: to.center_x(),
                y: to.bottom() + 1,
            },
        ],
        StepDirection::Down,
        StepDirection::Up,
        resources,
        AsciiExecution::for_test(&policy),
    )
    .unwrap()
    .unwrap();
    let attachments = NodeAttachments {
        start: NodeAttachment::resolve(from, path[0], StepDirection::Up, charset).unwrap(),
        end: NodeAttachment::resolve(to, *path.last().unwrap(), StepDirection::Up, charset)
            .unwrap(),
    };
    orthogonal::plan_terminal_path(&path, attachments, edge, None, charset, resources)
        .unwrap()
        .unwrap()
}

fn node(id: &str, x: usize, y: usize, width: usize, height: usize) -> NodeLayout {
    NodeLayout {
        id: id.to_string(),
        label: GraphLabel::new(id),
        shape: GraphNodeShape::Rect,
        style: GraphNodeStyle::default(),
        grid: GridCoord { x: 0, y: 0 },
        x,
        y,
        width,
        height,
    }
}
