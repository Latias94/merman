//! Mermaid 12.4's lexicographic A* over one immutable Grid routing topology.

use crate::Error;
use crate::graph_layout::{Axis, Side};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use kurbo::Point;
use merman_model::OperationPhase;
use std::cmp::Ordering;

mod state;
pub use state::GridSearchWorkspace;
use state::{Channel, SearchState, compare_chains, reconstruct};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub struct GridSearchArc {
    pub to: usize,
    pub axis: Axis,
    pub length: f64,
    /// Only a portal arc adds one boundary transition.
    pub crosses_boundary: bool,
}

/// A base topology or one borrowed endpoint overlay in its canonical vertex and arc order.
pub trait GridSearchTopology {
    fn vertex_count(&self) -> usize;
    fn point(&self, vertex: usize) -> Option<Point>;
    fn outgoing(&self, vertex: usize) -> &[GridSearchArc];
}

/// Validated view of one immutable topology snapshot with dense vertex IDs.
pub struct ValidatedGridSearchTopology<'a, T: ?Sized> {
    graph: &'a T,
    count: usize,
}

impl<'a, T: GridSearchTopology + ?Sized> ValidatedGridSearchTopology<'a, T> {
    pub fn new(graph: &'a T, work: &OperationWorkMeter) -> crate::Result<Self> {
        let count = graph.vertex_count();
        work.charge(count)?;
        let topology = Self { graph, count };
        for vertex in 0..count {
            work.checkpoint(OperationPhase::Layout)?;
            let point = topology.point(vertex)?;
            for arc in graph.outgoing(vertex) {
                work.charge(1)?;
                let target = topology.point(arc.to)?;
                let (dx, dy) = delta(point, target);
                let aligned = match arc.axis {
                    Axis::Horizontal => dy == 0.0,
                    Axis::Vertical => dx == 0.0,
                };
                if !aligned || !arc.length.is_finite() || arc.length <= 0.0 || arc.length != dx + dy
                {
                    return Err(invalid("Malformed grid routing arc"));
                }
            }
        }
        work.checkpoint(OperationPhase::Layout)?;
        Ok(topology)
    }

