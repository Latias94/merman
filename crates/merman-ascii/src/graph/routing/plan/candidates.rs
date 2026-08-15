use super::RoutePlan;
use super::boundary::{EdgeBoundaryContext, edge_boundary_context_with_resources};
use super::compound::{
    plan_axis_aligned_compound_endpoint_route_with_resources,
    plan_compound_endpoint_route_with_resources,
};
use super::edges::parallel_edge_index;
use super::grid::{
    GridRouteOptions, plan_left_right_grid_path_route_with_options_resources_and_execution,
};
use super::left_right::plan_left_right_self_loop_route_with_resources;
use super::same_rank::plan_same_rank_bottom_lane_route_with_index_and_resources;
use super::select::{
    EdgeRoutePlan, EdgeRouteRequest, UnsupportedEdgeRoute, plan_edge_route_with_topology,
};
use crate::error::{AsciiError, Result};
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::graph::routing::path::Port;
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::operation::AsciiExecution;
use crate::resource::AsciiResourceLimitPhase;
use crate::resource::ResourceContext;

const ADDITIONAL_LANE_CANDIDATES: usize = 4;

pub(in crate::graph::routing) enum EdgeRouteCandidates {
    Routed(Vec<RoutePlan>),
    Unsupported(UnsupportedEdgeRoute),
}

pub(in crate::graph::routing) fn plan_edge_route_candidates_with_topology(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<EdgeRouteCandidates> {
    let primary =
        match plan_edge_route_with_topology(request, topology, label, resources, execution)? {
            EdgeRoutePlan::Routed(plan) => plan,
            EdgeRoutePlan::Unsupported(route) => {
                return Ok(EdgeRouteCandidates::Unsupported(route));
            }
        };
    let mut candidates = Vec::new();
    candidates
        .try_reserve(1 + ADDITIONAL_LANE_CANDIDATES + PORT_PAIRS.len())
        .map_err(|_| layout_allocation_failed())?;
    candidates.push(primary);

    let parallel_index = parallel_edge_index(request.edges, request.edge_index);
    if request.from.id == request.to.id {
        for offset in 1..=ADDITIONAL_LANE_CANDIDATES {
            let lane_index = resources.checked_work_add(parallel_index, offset)?;
            if let Some(plan) = plan_left_right_self_loop_route_with_resources(
                &request.graph_layout.nodes,
                request.edges,
                request.from,
                request.edge,
                lane_index,
                label,
                request.charset,
                resources,
            )? {
                push_unique_candidate(&mut candidates, plan, resources)?;
            }
        }
        return Ok(EdgeRouteCandidates::Routed(candidates));
    }

    if endpoint_is_group(request, topology)
        && let Some(plan) = plan_axis_aligned_compound_endpoint_route_with_resources(
            request.from,
            request.to,
            request.edge,
            label,
            request.charset,
            resources,
        )?
    {
        push_unique_candidate(&mut candidates, plan, resources)?;
    }

    for offset in 0..ADDITIONAL_LANE_CANDIDATES {
        let lane_index = resources.checked_work_add(parallel_index, offset)?;
        if request.from.center_y() == request.to.center_y()
            && let Some(plan) = plan_same_rank_bottom_lane_route_with_index_and_resources(
                request.from,
                request.to,
                request.edge,
                lane_index,
                label,
                request.charset,
                resources,
            )?
        {
            push_unique_candidate(&mut candidates, plan, resources)?;
        }
        if let Some(plan) = plan_compound_endpoint_route_with_resources(
            request.from,
            request.to,
            request.edge,
            lane_index,
            label,
            request.charset,
            resources,
        )? {
            push_unique_candidate(&mut candidates, plan, resources)?;
        }
    }

    let boundary =
        edge_boundary_context_with_resources(request.graph, request.edge, topology, resources)?;
    if endpoints_are_nodes(request, topology) && boundary_stays_within_one_scope(boundary) {
        for (start, end) in PORT_PAIRS {
            if let Some(plan) =
                plan_left_right_grid_path_route_with_options_resources_and_execution(
                    request.graph_layout,
                    request.from,
                    request.to,
                    request.edge,
                    label,
                    request.charset,
                    GridRouteOptions::with_fixed_ports(start, end),
                    resources,
                    execution,
                )?
            {
                push_unique_candidate(&mut candidates, plan, resources)?;
            }
        }
    }

    Ok(EdgeRouteCandidates::Routed(candidates))
}

const PORT_PAIRS: [(Port, Port); 8] = [
    (Port::Right, Port::Left),
    (Port::Left, Port::Right),
    (Port::Down, Port::Up),
    (Port::Up, Port::Down),
    (Port::Right, Port::Right),
    (Port::Left, Port::Left),
    (Port::Down, Port::Down),
    (Port::Up, Port::Up),
];

fn endpoints_are_nodes(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
) -> bool {
    let Some(topology) = topology else {
        return true;
    };
    matches!(
        topology.endpoint_index(&request.edge.from),
        Some(GraphEndpointIndex::Node(_))
    ) && matches!(
        topology.endpoint_index(&request.edge.to),
        Some(GraphEndpointIndex::Node(_))
    )
}

fn endpoint_is_group(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
) -> bool {
    let Some(topology) = topology else {
        return false;
    };
    matches!(
        topology.endpoint_index(&request.edge.from),
        Some(GraphEndpointIndex::Group(_))
    ) || matches!(
        topology.endpoint_index(&request.edge.to),
        Some(GraphEndpointIndex::Group(_))
    )
}

fn boundary_stays_within_one_scope(boundary: EdgeBoundaryContext<'_>) -> bool {
    matches!(
        boundary,
        EdgeBoundaryContext::External { .. } | EdgeBoundaryContext::Internal { .. }
    )
}

fn push_unique_candidate(
    candidates: &mut Vec<RoutePlan>,
    candidate: RoutePlan,
    resources: &mut ResourceContext,
) -> Result<()> {
    let candidate_items =
        resources.checked_work_add(candidate.cells.len(), candidate.labels.len())?;
    let mut comparison_work = 0usize;
    for existing in candidates.iter() {
        comparison_work = resources.checked_work_add(
            comparison_work,
            resources.checked_work_add(
                resources.checked_work_add(existing.cells.len(), existing.labels.len())?,
                candidate_items,
            )?,
        )?;
    }
    resources.charge_layout_work(comparison_work.max(1))?;
    if candidates.iter().any(|existing| existing == &candidate) {
        return Ok(());
    }
    candidates
        .try_reserve(1)
        .map_err(|_| layout_allocation_failed())?;
    candidates.push(candidate);
    Ok(())
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}
