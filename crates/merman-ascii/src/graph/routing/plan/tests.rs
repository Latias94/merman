use super::grid::{plan_left_right_grid_path_route, plan_left_right_grid_path_route_with_ports};
use super::left_right::{
    plan_left_right_bottom_lane_route, plan_left_right_direct_route, plan_left_right_down_route,
    plan_left_right_down_then_right_route, plan_left_right_reverse_over_self_loop_route,
    plan_left_right_right_then_up_route, plan_left_right_self_loop_route,
};
use super::top_down::{
    plan_top_down_back_route, plan_top_down_bent_route, plan_top_down_direct_route,
};
use super::*;
use crate::AsciiRenderOptions;
use crate::graph::charset::GraphCharset;
use crate::graph::label::GraphLabel;
use crate::graph::layout::{GraphLayout, GridCoord, NodeLayout, layout_graph};
use crate::graph::model::{
    AsciiGraph, AsciiGraphEdge, GraphDirection, GraphEdgeArrow, GraphEdgeStroke, GraphEdgeStyle,
    GraphNodeShape, GraphNodeStyle,
};
use crate::graph::routing::plan::select::{EdgeBoundaryContext, edge_boundary_context};
use crate::graph::routing::plan::PlannedRouteSegment;

#[test]
fn edge_route_selects_left_right_parallel_bottom_lane() {
    let options = AsciiRenderOptions::ascii();
    let layout = left_right_layout(&[("a", "b"), ("a", "b")], &options);
    let from = layout_node(&layout, "a");
    let to = layout_node(&layout, "b");
    let edges = vec![
        edge(Some("parallel"), GraphEdgeArrow::Point),
        edge(Some("parallel"), GraphEdgeArrow::Point),
    ];
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
    let expected = plan_left_right_bottom_lane_route(from, to, &edges[1], &charset).unwrap();

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
fn route_canvas_extent_accounts_for_left_right_back_lane() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let layouts = vec![from, to];
    let edges = vec![edge_between("a", "b", None, GraphEdgeArrow::Point)];
    let graph = test_graph(GraphDirection::LeftRight, &[("a", "b")]);

    assert_eq!(
        route_canvas_extent(&graph, &layouts, &edges, GraphDirection::LeftRight),
        (14, 5)
    );
}

#[test]
fn route_canvas_extent_accounts_for_top_down_back_label_width() {
    let from = node("a", 0, 6, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let layouts = vec![from, to];
    let edges = vec![edge_between("a", "b", Some("back"), GraphEdgeArrow::Point)];
    let graph = test_graph(GraphDirection::TopDown, &[("a", "b")]);

    assert_eq!(
        route_canvas_extent(&graph, &layouts, &edges, GraphDirection::TopDown),
        (9, 0)
    );
}

#[test]
fn route_canvas_extent_uses_local_direction_only_for_internal_subgraph_edges() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_node("x", "X");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );

    let layouts = vec![
        node("x", 8, 0, 3, 3),
        node("a", 0, 8, 3, 3),
        node("b", 8, 8, 3, 3),
    ];
    let internal_edge = edge_between("a", "b", None, GraphEdgeArrow::Point);
    let external_edge = edge_between("x", "a", None, GraphEdgeArrow::Point);

    assert_eq!(
        route_canvas_extent(
            &graph,
            &layouts,
            &[internal_edge.clone()],
            GraphDirection::TopDown
        ),
        (0, 0)
    );
    assert_eq!(
        route_canvas_extent(&graph, &layouts, &[external_edge], GraphDirection::TopDown),
        (0, 0)
    );
}

