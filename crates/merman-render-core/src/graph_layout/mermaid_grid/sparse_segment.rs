//! Candidate selection for a sibling Grid edge within one container.

use super::routing::GridRoutingChild;
use super::sparse_search::{
    GridRouteCost, GridSearchBudget, GridSearchCaps, GridSearchError, GridSearchLimit,
    GridSearchOptions, GridSearchQuery, GridSearchTerminal, GridSearchWorkspace,
    ValidatedGridSearchTopology, find_grid_sparse_route,
};
use super::sparse_topology::{
    GRID_ROUTING_ESTIMATED_BYTES, GridEndpointOverlayScratch, GridOverlayInput,
    GridRoutingTopology, GridTopologyError, GridTopologyLimit,
};
use crate::Error;
use crate::graph_layout::{Axis, Side};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use kurbo::Point;
use merman_model::OperationPhase;

mod constraints;
pub(super) mod hierarchy;
use constraints::{Constraints, attach_ports};

const APPROACH: f64 = 20.0;

#[derive(Clone, Copy, Debug)]
pub struct GridSegmentEndpoint {
    pub port: Point,
    pub connect: Point,
    pub side: Side,
    pub rank: usize,
    /// Set to true for candidates on the assigned side and owners without coordinated slots.
    pub on_assigned_side: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct GridSiblingSegment<'a> {
    pub sources: &'a [GridSegmentEndpoint],
    pub targets: &'a [GridSegmentEndpoint],
    pub children: &'a [GridRoutingChild],
    pub source_child: usize,
    pub target_child: usize,
    pub bundle_size: usize,
    pub prioritize_assigned_ports: bool,
    pub committed_pair_routes: &'a [&'a [Point]],
    /// Committed routes in this container with a different unordered endpoint pair.
    pub foreign_routes: &'a [&'a [Point]],
}

#[derive(Clone, Copy)]
pub enum GridSegmentTopology<'base, 'id> {
    Sparse(&'base GridRoutingTopology<'id>),
    Compatibility(GridTopologyLimit),
}

#[derive(Clone, Copy, Debug)]
pub struct GridSegmentCaps {
    /// Sum of all retained base estimates, including this segment's base. Zero uses this base.
    pub retained_base_bytes: usize,
    pub max_topology_bytes: usize,
    pub search: GridSearchCaps,
}

impl Default for GridSegmentCaps {
    fn default() -> Self {
        Self {
            retained_base_bytes: 0,
            max_topology_bytes: GRID_ROUTING_ESTIMATED_BYTES,
            search: GridSearchCaps::default(),
        }
    }
}