    fn point(&self, id: usize) -> crate::Result<Point> {
        if id < self.count
            && let Some(point) = self.graph.point(id)
            && point.x.is_finite()
            && point.y.is_finite()
        {
            return Ok(point);
        }
        Err(invalid("Missing or malformed grid routing vertex"))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GridSearchTerminal {
    pub vertex: usize,
    pub orientation: Option<Axis>,
    /// Enables the upstream directional restriction on the first or last search arc.
    pub owner_side: Option<Side>,
    pub approach_length: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GridSearchQuery {
    pub source: GridSearchTerminal,
    pub target: GridSearchTerminal,
    pub endpoint_candidate_rank: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridRouteCost {
    pub length: f64,
    pub bends: usize,
    pub boundary_transitions: usize,
    pub endpoint_candidate_rank: usize,
}

impl GridRouteCost {
    /// Compare finite cost tuples, including session-supplied candidate lower bounds.
    pub fn compare(self, other: Self) -> Ordering {
        // JavaScript's numeric comparison treats signed zeros as equal.
        let length_order = if self.length == other.length {
            Ordering::Equal
        } else {
            self.length.total_cmp(&other.length)
        };
        length_order
            .then_with(|| self.bends.cmp(&other.bends))
            .then_with(|| self.boundary_transitions.cmp(&other.boundary_transitions))
            .then_with(|| {
                self.endpoint_candidate_rank
                    .cmp(&other.endpoint_candidate_rank)
            })
    }
}

#[derive(Debug, PartialEq)]
pub struct GridSparseRoute {
    pub points: Vec<Point>,
    /// The raw predecessor chain includes collinear vertices and portal arc endpoints.
    pub vertex_ids: Vec<usize>,
    pub cost: GridRouteCost,
}

#[derive(Clone, Copy, Debug)]
pub struct GridSearchCaps {
    pub max_expanded_states: usize,
    pub max_invocation_expanded_states: usize,
    pub max_estimated_bytes: Option<usize>,
}

impl Default for GridSearchCaps {
    fn default() -> Self {
        Self {
            max_expanded_states: 100_000,
            max_invocation_expanded_states: 2_000_000,
            max_estimated_bytes: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum GridSearchHeuristic {
    #[default]
    BendAware,
    #[cfg(test)]
    Zero,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum GridSearchQueueOrder {
    #[default]
    Canonical,
    #[cfg(test)]
    Reverse,
    /// Diagnostic perturbation of equal-priority queue keys. Production uses `Canonical`.
    #[cfg(test)]
    Seeded(u64),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GridSearchOptions {
    pub caps: GridSearchCaps,
    /// Live base and overlay bytes owned by the routing session.
    pub estimated_bytes_base: usize,
    pub heuristic: GridSearchHeuristic,
    pub queue_order: GridSearchQueueOrder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridSearchStateScope {
    Edge,
    Invocation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GridSearchLimit {
    #[error("Grid routing search-state cap exceeded ({scope:?})")]
    StateCap { scope: GridSearchStateScope },
    #[error("Grid routing search workspace memory cap exceeded ({actual} > {max})")]
    EstimatedMemory { actual: usize, max: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum GridSearchError {
    #[error(transparent)]
    Limit(#[from] GridSearchLimit),
    #[error(transparent)]
    Layout(#[from] Error),
}

impl From<OperationWorkError> for GridSearchError {
    fn from(error: OperationWorkError) -> Self {
        Self::Layout(error.into())
    }
}

/// Keep one budget for the entire diagram, across failed searches and bundle rollbacks.
#[derive(Debug, Default)]
pub struct GridSearchBudget {
    expanded: usize,
}

impl GridSearchBudget {
    #[cfg(test)]
    pub fn expanded_states(&self) -> usize {
        self.expanded
    }

    /// Call before making an endpoint overlay to avoid building one after exhaustion.
    pub fn check_available(&self, caps: GridSearchCaps) -> Result<(), GridSearchLimit> {
        self.check_expansion(0, caps)
    }

    fn check_expansion(&self, search: usize, caps: GridSearchCaps) -> Result<(), GridSearchLimit> {
        let scope = if self.expanded >= caps.max_invocation_expanded_states {
            GridSearchStateScope::Invocation
        } else if search >= caps.max_expanded_states {
            GridSearchStateScope::Edge
        } else {
            return Ok(());
        };
        Err(GridSearchLimit::StateCap { scope })
    }
}

/// Search one planned terminal pair. The filter may meter pair or occupancy checks on this
/// operation's ledger. The caller validates the reconstructed route before committing it.
pub fn find_grid_sparse_route<T: GridSearchTopology + ?Sized>(
    topology: &ValidatedGridSearchTopology<'_, T>,
    query: GridSearchQuery,
    options: GridSearchOptions,
    workspace: &mut GridSearchWorkspace,
    budget: &mut GridSearchBudget,
    work: &OperationWorkMeter,
    mut arc_allowed: impl FnMut(Point, Point) -> crate::Result<bool>,
) -> Result<Option<GridSparseRoute>, GridSearchError> {
    work.checkpoint(OperationPhase::Layout)?;
    let source_point = topology.point(query.source.vertex)?;
    let target_point = topology.point(query.target.vertex)?;
    if [query.source.approach_length, query.target.approach_length]
        .into_iter()
        .any(|length| !length.is_finite() || length < 0.0)
    {
        return Err(invalid("Malformed grid routing approach length").into());
    }

    work.checkpoint(OperationPhase::Layout)?;
    budget.check_available(options.caps)?;
    let slots = topology
        .count
        .checked_mul(3)
        .ok_or_else(|| work.arithmetic_overflow())?;
    workspace.reset(slots, options, work)?;
    let target_channel = Channel::from(query.target.orientation);
    let root = SearchState {
        vertex: query.source.vertex,
        channel: Channel::from(query.source.orientation),
        cost: GridRouteCost {
            length: query.source.approach_length,
            bends: 0,
            boundary_transitions: 0,
            endpoint_candidate_rank: query.endpoint_candidate_rank,
        },
        parent: None,
        depth: 1,
    };
    let priority = lower_bound(
        root,
        source_point,
        target_point,
        target_channel,
        options.heuristic,
    )?;
    workspace.push(root, priority, options, work)?;
    let mut expanded_here = 0;
    let mut best_goal: Option<(usize, GridRouteCost)> = None;

    while let Some(head) = workspace.peek() {
        if best_goal.is_some_and(|(_, cost)| head.priority.compare(cost).is_gt()) {
            break;
        }
        workspace.pop(work)?;
        let current = workspace.states[head.id];
        if workspace.best(current.vertex, current.channel) != Some(head.id) {
            continue;
        }
        if let Err(limit) = budget.check_expansion(expanded_here, options.caps) {
            if best_goal.is_some() {
                break;
            }
            return Err(limit.into());
        }
        work.charge(1)?;
        expanded_here += 1;
        budget.expanded += 1;

        if current.vertex == query.target.vertex {
            let cost = GridRouteCost {
                length: checked_length(current.cost.length + query.target.approach_length)?,
                bends: current.cost.bends + current.channel.turn(target_channel),
                ..current.cost
            };
            if let Some((old_id, old_cost)) = best_goal {
                let order = cost.compare(old_cost);
                if order.is_gt()
                    || (order.is_eq()
                        && !compare_chains(&workspace.states, Some(head.id), Some(old_id), work)?
                            .is_lt())
                {
                    continue;
                }
            }
            best_goal = Some((head.id, cost));
            continue;
        }

        let from = topology.point(current.vertex)?;
        for &arc in topology.graph.outgoing(current.vertex) {
            work.charge(1)?;
            let to = topology.point(arc.to)?;
            if !arc_allowed(from, to)?
                || (current.parent.is_none() && enters_owner(from, to, query.source.owner_side))
                || (arc.to == query.target.vertex
                    && enters_owner(to, from, query.target.owner_side))
            {
                continue;
            }
            let channel = Channel::from(Some(arc.axis));
            let cost = GridRouteCost {
                length: checked_length(current.cost.length + arc.length)?,
                bends: current.cost.bends + current.channel.turn(channel),
                boundary_transitions: current.cost.boundary_transitions
                    + usize::from(arc.crosses_boundary),
                ..current.cost
            };
            if let Some(previous_id) = workspace.best(arc.to, channel) {
                let previous = workspace.states[previous_id];
                let order = cost.compare(previous.cost);
                if order.is_gt()
                    || (order.is_eq()
                        && !compare_chains(
                            &workspace.states,
                            Some(head.id),
                            previous.parent,
                            work,
                        )?
                        .is_lt())
                {
                    continue;
                }
            }
            let next = SearchState {
                vertex: arc.to,
                channel,
                cost,
                parent: Some(head.id),
                depth: current.depth + 1,
            };
            let priority = lower_bound(next, to, target_point, target_channel, options.heuristic)?;
            workspace.push(next, priority, options, work)?;
        }
    }

    let result = best_goal
        .map(|(id, cost)| {
            reconstruct(
                &workspace.states,
                id,
                cost,
                query.source.vertex,
                |vertex| topology.point(vertex),
                work,
            )
        })
        .transpose()?;
    work.checkpoint(OperationPhase::Layout)?;
    Ok(result)
}

fn lower_bound(
    state: SearchState,
    point: Point,
    target: Point,
    target_channel: Channel,
    heuristic: GridSearchHeuristic,
) -> crate::Result<GridRouteCost> {
    match heuristic {
        GridSearchHeuristic::BendAware => {}
        #[cfg(test)]
        GridSearchHeuristic::Zero => return Ok(state.cost),
    }
    let (dx, dy) = delta(point, target);
    let on_axes =
        |first, last| state.channel.turn(first) + first.turn(last) + last.turn(target_channel);
    let bends = match (dx == 0.0, dy == 0.0) {
        (true, true) => state.channel.turn(target_channel),
        (true, false) => on_axes(Channel::Vertical, Channel::Vertical),
        (false, true) => on_axes(Channel::Horizontal, Channel::Horizontal),
        (false, false) => on_axes(Channel::Horizontal, Channel::Vertical)
            .min(on_axes(Channel::Vertical, Channel::Horizontal)),
    };
    Ok(GridRouteCost {
        length: checked_length(state.cost.length + (dx + dy))?,
        bends: state.cost.bends + bends,
        ..state.cost
    })
}

fn enters_owner(from: Point, to: Point, side: Option<Side>) -> bool {
    match side {
        Some(Side::Left) => from.y == to.y && to.x > from.x,
        Some(Side::Right) => from.y == to.y && to.x < from.x,
        Some(Side::Top) => from.x == to.x && to.y > from.y,
        Some(Side::Bottom) => from.x == to.x && to.y < from.y,
        None => false,
    }
}

fn delta(a: Point, b: Point) -> (f64, f64) {
    ((a.x - b.x).abs(), (a.y - b.y).abs())
}

fn checked_length(length: f64) -> crate::Result<f64> {
    if length.is_finite() {
        Ok(length)
    } else {
        Err(invalid("Non-finite grid route cost"))
    }
}

fn invalid(message: &'static str) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}
