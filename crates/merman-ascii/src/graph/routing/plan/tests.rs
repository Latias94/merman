use super::grid::{
    GridRouteOptions, plan_left_right_grid_path_route, plan_left_right_grid_path_route_with_options,
};
use super::left_right::{
    plan_left_right_down_route, plan_left_right_down_then_right_route,
    plan_left_right_reverse_over_self_loop_route, plan_left_right_right_then_up_route,
    plan_left_right_self_loop_route, plan_left_right_self_loop_route_with_resources,
};
use super::same_rank::{plan_same_rank_bottom_lane_route, plan_same_rank_direct_route};
use super::top_down::{
    plan_top_down_back_route, plan_top_down_bent_route, plan_top_down_direct_route,
    plan_top_down_side_entry_route,
};
use super::*;
use crate::AsciiRenderOptions;
use crate::color::AsciiColorRole;
use crate::graph::charset::GraphCharset;
use crate::graph::label::GraphLabel;
use crate::graph::layout::{GraphLayout, GridCoord, NodeLayout, layout_graph};
use crate::graph::model::{
    AsciiGraph, AsciiGraphEdge, GraphDirection, GraphEdgeArrow, GraphEdgeMarker, GraphEdgeStroke,
    GraphEdgeStyle, GraphNodeShape, GraphNodeStyle,
};
use crate::graph::routing::label::RoutedLabelPlacement;
use crate::graph::routing::label::RoutedLabelText;
use crate::graph::routing::plan::PlannedRoutePaint;
use crate::graph::routing::plan::PlannedRouteSegment;
use crate::graph::routing::plan::select::{
    EdgeBoundaryContext, UnsupportedEdgeRouteReason, edge_boundary_context,
};
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_core::resources::ResourceProfile;
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
        .unwrap();

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
fn self_loop_marker_candidates_stop_before_the_terminal_corner() {
    let from = node("a", 0, 0, 3, 3);
    let layouts = vec![from.clone()];
    let edge = edge_between("a", "a", None, GraphEdgeArrow::Point);
    let edges = vec![edge.clone()];
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let plan = plan_left_right_self_loop_route(&layouts, &edges, &from, &edge, &charset).unwrap();
    let mut resources = unbounded_route_resources();

    let candidates = plan
        .marker_candidates(MarkerEndpoint::End, "flowchart", &mut resources)
        .unwrap();

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].is_primary());
    assert!(candidates[0].terminal_tail().is_empty());
}

#[test]
fn edge_route_selects_left_right_parallel_bottom_lane() {
    let options = AsciiRenderOptions::ascii();
    let layout = left_right_layout(&[("a", "b"), ("a", "b"), ("a", "b")], &options);
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "b");
    let edges = vec![
        edge(Some("parallel"), GraphEdgeArrow::Point),
        edge(Some("parallel"), GraphEdgeArrow::Point),
        edge(Some("parallel"), GraphEdgeArrow::Point),
    ];
    let charset = GraphCharset::for_options(&options);

    let second = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::LeftRight),
        graph_layout: &layout,
        edges: &edges,
        from,
        to,
        edge_index: 1,
        edge: &edges[1],
        charset: &charset,
    })
    .unwrap();
    let third = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::LeftRight),
        graph_layout: &layout,
        edges: &edges,
        from,
        to,
        edge_index: 2,
        edge: &edges[2],
        charset: &charset,
    })
    .unwrap();
    let expected = plan_same_rank_bottom_lane_route(from, to, &edges[1], &charset).unwrap();

    assert_eq!(second, expected);
    let second_lane_y = second
        .cells
        .iter()
        .map(|cell| cell.coord.y)
        .max()
        .expect("second edge should have route cells");
    let third_lane_y = third
        .cells
        .iter()
        .map(|cell| cell.coord.y)
        .max()
        .expect("third edge should have route cells");
    assert_eq!(third_lane_y, second_lane_y + 2);
    let second_marker = second
        .cells
        .iter()
        .find(|cell| cell.kind == PlannedRouteCellKind::EdgeArrow)
        .expect("second edge should retain its marker");
    let third_marker = third
        .cells
        .iter()
        .find(|cell| cell.kind == PlannedRouteCellKind::EdgeArrow)
        .expect("third edge should retain its marker");
    assert_ne!(second_marker.coord, third_marker.coord);
    assert_ne!(
        second.labels[0].placement.y(),
        third.labels[0].placement.y()
    );
}