#[derive(Default)]
pub struct GridSegmentWorkspace {
    pub overlay: GridEndpointOverlayScratch,
    pub search: GridSearchWorkspace,
    pub budget: GridSearchBudget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GridSegmentLimit {
    #[error(transparent)]
    Topology(#[from] GridTopologyLimit),
    #[error(transparent)]
    Search(#[from] GridSearchLimit),
}

#[derive(Debug, thiserror::Error)]
pub enum GridSegmentError {
    #[error(transparent)]
    Layout(#[from] Error),
    #[error("No legal endpoint candidates for Grid edge")]
    NoEndpoints,
    #[error("No cell-aware route for Grid edge")]
    NoRoute,
    #[error("Invalid compatibility route for Grid edge after {0}")]
    InvalidCompatibility(GridSegmentLimit),
}

impl From<OperationWorkError> for GridSegmentError {
    fn from(value: OperationWorkError) -> Self {
        Self::Layout(value.into())
    }
}

#[derive(Debug, PartialEq)]
pub enum GridSiblingRoute {
    Sparse {
        points: Vec<Point>,
        cost: GridRouteCost,
    },
    Compatibility {
        points: Vec<Point>,
        reason: GridSegmentLimit,
    },
}

/// Route prepared item endpoints after the facade's correspondence and compatibility-fast pass.
/// `compatibility_route` supplies the ordinary corridor fallback, which is validated here.
pub fn route_grid_sibling_segment(
    topology: GridSegmentTopology<'_, '_>,
    input: GridSiblingSegment<'_>,
    caps: GridSegmentCaps,
    workspace: &mut GridSegmentWorkspace,
    work: &OperationWorkMeter,
    compatibility_route: impl FnOnce(&OperationWorkMeter) -> crate::Result<Option<Vec<Point>>>,
) -> Result<GridSiblingRoute, GridSegmentError> {
    work.checkpoint(OperationPhase::Layout)?;
    if input.source_child == input.target_child
        || input.source_child >= input.children.len()
        || input.target_child >= input.children.len()
    {
        return Err(invalid("Invalid Grid sibling route endpoints").into());
    }

    let base = match topology {
        GridSegmentTopology::Sparse(base) => base,
        GridSegmentTopology::Compatibility(limit) => {
            let constraints = Constraints::new(input, false, work)?;
            return try_compatibility(limit.into(), &constraints, work, compatibility_route);
        }
    };
    if input.sources.is_empty() || input.targets.is_empty() {
        return Err(GridSegmentError::NoEndpoints);
    }
    let pairs = ordered_pairs(input.sources, input.targets, work)?;
    let constraints = Constraints::new(input, true, work)?;
    let avoid_occupied = constraints.has_occupancy();
    let mut search = SegmentSearch {
        base,
        pairs: &pairs,
        constraints: &constraints,
        caps,
        workspace,
        work,
        bundled: input.bundle_size > 1,
        best: None,
        limit: None,
    };
    if input.prioritize_assigned_ports {
        search.run(CandidateSet::Preferred, avoid_occupied)?;
        search.run(CandidateSet::AssignedSide, avoid_occupied)?;
    }
    if search.best.is_none() {
        search.run(CandidateSet::All, avoid_occupied)?;
    }
    if search.best.is_none() && search.limit.is_none() && avoid_occupied {
        search.run(CandidateSet::All, false)?;
    }
    work.checkpoint(OperationPhase::Layout)?;
    if let Some(limit) = search.limit {
        return try_compatibility(limit, &constraints, work, compatibility_route);
    }
    let (points, cost) = search.best.ok_or(GridSegmentError::NoRoute)?;
    Ok(GridSiblingRoute::Sparse { points, cost })
}

fn try_compatibility(
    limit: GridSegmentLimit,
    constraints: &Constraints<'_>,
    work: &OperationWorkMeter,
    compatibility_route: impl FnOnce(&OperationWorkMeter) -> crate::Result<Option<Vec<Point>>>,
) -> Result<GridSiblingRoute, GridSegmentError> {
    work.checkpoint(OperationPhase::Layout)?;
    let points = compatibility_route(work)?;
    work.checkpoint(OperationPhase::Layout)?;
    if let Some(points) = points {
        let valid = constraints.validates(&points, false, work)?;
        work.checkpoint(OperationPhase::Layout)?;
        if valid {
            return Ok(GridSiblingRoute::Compatibility {
                points,
                reason: limit,
            });
        }
    }
    Err(GridSegmentError::InvalidCompatibility(limit))
}

#[derive(Clone, Copy)]
enum CandidateSet {
    Preferred,
    AssignedSide,
    All,
}

#[derive(Clone, Copy)]
struct Pair {
    source: GridSegmentEndpoint,
    target: GridSegmentEndpoint,
    lower: GridRouteCost,
    order: usize,
}

impl Pair {
    fn matches(self, set: CandidateSet) -> bool {
        match set {
            CandidateSet::Preferred => self.lower.endpoint_candidate_rank == 0,
            CandidateSet::AssignedSide => {
                self.source.on_assigned_side && self.target.on_assigned_side
            }
            CandidateSet::All => true,
        }
    }
}

struct SegmentSearch<'base, 'id, 'a, 'input> {
    base: &'base GridRoutingTopology<'id>,
    pairs: &'a [Pair],
    constraints: &'a Constraints<'input>,
    caps: GridSegmentCaps,
    workspace: &'a mut GridSegmentWorkspace,
    work: &'a OperationWorkMeter,
    bundled: bool,
    best: Option<(Vec<Point>, GridRouteCost)>,
    limit: Option<GridSegmentLimit>,
}

impl SegmentSearch<'_, '_, '_, '_> {
    fn run(&mut self, set: CandidateSet, avoid_occupied: bool) -> Result<(), GridSegmentError> {
        for &pair in self.pairs {
            self.work.charge(1)?;
            if !pair.matches(set) {
                continue;
            }
            if self
                .best
                .as_ref()
                .is_some_and(|(_, best)| !best.compare(pair.lower).is_gt())
            {
                break;
            }
            match self.try_pair(pair, avoid_occupied) {
                Ok(Some((points, cost)))
                    if self
                        .best
                        .as_ref()
                        .is_none_or(|(_, best)| cost.compare(*best).is_lt()) =>
                {
                    self.best = Some((points, cost));
                    self.limit = None;
                }
                Ok(_) => {}
                Err(AttemptError::Limit(limit)) => {
                    if self.best.is_none() {
                        self.limit = Some(limit);
                    }
                    break;
                }
                Err(AttemptError::Layout(error)) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn try_pair(
        &mut self,
        pair: Pair,
        avoid_occupied: bool,
    ) -> Result<Option<(Vec<Point>, GridRouteCost)>, AttemptError> {
        let work = self.work;
        work.checkpoint(OperationPhase::Layout)?;
        self.workspace
            .budget
            .check_available(self.caps.search)
            .map_err(GridSegmentLimit::from)?;
        let base_bytes = self.base.estimated_bytes();
        let retained = match self.caps.retained_base_bytes {
            0 => base_bytes,
            bytes if bytes >= base_bytes => bytes,
            _ => {
                return Err(
                    invalid("Grid retained topology estimate excludes the current base").into(),
                );
            }
        };
        let source = pair.source.connect;
        let target = pair.target.connect;
        let aligned = source.x == target.x || source.y == target.y;
        let lane = [Point::new(
            (source.x + target.x) / 2.0,
            (source.y + target.y) / 2.0,
        )];
        let lane_points = if self.bundled && aligned {
            &lane[..]
        } else {
            &[]
        };
        let overlay = self.base.overlay(
            GridOverlayInput {
                source,
                target,
                lane_points,
            },
            &mut self.workspace.overlay,
            work,
        )?;
        let live_bytes = retained
            .checked_add(overlay.estimated_bytes() - base_bytes)
            .ok_or_else(|| work.arithmetic_overflow())?;
        work.checkpoint(OperationPhase::Layout)?;
        if live_bytes > self.caps.max_topology_bytes {
            return Err(
                GridSegmentLimit::Topology(GridTopologyLimit::EstimatedMemory {
                    actual: live_bytes,
                    max: self.caps.max_topology_bytes,
                })
                .into(),
            );
        }
        let terminal = |connect, side: Side| -> crate::Result<GridSearchTerminal> {
            Ok(GridSearchTerminal {
                vertex: overlay
                    .vertex_id(connect, work)?
                    .ok_or_else(|| invalid("Missing Grid overlay endpoint"))?,
                orientation: Some(side.axis()),
                owner_side: None,
                approach_length: APPROACH,
            })
        };
        let query = GridSearchQuery {
            source: terminal(source, pair.source.side)?,
            target: terminal(target, pair.target.side)?,
            endpoint_candidate_rank: pair.lower.endpoint_candidate_rank,
        };
        let topology = ValidatedGridSearchTopology::new(&overlay, work)?;
        let mut caps = self.caps.search;
        caps.max_estimated_bytes
            .get_or_insert(self.caps.max_topology_bytes);
        let route = find_grid_sparse_route(
            &topology,
            query,
            GridSearchOptions {
                caps,
                estimated_bytes_base: live_bytes,
                ..GridSearchOptions::default()
            },
            &mut self.workspace.search,
            &mut self.workspace.budget,
            work,
            |a, b| self.constraints.allows_arc(a, b, avoid_occupied, work),
        )?;
        if let Some(route) = route {
            let points = attach_ports(pair.source.port, &route.points, pair.target.port, work)?;
            if self.constraints.validates(&points, avoid_occupied, work)? {
                return Ok(Some((points, route.cost)));
            }
        }
        Ok(None)
    }
}

enum AttemptError {
    Layout(Error),
    Limit(GridSegmentLimit),
}

impl From<Error> for AttemptError {
    fn from(value: Error) -> Self {
        Self::Layout(value)
    }
}

impl From<OperationWorkError> for AttemptError {
    fn from(value: OperationWorkError) -> Self {
        Self::Layout(value.into())
    }
}

impl From<GridSegmentLimit> for AttemptError {
    fn from(value: GridSegmentLimit) -> Self {
        Self::Limit(value)
    }
}

impl From<GridTopologyError> for AttemptError {
    fn from(value: GridTopologyError) -> Self {
        match value {
            GridTopologyError::Layout(error) => Self::Layout(error),
            GridTopologyError::Limit(limit) => Self::Limit(limit.into()),
        }
    }
}

impl From<GridSearchError> for AttemptError {
    fn from(value: GridSearchError) -> Self {
        match value {
            GridSearchError::Layout(error) => Self::Layout(error),
            GridSearchError::Limit(limit) => Self::Limit(limit.into()),
        }
    }
}

fn ordered_pairs(
    sources: &[GridSegmentEndpoint],
    targets: &[GridSegmentEndpoint],
    work: &OperationWorkMeter,
) -> crate::Result<Vec<Pair>> {
    for endpoint in sources.iter().chain(targets) {
        work.charge(1)?;
        if !finite(endpoint.port) || !finite(endpoint.connect) {
            return Err(invalid("Non-finite Grid endpoint candidate"));
        }
    }
    let count = sources
        .len()
        .checked_mul(targets.len())
        .ok_or_else(|| work.arithmetic_overflow())?;
    let mut pairs = reserved(count, work)?;
    for &source in sources {
        for &target in targets {
            work.charge(1)?;
            let rank = source
                .rank
                .checked_mul(targets.len())
                .and_then(|rank| rank.checked_add(target.rank))
                .ok_or_else(|| work.arithmetic_overflow())?;
            let lower = pair_lower_bound(source, target, rank)?;
            pairs.push(Pair {
                source,
                target,
                lower,
                order: pairs.len(),
            });
        }
    }
    let levels = count.checked_ilog2().map_or(0, |depth| depth as usize + 1);
    work.charge(
        count
            .checked_mul(levels)
            .ok_or_else(|| work.arithmetic_overflow())?,
    )?;
    pairs.sort_unstable_by(|a, b| {
        a.lower
            .compare(b.lower)
            .then_with(|| side_preference(a.source.side).cmp(&side_preference(b.source.side)))
            .then_with(|| side_preference(a.target.side).cmp(&side_preference(b.target.side)))
            .then_with(|| a.order.cmp(&b.order))
    });
    work.checkpoint(OperationPhase::Layout)?;
    Ok(pairs)
}

fn pair_lower_bound(
    source: GridSegmentEndpoint,
    target: GridSegmentEndpoint,
    rank: usize,
) -> crate::Result<GridRouteCost> {
    let dx = (source.connect.x - target.connect.x).abs();
    let dy = (source.connect.y - target.connect.y).abs();
    let length = APPROACH * 2.0 + dx + dy;
    if !length.is_finite() {
        return Err(invalid("Non-finite Grid endpoint pair lower bound"));
    }
    let from = source.side.axis();
    let to = target.side.axis();
    let turns = |a, b| usize::from(from != a) + usize::from(a != b) + usize::from(b != to);
    let bends = match (dx == 0.0, dy == 0.0) {
        (true, true) => usize::from(from != to),
        (true, false) => turns(Axis::Vertical, Axis::Vertical),
        (false, true) => turns(Axis::Horizontal, Axis::Horizontal),
        (false, false) => {
            turns(Axis::Horizontal, Axis::Vertical).min(turns(Axis::Vertical, Axis::Horizontal))
        }
    };
    Ok(GridRouteCost {
        length,
        bends,
        boundary_transitions: 0,
        endpoint_candidate_rank: rank,
    })
}

fn side_preference(side: Side) -> u8 {
    match side {
        Side::Right => 0,
        Side::Bottom => 1,
        Side::Left => 2,
        Side::Top => 3,
    }
}

fn finite(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn invalid(message: &'static str) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn reserved<T>(count: usize, work: &OperationWorkMeter) -> crate::Result<Vec<T>> {
    work.charge(count)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::LayoutAllocationFailed { backend: "grid" })?;
    Ok(values)
}
