use super::label::{RoutedLabelCatalogPlan, RoutedLabelDescriptor};
use super::occupancy::{OccupiedRect, SceneOccupancy};
use super::path::RouteObstacles;
use super::plan::{EdgeRouteRequest, MarkerEndpoint, RoutePlan};
use super::terminal::{self, SearchBudget};
use super::{
    PreparedRoute, RouteCandidateScore, RouteOwner, admit_route_candidate,
    allocate_committed_route_labels, allocate_committed_route_marker_berths, endpoint_layout,
    plan_edge_route_candidates_with_topology,
};
use crate::error::{AsciiError, Result};
use crate::graph::layout::ROUTING_GRID_MARGIN;
use crate::graph::topology::GraphGroupTopology;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

#[cfg(test)]
mod tests;

// A scene shares both allowances. A failed proposal consumes its attempt and search work.
const MAX_REORDER_ATTEMPTS: usize = 4;

pub(super) struct SceneRepairBudget {
    pub(super) expansions: usize,
    attempts: usize,
}

impl Default for SceneRepairBudget {
    fn default() -> Self {
        Self {
            expansions: terminal::MAX_REPAIR_EXPANSIONS,
            attempts: MAX_REORDER_ATTEMPTS,
        }
    }
}

/// Give a blocked edge priority over one earlier contributor. The original occupancy
/// remains live until both replacement routes, their markers, and their labels are valid.
#[allow(clippy::too_many_arguments)]
pub(super) fn reorder_failed_route(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    labels: &RoutedLabelCatalogPlan<'_>,
    owner: &RouteOwner,
    routes: &mut Vec<PreparedRoute>,
    occupancy: &mut SceneOccupancy<'_>,
    budget: &mut SceneRepairBudget,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<bool> {
    if budget.attempts == 0 || routes.is_empty() {
        return Ok(false);
    }
    let left = request
        .from
        .x
        .min(request.to.x)
        .saturating_sub(ROUTING_GRID_MARGIN);
    let top = request
        .from
        .y
        .min(request.to.y)
        .saturating_sub(ROUTING_GRID_MARGIN);
    let right = resources.checked_grid_add(
        request.from.right().max(request.to.right()),
        ROUTING_GRID_MARGIN + 1,
    )?;
    let bottom = resources.checked_grid_add(
        request.from.bottom().max(request.to.bottom()),
        ROUTING_GRID_MARGIN + 1,
    )?;
    let corridor = OccupiedRect::try_new(left, top, right - left, bottom - top, resources)?;
    let original_len = routes.len();
    for removed_index in (0..original_len).rev() {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if budget.attempts == 0 {
            break;
        }
        if !occupancy.route_bounds[removed_index].is_some_and(|bounds| bounds.intersects(corridor))
        {
            continue;
        }
        budget.attempts -= 1;
        let mut trial = occupancy.rebuild_for_routes(
            routes,
            Some(removed_index),
            resources,
            request.graph.diagram_type(),
        )?;
        let displaced = routes.remove(removed_index);
        let result = (|| {
            let Some(plan) = select_route(
                request,
                topology,
                labels.descriptor(request.edge_index),
                owner,
                routes,
                &trial,
                budget,
                resources,
                execution,
            )?
            else {
                return Ok(false);
            };
            append_route(request, owner, plan, routes, &mut trial, resources)?;
            let edge_index = displaced.owner.canonical_edge_index;
            let edge = &request.edges[edge_index];
            let Some(from) = endpoint_layout(request.graph_layout, &edge.from, request.charset)
            else {
                return Ok(false);
            };
            let Some(to) = endpoint_layout(request.graph_layout, &edge.to, request.charset) else {
                return Ok(false);
            };
            let displaced_request = EdgeRouteRequest {
                from: &from,
                to: &to,
                edge_index,
                edge,
                ..request
            };
            let Some(plan) = select_route(
                displaced_request,
                topology,
                labels.descriptor(edge_index),
                &displaced.owner,
                routes,
                &trial,
                budget,
                resources,
                execution,
            )?
            else {
                return Ok(false);
            };
            append_route(
                displaced_request,
                &displaced.owner,
                plan,
                routes,
                &mut trial,
                resources,
            )?;
            execution.checkpoint(OperationPhase::Layout)?;
            Ok(true)
        })();
        if matches!(result, Ok(true)) {
            *occupancy = trial;
            return Ok(true);
        }
        // The original plans and occupancy were retained. No replay, allocation, or
        // cancellation checkpoint is needed to roll back an unsuccessful proposal.
        routes.truncate(original_len - 1);
        routes.insert(removed_index, displaced);
        match result {
            Ok(false) | Err(AsciiError::UnsupportedFeature { .. }) => {}
            Err(error) => return Err(error),
            Ok(true) => unreachable!("successful proposals return before rollback"),
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
fn select_route(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    label: Option<RoutedLabelDescriptor>,
    owner: &RouteOwner,
    routes: &[PreparedRoute],
    occupancy: &SceneOccupancy<'_>,
    budget: &mut SceneRepairBudget,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<RoutePlan>> {
    let obstacles = RouteObstacles {
        scene: occupancy,
        routes,
        owner,
    };
    let mut best: Option<(RouteCandidateScore, RoutePlan)> = None;
    for plan in
        plan_edge_route_candidates_with_topology(request, topology, label, resources, execution)?
            .plans
    {
        if let Some(candidate) = admit_route_candidate(request, obstacles, plan, resources)?
            && best.as_ref().is_none_or(|(score, _)| candidate.0 < *score)
        {
            best = Some(candidate);
        }
    }
    if !request.graph_layout.grid_aligned
        && let Some(candidate) =
            terminal::plan_direct_route(request, obstacles, label, resources, execution)?
        && best.as_ref().is_none_or(|(score, _)| candidate.0 < *score)
    {
        best = Some(candidate);
    }
    if best
        .as_ref()
        .is_none_or(|(score, _)| score.has_congestion() || score.grows_scene())
        && let Some(candidate) = terminal::plan_terminal_route_with_budget(
            request,
            obstacles,
            label,
            best.as_ref().map(|(score, _)| *score),
            SearchBudget::Optional(&mut budget.expansions),
            resources,
            execution,
        )?
    {
        best = Some(candidate);
    }
    Ok(best.map(|(_, plan)| plan))
}

fn append_route(
    request: EdgeRouteRequest<'_>,
    owner: &RouteOwner,
    plan: RoutePlan,
    routes: &mut Vec<PreparedRoute>,
    occupancy: &mut SceneOccupancy<'_>,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(resources.checked_work_add(
        resources.checked_work_add(owner.from.len(), owner.to.len())?,
        owner.boundary_group_indices.len(),
    )?)?;
    let mut plan = plan.try_with_stroke(
        request.edge.stroke,
        request.charset,
        request.graph.diagram_type(),
    )?;
    occupancy.preserve_group_crossings(&mut plan, owner, resources)?;
    let start = plan.terminal_candidate(MarkerEndpoint::Start, request.graph.diagram_type())?;
    let end = plan.terminal_candidate(MarkerEndpoint::End, request.graph.diagram_type())?;
    let route = PreparedRoute {
        plan,
        owner: owner.clone(),
    };
    let index = routes.len();
    resources.charge_layout_work(route.plan.cells.len())?;
    occupancy
        .route_cells
        .try_reserve(route.plan.cells.len())
        .map_err(|_| super::layout_allocation_failed())?;
    occupancy.commit_route(index, &route, start, end, resources)?;
    routes.push(route);
    allocate_committed_route_marker_berths(
        routes,
        index,
        occupancy,
        request.charset,
        resources,
        request.graph.diagram_type(),
    )?;
    allocate_committed_route_labels(
        &mut routes[index],
        index,
        occupancy,
        resources,
        request.graph.diagram_type(),
    )
}