#[test]
fn edge_route_assigns_parallel_self_loops_distinct_lanes_and_marker_berths() {
    let options = AsciiRenderOptions::ascii();
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    let layout = layout_graph(&graph, &options);
    let from = layout_node(&layout, "a");
    let edges = vec![
        edge_between("a", "a", Some("alpha"), GraphEdgeArrow::Point),
        edge_between("a", "a", Some("beta"), GraphEdgeArrow::Circle),
        edge_between("a", "a", Some("gamma"), GraphEdgeArrow::Cross),
    ];
    let charset = GraphCharset::for_options(&options);

    let plans = edges
        .iter()
        .enumerate()
        .map(|(edge_index, edge)| {
            plan_edge_route(EdgeRouteRequest {
                graph: &graph,
                graph_layout: &layout,
                edges: &edges,
                from,
                to: from,
                edge_index,
                edge,
                charset: &charset,
            })
            .unwrap()
        })
        .collect::<Vec<_>>();

    let lane_bounds = plans
        .iter()
        .map(|plan| {
            (
                plan.cells.iter().map(|cell| cell.coord.x).max().unwrap(),
                plan.cells.iter().map(|cell| cell.coord.y).max().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(lane_bounds[1], (lane_bounds[0].0 + 2, lane_bounds[0].1 + 2));
    assert_eq!(lane_bounds[2], (lane_bounds[1].0 + 2, lane_bounds[1].1 + 2));

    let markers = plans
        .iter()
        .map(|plan| {
            plan.cells
                .iter()
                .find(|cell| cell.kind == PlannedRouteCellKind::EdgeArrow)
                .expect("each self-loop should retain one target marker")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        markers.iter().map(|cell| cell.ch).collect::<Vec<_>>(),
        vec!['^', 'o', 'x']
    );
    assert_eq!(markers[1].coord.y, markers[0].coord.y);
    assert_eq!(markers[2].coord.y, markers[1].coord.y);
    assert_eq!(markers[1].coord.x, markers[0].coord.x + 1);
    assert_eq!(markers[2].coord.x, markers[1].coord.x + 1);

    let label_rows = plans
        .iter()
        .map(|plan| plan.labels[0].placement.y())
        .collect::<Vec<_>>();
    assert_ne!(label_rows[0], label_rows[1]);
    assert_ne!(label_rows[1], label_rows[2]);
    assert_ne!(label_rows[0], label_rows[2]);
}

#[test]
fn parallel_self_loop_lane_index_reports_checked_grid_overflow() {
    let from = node("a", 0, 0, 3, 3);
    let layouts = vec![from.clone()];
    let edge = edge_between("a", "a", None, GraphEdgeArrow::Point);
    let edges = vec![edge.clone()];
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let mut resources = crate::resource::ResourceContext::new(AsciiResourcePolicy::for_profile(
        ResourceProfile::UnboundedForTrustedInput,
    ));

    let error = plan_left_right_self_loop_route_with_resources(
        &layouts,
        &edges,
        &from,
        &edge,
        usize::MAX,
        &charset,
        &mut resources,
    )
    .expect_err("parallel self-loop lane multiplication should remain checked");

    let crate::AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected a grid resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
}

#[test]
fn invisible_edge_does_not_consume_a_visible_parallel_lane() {
    let options = AsciiRenderOptions::ascii();
    let layout = left_right_layout(&[("a", "b")], &options);
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "b");
    let mut invisible = edge(None, GraphEdgeArrow::Point);
    invisible.stroke = GraphEdgeStroke::Invisible;
    let visible = edge(None, GraphEdgeArrow::Point);
    let edges = vec![invisible, visible];
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::LeftRight),
        graph_layout: &layout,
        edges: &edges,
        from,
        to,
        edge_index: 1,
        edge: &edges[1],
        charset: &charset,
    })
    .unwrap();
    let expected =
        plan_same_rank_direct_route(&layout.nodes, from, to, &edges[1], &charset).unwrap();

    assert_eq!(selected, expected);
}

#[test]
fn edge_route_selects_top_down_back_route() {
    let options = AsciiRenderOptions::ascii();
    let layout = left_right_layout(&[("a", "b")], &options);
    let from = node("a", 0, 6, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let edge = edge(Some("back"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::TopDown),
        graph_layout: &layout,
        edges: &[],
        from: &from,
        to: &to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .unwrap();
    let expected = plan_top_down_back_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(selected, expected);
}

#[test]
fn top_down_skip_edge_uses_side_bypass_before_direct_route() {
    let options = AsciiRenderOptions::ascii();
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    for id in ["a", "b", "c"] {
        graph.add_node(id, id.to_ascii_uppercase());
    }
    graph.add_edge("a", "c");
    graph.add_edge("a", "b");
    graph.add_edge("b", "c");
    let layout = layout_graph(&graph, &options);
    let edge_index = graph
        .edges
        .iter()
        .position(|edge| edge.from == "a" && edge.to == "c")
        .expect("skip edge should be present");
    let edge = &graph.edges[edge_index];
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "c");
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: &graph.edges,
        from,
        to,
        edge_index,
        edge,
        charset: &charset,
    })
    .expect("top-down skip edge should route around the occupied rank");
    let direct = plan_top_down_direct_route(from, to, edge, &charset)
        .expect("the direct route should be geometrically constructible for comparison");

    assert_ne!(selected, direct);
    assert!(
        selected
            .cells
            .iter()
            .any(|cell| cell.coord.x != from.center_x()),
        "skip edge should leave the source centerline before reaching the target:\n{selected:?}"
    );
}

