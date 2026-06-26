use super::super::super::charset::GraphCharset;
use super::super::super::layout::{GraphLayout, NodeLayout};
use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
use super::super::path::Port;
use super::PlannedRouteSegment;
use super::RoutePlan;
pub(super) use super::boundary::{EdgeBoundaryContext, edge_boundary_context};
use super::edges::parallel_edge_index;
use super::grid::{
    GridRouteOptions, plan_left_right_grid_path_route, plan_left_right_grid_path_route_with_options,
};
use super::left_right::{
    plan_left_right_bottom_lane_route, plan_left_right_direct_route, plan_left_right_down_route,
    plan_left_right_down_then_right_route, plan_left_right_reverse_over_self_loop_route,
    plan_left_right_right_then_up_route, plan_left_right_self_loop_route,
};
use super::top_down::{
    plan_top_down_back_route, plan_top_down_bent_route, plan_top_down_direct_route,
    plan_top_down_side_entry_route,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::graph::routing) enum EdgeRoutePlan {
    Routed(RoutePlan),
    Unsupported(UnsupportedEdgeRoute),
}

impl EdgeRoutePlan {
    #[cfg(test)]
    pub(in crate::graph::routing) fn unwrap(self) -> RoutePlan {
        self.expect("edge route should be supported")
    }

    #[cfg(test)]
    pub(in crate::graph::routing) fn expect(self, message: &str) -> RoutePlan {
        match self {
            Self::Routed(plan) => plan,
            Self::Unsupported(route) => panic!("{message}: {route:?}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) struct UnsupportedEdgeRoute {
    reason: UnsupportedEdgeRouteReason,
}

impl UnsupportedEdgeRoute {
    fn new(reason: UnsupportedEdgeRouteReason) -> Self {
        Self { reason }
    }

    pub(in crate::graph::routing) fn feature(self) -> &'static str {
        match self.reason {
            UnsupportedEdgeRouteReason::NoRouteFamily => "unroutable graph edges",
            UnsupportedEdgeRouteReason::BoundaryDirection => "unsupported graph boundary routes",
        }
    }

    #[cfg(test)]
    pub(in crate::graph::routing) fn reason(self) -> UnsupportedEdgeRouteReason {
        self.reason
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) enum UnsupportedEdgeRouteReason {
    NoRouteFamily,
    BoundaryDirection,
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

pub(in crate::graph::routing) fn plan_edge_route(request: EdgeRouteRequest<'_>) -> EdgeRoutePlan {
    let boundary = edge_boundary_context(request.graph, request.edge);
    if let Some(plan) = plan_boundary_route(boundary, request) {
        return EdgeRoutePlan::Routed(plan);
    }

    let plan = match boundary.direction().canonical() {
        GraphDirection::LeftRight => plan_left_right_route(request),
        GraphDirection::TopDown => plan_top_down_route(request),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    };

    match plan {
        Some(plan) => EdgeRoutePlan::Routed(plan),
        None => EdgeRoutePlan::Unsupported(UnsupportedEdgeRoute::new(unsupported_reason(
            boundary, request,
        ))),
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

    if from.center_y() == to.center_y()
        && from.x < to.x
        && let Some(plan) =
            plan_left_right_direct_route(&graph_layout.nodes, from, to, edge, charset)
    {
        return Some(plan);
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

fn plan_top_down_route(request: EdgeRouteRequest<'_>) -> Option<RoutePlan> {
    let from = request.from;
    let to = request.to;
    let edge = request.edge;
    let charset = request.charset;

    if from.center_y() > to.center_y() {
        return plan_top_down_back_route(from, to, edge, charset);
    }

    if from.center_y() == to.center_y()
        && let Some(plan) =
            plan_left_right_direct_route(&request.graph_layout.nodes, from, to, edge, charset)
    {
        return Some(plan);
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
        } => plan_left_right_grid_path_route_with_options(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            request.charset,
            GridRouteOptions::with_fixed_ports(Port::Right, Port::Left)
                .with_segment(PlannedRouteSegment::Boundary)
                .with_first_vertical_transit_label(),
        ),
        EdgeBoundaryContext::Leaving {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => plan_left_right_grid_path_route_with_options(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            request.charset,
            GridRouteOptions::with_fixed_ports(Port::Right, Port::Right)
                .with_segment(PlannedRouteSegment::Boundary)
                .with_last_vertical_transit_label(),
        ),
        EdgeBoundaryContext::Entering {
            group_id,
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::TopDown,
        } if request.edge.to == group_id => {
            plan_top_down_side_entry_route(request.from, request.to, request.edge, request.charset)
        }
        EdgeBoundaryContext::External { .. }
        | EdgeBoundaryContext::Internal { .. }
        | EdgeBoundaryContext::Entering { .. }
        | EdgeBoundaryContext::Leaving { .. } => None,
    }
}

fn unsupported_reason(
    boundary: EdgeBoundaryContext<'_>,
    request: EdgeRouteRequest<'_>,
) -> UnsupportedEdgeRouteReason {
    match boundary {
        EdgeBoundaryContext::Entering {
            group_id,
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::TopDown,
        } if request.edge.to == group_id => UnsupportedEdgeRouteReason::NoRouteFamily,
        EdgeBoundaryContext::Entering {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        }
        | EdgeBoundaryContext::Leaving {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => UnsupportedEdgeRouteReason::NoRouteFamily,
        EdgeBoundaryContext::Entering { .. } | EdgeBoundaryContext::Leaving { .. } => {
            UnsupportedEdgeRouteReason::BoundaryDirection
        }
        EdgeBoundaryContext::External { .. } | EdgeBoundaryContext::Internal { .. } => {
            UnsupportedEdgeRouteReason::NoRouteFamily
        }
    }
}

fn has_self_loop(edges: &[AsciiGraphEdge], node_id: &str) -> bool {
    edges
        .iter()
        .any(|edge| edge.from == node_id && edge.to == node_id)
}
