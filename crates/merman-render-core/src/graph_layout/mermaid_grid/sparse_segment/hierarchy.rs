//! Mermaid 12.4 Grid hierarchy segments and self-loops.

use super::super::port_plan::GridPortPlan;
use super::super::prepared::PreparedGrid;
use super::super::route_plan::{GridEndpointPlan, GridRoutePlan, parent};
use super::super::routing::{GridRoutingView, map_grid_corridor};
use super::super::routing_context::{GridRoutingBase, GridRoutingBases, GridRoutingContainers};
use super::constraints::{Constraints, RouteGeometry, normalize_points};
use super::{
    GridSegmentCaps, GridSegmentLimit, GridSegmentTopology, GridSegmentWorkspace, invalid, reserved,
};
use crate::Error;
use crate::resources::{OperationWorkError, OperationWorkMeter};
use kurbo::Point;
use merman_model::OperationPhase;

mod attachments;
mod loops;
mod segment;

use attachments::{AttachmentMode, EndpointRef, Terminal};
pub(in super::super) use loops::{GridLoopInput, GridLoopRoute, route_grid_self_loop};

#[derive(Clone, Copy)]
pub(in super::super) struct GridHierarchyOccupancy<'a> {
    /// True if the index contains segments, including segments of this pair.
    pub indexed_segments: bool,
    pub foreign_routes: &'a [&'a [Point]],
}