#[test]
fn adjacent_top_down_edge_remains_direct() {
    let options = AsciiRenderOptions::ascii();
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    let layout = layout_graph(&graph, &options);
    let edge = &graph.edges[0];
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "b");
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: &graph.edges,
        from,
        to,
        edge_index: 0,
        edge,
        charset: &charset,
    })
    .expect("adjacent top-down edge should route");
    let direct = plan_top_down_direct_route(from, to, edge, &charset)
        .expect("adjacent top-down edge should have a direct route");

    assert_eq!(selected, direct);
}

#[test]
fn edge_route_selects_top_down_same_rank_direct_route() {
    let options = AsciiRenderOptions::unicode();
    let graph_layout = left_right_layout(&[("a", "b")], &options);
    let from = layout_node(&graph_layout, "a");
    let to = layout_node(&graph_layout, "b");
    let edge = edge_between("a", "b", None, GraphEdgeArrow::Point);
    let edges = vec![edge.clone()];
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::TopDown),
        graph_layout: &graph_layout,
        edges: &edges,
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .unwrap();
    let expected =
        plan_same_rank_direct_route(&graph_layout.nodes, from, to, &edge, &charset).unwrap();

    assert_eq!(selected, expected);
}

#[test]
fn edge_route_selects_top_down_same_rank_left_direct_route() {
    let options = AsciiRenderOptions::unicode();
    let graph_layout = left_right_layout(&[("a", "b")], &options);
    let from = layout_node(&graph_layout, "b");
    let to = layout_node(&graph_layout, "a");
    let edge = edge_between("b", "a", None, GraphEdgeArrow::Point);
    let edges = vec![edge.clone()];
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::TopDown),
        graph_layout: &graph_layout,
        edges: &edges,
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .unwrap();
    let expected =
        plan_same_rank_direct_route(&graph_layout.nodes, from, to, &edge, &charset).unwrap();

    assert_eq!(selected, expected);
}

#[test]
fn edge_route_selects_top_down_bottom_lane_when_label_would_cover_arrow() {
    let options = AsciiRenderOptions::unicode();
    let graph_layout = left_right_layout(&[("a", "b")], &options);
    let from = layout_node(&graph_layout, "b");
    let to = layout_node(&graph_layout, "a");
    let edge = edge_between("b", "a", Some("label"), GraphEdgeArrow::Point);
    let edges = vec![edge.clone()];
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::TopDown),
        graph_layout: &graph_layout,
        edges: &edges,
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .unwrap();
    let expected = plan_same_rank_bottom_lane_route(from, to, &edge, &charset).unwrap();

    assert_eq!(selected, expected);
}

#[test]
fn edge_route_selects_top_down_blocked_same_rank_bottom_lane() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = left_right_layout(&[("a", "b"), ("b", "c")], &options);
    let from = layout_node(&graph_layout, "a");
    let to = layout_node(&graph_layout, "c");
    let edge = edge_between("a", "c", None, GraphEdgeArrow::Point);
    let edges = vec![edge.clone()];
    let charset = GraphCharset::for_options(&options);

    let selected = plan_edge_route(EdgeRouteRequest {
        graph: &AsciiGraph::new(GraphDirection::TopDown),
        graph_layout: &graph_layout,
        edges: &edges,
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .unwrap();
    let expected = plan_same_rank_bottom_lane_route(from, to, &edge, &charset).unwrap();

    assert_eq!(selected, expected);
}

#[test]
fn edge_route_reports_unsupported_boundary_direction() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("x", "X");
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_group_with_style(
        "one",
        "TD Group",
        Some(GraphDirection::TopDown),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );
    let edge = edge_between("x", "a", None, GraphEdgeArrow::Point);
    let layout = layout_graph(&graph, &options);
    let from = node("x", 0, 0, 3, 3);
    let to = node("a", 0, 0, 3, 3);

    let planned = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: std::slice::from_ref(&edge),
        from: &from,
        to: &to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    });

    let EdgeRoutePlan::Unsupported(route) = planned else {
        panic!("unsupported boundary route should not be silently treated as routed");
    };
    assert_eq!(
        route.reason(),
        UnsupportedEdgeRouteReason::BoundaryDirection
    );
    assert_eq!(route.feature(), "unsupported graph boundary routes");
}

#[test]
fn route_plan_canvas_extent_accounts_for_same_rank_bottom_lane() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let edge = edge_between("a", "b", None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let plan = plan_same_rank_bottom_lane_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(plan.canvas_extent(), (14, 5));
}

#[test]
fn same_rank_bottom_lane_route_rejects_different_rows() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 6, 3, 3);
    let edge = edge_between("a", "b", None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    assert!(plan_same_rank_bottom_lane_route(&from, &to, &edge, &charset).is_none());
}

