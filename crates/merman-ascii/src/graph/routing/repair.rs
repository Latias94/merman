use super::conflict::BlockerWitnesses;
use super::label::{RoutedLabelCatalogPlan, RoutedLabelDescriptor};
use super::occupancy::SceneOccupancy;
use super::path::RouteObstacles;
use super::plan::{EdgeRouteRequest, MarkerEndpoint, RoutePlan};
use super::terminal::{self, SearchBudget};
use super::{
    PreparedRoute, RouteCandidateScore, RouteOwner, admit_route_candidate,
    allocate_committed_route_labels, allocate_committed_route_marker_berths, endpoint_layout,
};
use crate::error::{AsciiError, Result};
use crate::graph::topology::GraphGroupTopology;
use crate::operation::{AsciiExecution, GraphPlanningBudget};
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod witness_tests;

// A scene shares both allowances. A failed proposal consumes its attempt and search work.
const MAX_REORDER_ATTEMPTS: usize = 4;

enum RepairSearch {
    Missing,
    Displaced,
}

pub(super) struct SceneRepairBudget<'budget> {
    pub(super) expansions: usize,
    attempts: usize,
    shared: Option<&'budget GraphPlanningBudget>,
}

impl Default for SceneRepairBudget<'_> {
    fn default() -> Self {
        Self {
            expansions: terminal::MAX_REPAIR_EXPANSIONS,
            attempts: MAX_REORDER_ATTEMPTS,
            shared: None,
        }
    }
}

impl<'budget> SceneRepairBudget<'budget> {
    pub(super) fn for_execution(execution: AsciiExecution<'budget>) -> Self {
        match execution.graph_planning() {
            Some(shared) => Self {
                expansions: shared.replacement.remaining().0,
                attempts: shared.repair_attempts.get(),
                shared: Some(shared),
            },
            None => Self::default(),
        }
    }
}

impl Drop for SceneRepairBudget<'_> {
    fn drop(&mut self) {
        if let Some(shared) = self.shared {
            shared.repair_attempts.set(self.attempts);
        }
    }
}