#[derive(Clone, Copy)]
pub(in super::super) struct GridHierarchyOptions<'a> {
    pub sparse_lca: bool,
    pub sparse_interiors: bool,
    pub committed_pair_routes: &'a [&'a [Point]],
    pub lca_occupancy: Option<GridHierarchyOccupancy<'a>>,
    /// False on the first pass of a strict bundle retry.
    pub allow_hierarchy_relaxation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum GridHierarchyEnd {
    Source,
    Target,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in super::super) struct GridPortalSelection {
    pub end: GridHierarchyEnd,
    pub chain_index: usize,
    pub coordinate: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum GridHierarchySegmentKind {
    Direct,
    Sparse,
    LabelAwareLoop,
    Compatibility,
    CompatibilityRecovery,
    ResourceFallback(GridSegmentLimit),
}

#[derive(Debug)]
pub(in super::super) struct GridHierarchyRoute {
    pub points: Vec<Point>,
    pub lca_points: Vec<Point>,
    /// LCA, source ascents, and target ascents in execution order.
    pub segment_kinds: Vec<GridHierarchySegmentKind>,
    /// Successful alternative portals, staged in the order first selected.
    pub portal_selections: Vec<GridPortalSelection>,
    pub relaxed_pair_separation: bool,
}

#[derive(Debug, thiserror::Error)]
pub(in super::super) enum GridHierarchyError {
    #[error(transparent)]
    Layout(#[from] Error),
    #[error("Missing Grid routing topology in container slot {container}")]
    MissingTopology { container: usize },
    #[error("Grid self-loop endpoint projection is missing in container slot {container}")]
    MissingEndpointProjection { container: usize },
    #[error("Invalid Grid compatibility route in container slot {container}")]
    InvalidCompatibility { container: usize },
    #[error("Invalid Grid compatibility route in container slot {container} after {limit}")]
    InvalidResourceFallback {
        container: usize,
        limit: GridSegmentLimit,
    },
    #[error("No Grid hierarchy route in container slot {container}")]
    NoHierarchyRoute { container: usize },
    #[error("No obstacle-clear compatibility self-loop for Grid item slot {owner}")]
    NoCompatibilityLoop { owner: usize },
    #[error("No distinct self-loop route for Grid item slot {owner}")]
    NoSelfLoop { owner: usize },
    #[error("Grid hierarchy route requires relaxed pair separation")]
    PairSeparation,
    #[error("No distinct Grid bundle route")]
    NoDistinctRoute,
}

impl From<OperationWorkError> for GridHierarchyError {
    fn from(error: OperationWorkError) -> Self {
        Self::Layout(error.into())
    }
}

/// Validates the compatibility chain during mode preparation. The LCA points can be used to
/// check planning occupancy before any sparse bases are built.
pub(in super::super) fn check_grid_hierarchy_compatibility(
    prepared: &PreparedGrid<'_>,
    ports: &GridPortPlan,
    containers: &GridRoutingContainers<'_>,
    route_slot: usize,
    work: &OperationWorkMeter,
) -> crate::Result<Option<Vec<Point>>> {
    work.checkpoint(OperationPhase::Layout)?;
    let plan = HierarchyPlan::bind(prepared, ports, route_slot)?;
    let context = AttachmentContext {
        prepared,
        containers,
    };
    let compatibility = |segment: PlannedSegment<'_>| -> crate::Result<_> {
        let view = containers.container(segment.container)?.view();
        let [a, b] = segment.attachments(context, view, &[], work)?;
        let geometry = context.geometry(view, segment.container, [a.owner, b.owner])?;
        let constraints = Constraints::from_routes(geometry, None, None, work)?;
        let points = map_grid_corridor(
            view,
            a.corridor(),
            b.corridor(),
            plan.route.lane.index,
            work,
        )?;
        Ok(constraints
            .clears_geometry(&points, work)?
            .then_some(points))
    };
    for endpoint in plan.ends {
        for segment in ascents(endpoint, false) {
            if compatibility(segment)?.is_none() {
                return Ok(None);
            }
        }
    }
    let lca = compatibility(plan.lca(false))?;
    work.checkpoint(OperationPhase::Layout)?;
    Ok(lca)
}

#[allow(
    clippy::too_many_arguments,
    reason = "A route borrows its session's inputs, state and meter."
)]
pub(in super::super) fn route_grid_hierarchy_edge(
    prepared: &PreparedGrid<'_>,
    ports: &GridPortPlan,
    containers: &GridRoutingContainers<'_>,
    bases: &GridRoutingBases<'_>,
    route_slot: usize,
    options: GridHierarchyOptions<'_>,
    caps: GridSegmentCaps,
    workspace: &mut GridSegmentWorkspace,
    work: &OperationWorkMeter,
) -> Result<GridHierarchyRoute, GridHierarchyError> {
    work.checkpoint(OperationPhase::Layout)?;
    let plan = HierarchyPlan::bind(prepared, ports, route_slot)?;
    let [source, target] = plan.ends;
    let count = source
        .plan
        .chain
        .len()
        .checked_add(target.plan.chain.len())
        .ok_or_else(|| work.arithmetic_overflow())?;
    let mut route = EdgeRouting {
        context: AttachmentContext {
            prepared,
            containers,
        },
        bases,
        options,
        caps: GridSegmentCaps {
            retained_base_bytes: bases.retained_bytes,
            ..caps
        },
        variant: plan.route.lane.index,
        lane_offset: plan.route.lane.offset,
        bundle_size: plan.route.lane.count,
        workspace,
        work,
        selections: reserved(count, work)?,
        kinds: reserved(count - 1, work)?,
    };

    // The LCA is routed first so its portal selections inform the following ascents.
    let lca = route.segment(
        plan.lca(options.sparse_lca),
        options.sparse_lca,
        options.lca_occupancy,
    )?;
    let source_segments = route.ascent(source)?;
    let target_segments = route.ascent(target)?;
    let points = combine(source_segments, &lca, target_segments, work)?;

    let mut relaxed_pair_separation = false;
    if plan.route.lane.count > 1 {
        let pairs = Constraints::from_routes(
            RouteGeometry {
                children: &[],
                terminals: [None; 2],
                require_approaches: false,
            },
            Some(options.committed_pair_routes),
            None,
            work,
        )?;
        if !pairs.respects_pair(&points, false, work)? {
            if count <= 2 || !pairs.respects_pair(&points, true, work)? {
                return Err(GridHierarchyError::NoDistinctRoute);
            }
            if !options.allow_hierarchy_relaxation {
                return Err(GridHierarchyError::PairSeparation);
            }
            relaxed_pair_separation = true;
        }
    }
    work.checkpoint(OperationPhase::Layout)?;
    Ok(GridHierarchyRoute {
        points,
        lca_points: lca,
        segment_kinds: route.kinds,
        portal_selections: route.selections,
        relaxed_pair_separation,
    })
}

#[derive(Clone, Copy)]
struct AttachmentContext<'a> {
    prepared: &'a PreparedGrid<'a>,
    containers: &'a GridRoutingContainers<'a>,
}