#[test]
fn internal_subgraph_edge_marks_local_group_context_through_extent_behavior() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );
    let layouts = vec![node("a", 0, 8, 3, 3), node("b", 8, 8, 3, 3)];
    let internal_edge = edge_between("a", "b", None, GraphEdgeArrow::Point);

    assert_eq!(
        route_canvas_extent(&graph, &layouts, &[internal_edge], GraphDirection::TopDown),
        (0, 0)
    );
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
    let edge = edge_between("x", "a", None, GraphEdgeArrow::Point);
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

    let expected = plan_left_right_grid_path_route(&layout, from, to, &edge, &charset)
        .expect("grid path should exist");
    assert_eq!(plan, expected);
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
    let edge = edge_between("b", "y", None, GraphEdgeArrow::Point);
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

    let expected = plan_left_right_grid_path_route_with_ports(
        &layout,
        from,
        to,
        &edge,
        &charset,
        Some(crate::graph::routing::path::Port::Right),
        Some(crate::graph::routing::path::Port::Right),
    )
    .expect("grid path should exist");
    assert_eq!(plan, expected);
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

    let expected = plan_left_right_grid_path_route_with_ports(
        &layout,
        from,
        to,
        &edge,
        &charset,
        Some(crate::graph::routing::path::Port::Right),
        Some(crate::graph::routing::path::Port::Left),
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

    let expected = plan_left_right_grid_path_route_with_ports(
        &layout,
        from,
        to,
        &edge,
        &charset,
        Some(crate::graph::routing::path::Port::Right),
        Some(crate::graph::routing::path::Port::Right),
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
    assert!(actual
        .cells
        .iter()
        .all(|cell| cell.segment == PlannedRouteSegment::Boundary));
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
    assert!(plan
        .cells
        .iter()
        .all(|cell| cell.segment == PlannedRouteSegment::Direct));
}

#[test]
fn left_right_direct_route_plans_ascii_line_arrow_and_label_without_connector() {
    let from = node("a", 0, 0, 5, 3);
    let to = node("b", 10, 0, 5, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(Some("label"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells,
        vec![
            cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(9, 1, '>', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel {
            start: CanvasCoord { x: 5, y: 1 },
            end: CanvasCoord { x: 9, y: 1 },
            text: "label".to_string(),
        }]
    );
}

#[test]
fn left_right_direct_route_plans_unicode_connector() {
    let from = node("a", 0, 0, 5, 3);
    let to = node("b", 10, 0, 5, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

    let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

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
fn left_right_direct_open_route_plans_line_endpoint_without_arrow() {
    let from = node("a", 0, 0, 3, 3);
    let to = node("b", 6, 0, 3, 3);
    let layouts = vec![from.clone(), to.clone()];
    let edge = edge(None, GraphEdgeArrow::Open);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

    assert_eq!(
        plan.cells.last(),
        Some(&cell(5, 1, '-', PlannedRouteCellKind::RouteCell))
    );
}

#[test]
fn left_right_direct_route_rejects_blocked_same_row_path() {
    let from = node("a", 0, 0, 3, 3);
    let blocker = node("blocker", 5, 0, 3, 3);
    let to = node("b", 10, 0, 3, 3);
    let layouts = vec![from.clone(), blocker, to.clone()];
    let edge = edge(None, GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    assert!(plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).is_none());
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
            cell(9, 2, '─', PlannedRouteCellKind::RouteCell),
            cell(4, 2, '├', PlannedRouteCellKind::EdgeLine),
            cell(9, 2, '►', PlannedRouteCellKind::EdgeArrow),
        ]
    );
    assert_eq!(
        plan.labels,
        vec![PlannedRouteLabel {
            start: CanvasCoord { x: 5, y: 2 },
            end: CanvasCoord { x: 9, y: 2 },
            text: "go".to_string(),
        }]
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
        plan.labels.first().map(|label| label.text.as_str()),
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
fn left_right_bottom_lane_route_plans_reverse_lane_and_label() {
    let from = node("a", 10, 0, 3, 3);
    let to = node("b", 0, 0, 3, 3);
    let edge = edge(Some("back"), GraphEdgeArrow::Point);
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

    let plan = plan_left_right_bottom_lane_route(&from, &to, &edge, &charset).unwrap();

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
        vec![PlannedRouteLabel {
            start: CanvasCoord { x: 1, y: 4 },
            end: CanvasCoord { x: 11, y: 4 },
            text: "back".to_string(),
        }]
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
        vec![PlannedRouteLabel {
            start: CanvasCoord { x: 3, y: 1 },
            end: CanvasCoord { x: 9, y: 1 },
            text: "rev".to_string(),
        }]
    );
}

#[test]
fn left_right_self_loop_route_plans_loop_and_arrow() {
    let from = node("a", 0, 0, 3, 3);
    let layouts = vec![from.clone()];
    let edge = edge_between("a", "a", None, GraphEdgeArrow::Point);
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
    assert!(plan.labels.is_empty());
}

#[test]
fn top_down_bent_route_plans_right_bend_arrow_and_label() {
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
        vec![PlannedRouteLabel {
            start: CanvasCoord { x: 7, y: 2 },
            end: CanvasCoord { x: 7, y: 4 },
            text: "bend".to_string(),
        }]
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
            cell(2, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(3, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(9, 1, '-', PlannedRouteCellKind::RouteCell),
            cell(1, 1, '+', PlannedRouteCellKind::RouteCell),
            cell(1, 2, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
            cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
        ]
    );
    assert!(plan.labels.is_empty());
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
        vec![PlannedRouteLabel {
            start: CanvasCoord { x: 6, y: 1 },
            end: CanvasCoord { x: 6, y: 7 },
            text: "back".to_string(),
        }]
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
        vec![PlannedRouteLabel {
            start: CanvasCoord { x: 4, y: 3 },
            end: CanvasCoord { x: 4, y: 5 },
            text: "label".to_string(),
        }]
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

fn cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind,
        segment: PlannedRouteSegment::Direct,
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
    AsciiGraphEdge {
        from: from.to_string(),
        to: to.to_string(),
        label: label.map(ToOwned::to_owned),
        stroke: GraphEdgeStroke::Normal,
        arrow,
        length: 1,
        style: GraphEdgeStyle::default(),
    }
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

fn test_graph(direction: GraphDirection, edges: &[(&str, &str)]) -> AsciiGraph {
    let mut graph = AsciiGraph::new(direction);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    if edges.iter().any(|(_, to)| *to == "c") {
        graph.add_node("c", "C");
    }
    for (from, to) in edges {
        graph.add_edge(*from, *to);
    }
    graph
}

fn layout_node<'a>(layout: &'a GraphLayout, id: &str) -> &'a NodeLayout {
    layout
        .nodes
        .iter()
        .find(|node| node.id == id)
        .expect("layout should contain test node")
}
