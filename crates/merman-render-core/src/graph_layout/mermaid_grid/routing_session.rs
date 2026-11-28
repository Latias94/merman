//! Grid route commitment in plan order with one topology and search budget per invocation.

use super::geometry::reserve;
use super::port_plan::{
    GridEndpointCandidate, GridPortPlan, GridRoutePorts, has_ancestor_endpoint, prepare_grid_ports,
};
use super::prepared::{GridRoutedEdge, PreparedGrid};
use super::route_plan::{GridEndpoint, GridRoutePlan, allocation_error, side_index};
use super::routing::{
    GridPort, GridSegment, grid_item_attachment, map_grid_corridor, route_grid_corridor,
};
use super::routing_context::{
    GridOccupiedSpan, GridRoutingBase, GridRoutingBases, GridRoutingContainers, grid_occupied_spans,
};
use super::sparse_segment::hierarchy::{
    GridHierarchyError, GridHierarchyOccupancy, GridHierarchyOptions, GridHierarchyRoute,
    GridLoopInput, GridLoopRoute, check_grid_hierarchy_compatibility, route_grid_hierarchy_edge,
    route_grid_self_loop,
};
use super::sparse_segment::{
    GridSegmentCaps, GridSegmentEndpoint, GridSegmentError, GridSegmentTopology,
    GridSegmentWorkspace, GridSiblingRoute, GridSiblingSegment, route_grid_sibling_segment,
};
use crate::graph_layout::Axis;
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use kurbo::Point;
use merman_model::OperationPhase;

const OVERLAP_PLAN_LIMIT: usize = 64;
const STRICT_BUNDLE_LIMIT: usize = 8;

impl PreparedGrid<'_> {
    /// Routes measured edges and positions their labels in original edge occurrence order.
    pub fn route_edges(&self, work: &OperationWorkMeter) -> Result<Vec<GridRoutedEdge>> {
        let mut routes = self.route_unlabelled_edges(work)?;
        self.position_labels(&mut routes, work)?;
        work.checkpoint(OperationPhase::Layout)?;
        Ok(routes)
    }

    #[cfg(test)]
    pub(super) fn route_sibling_edges(&self, work: &OperationWorkMeter) -> Result<Vec<Vec<Point>>> {
        // Older preparation tests inspect the unlabelled routing output.
        Ok(self
            .route_unlabelled_edges(work)?
            .into_iter()
            .map(|route| route.points)
            .collect())
    }

    fn route_unlabelled_edges(&self, work: &OperationWorkMeter) -> Result<Vec<GridRoutedEdge>> {
        work.checkpoint(OperationPhase::Layout)?;
        let ports = prepare_grid_ports(&self.forest, &self.geometry, &self.plans, work)?;
        let containers = GridRoutingContainers::build(&self.forest, &self.geometry, work)?;
        let overlap = self.plans.len() <= OVERLAP_PLAN_LIMIT;
        let mut committed = CommittedRoutes::new(self, work)?;
        prepare_fast_routes(self, &ports, &containers, &mut committed, work)?;
        let (modes, requested) =
            prepare_modes(self, &ports, &containers, &committed, overlap, work)?;
        let bases =
            GridRoutingBases::build(&self.forest, &self.geometry, &containers, &requested, work)?;
        let mut session = RouteSession {
            prepared: self,
            ports: &ports,
            containers: &containers,
            bases: &bases,
            modes: &modes,
            overlap,
            committed,
            owner_sides: owner_side_counts(self, &ports, work)?,
            loop_sides: empty_slots(self.forest.items.len(), work)?,
            workspace: GridSegmentWorkspace::default(),
            work,
        };
        session.route_bundles()?;
        session.committed.into_output(self, work)
    }
}

#[derive(Clone, Copy, Default)]
enum RouteMode {
    #[default]
    Sibling,
    Hierarchy {
        sparse_lca: bool,
        sparse_interiors: bool,
    },
    Loop,
}