/// Give a blocked edge priority over a bounded conflict set. The original occupancy
/// remains live until every replacement, its markers, and its labels are valid.
#[allow(clippy::too_many_arguments)]
pub(super) fn repair_blocked_scene(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    labels: &RoutedLabelCatalogPlan<'_>,
    owner: &RouteOwner,
    witnesses: &BlockerWitnesses,
    routes: &mut Vec<PreparedRoute>,
    occupancy: &mut SceneOccupancy<'_>,
    budget: &mut SceneRepairBudget<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<bool> {
    let mut start_work = resources.layout_work_used();
    let initial_allowance = budget.shared.map(|shared| shared.replacement.remaining().1);
    if budget.attempts == 0 || routes.is_empty() {
        return Ok(false);
    }
    let original_len = routes.len();
    let mut conflicts = Vec::new();
    conflicts
        .try_reserve_exact(witnesses.blockers.len())
        .map_err(|_| super::layout_allocation_failed())?;
    for blocker in &witnesses.blockers {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(routes.len())?;
        if routes
            .iter()
            .any(|route| route.owner.canonical_edge_index == blocker.canonical_edge_index)
        {
            conflicts.push(blocker.canonical_edge_index);
        }
    }
    let mut try_closure = true;
    let mut single_cursor = 0;
    while budget.attempts != 0 && !conflicts.is_empty() {
        if let (Some(shared), Some(initial)) = (budget.shared, initial_allowance) {
            shared
                .replacement
                .account_scene_work(start_work, initial, resources);
            if shared.replacement.remaining().1 == 0 {
                break;
            }
        }
        execution.checkpoint(OperationPhase::Layout)?;
        let selected = if try_closure {
            try_closure = false;
            &conflicts[..]
        } else {
            if single_cursor == conflicts.len() || conflicts.len() == 1 {
                break;
            }
            single_cursor += 1;
            &conflicts[single_cursor - 1..single_cursor]
        };
        let mut omitted = conflict_closure(routes, selected, resources)?;
        omitted.sort_unstable();
        let omitted = omitted.as_slice();
        let count = omitted.len();
        resources.charge_layout_work(count)?;
        let mut displaced = Vec::new();
        displaced
            .try_reserve_exact(count)
            .map_err(|_| super::layout_allocation_failed())?;
        budget.attempts -= 1;
        let mut trial = occupancy.rebuild_for_routes(
            routes,
            omitted,
            resources,
            request.graph.diagram_type(),
        )?;
        trial.begin_blocker_collection(resources)?;
        for &index in omitted.iter().rev() {
            displaced.push((index, routes.remove(index)));
        }
        displaced.reverse();
        let result = (|| {
            let feasibility_start = resources.layout_work_used();
            let feasibility_credit = budget.shared.map(|shared| shared.feasibility.remaining().1);
            let selected = select_route(
                request,
                topology,
                labels.descriptor(request.edge_index),
                owner,
                routes,
                &trial,
                budget,
                RepairSearch::Missing,
                resources,
                execution,
            );
            if let (Some(shared), Some(initial)) = (budget.shared, feasibility_credit) {
                shared
                    .feasibility
                    .account_scene_work(feasibility_start, initial, resources);
            }
            start_work = start_work.saturating_add(
                resources
                    .layout_work_used()
                    .saturating_sub(feasibility_start),
            );
            let Some(plan) = selected? else {
                return Ok(false);
            };
            append_route(request, owner, plan, routes, &mut trial, resources)?;
            for (_, displaced) in &displaced {
                let edge_index = displaced.owner.canonical_edge_index;
                let edge = &request.edges[edge_index];
                let Some(from) = endpoint_layout(request.graph_layout, &edge.from, request.charset)
                else {
                    return Ok(false);
                };
                let Some(to) = endpoint_layout(request.graph_layout, &edge.to, request.charset)
                else {
                    return Ok(false);
                };
                let displaced_request = EdgeRouteRequest {
                    from: &from,
                    to: &to,
                    edge_index,
                    edge,
                    ..request
                };
                let mut displaced_owner = displaced.owner.try_clone(resources)?;
                displaced_owner.allocated_branch = None;
                let Some(plan) = select_route(
                    displaced_request,
                    topology,
                    labels.descriptor(edge_index),
                    &displaced_owner,
                    routes,
                    &trial,
                    budget,
                    RepairSearch::Displaced,
                    resources,
                    execution,
                )?
                else {
                    return Ok(false);
                };
                append_route(
                    displaced_request,
                    &displaced_owner,
                    plan,
                    routes,
                    &mut trial,
                    resources,
                )?;
            }
            execution.checkpoint(OperationPhase::Layout)?;
            let valid = super::validate::first_invalid_retained_route(
                request.graph,
                request.graph_layout,
                request.edges,
                routes,
                labels,
                request.charset,
                resources,
                execution,
            )?
            .is_none();
            if valid {
                trial.take_blocker_collection(resources)?;
            }
            Ok(valid)
        })();
        if matches!(result, Ok(true)) {
            if let (Some(shared), Some(initial)) = (budget.shared, initial_allowance) {
                shared
                    .replacement
                    .account_scene_work(start_work, initial, resources);
            }
            *occupancy = trial;
            return Ok(true);
        }
        // The original plans and occupancy were retained. No replay, allocation, or
        // cancellation checkpoint is needed to roll back an unsuccessful proposal.
        routes.truncate(original_len - count);
        for (index, route) in displaced {
            routes.insert(index, route);
        }
        match result {
            Ok(false) | Err(AsciiError::RoutingSearchExhausted { .. }) => {
                if let Some(additional) = trial.take_blocker_collection(resources)? {
                    for blocker in additional.blockers {
                        resources.charge_layout_work(
                            resources.checked_work_add(routes.len(), conflicts.len())?,
                        )?;
                        if !conflicts.contains(&blocker.canonical_edge_index)
                            && conflicts.len() < super::conflict::MAX_BLOCKER_WITNESSES
                            && routes.iter().any(|route| {
                                route.owner.canonical_edge_index == blocker.canonical_edge_index
                            })
                        {
                            conflicts
                                .try_reserve(1)
                                .map_err(|_| super::layout_allocation_failed())?;
                            conflicts.push(blocker.canonical_edge_index);
                            try_closure = true;
                        }
                    }
                    conflicts.sort_unstable();
                }
            }
            Err(error) => return Err(error),
            Ok(true) => unreachable!("successful proposals return before rollback"),
        }
    }
    if let (Some(shared), Some(initial)) = (budget.shared, initial_allowance) {
        shared
            .replacement
            .account_scene_work(start_work, initial, resources);
    }
    Ok(false)
}

fn conflict_closure(
    routes: &[PreparedRoute],
    canonical_indices: &[usize],
    resources: &ResourceContext,
) -> Result<Vec<usize>> {
    let mut omitted = Vec::new();
    omitted
        .try_reserve_exact(routes.len())
        .map_err(|_| super::layout_allocation_failed())?;
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(canonical_indices.len())
        .map_err(|_| super::layout_allocation_failed())?;
    for route in routes {
        resources.charge_layout_work(canonical_indices.len())?;
        if canonical_indices.contains(&route.owner.canonical_edge_index)
            && let Some(group) = route.owner.shared_channel_group()
        {
            groups.push(group);
        }
    }
    for (index, route) in routes.iter().enumerate() {
        resources.checkpoint()?;
        resources.charge_layout_work(
            resources.checked_work_add(canonical_indices.len(), groups.len())?,
        )?;
        if canonical_indices.contains(&route.owner.canonical_edge_index)
            || route
                .owner
                .shared_channel_group()
                .is_some_and(|group| groups.contains(&group))
        {
            omitted.push(index);
        }
    }
    Ok(omitted)
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn reorder_failed_route(
    request: EdgeRouteRequest<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    labels: &RoutedLabelCatalogPlan<'_>,
    owner: &RouteOwner,
    routes: &mut Vec<PreparedRoute>,
    occupancy: &mut SceneOccupancy<'_>,
    budget: &mut SceneRepairBudget<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<bool> {
    occupancy.begin_blocker_collection(resources)?;
    let _ = select_route(
        request,
        topology,
        labels.descriptor(request.edge_index),
        owner,
        routes,
        occupancy,
        budget,
        RepairSearch::Missing,
        resources,
        execution,
    )?;
    let witnesses = occupancy
        .take_blocker_collection(resources)?
        .unwrap_or_default();
    repair_blocked_scene(
        request, topology, labels, owner, &witnesses, routes, occupancy, budget, resources,
        execution,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_route(
    request: EdgeRouteRequest<'_>,
    _topology: Option<&GraphGroupTopology<'_>>,
    label: Option<RoutedLabelDescriptor>,
    owner: &RouteOwner,
    routes: &[PreparedRoute],
    occupancy: &SceneOccupancy<'_>,
    budget: &mut SceneRepairBudget<'_>,
    search: RepairSearch,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<RoutePlan>> {
    let obstacles = RouteObstacles {
        scene: occupancy,
        routes,
        owner,
    };
    if let Some(branch) = owner.allocated_branch {
        let Some(plan) =
            super::plan_allocated_channel(request, &branch, label, resources, execution)?
        else {
            return Ok(None);
        };
        // A branch cannot leave a joint channel while retaining its sharing identity.
        // If replaying that channel fails, reject the whole reorder proposal.
        return Ok(
            admit_route_candidate(request, obstacles, plan, resources)?.map(|(_, plan)| plan)
        );
    }
    let forward = super::forward_route_direction(request, resources)?;
    let mut best: Option<(RouteCandidateScore, RoutePlan)> = None;
    if request.edge.from == request.edge.to {
        for plan in terminal::plan_self_loop_candidates(request, label, resources, execution)? {
            if let Some(candidate) = admit_route_candidate(request, obstacles, plan, resources)?
                && best.as_ref().is_none_or(|(score, _)| candidate.0 < *score)
            {
                best = Some(candidate);
            }
        }
    }
    if let Some(candidate) =
        terminal::plan_direct_route(request, obstacles, label, resources, execution)?
        && best.as_ref().is_none_or(|(score, plan)| {
            super::route_candidate_is_better(
                forward,
                (candidate.0, candidate.1.attachments),
                (*score, plan.attachments),
            )
        })
    {
        best = Some(candidate);
    }
    if best
        .as_ref()
        .is_none_or(|(score, plan)| super::route_needs_search(forward, *score, plan, false))
        && let Some(candidate) = terminal::plan_terminal_route_with_budget(
            request,
            obstacles,
            label,
            best.as_ref()
                .map(|(score, plan)| (*score, plan.attachments)),
            match budget.shared {
                Some(shared) => match search {
                    RepairSearch::Missing => SearchBudget::Limited(&shared.feasibility),
                    RepairSearch::Displaced => SearchBudget::Replacement(&shared.replacement),
                },
                None => SearchBudget::Optional(&mut budget.expansions),
            },
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
        owner: owner.try_clone(resources)?,
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
        routes,
        index,
        occupancy,
        resources,
        request.graph.diagram_type(),
    )
}