impl AttachmentContext<'_> {
    fn child(self, container: usize, owner: usize) -> crate::Result<Option<usize>> {
        if parent(&self.prepared.forest, owner) == Some(container) {
            return self.containers.local_slot(owner).map(Some);
        }
        Ok(None)
    }

    fn geometry<'a>(
        self,
        view: GridRoutingView<'a>,
        container: usize,
        [start, end]: [usize; 2],
    ) -> crate::Result<RouteGeometry<'a>> {
        Ok(RouteGeometry {
            children: view.children,
            terminals: [self.child(container, start)?, self.child(container, end)?],
            require_approaches: false,
        })
    }
}

struct HierarchyPlan<'a> {
    route: &'a GridRoutePlan<'a>,
    ends: [EndpointRef<'a>; 2],
}

impl<'a> HierarchyPlan<'a> {
    fn bind(
        prepared: &'a PreparedGrid<'a>,
        ports: &'a GridPortPlan,
        slot: usize,
    ) -> crate::Result<Self> {
        let route = prepared
            .plans
            .get(slot)
            .ok_or_else(|| invalid("Missing Grid route slot"))?;
        let ports = ports
            .routes
            .get(slot)
            .ok_or_else(|| invalid("Missing Grid route ports"))?;
        let ends = [
            EndpointRef {
                end: GridHierarchyEnd::Source,
                plan: &route.source,
                coordinates: &ports.source_coordinates,
            },
            EndpointRef {
                end: GridHierarchyEnd::Target,
                plan: &route.target,
                coordinates: &ports.target_coordinates,
            },
        ];
        if route.edge.source == route.edge.target
            || ends.iter().any(|end| {
                end.plan.chain.is_empty() || end.plan.chain.len() != end.coordinates.len()
            })
        {
            return Err(invalid("Invalid Grid hierarchy endpoint plan"));
        }
        Ok(Self { route, ends })
    }

    fn lca(&self, sparse: bool) -> PlannedSegment<'a> {
        let mode = |endpoint: &GridEndpointPlan| {
            if endpoint.boundary {
                if sparse {
                    AttachmentMode::InteriorEndpoint
                } else {
                    AttachmentMode::CompatibilityBoundary
                }
            } else if sparse && endpoint.chain.len() > 1 {
                AttachmentMode::ExteriorPortal
            } else {
                AttachmentMode::Item
            }
        };
        PlannedSegment {
            container: self.route.container,
            endpoints: self.ends,
            indices: self.ends.map(|endpoint| endpoint.plan.chain.len() - 1),
            modes: self.ends.map(|endpoint| mode(endpoint.plan)),
            start_retry: StartRetry::Portal,
        }
    }
}

#[derive(Clone, Copy)]
struct PlannedSegment<'a> {
    container: usize,
    endpoints: [EndpointRef<'a>; 2],
    indices: [usize; 2],
    modes: [AttachmentMode; 2],
    start_retry: StartRetry,
}

#[derive(Clone, Copy)]
enum StartRetry {
    Portal,
    Item,
    None,
}

impl PlannedSegment<'_> {
    fn attachments(
        self,
        context: AttachmentContext<'_>,
        view: GridRoutingView<'_>,
        selections: &[GridPortalSelection],
        work: &OperationWorkMeter,
    ) -> crate::Result<[Terminal; 2]> {
        let terminal = |index| attachments::resolve(context, view, &self, index, selections, work);
        Ok([terminal(0)?, terminal(1)?])
    }
}