fn prepare_fast_routes(
    prepared: &PreparedGrid<'_>,
    ports: &GridPortPlan,
    containers: &GridRoutingContainers<'_>,
    committed: &mut CommittedRoutes,
    work: &OperationWorkMeter,
) -> Result<()> {
    let mut occupancy: Vec<Vec<GridSegment>> =
        empty_slots(prepared.geometry.containers.len(), work)?;
    for (slot, (plan, route_ports)) in prepared.plans.iter().zip(&ports.routes).enumerate() {
        work.charge(1)?;
        if !route_ports.fast_eligible {
            continue;
        }
        let view = containers.container(plan.container)?.view();
        let source_ports = ports_as_points(&route_ports.sources, work)?;
        let target_ports = ports_as_points(&route_ports.targets, work)?;
        let source = item_port(
            containers.local_slot(plan.edge.source)?,
            plan.source.chain[0],
            route_ports.source_coordinates[0],
            view.children,
            &source_ports,
        );
        let target = item_port(
            containers.local_slot(plan.edge.target)?,
            plan.target.chain[0],
            route_ports.target_coordinates[0],
            view.children,
            &target_ports,
        );
        let occupied = &mut occupancy[plan.container];
        if let Some(route) =
            route_grid_corridor(view, source, target, occupied, prepared.plans.len(), work)?
        {
            let lca = if route.occupancy.is_empty() {
                LcaRoute::Unindexed
            } else {
                LcaRoute::Whole
            };
            work.charge(route.occupancy.len())?;
            reserve(occupied, route.occupancy.len())?;
            occupied.extend(route.occupancy);
            committed.publish(
                slot,
                plan,
                Routed {
                    points: route.points,
                    lca,
                },
                work,
            )?;
        }
    }
    Ok(())
}

fn prepare_modes(
    prepared: &PreparedGrid<'_>,
    ports: &GridPortPlan,
    containers: &GridRoutingContainers<'_>,
    committed: &CommittedRoutes,
    overlap: bool,
    work: &OperationWorkMeter,
) -> Result<(Vec<RouteMode>, Vec<usize>)> {
    let mut modes = empty_slots(prepared.plans.len(), work)?;
    let mut requested = Vec::new();
    let mut planning: Vec<Vec<(usize, Vec<GridOccupiedSpan>)>> = if overlap {
        empty_slots(prepared.geometry.containers.len(), work)?
    } else {
        Vec::new()
    };
    for &slot in &ports.ordered_routes {
        work.charge(1)?;
        if committed.routes[slot].is_some() {
            continue;
        }
        let plan = &prepared.plans[slot];
        let edge = plan.edge;
        let mode = if edge.source == edge.target {
            RouteMode::Loop
        } else if ports.routes[slot].eligible {
            RouteMode::Sibling
        } else {
            let ancestor = has_ancestor_endpoint(plan);
            let isolated = [edge.source, edge.target]
                .iter()
                .all(|&owner| ports.endpoint_incident_counts[owner] == plan.lane.count);
            let sparse_interiors = if plan.lane.count > 1 || isolated {
                true
            } else if let Some(lca) =
                check_grid_hierarchy_compatibility(prepared, ports, containers, slot, work)?
            {
                overlap
                    && !ancestor
                    && plan_occupancy_conflict(
                        &mut planning[plan.container],
                        plan.lane.pair,
                        &lca,
                        work,
                    )?
            } else {
                true
            };
            RouteMode::Hierarchy {
                sparse_lca: sparse_interiors || ancestor,
                sparse_interiors,
            }
        };
        modes[slot] = mode;
        if !matches!(
            mode,
            RouteMode::Hierarchy {
                sparse_lca: false,
                ..
            }
        ) {
            let count = plan
                .source
                .chain
                .len()
                .checked_add(plan.target.chain.len())
                .and_then(|count| count.checked_sub(1))
                .ok_or_else(|| work.arithmetic_overflow())?;
            work.charge(count)?;
            requested
                .try_reserve(count)
                .map_err(|_| allocation_error())?;
            requested.push(plan.container);
            requested.extend(
                plan.source.chain[1..]
                    .iter()
                    .chain(&plan.target.chain[1..])
                    .map(|end| end.owner),
            );
        }
    }
    Ok((modes, requested))
}

