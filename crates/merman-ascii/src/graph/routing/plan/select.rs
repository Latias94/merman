use super::super::super::charset::GraphCharset;
use super::super::super::layout::{GraphLayout, NodeLayout};
use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
use super::super::path::Port;
use super::PlannedRouteSegment;
use super::RoutePlan;
use super::grid::{
    plan_left_right_grid_path_route, plan_left_right_grid_path_route_with_ports,
    plan_left_right_grid_path_route_with_ports_and_segment,
};
use super::left_right::{
    left_right_back_edge_bottom_y, plan_left_right_bottom_lane_route, plan_left_right_direct_route,
    plan_left_right_down_route, plan_left_right_down_then_right_route,
    plan_left_right_reverse_over_self_loop_route, plan_left_right_right_then_up_route,
    plan_left_right_self_loop_route, self_loop_bottom_y_for_edges, self_loop_right_x,
};
use super::top_down::{
    plan_top_down_back_route, plan_top_down_bent_route, plan_top_down_direct_route,
    top_down_back_edge_lane_x,
};
use crate::text::display_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) enum EdgeBoundaryContext<'a> {
    External {
        direction: GraphDirection,
    },
    Internal {
        group_id: &'a str,
        direction: GraphDirection,
    },
    Entering {
        group_id: &'a str,
        root_direction: GraphDirection,
        local_direction: GraphDirection,
    },
    Leaving {
        group_id: &'a str,
        root_direction: GraphDirection,
        local_direction: GraphDirection,
    },
}

#[derive(Debug, Clone, Copy)]
pub(in crate::graph::routing) struct EdgeRouteRequest<'a> {
    pub(in crate::graph::routing) graph: &'a AsciiGraph,
    pub(in crate::graph::routing) graph_layout: &'a GraphLayout,
    pub(in crate::graph::routing) edges: &'a [AsciiGraphEdge],
    pub(in crate::graph::routing) from: &'a NodeLayout,
    pub(in crate::graph::routing) to: &'a NodeLayout,
    pub(in crate::graph::routing) edge_index: usize,
    pub(in crate::graph::routing) edge: &'a AsciiGraphEdge,
    pub(in crate::graph::routing) charset: &'a GraphCharset,
}