#[test]
fn route_plan_canvas_extent_accounts_for_top_down_back_label_width() {
    let from = node("a", 0, 6, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let edge = edge_between("a", "b", Some("back"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let plan = plan_top_down_back_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(plan.canvas_extent(), (12, 8));
}

#[test]
fn edge_boundary_context_classifies_external_internal_entering_and_leaving_edges() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("x", "X");
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_node("y", "Y");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );

    assert_eq!(
        edge_boundary_context(&graph, &edge_between("x", "y", None, GraphEdgeArrow::Point)),
        EdgeBoundaryContext::External {
            direction: GraphDirection::TopDown
        }
    );
    assert_eq!(
        edge_boundary_context(&graph, &edge_between("a", "b", None, GraphEdgeArrow::Point)),
        EdgeBoundaryContext::Internal {
            group_id: "one",
            direction: GraphDirection::LeftRight
        }
    );
    assert_eq!(
        edge_boundary_context(&graph, &edge_between("x", "a", None, GraphEdgeArrow::Point)),
        EdgeBoundaryContext::Entering {
            group_id: "one",
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight
        }
    );
    assert_eq!(
        edge_boundary_context(&graph, &edge_between("b", "y", None, GraphEdgeArrow::Point)),
        EdgeBoundaryContext::Leaving {
            group_id: "one",
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight
        }
    );
}

#[test]
fn edge_boundary_context_prefers_the_narrowest_nested_group_boundary() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_node("c", "C");
    graph.add_group_with_style(
        "outer",
        "Outer",
        Some(GraphDirection::TopDown),
        vec!["a".to_string(), "inner".to_string()],
        Default::default(),
    );
    graph.add_group_with_style(
        "inner",
        "Inner",
        Some(GraphDirection::LeftRight),
        vec!["b".to_string(), "c".to_string()],
        Default::default(),
    );

    assert_eq!(
        edge_boundary_context(&graph, &edge_between("a", "b", None, GraphEdgeArrow::Point)),
        EdgeBoundaryContext::Entering {
            group_id: "inner",
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight
        }
    );
    assert_eq!(
        edge_boundary_context(&graph, &edge_between("b", "a", None, GraphEdgeArrow::Point)),
        EdgeBoundaryContext::Leaving {
            group_id: "inner",
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight
        }
    );
}

#[test]
fn entering_boundary_route_prefers_grid_path_for_td_root_lr_subgraph_slice() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("x", "X");
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );
    let layout = layout_graph(&graph, &options);
    let edge = edge_between("x", "a", Some("enter"), GraphEdgeArrow::Point);
    let from = layout_node(&layout, "x");
    let to = layout_node(&layout, "a");

    let plan = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: std::slice::from_ref(&edge),
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .expect("entering boundary route should use the grid path stub");

    let expected = plan_left_right_grid_path_route_with_options(
        &layout,
        from,
        to,
        &edge,
        &charset,
        GridRouteOptions::with_fixed_ports(
            crate::graph::routing::path::Port::Right,
            crate::graph::routing::path::Port::Left,
        )
        .with_segment(PlannedRouteSegment::Boundary)
        .with_first_vertical_transit_label(),
    )
    .expect("grid path should exist");
    assert_eq!(plan, expected);
    assert_eq!(
        plan.labels.first().map(|label| label.placement),
        Some(RoutedLabelPlacement::new(21, 10, 5))
    );
}

#[test]
fn leaving_boundary_route_prefers_grid_path_for_td_root_lr_subgraph_slice() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_node("y", "Y");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );
    let layout = layout_graph(&graph, &options);
    let edge = edge_between("b", "y", Some("leave"), GraphEdgeArrow::Point);
    let from = layout_node(&layout, "b");
    let to = layout_node(&layout, "y");

    let plan = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: std::slice::from_ref(&edge),
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .expect("leaving boundary route should use the grid path stub");

    let expected = plan_left_right_grid_path_route_with_options(
        &layout,
        from,
        to,
        &edge,
        &charset,
        GridRouteOptions::with_fixed_ports(
            crate::graph::routing::path::Port::Right,
            crate::graph::routing::path::Port::Right,
        )
        .with_segment(PlannedRouteSegment::Boundary)
        .with_last_vertical_transit_label(),
    )
    .expect("grid path should exist");
    assert_eq!(plan, expected);
    assert_eq!(
        plan.labels.first().map(|label| label.placement),
        Some(RoutedLabelPlacement::new(18, 10, 5))
    );
}

#[test]
fn entering_boundary_route_uses_explicit_left_boundary_ports() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("x", "X");
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );
    let layout = layout_graph(&graph, &options);
    let edge = edge_between("x", "a", None, GraphEdgeArrow::Point);
    let from = layout_node(&layout, "x");
    let to = layout_node(&layout, "a");

    let expected = plan_left_right_grid_path_route_with_options(
        &layout,
        from,
        to,
        &edge,
        &charset,
        GridRouteOptions::with_fixed_ports(
            crate::graph::routing::path::Port::Right,
            crate::graph::routing::path::Port::Left,
        )
        .with_segment(PlannedRouteSegment::Boundary)
        .with_first_vertical_transit_label(),
    )
    .expect("grid path should exist");

    let actual = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: std::slice::from_ref(&edge),
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .expect("entering boundary route should use explicit left boundary ports");
    assert_eq!(actual, expected);
}