fn plan_occupancy_conflict(
    prior: &mut Vec<(usize, Vec<GridOccupiedSpan>)>,
    pair: usize,
    points: &[Point],
    work: &OperationWorkMeter,
) -> Result<bool> {
    let candidate = grid_occupied_spans(points, work)?;
    if candidate.is_empty() {
        return Ok(false);
    }
    for (other_pair, segments) in prior.iter() {
        work.charge(1)?;
        if *other_pair == pair {
            continue;
        }
        for &a in &candidate {
            for &b in segments {
                work.charge(1)?;
                if a.conflicts(b) {
                    return Ok(true);
                }
            }
        }
    }
    prior.try_reserve(1).map_err(|_| allocation_error())?;
    prior.push((pair, candidate));
    Ok(false)
}

fn owner_side_counts(
    prepared: &PreparedGrid<'_>,
    ports: &GridPortPlan,
    work: &OperationWorkMeter,
) -> Result<Vec<[usize; 4]>> {
    let mut counts: Vec<[usize; 4]> = empty_slots(prepared.forest.items.len(), work)?;
    for &slot in &ports.ordered_routes {
        work.charge(1)?;
        let plan = &prepared.plans[slot];
        if plan.edge.source == plan.edge.target {
            continue;
        }
        // The endpoint plan owns demands by occurrence, end and chain index. Each owner
        // appears once in a chain, including when a group is also an authored endpoint.
        for end in plan.source.chain.iter().chain(&plan.target.chain) {
            work.charge(1)?;
            let count = &mut counts[end.owner][side_index(end.side)];
            *count = count
                .checked_add(1)
                .ok_or_else(|| work.arithmetic_overflow())?;
        }
    }
    Ok(counts)
}

struct Routed {
    points: Vec<Point>,
    lca: LcaRoute,
}

enum LcaRoute {
    Unindexed,
    Whole,
    Segment(Vec<Point>),
}

impl Routed {
    fn occupancy(&self) -> Option<&[Point]> {
        match &self.lca {
            LcaRoute::Unindexed => None,
            LcaRoute::Whole => Some(&self.points),
            LcaRoute::Segment(points) => Some(points),
        }
    }
}

struct CommittedRoutes {
    routes: Vec<Option<Routed>>,
    occupancy: Vec<Vec<usize>>,
}

impl CommittedRoutes {
    fn new(prepared: &PreparedGrid<'_>, work: &OperationWorkMeter) -> Result<Self> {
        Ok(Self {
            routes: empty_slots(prepared.plans.len(), work)?,
            occupancy: empty_slots(prepared.geometry.containers.len(), work)?,
        })
    }

    fn publish(
        &mut self,
        slot: usize,
        plan: &GridRoutePlan<'_>,
        routed: Routed,
        work: &OperationWorkMeter,
    ) -> Result<()> {
        work.charge(1)?;
        if routed.occupancy().is_some() {
            let occupied = &mut self.occupancy[plan.container];
            occupied.try_reserve(1).map_err(|_| allocation_error())?;
            occupied.push(slot);
        }
        self.routes[slot] = Some(routed);
        Ok(())
    }

