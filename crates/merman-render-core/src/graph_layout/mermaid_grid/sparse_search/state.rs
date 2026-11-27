use super::{
    GridRouteCost, GridSearchError, GridSearchLimit, GridSearchOptions, GridSearchQueueOrder,
    GridSparseRoute, invalid,
};
use crate::Error;
use crate::graph_layout::Axis;
use crate::resources::OperationWorkMeter;
use kurbo::Point;
use merman_model::OperationPhase;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::num::NonZeroUsize;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Channel {
    Start,
    Horizontal,
    Vertical,
}

impl From<Option<Axis>> for Channel {
    fn from(axis: Option<Axis>) -> Self {
        match axis {
            None => Self::Start,
            Some(Axis::Horizontal) => Self::Horizontal,
            Some(Axis::Vertical) => Self::Vertical,
        }
    }
}

impl Channel {
    pub(super) fn turn(self, next: Self) -> usize {
        usize::from(self != Self::Start && next != Self::Start && self != next)
    }

    fn slot(self, vertex: usize) -> usize {
        vertex * 3 + self as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SearchState {
    pub(super) vertex: usize,
    pub(super) channel: Channel,
    pub(super) cost: GridRouteCost,
    pub(super) parent: Option<usize>,
    pub(super) depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct QueueTie {
    perturbation: u64,
    vertex: usize,
    channel: u8,
    parent: usize,
    id: usize,
}

impl QueueTie {
    fn new(state: SearchState, id: usize, order: GridSearchQueueOrder) -> Self {
        let tie = Self {
            perturbation: 0,
            vertex: state.vertex,
            channel: state.channel as u8,
            parent: state.parent.map_or(0, |parent| parent + 1),
            id,
        };
        #[cfg(test)]
        let mut tie = tie;
        match order {
            GridSearchQueueOrder::Canonical => {}
            #[cfg(test)]
            GridSearchQueueOrder::Reverse => {
                tie.vertex = !tie.vertex;
                tie.channel = !tie.channel;
                tie.parent = !tie.parent;
                tie.id = !tie.id;
            }
            #[cfg(test)]
            GridSearchQueueOrder::Seeded(seed) => {
                // SplitMix64 finalizer for a deterministic permutation of equal-priority slots.
                let mut value = seed.wrapping_add((id as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
                value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                tie.perturbation = value ^ (value >> 31);
            }
        }
        tie
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct OpenEntry {
    pub(super) id: usize,
    pub(super) priority: GridRouteCost,
    tie: QueueTie,
}

impl Ord for OpenEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap. Reverse the complete ordering to pop the minimum.
        other
            .priority
            .compare(self.priority)
            .then_with(|| other.tie.cmp(&self.tie))
    }
}

impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for OpenEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for OpenEntry {}

/// Operation-local scratch. It retains capacities across searches and clears logical state on
/// every admitted search, including the next search after a local-cap failure.
#[derive(Debug, Default)]
pub struct GridSearchWorkspace {
    pub(super) states: Vec<SearchState>,
    best: Vec<Option<NonZeroUsize>>,
    touched: Vec<usize>,
    open: BinaryHeap<OpenEntry>,
}

impl GridSearchWorkspace {
    pub(super) fn reset(
        &mut self,
        slots: usize,
        options: GridSearchOptions,
        work: &OperationWorkMeter,
    ) -> Result<(), GridSearchError> {
        let best = slots.max(self.best.capacity());
        admit_storage(
            best,
            self.states.capacity(),
            self.open.capacity(),
            self.touched.capacity(),
            options,
            work,
        )?;
        let extended = slots.saturating_sub(self.best.len());
        work.charge(
            extended
                .checked_add(self.touched.len())
                .ok_or_else(|| work.arithmetic_overflow())?,
        )?;
        self.best
            .try_reserve_exact(extended)
            .map_err(|_| allocation_failed())?;
        self.best.resize(self.best.len() + extended, None);
        for slot in self.touched.drain(..) {
            self.best[slot] = None;
        }
        self.states.clear();
        self.open.clear();
        Ok(())
    }

    pub(super) fn best(&self, vertex: usize, channel: Channel) -> Option<usize> {
        self.best[channel.slot(vertex)].map(|id| id.get() - 1)
    }

    pub(super) fn peek(&self) -> Option<OpenEntry> {
        self.open.peek().copied()
    }

    pub(super) fn pop(&mut self, work: &OperationWorkMeter) -> crate::Result<()> {
        // At most two heap-key comparisons per level on the way down and one on the way up.
        let depth = self.open.len().checked_ilog2().unwrap_or(0) as usize;
        work.charge(1 + 3 * depth)?;
        self.open.pop();
        Ok(())
    }

    pub(super) fn push(
        &mut self,
        state: SearchState,
        priority: GridRouteCost,
        options: GridSearchOptions,
        work: &OperationWorkMeter,
    ) -> Result<(), GridSearchError> {
        let id = self.states.len();
        let slot = state.channel.slot(state.vertex);
        let first_visit = self.best[slot].is_none();
        let required_states = id
            .checked_add(1)
            .ok_or_else(|| work.arithmetic_overflow())?;
        let required_open = self
            .open
            .len()
            .checked_add(1)
            .ok_or_else(|| work.arithmetic_overflow())?;
        let required_touched = self
            .touched
            .len()
            .checked_add(usize::from(first_visit))
            .ok_or_else(|| work.arithmetic_overflow())?;
        let state_capacity = grown_capacity(self.states.capacity(), required_states, work)?;
        let open_capacity = grown_capacity(self.open.capacity(), required_open, work)?;
        let touched_capacity = grown_capacity(self.touched.capacity(), required_touched, work)?;
        admit_storage(
            self.best.capacity(),
            state_capacity,
            open_capacity,
            touched_capacity,
            options,
            work,
        )?;
        work.charge(1 + required_open.ilog2() as usize)?;
        if state_capacity > self.states.capacity() {
            work.charge(self.states.len())?;
            self.states
                .try_reserve_exact(state_capacity - self.states.len())
                .map_err(|_| allocation_failed())?;
        }
        if open_capacity > self.open.capacity() {
            work.charge(self.open.len())?;
            self.open
                .try_reserve_exact(open_capacity - self.open.len())
                .map_err(|_| allocation_failed())?;
        }
        if touched_capacity > self.touched.capacity() {
            work.charge(self.touched.len())?;
            self.touched
                .try_reserve_exact(touched_capacity - self.touched.len())
                .map_err(|_| allocation_failed())?;
        }
        let tie = QueueTie::new(state, id, options.queue_order);
        self.states.push(state);
        self.open.push(OpenEntry { id, priority, tie });
        if first_visit {
            self.touched.push(slot);
        }
        self.best[slot] = NonZeroUsize::new(required_states);
        Ok(())
    }
}

fn grown_capacity(
    current: usize,
    required: usize,
    work: &OperationWorkMeter,
) -> crate::Result<usize> {
    if required <= current {
        return Ok(current);
    }
    let doubled = current
        .checked_mul(2)
        .ok_or_else(|| work.arithmetic_overflow())?;
    Ok(required.max(doubled))
}

fn admit_storage(
    best: usize,
    states: usize,
    open: usize,
    touched: usize,
    options: GridSearchOptions,
    work: &OperationWorkMeter,
) -> Result<(), GridSearchError> {
    // An operation terminal has priority over recoverable local admission.
    work.checkpoint(OperationPhase::Layout)?;
    let bytes = [
        (best, size_of::<Option<NonZeroUsize>>()),
        (states, size_of::<SearchState>()),
        (open, size_of::<OpenEntry>()),
        (touched, size_of::<usize>()),
    ]
    .into_iter()
    .try_fold(options.estimated_bytes_base, |total, (count, size)| {
        total.checked_add(count.checked_mul(size)?)
    })
    .ok_or_else(|| work.arithmetic_overflow())?;
    if let Some(max) = options.caps.max_estimated_bytes
        && bytes > max
    {
        return Err(GridSearchLimit::EstimatedMemory { actual: bytes, max }.into());
    }
    Ok(())
}

fn allocation_failed() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

fn state_at(states: &[SearchState], id: usize) -> crate::Result<SearchState> {
    states.get(id).copied().ok_or_else(invalid_chain)
}

fn invalid_chain() -> Error {
    invalid("Malformed grid routing predecessor chain")
}

fn parent(
    states: &[SearchState],
    id: usize,
    work: &OperationWorkMeter,
) -> crate::Result<Option<usize>> {
    work.charge(1)?;
    let state = state_at(states, id)?;
    match state.parent {
        Some(previous)
            if previous < id
                && state_at(states, previous)?.depth == state.depth.saturating_sub(1) =>
        {
            Ok(Some(previous))
        }
        None if id == 0 && state.depth == 1 => Ok(None),
        _ => Err(invalid_chain()),
    }
}

pub(super) fn compare_chains(
    states: &[SearchState],
    a: Option<usize>,
    b: Option<usize>,
    work: &OperationWorkMeter,
) -> crate::Result<Ordering> {
    work.charge(1)?;
    let (Some(mut a), Some(mut b)) = (a, b) else {
        return Ok(a.cmp(&b));
    };
    if a == b {
        return Ok(Ordering::Equal);
    }
    let a_depth = state_at(states, a)?.depth;
    let b_depth = state_at(states, b)?.depth;
    for _ in b_depth..a_depth {
        a = parent(states, a, work)?.ok_or_else(invalid_chain)?;
    }
    for _ in a_depth..b_depth {
        b = parent(states, b, work)?.ok_or_else(invalid_chain)?;
    }
    if a == b {
        return Ok(a_depth.cmp(&b_depth));
    }
    // Follow decreasing predecessor IDs without a path copy. The first children of the common
    // prefix determine the pinned search's tie order.
    loop {
        let a_parent = parent(states, a, work)?;
        let b_parent = parent(states, b, work)?;
        if a_parent == b_parent {
            let a = state_at(states, a)?;
            let b = state_at(states, b)?;
            return Ok((a.vertex, a.channel).cmp(&(b.vertex, b.channel)));
        }
        a = a_parent.ok_or_else(invalid_chain)?;
        b = b_parent.ok_or_else(invalid_chain)?;
    }
}

pub(super) fn reconstruct(
    states: &[SearchState],
    goal: usize,
    cost: GridRouteCost,
    source: usize,
    mut point_at: impl FnMut(usize) -> crate::Result<Point>,
    work: &OperationWorkMeter,
) -> crate::Result<GridSparseRoute> {
    let depth = state_at(states, goal)?.depth;
    if depth == 0 || depth > states.len() {
        return Err(invalid_chain());
    }
    work.charge(depth)?;
    let mut vertex_ids = Vec::new();
    vertex_ids
        .try_reserve_exact(depth)
        .map_err(|_| allocation_failed())?;
    let mut current = Some(goal);
    while let Some(id) = current {
        current = parent(states, id, work)?;
        vertex_ids.push(state_at(states, id)?.vertex);
    }
    if vertex_ids.len() != depth || vertex_ids.last().copied() != Some(source) {
        return Err(invalid_chain());
    }
    vertex_ids.reverse();

    let mut points = Vec::new();
    points
        .try_reserve_exact(depth)
        .map_err(|_| allocation_failed())?;
    for &vertex in &vertex_ids {
        work.charge(1)?;
        let point = point_at(vertex)?;
        match points.as_mut_slice() {
            [.., last] if *last == point => continue,
            [.., previous, last] if collinear(*previous, *last, point) => *last = point,
            _ => points.push(point),
        }
    }
    Ok(GridSparseRoute {
        points,
        vertex_ids,
        cost,
    })
}

fn collinear(a: Point, b: Point, c: Point) -> bool {
    (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y)
}