pub(in crate::graph::routing) fn plan_edge_route(
    request: EdgeRouteRequest<'_>,
) -> Option<RoutePlan> {
    let boundary = edge_boundary_context(request.graph, request.edge);
    if let Some(plan) = plan_boundary_route(boundary, request) {
        return Some(plan);
    }

    match boundary.direction().canonical() {
        GraphDirection::LeftRight => plan_left_right_route(request),
        GraphDirection::TopDown => {
            plan_top_down_route(request.from, request.to, request.edge, request.charset)
        }
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

fn plan_left_right_route(request: EdgeRouteRequest<'_>) -> Option<RoutePlan> {
    let graph_layout = request.graph_layout;
    let from = request.from;
    let to = request.to;
    let edge = request.edge;
    let charset = request.charset;

    if from.id == to.id {
        return plan_left_right_self_loop_route(
            &graph_layout.nodes,
            request.edges,
            from,
            edge,
            charset,
        );
    }

    let parallel_index = parallel_edge_index(request.edges, request.edge_index);
    if from.center_y() == to.center_y() && from.x < to.x && parallel_index > 0 {
        return plan_left_right_bottom_lane_route(from, to, edge, charset);
    }

    if from.center_y() == to.center_y() && from.x > to.x {
        if has_self_loop(request.edges, &to.id) {
            return plan_left_right_reverse_over_self_loop_route(
                &graph_layout.nodes,
                from,
                to,
                edge,
                charset,
            );
        }
        return plan_left_right_bottom_lane_route(from, to, edge, charset);
    }

    if from.center_y() == to.center_y() && from.x < to.x {
        if let Some(plan) =
            plan_left_right_direct_route(&graph_layout.nodes, from, to, edge, charset)
        {
            return Some(plan);
        }
    }

    if let Some(plan) = plan_left_right_grid_path_route(graph_layout, from, to, edge, charset) {
        return Some(plan);
    }

    if from.center_y() < to.center_y() && to.x > from.x {
        return plan_left_right_down_then_right_route(
            &graph_layout.nodes,
            request.edges,
            from,
            to,
            edge,
            charset,
        );
    }

    if from.center_y() < to.center_y() && to.x == from.x {
        return plan_left_right_down_route(from, to, edge, charset);
    }

    if from.center_y() > to.center_y() && to.x > from.x {
        return plan_left_right_right_then_up_route(
            &graph_layout.nodes,
            request.edges,
            from,
            to,
            edge,
            charset,
        );
    }

    None
}

fn plan_top_down_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if from.center_y() > to.center_y() {
        return plan_top_down_back_route(from, to, edge, charset);
    }

    if from.center_x() != to.center_x() {
        return plan_top_down_bent_route(from, to, edge, charset);
    }

    plan_top_down_direct_route(from, to, edge, charset)
}

fn plan_boundary_route(
    boundary: EdgeBoundaryContext<'_>,
    request: EdgeRouteRequest<'_>,
) -> Option<RoutePlan> {
    match boundary {
        EdgeBoundaryContext::Entering {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => plan_left_right_grid_path_route_with_ports(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            request.charset,
            Some(Port::Right),
            Some(Port::Left),
        ),
        EdgeBoundaryContext::Leaving {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => plan_left_right_grid_path_route_with_ports_and_segment(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            request.charset,
            Some(Port::Right),
            Some(Port::Right),
            PlannedRouteSegment::Boundary,
        ),
        EdgeBoundaryContext::External { .. } | EdgeBoundaryContext::Internal { .. } => None,
        EdgeBoundaryContext::Entering { .. } | EdgeBoundaryContext::Leaving { .. } => None,
    }
}

pub(in crate::graph::routing) fn route_canvas_extent(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    _direction: GraphDirection,
) -> (usize, usize) {
    let mut width = 0;
    let mut height = 0;

    for edge in edges.iter().filter(|edge| edge.from == edge.to) {
        let Some(layout) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        width = width.max(self_loop_right_x(layouts, layout) + 1);
        height = height.max(self_loop_bottom_y_for_edges(layouts, edges, layout) + 1);
    }
    for (edge_index, edge) in edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.from != edge.to)
    {
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
            continue;
        };
        match edge_boundary_context(graph, edge).direction().canonical() {
            GraphDirection::LeftRight => {
                if from.center_y() == to.center_y()
                    && (from.x > to.x || parallel_edge_index(edges, edge_index) > 0)
                {
                    width = width.max(from.center_x().max(to.center_x()) + 3);
                    height = height.max(left_right_back_edge_bottom_y(from) + 1);
                }
            }
            GraphDirection::TopDown => {
                if from.center_y() > to.center_y() {
                    let lane_x = top_down_back_edge_lane_x(from, to);
                    width = width.max(lane_x + 3);
                    if let Some(label) = edge.label.as_deref() {
                        let label_start = lane_x.saturating_sub(display_width(label) / 2);
                        width = width.max(label_start + display_width(label) + 1);
                    }
                }
            }
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    (width, height)
}

pub(in crate::graph::routing) fn edge_boundary_context<'a>(
    graph: &'a AsciiGraph,
    edge: &AsciiGraphEdge,
) -> EdgeBoundaryContext<'a> {
    for group in &graph.groups {
        let Some(local_direction) = group.direction else {
            continue;
        };
        let from_inside = group.nodes.iter().any(|node| node == &edge.from);
        let to_inside = group.nodes.iter().any(|node| node == &edge.to);
        match (from_inside, to_inside) {
            (true, true) => {
                return EdgeBoundaryContext::Internal {
                    group_id: group.id.as_str(),
                    direction: local_direction,
                };
            }
            (false, true) => {
                return EdgeBoundaryContext::Entering {
                    group_id: group.id.as_str(),
                    root_direction: graph.direction,
                    local_direction,
                };
            }
            (true, false) => {
                return EdgeBoundaryContext::Leaving {
                    group_id: group.id.as_str(),
                    root_direction: graph.direction,
                    local_direction,
                };
            }
            (false, false) => {}
        }
    }

    EdgeBoundaryContext::External {
        direction: graph.direction,
    }
}

impl EdgeBoundaryContext<'_> {
    fn direction(self) -> GraphDirection {
        match self {
            Self::External { direction } | Self::Internal { direction, .. } => direction,
            Self::Entering {
                root_direction: _,
                local_direction,
                ..
            }
            | Self::Leaving {
                root_direction: _,
                local_direction,
                ..
            } => local_direction,
        }
    }
}

fn has_self_loop(edges: &[AsciiGraphEdge], node_id: &str) -> bool {
    edges
        .iter()
        .any(|edge| edge.from == node_id && edge.to == node_id)
}

fn parallel_edge_index(edges: &[AsciiGraphEdge], edge_index: usize) -> usize {
    let Some(edge) = edges.get(edge_index) else {
        return 0;
    };
    edges[..edge_index]
        .iter()
        .filter(|previous| same_edge_pair(previous, edge))
        .count()
}

fn same_edge_pair(left: &AsciiGraphEdge, right: &AsciiGraphEdge) -> bool {
    (left.from == right.from && left.to == right.to)
        || (left.from == right.to && left.to == right.from)
}