    fn foreign<'a>(
        &'a self,
        plans: &[GridRoutePlan<'_>],
        plan: &GridRoutePlan<'_>,
        work: &OperationWorkMeter,
    ) -> Result<Vec<&'a [Point]>> {
        let slots = &self.occupancy[plan.container];
        work.charge(slots.len())?;
        let mut routes = Vec::new();
        reserve(&mut routes, slots.len())?;
        for &slot in slots {
            work.charge(1)?;
            if plans[slot].lane.pair != plan.lane.pair {
                let points = self.routes[slot]
                    .as_ref()
                    .and_then(Routed::occupancy)
                    .ok_or_else(|| route_error(&plans[slot], "Missing committed LCA occupancy"))?;
                routes.push(points);
            }
        }
        Ok(routes)
    }

    fn into_output(
        self,
        prepared: &PreparedGrid<'_>,
        work: &OperationWorkMeter,
    ) -> Result<Vec<GridRoutedEdge>> {
        let mut output = Vec::new();
        work.charge(self.routes.len())?;
        reserve(&mut output, self.routes.len())?;
        for (plan, route) in prepared.plans.iter().zip(self.routes) {
            work.charge(1)?;
            let route = route.ok_or_else(|| route_error(plan, "Missing committed Grid route"))?;
            output.push(GridRoutedEdge {
                points: route.points,
                label_center: None,
            });
        }
        work.checkpoint(OperationPhase::Layout)?;
        Ok(output)
    }
}

struct RouteSession<'a> {
    prepared: &'a PreparedGrid<'a>,
    ports: &'a GridPortPlan,
    containers: &'a GridRoutingContainers<'a>,
    bases: &'a GridRoutingBases<'a>,
    modes: &'a [RouteMode],
    overlap: bool,
    committed: CommittedRoutes,
    owner_sides: Vec<[usize; 4]>,
    loop_sides: Vec<[usize; 4]>,
    workspace: GridSegmentWorkspace,
    work: &'a OperationWorkMeter,
}