#[test]
fn leaving_boundary_route_uses_explicit_right_boundary_ports() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_node("y", "Y");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );
    let layout = layout_graph(&graph, &options);
    let edge = edge_between("b", "y", None, GraphEdgeArrow::Point);
    let from = layout_node(&layout, "b");
    let to = layout_node(&layout, "y");

    let expected = plan_left_right_grid_path_route_with_options(
        &layout,
        from,
        to,
        &edge,
        &charset,
        GridRouteOptions::with_fixed_ports(
            crate::graph::routing::path::Port::Right,
            crate::graph::routing::path::Port::Right,
        )
        .with_segment(PlannedRouteSegment::Boundary)
        .with_last_vertical_transit_label(),
    )
    .expect("grid path should exist");

    let actual = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: std::slice::from_ref(&edge),
        from,
        to,
        edge_index: 0,
        edge: &edge,
        charset: &charset,
    })
    .expect("leaving boundary route should use explicit right boundary ports");
    assert_eq!(actual.labels, expected.labels);
    assert_eq!(
        actual
            .cells
            .iter()
            .map(|cell| (cell.coord, cell.ch, cell.kind))
            .collect::<Vec<_>>(),
        expected
            .cells
            .iter()
            .map(|cell| (cell.coord, cell.ch, cell.kind))
            .collect::<Vec<_>>()
    );
    assert!(
        actual
            .cells
            .iter()
            .all(|cell| cell.segment == PlannedRouteSegment::Boundary)
    );
}

#[test]
fn direct_grid_route_cells_keep_direct_segment_marker() {
    let options = AsciiRenderOptions::ascii();
    let layout = left_right_layout(&[("a", "b")], &options);
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "b");
    let edge = edge(Some("go"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&options);

    let plan = plan_left_right_grid_path_route(&layout, from, to, &edge, &charset).unwrap();
    assert!(
        plan.cells
            .iter()
            .all(|cell| cell.segment == PlannedRouteSegment::Direct)
    );
}

#[test]
fn same_rank_direct_route_plans_ascii_right_arrow_and_label_without_connector() {
    let from = node("a", 0, 0, 5, 3);
    let to = node("b", 11, 0, 5, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(Some("label"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_same_rank_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(9, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(10, 1, '>', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("label").expect("single-line label should exist"),
            RoutedLabelPlacement::new(5, 1, 5),
        )]
    );
}

#[test]
fn same_rank_direct_route_plans_unicode_right_connector() {
    let from = node("a", 0, 0, 5, 3);
    let to = node("b", 10, 0, 5, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

    let plan = plan_same_rank_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(4, 1, '├', PlannedRouteCellKind::EdgeLine),
            cell(5, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(6, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(8, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(9, 1, '►', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert!(plan.labels.is_empty());
}

#[test]
fn same_rank_direct_route_plans_unicode_left_connector_arrow_and_label() {
    let from = node("a", 10, 0, 5, 3);
    let to = node("b", 0, 0, 5, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(Some("back"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

    let plan = plan_same_rank_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(10, 1, '┤', PlannedRouteCellKind::EdgeLine),
            cell(5, 1, '◄', PlannedRouteCellKind::EdgeArrow),
            cell(6, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(8, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(9, 1, '─', PlannedRouteCellKind::RouteCell),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("back").expect("single-line label should exist"),
            RoutedLabelPlacement::new(6, 1, 4),
        )]
    );
}

#[test]
fn same_rank_direct_open_route_plans_line_endpoint_without_arrow() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 6, 0, 3, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(None, GraphEdgeArrow::Open);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_same_rank_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells.last(),
        Some(&cell(5, 1, '-', PlannedRouteCellKind::RouteCell))
    );
}

#[test]
fn same_rank_direct_route_rejects_blocked_path() {
    let from = node("a", 0, 0, 3, 3);
    let blocker = node("blocker", 5, 0, 3, 3);
    let to = node("b", 10, 0, 3, 3);
    let layouts = vec![from.clone(), blocker, to.clone()];
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    assert!(plan_same_rank_direct_route(&layouts, &from, &to, &edge, &charset).is_none());
}

#[test]
fn left_right_grid_path_route_plans_unicode_connector_arrow_and_label() {
    let options = AsciiRenderOptions::unicode();
    let layout = left_right_layout(&[("a", "b")], &options);
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "b");
    let edge = edge(Some("go"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&options);

    let plan = plan_left_right_grid_path_route(&layout, from, to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(5, 2, '─', PlannedRouteCellKind::RouteCell),
            cell(6, 2, '─', PlannedRouteCellKind::RouteCell),
            cell(7, 2, '─', PlannedRouteCellKind::RouteCell),
            cell(8, 2, '─', PlannedRouteCellKind::RouteCell),
            cell(9, 2, '►', PlannedRouteCellKind::EdgeArrow),
            cell(4, 2, '├', PlannedRouteCellKind::EdgeLine),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("go").expect("single-line label should exist"),
            RoutedLabelPlacement::new(6, 2, 2),
        )]
    );
}

#[test]
fn left_right_grid_path_route_plans_bent_path_cells_and_corner() {
    let options = AsciiRenderOptions::ascii();
    let layout = left_right_layout(&[("a", "b"), ("a", "c")], &options);
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "c");
    let edge = edge_between("a", "c", Some("down"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&options);

    let plan = plan_left_right_grid_path_route(&layout, from, to, &edge, &charset).unwrap();

    assert!(
        plan.cells
            .iter()
            .any(|cell| cell.kind == PlannedRouteCellKind::RouteCell && cell.ch == '+')
    );
    assert!(
        plan.cells
            .iter()
            .any(|cell| cell.kind == PlannedRouteCellKind::RouteCell && cell.ch == '|')
    );
    assert!(
        plan.cells
            .iter()
            .any(|cell| cell.kind == PlannedRouteCellKind::EdgeArrow)
    );
    assert_eq!(
        plan.labels
            .first()
            .and_then(|label| label.text.lines().first().map(String::as_str)),
        Some("down")
    );
}

#[test]
fn left_right_down_route_plans_vertical_bent_line() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 0, 6, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_down_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
            cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 5, 'v', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert!(plan.labels.is_empty());
}

#[test]
fn left_right_down_then_right_route_plans_basic_bend() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 6, 4, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_down_then_right_route(
        &[from.clone(), to.clone()],
        &[],
        &from,
        &to,
        &edge,
        &charset,
    )
    .unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
            cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 5, '+', PlannedRouteCellKind::RouteCell),
            cell(2, 5, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 5, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 5, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 5, '>', PlannedRouteCellKind::EdgeArrow),
        ]
    );
}

