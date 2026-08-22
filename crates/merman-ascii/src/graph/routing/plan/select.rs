use super::super::super::charset::GraphCharset;
use super::super::super::layout::{GraphLayout, NodeLayout};
use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
use super::super::super::topology::GraphGroupTopology;
use super::super::path::Port;
use super::PlannedRouteSegment;
use super::RoutePlan;
pub(super) use super::boundary::EdgeBoundaryContext;
#[cfg(test)]
pub(super) use super::boundary::edge_boundary_context;
use super::boundary::edge_boundary_context_with_resources;
use super::compound::plan_compound_endpoint_route_with_resources;
use super::edges::parallel_edge_index;
use super::grid::{
    GridRouteOptions, plan_left_right_grid_path_route_with_options_resources_and_execution,
};
use super::left_right::{
    plan_left_right_down_route_with_resources,
    plan_left_right_down_then_right_route_with_resources,
    plan_left_right_reverse_over_self_loop_route_with_resources,
    plan_left_right_right_then_up_route_with_resources,
    plan_left_right_self_loop_route_with_resources,
};
use super::same_rank::{
    plan_same_rank_bottom_lane_route_with_index_and_resources,
    plan_same_rank_bottom_lane_route_with_resources, plan_same_rank_direct_route_with_resources,
};
use super::top_down::{
    plan_top_down_back_route_with_resources, plan_top_down_bent_route_with_resources,
    plan_top_down_direct_route_with_resources, plan_top_down_side_entry_route_with_resources,
};
use crate::error::Result;
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::graph::topology::GraphEndpointIndex;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;

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

#[cfg(test)]
pub(in crate::graph::routing) fn plan_edge_route(request: EdgeRouteRequest<'_>) -> EdgeRoutePlan {
    let mut resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ));
    let label = request
        .edge
        .label
        .as_deref()
        .and_then(|raw| RoutedLabelDescriptor::for_test(0, raw, request.charset.width_profile));
    let planned = plan_edge_route_with_resources(request, label, &mut resources)
        .expect("test route planning work must remain representable");
    match planned {
        EdgeRoutePlan::Routed(plan) => EdgeRoutePlan::Routed(
            plan.with_markers(
                request.edge.start_marker,
                request.edge.end_marker,
                request.charset,
                request.graph.diagram_type(),
            )
            .expect("test endpoint markers must fit the planned route"),
        ),
        EdgeRoutePlan::Unsupported(route) => EdgeRoutePlan::Unsupported(route),
    }
}

#[cfg(test)]
pub(in crate::graph::routing) fn plan_edge_route_with_resources(
    request: EdgeRouteRequest<'_>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
) -> Result<EdgeRoutePlan> {
    let topology = if request.graph.groups.is_empty() {
        None
    } else {
        Some(GraphGroupTopology::try_new(request.graph, resources)?)
    };
    let policy = resources.policy();
    plan_edge_route_with_topology(
        request,
        topology.as_ref(),
        label,
        resources,
        AsciiExecution::for_test(&policy),
    )
}