impl RouteSession<'_> {
    fn route_bundles(&mut self) -> Result<()> {
        let work = self.work;
        let plans = &self.prepared.plans;
        let mut pairs: Vec<Vec<usize>> = empty_slots(plans.len(), work)?;
        let mut order = Vec::new();
        reserve(&mut order, plans.len())?;
        for &slot in &self.ports.ordered_routes {
            work.charge(1)?;
            let pair = plans[slot].lane.pair;
            if pairs[pair].is_empty() {
                order.push(pair);
            }
            pairs[pair].try_reserve(1).map_err(|_| allocation_error())?;
            pairs[pair].push(slot);
        }
        let mut staged = Vec::new();
        for pair in order {
            work.charge(1)?;
            let slots = &pairs[pair];
            work.charge(slots.len())?;
            reserve(&mut staged, slots.len())?;
            let strict = (2..=STRICT_BUNDLE_LIMIT).contains(&slots.len())
                && slots
                    .iter()
                    .all(|&slot| !matches!(self.modes[slot], RouteMode::Loop));
            let mut attempt = self.try_bundle(slots, !strict, &mut staged);
            if strict && matches!(attempt, Err((_, GridHierarchyError::PairSeparation))) {
                // Only the bundle's tentative routes were mutated. Keep the invocation's
                // expanded-state budget and work debits when discarding this attempt.
                staged.clear();
                attempt = self.try_bundle(slots, true, &mut staged);
            }
            if let Err((slot, error)) = attempt {
                return Err(match error {
                    GridHierarchyError::Layout(error) => error,
                    error => route_error(&plans[slot], &error.to_string()),
                });
            }
            for (slot, route) in staged.drain(..) {
                self.committed.publish(slot, &plans[slot], route, work)?;
            }
        }
        Ok(())
    }

    fn try_bundle(
        &mut self,
        slots: &[usize],
        allow_relaxation: bool,
        staged: &mut Vec<(usize, Routed)>,
    ) -> std::result::Result<(), (usize, GridHierarchyError)> {
        for &slot in slots {
            self.work
                .charge(1)
                .map_err(|error| (slot, GridHierarchyError::from(error)))?;
            if self.committed.routes[slot].is_some() {
                continue;
            }
            let route = self
                .route_one(slot, allow_relaxation, staged)
                .map_err(|error| (slot, error))?;
            staged.push((slot, route));
        }
        Ok(())
    }

    fn route_one(
        &mut self,
        slot: usize,
        allow_relaxation: bool,
        staged: &[(usize, Routed)],
    ) -> std::result::Result<Routed, GridHierarchyError> {
        let work = self.work;
        let plan = &self.prepared.plans[slot];
        let mode = self.modes[slot];
        let mut same_pair = Vec::new();
        work.charge(staged.len())?;
        reserve(&mut same_pair, staged.len())?;
        let mut indexed_segments = !self.committed.occupancy[plan.container].is_empty();
        for (previous, route) in staged {
            work.charge(1)?;
            same_pair.push(route.points.as_slice());
            indexed_segments |= self.prepared.plans[*previous].container == plan.container
                && route.occupancy().is_some();
        }
        let caps = GridSegmentCaps {
            retained_base_bytes: self.bases.retained_bytes,
            ..GridSegmentCaps::default()
        };
        match mode {
            RouteMode::Loop => {
                let owner = plan.edge.source;
                let GridLoopRoute {
                    points,
                    side,
                    index,
                    kind: _kind,
                } = route_grid_self_loop(
                    self.prepared,
                    self.containers,
                    self.bases,
                    slot,
                    GridLoopInput {
                        owner_side_counts: self.owner_sides[owner],
                        committed_loop_counts: self.loop_sides[owner],
                        committed_pair_routes: &same_pair,
                    },
                    caps,
                    &mut self.workspace,
                    work,
                )?;
                self.loop_sides[owner][side_index(side)] = index
                    .checked_add(1)
                    .ok_or_else(|| work.arithmetic_overflow())?;
                Ok(Routed {
                    points,
                    lca: LcaRoute::Unindexed,
                })
            }
            RouteMode::Sibling => {
                let foreign = self.committed.foreign(&self.prepared.plans, plan, work)?;
                let points = route_sibling(
                    plan,
                    &self.ports.routes[slot],
                    self.containers,
                    self.bases,
                    &same_pair,
                    &foreign,
                    caps,
                    &mut self.workspace,
                    work,
                )?;
                let lca = if grid_occupied_spans(&points, work)?.is_empty() {
                    LcaRoute::Unindexed
                } else {
                    LcaRoute::Whole
                };
                Ok(Routed { points, lca })
            }
            RouteMode::Hierarchy {
                sparse_lca,
                sparse_interiors,
            } => {
                let avoid_overlap = self.overlap && sparse_lca;
                let foreign = if avoid_overlap {
                    self.committed.foreign(&self.prepared.plans, plan, work)?
                } else {
                    Vec::new()
                };
                let GridHierarchyRoute {
                    points,
                    lca_points,
                    segment_kinds: _kinds,
                    portal_selections: _selections,
                    relaxed_pair_separation: _relaxed,
                } = route_grid_hierarchy_edge(
                    self.prepared,
                    self.ports,
                    self.containers,
                    self.bases,
                    slot,
                    GridHierarchyOptions {
                        sparse_lca,
                        sparse_interiors,
                        committed_pair_routes: &same_pair,
                        lca_occupancy: avoid_overlap.then_some(GridHierarchyOccupancy {
                            indexed_segments,
                            foreign_routes: &foreign,
                        }),
                        allow_hierarchy_relaxation: allow_relaxation,
                    },
                    caps,
                    &mut self.workspace,
                    work,
                )?;
                let lca = if grid_occupied_spans(&lca_points, work)?.is_empty() {
                    LcaRoute::Unindexed
                } else {
                    LcaRoute::Segment(lca_points)
                };
                Ok(Routed { points, lca })
            }
        }
    }
}

fn empty_slots<T: Default>(len: usize, work: &OperationWorkMeter) -> Result<Vec<T>> {
    work.charge(len)?;
    let mut slots = Vec::new();
    reserve(&mut slots, len)?;
    slots.resize_with(len, T::default);
    Ok(slots)
}