#[test]
fn left_right_right_then_up_route_plans_basic_bend() {
    let from = node("a", 0, 6, 3, 3);
    let to = node("b", 6, 0, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_right_then_up_route(
        &[from.clone(), to.clone()],
        &[],
        &from,
        &to,
        &edge,
        &charset,
    )
    .unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(2, 7, '|', PlannedRouteCellKind::EdgeLine),
            cell(3, 7, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 7, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 7, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 7, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 7, '+', PlannedRouteCellKind::RouteCell),
            cell(7, 4, '|', PlannedRouteCellKind::RouteCell),
            cell(7, 5, '|', PlannedRouteCellKind::RouteCell),
            cell(7, 6, '|', PlannedRouteCellKind::RouteCell),
            cell(7, 3, '^', PlannedRouteCellKind::EdgeArrow),
        ]
    );
}

#[test]
fn left_right_down_then_right_route_plans_crossing_lane() {
    let from = node("a", 0, 0, 3, 3);
    let lower_source = node("b", 0, 8, 3, 3);
    let upper_target = node("c", 10, 0, 3, 3);
    let to = node("d", 10, 8, 3, 3);
    let layouts = vec![
        from.clone(),
        lower_source.clone(),
        upper_target.clone(),
        to.clone(),
    ];
    let edge = edge_between("a", "d", None, GraphEdgeArrow::Point);
    let crossing_edge = edge_between("b", "c", None, GraphEdgeArrow::Point);
    let edges = vec![edge.clone(), crossing_edge];
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_down_then_right_route(&layouts, &edges, &from, &to, &edge, &charset)
        .unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
            cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 5, '+', PlannedRouteCellKind::RouteCell),
            cell(2, 5, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 5, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 5, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 5, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 5, '+', PlannedRouteCellKind::RouteCell),
            cell(6, 6, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 7, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 8, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 9, '+', PlannedRouteCellKind::RouteCell),
            cell(7, 9, '-', PlannedRouteCellKind::RouteCell),
            cell(8, 9, '-', PlannedRouteCellKind::RouteCell),
            cell(9, 9, '>', PlannedRouteCellKind::EdgeArrow),
        ]
    );
}

#[test]
fn invisible_crossing_edge_does_not_displace_a_visible_route() {
    let from = node("a", 0, 0, 3, 3);
    let lower_source = node("b", 0, 8, 3, 3);
    let upper_target = node("c", 10, 0, 3, 3);
    let to = node("d", 10, 8, 3, 3);
    let layouts = vec![from.clone(), lower_source, upper_target, to.clone()];
    let edge = edge_between("a", "d", None, GraphEdgeArrow::Point);
    let mut crossing_edge = edge_between("b", "c", None, GraphEdgeArrow::Point);
    crossing_edge.stroke = GraphEdgeStroke::Invisible;
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let with_invisible = plan_left_right_down_then_right_route(
        &layouts,
        &[crossing_edge],
        &from,
        &to,
        &edge,
        &charset,
    )
    .unwrap();
    let without_crossing =
        plan_left_right_down_then_right_route(&layouts, &[], &from, &to, &edge, &charset).unwrap();

    assert_eq!(with_invisible, without_crossing);
}