pub(in crate::graph::routing) fn plan_edge_route_with_topology(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<EdgeRoutePlan> {
    let boundary =
        edge_boundary_context_with_resources(request.graph, request.edge, topology, resources)?;
    if let Some(topology) = topology {
        let from_is_group = matches!(
            topology.endpoint_index(&request.edge.from),
            Some(GraphEndpointIndex::Group(_))
        );
        let to_is_group = matches!(
            topology.endpoint_index(&request.edge.to),
            Some(GraphEndpointIndex::Group(_))
        );
        if (from_is_group || to_is_group)
            && let Some(plan) = plan_compound_endpoint_route_with_resources(
                request.from,
                request.to,
                request.edge,
                parallel_edge_index(request.edges, request.edge_index),
                label,
                request.charset,
                resources,
            )?
        {
            return Ok(EdgeRoutePlan::Routed(plan));
        }
    }
    if let Some(plan) = plan_boundary_route(boundary, request, label, resources, execution)? {
        return Ok(EdgeRoutePlan::Routed(plan));
    }

    let plan = match boundary.direction().canonical() {
        GraphDirection::LeftRight => plan_left_right_route(request, label, resources, execution)?,
        GraphDirection::TopDown => plan_top_down_route(request, label, resources, execution)?,
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    };
    let plan = plan.map(|plan| match boundary {
        EdgeBoundaryContext::Entering { .. } | EdgeBoundaryContext::Leaving { .. } => {
            plan.with_segment(PlannedRouteSegment::Boundary)
        }
        EdgeBoundaryContext::External { .. } | EdgeBoundaryContext::Internal { .. } => plan,
    });

    Ok(match plan {
        Some(plan) => EdgeRoutePlan::Routed(plan),
        None => EdgeRoutePlan::Unsupported(UnsupportedEdgeRoute::new(unsupported_reason(
            boundary, request,
        ))),
    })
}

fn plan_left_right_route(
    request: EdgeRouteRequest<'_>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<RoutePlan>> {
    let graph_layout = request.graph_layout;
    let from = request.from;
    let to = request.to;
    let edge = request.edge;
    let charset = request.charset;
    let parallel_index = parallel_edge_index(request.edges, request.edge_index);

    if from.id == to.id {
        return plan_left_right_self_loop_route_with_resources(
            &graph_layout.nodes,
            request.edges,
            from,
            edge,
            parallel_index,
            label,
            charset,
            resources,
        );
    }

    if parallel_index > 0 {
        if from.center_y() == to.center_y() {
            return plan_same_rank_bottom_lane_route_with_index_and_resources(
                from,
                to,
                edge,
                parallel_index - 1,
                label,
                charset,
                resources,
            );
        }
        return plan_compound_endpoint_route_with_resources(
            from,
            to,
            edge,
            parallel_index - 1,
            label,
            charset,
            resources,
        );
    }

    if from.center_y() == to.center_y() && from.x > to.x {
        if has_self_loop(request.edges, &to.id) {
            return plan_left_right_reverse_over_self_loop_route_with_resources(
                &graph_layout.nodes,
                from,
                to,
                edge,
                label,
                charset,
                resources,
            );
        }
        return plan_same_rank_bottom_lane_route_with_index_and_resources(
            from,
            to,
            edge,
            parallel_index,
            label,
            charset,
            resources,
        );
    }

    if from.center_y() == to.center_y()
        && from.x < to.x
        && let Some(plan) = plan_same_rank_direct_route_with_resources(
            &graph_layout.nodes,
            from,
            to,
            edge,
            label,
            charset,
            resources,
        )?
    {
        return Ok(Some(plan));
    }

    if let Some(plan) = plan_left_right_grid_path_route_with_options_resources_and_execution(
        graph_layout,
        from,
        to,
        edge,
        label,
        charset,
        GridRouteOptions::direct(),
        resources,
        execution,
    )? {
        return Ok(Some(plan));
    }

    if from.center_y() < to.center_y() && to.x > from.x {
        return plan_left_right_down_then_right_route_with_resources(
            &graph_layout.nodes,
            request.edges,
            from,
            to,
            edge,
            label,
            charset,
            resources,
        );
    }

    if from.center_y() < to.center_y() && to.x == from.x {
        return plan_left_right_down_route_with_resources(
            from, to, edge, label, charset, resources,
        );
    }

    if from.center_y() > to.center_y() && to.x > from.x {
        return plan_left_right_right_then_up_route_with_resources(
            &graph_layout.nodes,
            request.edges,
            from,
            to,
            edge,
            label,
            charset,
            resources,
        );
    }

    Ok(None)
}

fn plan_top_down_route(
    request: EdgeRouteRequest<'_>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<RoutePlan>> {
    let from = request.from;
    let to = request.to;
    let edge = request.edge;
    let charset = request.charset;
    let parallel_index = parallel_edge_index(request.edges, request.edge_index);

    if from.id == to.id {
        return plan_left_right_self_loop_route_with_resources(
            &request.graph_layout.nodes,
            request.edges,
            from,
            edge,
            parallel_index,
            label,
            charset,
            resources,
        );
    }

    if parallel_index > 0 {
        if from.center_y() == to.center_y() {
            return plan_same_rank_bottom_lane_route_with_index_and_resources(
                from,
                to,
                edge,
                parallel_index - 1,
                label,
                charset,
                resources,
            );
        }
        return plan_compound_endpoint_route_with_resources(
            from,
            to,
            edge,
            parallel_index - 1,
            label,
            charset,
            resources,
        );
    }

    if from.center_y() > to.center_y() {
        return plan_top_down_back_route_with_resources(from, to, edge, label, charset, resources);
    }

    if from.center_y() == to.center_y() {
        if let Some(plan) = plan_same_rank_direct_route_with_resources(
            &request.graph_layout.nodes,
            from,
            to,
            edge,
            label,
            charset,
            resources,
        )? {
            return Ok(Some(plan));
        }

        // Same-rank edges use their natural horizontal ports when the span is clear. If another
        // node blocks that span, reuse the shared bottom lane instead of reporting the edge as
        // unroutable.
        return plan_same_rank_bottom_lane_route_with_resources(
            from, to, edge, label, charset, resources,
        );
    }

    if top_down_skips_occupied_rank(&request.graph_layout.nodes, from, to, resources)? {
        return plan_left_right_grid_path_route_with_options_resources_and_execution(
            request.graph_layout,
            from,
            to,
            edge,
            label,
            charset,
            GridRouteOptions::with_fixed_ports(Port::Right, Port::Right),
            resources,
            execution,
        );
    }

    if from.center_x() != to.center_x() {
        let source_reserves_bottom = source_has_direct_bottom_exit(request);
        let target_reserves_top = target_has_direct_top_entry(request);
        if source_reserves_bottom || target_reserves_top {
            let (start_port, end_port) = match (source_reserves_bottom, target_reserves_top) {
                (true, true) => (Port::Right, Port::Right),
                (true, false) => (Port::Right, Port::Up),
                (false, true) => (Port::Down, Port::Right),
                (false, false) => unreachable!(),
            };
            return plan_left_right_grid_path_route_with_options_resources_and_execution(
                request.graph_layout,
                from,
                to,
                edge,
                label,
                charset,
                GridRouteOptions::with_fixed_ports(start_port, end_port),
                resources,
                execution,
            );
        }
        return plan_top_down_bent_route_with_resources(from, to, edge, label, charset, resources);
    }

    plan_top_down_direct_route_with_resources(from, to, edge, label, charset, resources)
}

fn top_down_skips_occupied_rank(
    nodes: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    resources: &mut ResourceContext,
) -> Result<bool> {
    resources.charge_layout_work(nodes.len())?;
    Ok(from.grid.y < to.grid.y
        && nodes.iter().any(|node| {
            node.id != from.id
                && node.id != to.id
                && from.grid.y < node.grid.y
                && node.grid.y < to.grid.y
        }))
}

fn source_has_direct_bottom_exit(request: EdgeRouteRequest<'_>) -> bool {
    request
        .edges
        .iter()
        .enumerate()
        .filter(|(index, edge)| {
            *index != request.edge_index && edge.stroke.is_visible() && edge.from == request.from.id
        })
        .any(|(_, edge)| {
            request
                .graph_layout
                .nodes
                .iter()
                .find(|layout| layout.id == edge.to)
                .is_some_and(|target| {
                    request.from.center_y() < target.center_y()
                        && request.from.center_x() == target.center_x()
                })
        })
}

fn target_has_direct_top_entry(request: EdgeRouteRequest<'_>) -> bool {
    request
        .edges
        .iter()
        .enumerate()
        .filter(|(index, edge)| {
            *index != request.edge_index && edge.stroke.is_visible() && edge.to == request.to.id
        })
        .any(|(_, edge)| {
            request
                .graph_layout
                .nodes
                .iter()
                .find(|layout| layout.id == edge.from)
                .is_some_and(|source| {
                    source.center_y() < request.to.center_y()
                        && source.center_x() == request.to.center_x()
                })
        })
}

fn plan_boundary_route(
    boundary: EdgeBoundaryContext<'_>,
    request: EdgeRouteRequest<'_>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<RoutePlan>> {
    match boundary {
        EdgeBoundaryContext::Entering {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => plan_left_right_grid_path_route_with_options_resources_and_execution(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            label,
            request.charset,
            GridRouteOptions::with_fixed_ports(Port::Right, Port::Left)
                .with_segment(PlannedRouteSegment::Boundary)
                .with_first_vertical_transit_label(),
            resources,
            execution,
        ),
        EdgeBoundaryContext::Leaving {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => plan_left_right_grid_path_route_with_options_resources_and_execution(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            label,
            request.charset,
            GridRouteOptions::with_fixed_ports(Port::Right, Port::Right)
                .with_segment(PlannedRouteSegment::Boundary)
                .with_last_vertical_transit_label(),
            resources,
            execution,
        ),
        EdgeBoundaryContext::Entering {
            group_id,
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::TopDown,
        } if request.edge.to == group_id => plan_top_down_side_entry_route_with_resources(
            request.from,
            request.to,
            request.edge,
            label,
            request.charset,
            resources,
        ),
        EdgeBoundaryContext::External { .. }
        | EdgeBoundaryContext::Internal { .. }
        | EdgeBoundaryContext::Entering { .. }
        | EdgeBoundaryContext::Leaving { .. } => Ok(None),
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
        .any(|edge| edge.stroke.is_visible() && edge.from == node_id && edge.to == node_id)
}