#[allow(
    clippy::too_many_arguments,
    reason = "The segment borrows one operation's topology, occupancy and work controls."
)]
fn route_sibling(
    plan: &GridRoutePlan<'_>,
    ports: &GridRoutePorts,
    containers: &GridRoutingContainers<'_>,
    bases: &GridRoutingBases<'_>,
    same_pair: &[&[Point]],
    foreign: &[&[Point]],
    caps: GridSegmentCaps,
    workspace: &mut GridSegmentWorkspace,
    work: &OperationWorkMeter,
) -> Result<Vec<Point>> {
    let view = containers.container(plan.container)?.view();
    let source_child = containers.local_slot(plan.edge.source)?;
    let target_child = containers.local_slot(plan.edge.target)?;
    let sources = segment_endpoints(&ports.sources, work)?;
    let targets = segment_endpoints(&ports.targets, work)?;
    let base = match bases.bases[plan.container].as_ref() {
        Some(GridRoutingBase::Sparse(base)) => GridSegmentTopology::Sparse(base),
        Some(GridRoutingBase::Compatibility(limit)) => GridSegmentTopology::Compatibility(*limit),
        None => return Err(route_error(plan, "Missing sparse topology")),
    };
    let result = route_grid_sibling_segment(
        base,
        GridSiblingSegment {
            sources: &sources,
            targets: &targets,
            children: view.children,
            source_child,
            target_child,
            bundle_size: plan.lane.count,
            prioritize_assigned_ports: ports.coordinated,
            committed_pair_routes: same_pair,
            foreign_routes: foreign,
        },
        caps,
        workspace,
        work,
        |work| {
            let source = item_port(
                source_child,
                plan.source.chain[0],
                ports.source_coordinates[0],
                view.children,
                &[],
            );
            let target = item_port(
                target_child,
                plan.target.chain[0],
                ports.target_coordinates[0],
                view.children,
                &[],
            );
            let start = grid_item_attachment(view, source)?;
            let end = grid_item_attachment(view, target)?;
            map_grid_corridor(view, start, end, plan.lane.index, work).map(Some)
        },
    );
    match result {
        Ok(
            GridSiblingRoute::Sparse { points, .. }
            | GridSiblingRoute::Compatibility { points, .. },
        ) => Ok(points),
        Err(GridSegmentError::Layout(error)) => Err(error),
        Err(error) => Err(route_error(plan, &error.to_string())),
    }
}

fn item_port<'a>(
    child: usize,
    owner: GridEndpoint,
    coordinate: Option<f64>,
    children: &[super::routing::GridRoutingChild],
    candidates: &'a [Point],
) -> GridPort<'a> {
    let center = children[child].bounds.center();
    GridPort {
        child,
        side: owner.side,
        coordinate: coordinate.unwrap_or(match owner.side.axis() {
            Axis::Horizontal => center.y,
            Axis::Vertical => center.x,
        }),
        candidates,
    }
}

fn ports_as_points(
    candidates: &[GridEndpointCandidate],
    work: &OperationWorkMeter,
) -> Result<Vec<Point>> {
    let mut points = Vec::new();
    work.charge(candidates.len())?;
    reserve(&mut points, candidates.len())?;
    points.extend(
        candidates
            .iter()
            .map(|candidate| Point::new(candidate.port.x, candidate.port.y)),
    );
    Ok(points)
}

fn segment_endpoints(
    candidates: &[GridEndpointCandidate],
    work: &OperationWorkMeter,
) -> Result<Vec<GridSegmentEndpoint>> {
    let mut endpoints = Vec::new();
    work.charge(candidates.len())?;
    reserve(&mut endpoints, candidates.len())?;
    endpoints.extend(candidates.iter().map(|candidate| GridSegmentEndpoint {
        port: Point::new(candidate.port.x, candidate.port.y),
        connect: Point::new(candidate.connect.x, candidate.connect.y),
        side: candidate.side,
        rank: candidate.rank,
        on_assigned_side: candidate.on_assigned_side.unwrap_or(true),
    }));
    Ok(endpoints)
}

fn route_error(plan: &GridRoutePlan<'_>, reason: &str) -> Error {
    Error::InvalidModel {
        message: format!("GRID_ROUTE_NOT_FOUND: {reason} for \"{}\"", plan.edge.id),
    }
}