fn ascents(endpoint: EndpointRef<'_>, sparse: bool) -> impl Iterator<Item = PlannedSegment<'_>> {
    (0..endpoint.plan.chain.len() - 1).map(move |index| PlannedSegment {
        container: endpoint.plan.chain[index + 1].owner,
        endpoints: [endpoint; 2],
        indices: [index, index + 1],
        modes: [
            if sparse && index > 0 {
                AttachmentMode::ExteriorPortal
            } else {
                AttachmentMode::Item
            },
            if sparse {
                AttachmentMode::InteriorPortal
            } else {
                AttachmentMode::CompatibilityBoundary
            },
        ],
        start_retry: if index == 0 {
            StartRetry::Item
        } else {
            StartRetry::None
        },
    })
}

struct EdgeRouting<'context, 'a> {
    context: AttachmentContext<'context>,
    bases: &'context GridRoutingBases<'context>,
    options: GridHierarchyOptions<'a>,
    caps: GridSegmentCaps,
    variant: usize,
    lane_offset: f64,
    bundle_size: usize,
    workspace: &'a mut GridSegmentWorkspace,
    work: &'a OperationWorkMeter,
    selections: Vec<GridPortalSelection>,
    kinds: Vec<GridHierarchySegmentKind>,
}

impl EdgeRouting<'_, '_> {
    fn ascent(&mut self, endpoint: EndpointRef<'_>) -> Result<Vec<Vec<Point>>, GridHierarchyError> {
        let mut result = reserved(endpoint.plan.chain.len() - 1, self.work)?;
        let sparse = self.options.sparse_interiors;
        for segment in ascents(endpoint, sparse) {
            result.push(self.segment(segment, sparse, None)?);
        }
        Ok(result)
    }

    fn segment(
        &mut self,
        plan: PlannedSegment<'_>,
        sparse: bool,
        occupancy: Option<GridHierarchyOccupancy<'_>>,
    ) -> Result<Vec<Point>, GridHierarchyError> {
        let work = self.work;
        let container = plan.container;
        let view = self.context.containers.container(container)?.view();
        let terminals = plan.attachments(self.context, view, &self.selections, work)?;
        let geometry =
            self.context
                .geometry(view, container, terminals.map(|terminal| terminal.owner))?;
        let topology = sparse
            .then(|| topology(self.bases, container))
            .transpose()?;
        let alternates = |index: usize| -> crate::Result<Vec<Terminal>> {
            let endpoint = plan.endpoints[index];
            let chain_index = plan.indices[index];
            let mode = plan.modes[index];
            match (index == 0, plan.start_retry) {
                (false, _) | (true, StartRetry::Portal) if mode.is_portal() => {
                    attachments::portal_alternatives(
                        self.context,
                        endpoint,
                        chain_index,
                        mode,
                        &self.selections,
                        work,
                    )
                }
                (true, StartRetry::Item) => attachments::item_alternatives(
                    self.context,
                    view,
                    container,
                    endpoint,
                    chain_index,
                    self.lane_offset,
                    work,
                ),
                _ => Ok(Vec::new()),
            }
        };
        let result = segment::route(
            segment::Input {
                container,
                view,
                geometry,
                topology,
                terminals,
                variant: self.variant,
                allow_shared_pair_corridors: self.bundle_size > 1,
                pair_routes: self.options.committed_pair_routes,
                occupancy,
            },
            self.caps,
            self.workspace,
            work,
            || Ok([alternates(0)?, alternates(1)?]),
        )?;
        for selection in result.selections.into_iter().flatten() {
            work.charge(self.selections.len())?;
            if let Some(prior) = self.selections.iter_mut().find(|prior| {
                prior.end == selection.end && prior.chain_index == selection.chain_index
            }) {
                *prior = selection;
            } else {
                self.selections.push(selection);
            }
        }
        self.kinds.push(result.kind);
        Ok(result.points)
    }
}

fn topology<'a>(
    bases: &'a GridRoutingBases<'a>,
    container: usize,
) -> Result<GridSegmentTopology<'a, 'a>, GridHierarchyError> {
    match bases.bases.get(container).and_then(Option::as_ref) {
        Some(GridRoutingBase::Sparse(base)) => Ok(GridSegmentTopology::Sparse(base)),
        Some(GridRoutingBase::Compatibility(limit)) => {
            Ok(GridSegmentTopology::Compatibility(*limit))
        }
        None => Err(GridHierarchyError::MissingTopology { container }),
    }
}

fn combine(
    source: Vec<Vec<Point>>,
    lca: &[Point],
    target: Vec<Vec<Point>>,
    work: &OperationWorkMeter,
) -> crate::Result<Vec<Point>> {
    let chains = source.iter().chain(&target);
    let count = chains
        .map(Vec::len)
        .try_fold(lca.len(), usize::checked_add)
        .ok_or_else(|| work.arithmetic_overflow())?;
    let mut points = reserved(count, work)?;
    let mut add = |point: Point| -> crate::Result<()> {
        work.charge(1)?;
        let near =
            |prior: Point| (prior.x - point.x).abs() <= 1e-6 && (prior.y - point.y).abs() <= 1e-6;
        if !points.last().copied().is_some_and(near) {
            points.push(point);
        }
        Ok(())
    };
    for point in source.into_iter().flatten().chain(lca.iter().copied()) {
        add(point)?;
    }
    for chain in target.into_iter().rev() {
        for point in chain.into_iter().rev() {
            add(point)?;
        }
    }
    normalize_points(&points, work)
}