#[test]
fn same_rank_bottom_lane_route_plans_reverse_lane_and_label() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let edge = edge(Some("back"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_same_rank_bottom_lane_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(11, 2, '-', PlannedRouteCellKind::EdgeLine),
            cell(11, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(11, 4, '+', PlannedRouteCellKind::RouteCell),
            cell(2, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(8, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(9, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(10, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '+', PlannedRouteCellKind::RouteCell),
            cell(1, 3, '^', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("back").expect("single-line label should exist"),
            RoutedLabelPlacement::new(4, 4, 4),
        )]
    );
}

#[test]
fn left_right_reverse_over_self_loop_route_plans_target_side_lane() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(Some("rev"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_reverse_over_self_loop_route(&layouts, &from, &to, &edge, &charset)
        .unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(10, 1, '|', PlannedRouteCellKind::EdgeLine),
            cell(6, 1, '+', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(9, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 1, '<', PlannedRouteCellKind::EdgeArrow),
            cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("rev").expect("single-line label should exist"),
            RoutedLabelPlacement::new(5, 1, 3),
        )]
    );
}

#[test]
fn left_right_self_loop_route_plans_loop_and_arrow() {
    let from = node("a", 0, 0, 3, 3);
    let layouts = vec![from.clone()];
    let edge = edge_between("a", "a", Some("loop"), GraphEdgeArrow::Point);
    let edges = vec![edge.clone()];
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_self_loop_route(&layouts, &edges, &from, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(2, 1, '|', PlannedRouteCellKind::EdgeLine),
            cell(3, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 1, '+', PlannedRouteCellKind::RouteCell),
            cell(4, 2, '|', PlannedRouteCellKind::RouteCell),
            cell(4, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(4, 4, '+', PlannedRouteCellKind::RouteCell),
            cell(2, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '+', PlannedRouteCellKind::RouteCell),
            cell(1, 3, '^', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("loop").expect("single-line label should exist"),
            RoutedLabelPlacement::new(0, 4, 4),
        )]
    );
}

#[test]
fn top_down_bent_route_plans_side_bend_arrow_and_label() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 6, 5, 3, 3);
    let edge = edge(Some("bend"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_top_down_bent_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(2, 1, '|', PlannedRouteCellKind::EdgeLine),
            cell(3, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '+', PlannedRouteCellKind::RouteCell),
            cell(7, 2, '|', PlannedRouteCellKind::RouteCell),
            cell(7, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(7, 4, 'v', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("bend").expect("single-line label should exist"),
            RoutedLabelPlacement::new(2, 1, 4),
        )]
    );
}

#[test]
fn top_down_choice_bent_route_drops_before_turning_and_labels_horizontal_segment() {
    let from = node_with_shape("a", 0, 0, 3, 3, GraphNodeShape::Choice);
    let to = node("b", 6, 5, 3, 3);
    let edge = edge(Some("bend"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_top_down_bent_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
            cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '+', PlannedRouteCellKind::RouteCell),
            cell(2, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 4, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 4, 'v', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("bend").expect("single-line label should exist"),
            RoutedLabelPlacement::new(2, 4, 4),
        )]
    );
}

#[test]
fn top_down_bent_route_plans_right_bend_unicode_corner() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 6, 5, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

    let plan = plan_top_down_bent_route(&from, &to, &edge, &charset).unwrap();

    assert!(
        plan.cells
            .iter()
            .any(|cell| cell.coord == CanvasCoord { x: 2, y: 1 } && cell.ch == '├'),
        "right/down bend should leave the source side with a connector: {plan:?}"
    );
    assert!(
        plan.cells
            .iter()
            .any(|cell| cell.coord == CanvasCoord { x: 7, y: 1 } && cell.ch == '┐'),
        "right/down bend should turn down with a connected top-right corner: {plan:?}"
    );
}

#[test]
fn top_down_bent_route_plans_left_bend_open_endpoint() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 5, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Open);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_top_down_bent_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(10, 1, '|', PlannedRouteCellKind::EdgeLine),
            cell(9, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(2, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(1, 1, '+', PlannedRouteCellKind::RouteCell),
            cell(1, 2, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
        ]
    );
    assert!(plan.labels.is_empty());
}

#[test]
fn top_down_bent_route_plans_left_bend_unicode_corner() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 5, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

    let plan = plan_top_down_bent_route(&from, &to, &edge, &charset).unwrap();

    assert!(
        plan.cells
            .iter()
            .any(|cell| cell.coord == CanvasCoord { x: 10, y: 1 } && cell.ch == '┤'),
        "left/down bend should leave the source side with a connector: {plan:?}"
    );
    assert!(
        plan.cells
            .iter()
            .any(|cell| cell.coord == CanvasCoord { x: 1, y: 1 } && cell.ch == '┌'),
        "left/down bend should turn down with a connected top-left corner: {plan:?}"
    );
}

#[test]
fn top_down_back_route_plans_lane_arrow_and_label() {
    let from = node("a", 0, 6, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let edge = edge(Some("back"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_top_down_back_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(2, 7, '|', PlannedRouteCellKind::EdgeLine),
            cell(3, 7, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 7, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 7, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 7, '+', PlannedRouteCellKind::RouteCell),
            cell(6, 2, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 4, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 5, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 6, '|', PlannedRouteCellKind::RouteCell),
            cell(6, 1, '+', PlannedRouteCellKind::RouteCell),
            cell(3, 1, '<', PlannedRouteCellKind::EdgeArrow),
            cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("back").expect("single-line label should exist"),
            RoutedLabelPlacement::new(7, 4, 4),
        )]
    );
}

#[test]
fn top_down_direct_route_plans_connector_line_arrow_and_label() {
    let from = node("a", 2, 0, 5, 3);
    let to = node("b", 2, 6, 5, 3);
    let edge = edge(Some("label"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_top_down_direct_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(4, 2, '-', PlannedRouteCellKind::EdgeLine),
            cell(4, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(4, 4, '|', PlannedRouteCellKind::RouteCell),
            cell(4, 5, 'v', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("label").expect("single-line label should exist"),
            RoutedLabelPlacement::new(2, 4, 5),
        )]
    );
}

#[test]
fn top_down_direct_open_route_plans_line_endpoint_without_arrow() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 0, 5, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Open);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_top_down_direct_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells.last(),
        Some(&cell(1, 4, '|', PlannedRouteCellKind::RouteCell))
    );
    assert!(plan.labels.is_empty());
}

#[test]
fn top_down_direct_route_rejects_adjacent_boxes() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 0, 3, 3, 3);
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    assert!(plan_top_down_direct_route(&from, &to, &edge, &charset).is_none());
}

#[test]
fn top_down_side_entry_route_plans_unicode_connector_and_label() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("group", 6, 0, 3, 3);
    let edge = edge(Some("enter"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

    let plan = plan_top_down_side_entry_route(&from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(2, 1, '├', PlannedRouteCellKind::EdgeLine),
            cell(3, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(4, 1, '─', PlannedRouteCellKind::RouteCell),
            cell(5, 1, '►', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("enter").expect("single-line label should exist"),
            RoutedLabelPlacement::new(2, 1, 5),
        )]
    );
}

#[test]
fn reverse_ascii_side_entry_uses_explicit_source_anchor_for_double_markers() {
    let from = node("a", 8, 0, 3, 3);
    let to = node("group", 0, 0, 3, 3);
    let edge = edge_between_with_markers(
        "a",
        "group",
        None,
        GraphEdgeArrow::Circle,
        GraphEdgeArrow::Cross,
    );
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_top_down_side_entry_route(&from, &to, &edge, &charset)
        .unwrap()
        .with_markers(edge.start_marker, edge.end_marker, &charset, "flowchart")
        .unwrap();

    assert_eq!(
        plan.cells
            .iter()
            .find(|cell| cell.coord == CanvasCoord { x: 3, y: 1 })
            .map(|cell| (cell.ch, cell.kind)),
        Some(('x', PlannedRouteCellKind::EdgeArrow))
    );
    assert_eq!(
        plan.cells
            .iter()
            .find(|cell| cell.coord == CanvasCoord { x: 7, y: 1 })
            .map(|cell| (cell.ch, cell.kind)),
        Some(('o', PlannedRouteCellKind::EdgeArrow))
    );
}

fn cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind,
        segment: PlannedRouteSegment::Direct,
        paint: PlannedRoutePaint::role(match kind {
            PlannedRouteCellKind::EdgeArrow => AsciiColorRole::EdgeArrow,
            PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::RouteCell => {
                AsciiColorRole::EdgeLine
            }
        }),
    }
}

fn edge(label: Option<&str>, arrow: GraphEdgeArrow) -> AsciiGraphEdge {
    edge_between("a", "b", label, arrow)
}

fn edge_between(
    from: &str,
    to: &str,
    label: Option<&str>,
    arrow: GraphEdgeArrow,
) -> AsciiGraphEdge {
    edge_between_with_markers(from, to, label, GraphEdgeArrow::Open, arrow)
}

fn edge_between_with_markers(
    from: &str,
    to: &str,
    label: Option<&str>,
    start_marker: GraphEdgeArrow,
    end_marker: GraphEdgeArrow,
) -> AsciiGraphEdge {
    AsciiGraphEdge {
        id: None,
        is_user_defined_id: false,
        from: from.to_string(),
        to: to.to_string(),
        label: label.map(ToOwned::to_owned),
        stroke: GraphEdgeStroke::Normal,
        start_marker,
        end_marker,
        length: 1,
        style: GraphEdgeStyle::default(),
    }
}

fn node(id: &str, x: usize, y: usize, width: usize, height: usize) -> NodeLayout {
    node_with_shape(id, x, y, width, height, GraphNodeShape::Rect)
}

fn node_with_shape(
    id: &str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    shape: GraphNodeShape,
) -> NodeLayout {
    NodeLayout {
        id: id.to_string(),
        label: GraphLabel::new(id),
        shape,
        style: GraphNodeStyle::default(),
        grid: GridCoord { x: 0, y: 0 },
        x,
        y,
        width,
        height,
    }
}

fn left_right_layout(edges: &[(&str, &str)], options: &AsciiRenderOptions) -> GraphLayout {
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    if edges.iter().any(|(_, to)| *to == "c") {
        graph.add_node("c", "C");
    }
    for (from, to) in edges {
        graph.add_edge(*from, *to);
    }
    layout_graph(&graph, options)
}

fn layout_node<'a>(layout: &'a GraphLayout, id: &str) -> &'a NodeLayout {
    layout
        .nodes
        .iter()
        .find(|node| node.id == id)
        .expect("layout should contain test node")
}
