use super::super::layout::{GridCoord, NodeLayout};
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::AsciiResourceLimitPhase;
use crate::resource::ResourceContext;
use merman_core::OperationPhase;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GridPathPortPolicy {
    DirectionalShortest,
    Fixed(PortPair),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GridPathRoute {
    pub(super) path: Vec<GridCoord>,
    pub(super) ports: PortPair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PortPair {
    start: Port,
    end: Port,
}

impl PortPair {
    pub(super) fn new(start: Port, end: Port) -> Self {
        Self { start, end }
    }

    pub(super) fn start(self) -> Port {
        self.start
    }

    pub(super) fn end(self) -> Port {
        self.end
    }
}

#[cfg(test)]
pub(super) fn route_grid_path(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    port_policy: GridPathPortPolicy,
) -> Option<GridPathRoute> {
    let policy = crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    );
    let mut resources = ResourceContext::new(policy);
    route_grid_path_with_resources(layouts, from, to, port_policy, &mut resources)
        .expect("test grid routing work must remain representable")
}

#[cfg(test)]
pub(super) fn route_grid_path_with_resources(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    port_policy: GridPathPortPolicy,
    resources: &mut ResourceContext,
) -> Result<Option<GridPathRoute>> {
    let policy = resources.policy();
    route_grid_path_with_resources_and_execution(
        layouts,
        from,
        to,
        port_policy,
        resources,
        AsciiExecution::for_test(&policy),
    )
}

pub(super) fn route_grid_path_with_resources_and_execution(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    port_policy: GridPathPortPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    match port_policy {
        GridPathPortPolicy::DirectionalShortest => select_shortest_reachable_grid_path(
            layouts,
            from,
            to,
            directional_left_right_port_pairs(from, to),
            resources,
            execution,
        ),
        GridPathPortPolicy::Fixed(ports) => {
            plan_grid_path_for_ports(layouts, from, to, ports, resources, execution)
        }
    }
}

fn select_shortest_reachable_grid_path(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    candidates: [PortPair; 2],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    let mut selected: Option<GridPathRoute> = None;
    for ports in candidates {
        let Some(route) = plan_grid_path_for_ports(layouts, from, to, ports, resources, execution)?
        else {
            continue;
        };
        if selected
            .as_ref()
            .is_none_or(|current| route.path.len() < current.path.len())
        {
            selected = Some(route);
        }
    }
    Ok(selected)
}

fn plan_grid_path_for_ports(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    ports: PortPair,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    let start = from.grid_for_port(ports.start, resources)?;
    let target = to.grid_for_port(ports.end, resources)?;
    let Some(path) = find_grid_path(layouts, start, target, resources, execution)? else {
        return Ok(None);
    };
    Ok(Some(GridPathRoute {
        path: merge_grid_path(path, resources, execution)?,
        ports,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Port {
    Up,
    Down,
    Left,
    Right,
    Middle,
}

impl Port {
    fn offset(self) -> (usize, usize) {
        match self {
            Port::Up => (1, 0),
            Port::Down => (1, 2),
            Port::Left => (0, 1),
            Port::Right => (2, 1),
            Port::Middle => (1, 1),
        }
    }

    pub(super) fn terminal_direction(self) -> StepDirection {
        match self {
            Port::Up => StepDirection::Up,
            Port::Down => StepDirection::Down,
            Port::Left => StepDirection::Left,
            Port::Right => StepDirection::Right,
            Port::Middle => StepDirection::Right,
        }
    }
}

trait NodeGridPort {
    fn grid_for_port(&self, port: Port, resources: &ResourceContext) -> Result<GridCoord>;
}

impl NodeGridPort for NodeLayout {
    fn grid_for_port(&self, port: Port, resources: &ResourceContext) -> Result<GridCoord> {
        let (x, y) = port.offset();
        Ok(GridCoord {
            x: resources.checked_grid_add(self.grid.x, x)?,
            y: resources.checked_grid_add(self.grid.y, y)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeDirection {
    Up,
    Down,
    Left,
    Right,
    UpperRight,
    LowerRight,
    UpperLeft,
    LowerLeft,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StepDirection {
    Up,
    Down,
    Left,
    Right,
}

fn directional_left_right_port_pairs(from: &NodeLayout, to: &NodeLayout) -> [PortPair; 2] {
    match relative_direction(from.grid, to.grid) {
        RelativeDirection::LowerRight => [
            PortPair::new(Port::Down, Port::Left),
            PortPair::new(Port::Right, Port::Up),
        ],
        RelativeDirection::UpperRight => [
            PortPair::new(Port::Up, Port::Left),
            PortPair::new(Port::Right, Port::Down),
        ],
        RelativeDirection::LowerLeft => [
            PortPair::new(Port::Down, Port::Down),
            PortPair::new(Port::Left, Port::Up),
        ],
        RelativeDirection::UpperLeft => [
            PortPair::new(Port::Down, Port::Down),
            PortPair::new(Port::Left, Port::Down),
        ],
        RelativeDirection::Left => [
            PortPair::new(Port::Down, Port::Down),
            PortPair::new(Port::Left, Port::Right),
        ],
        RelativeDirection::Right => [
            PortPair::new(Port::Right, Port::Left),
            PortPair::new(Port::Right, Port::Left),
        ],
        RelativeDirection::Down => [
            PortPair::new(Port::Down, Port::Up),
            PortPair::new(Port::Down, Port::Up),
        ],
        RelativeDirection::Up => [
            PortPair::new(Port::Up, Port::Down),
            PortPair::new(Port::Up, Port::Down),
        ],
        RelativeDirection::Middle => [
            PortPair::new(Port::Middle, Port::Middle),
            PortPair::new(Port::Middle, Port::Middle),
        ],
    }
}

fn relative_direction(from: GridCoord, to: GridCoord) -> RelativeDirection {
    match (from.x.cmp(&to.x), from.y.cmp(&to.y)) {
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => RelativeDirection::Middle,
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => RelativeDirection::Down,
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Greater) => RelativeDirection::Up,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Equal) => RelativeDirection::Right,
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Equal) => RelativeDirection::Left,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Less) => RelativeDirection::LowerRight,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Greater) => RelativeDirection::UpperRight,
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Less) => RelativeDirection::LowerLeft,
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Greater) => RelativeDirection::UpperLeft,
    }
}

fn find_grid_path(
    layouts: &[NodeLayout],
    start: GridCoord,
    target: GridCoord,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<GridCoord>>> {
    let max_x = layouts.iter().try_fold(0usize, |current, layout| {
        Ok::<_, crate::error::AsciiError>(
            current.max(resources.checked_grid_add(layout.grid.x, 2)?),
        )
    })?;
    let max_y = layouts.iter().try_fold(0usize, |current, layout| {
        Ok::<_, crate::error::AsciiError>(
            current.max(resources.checked_grid_add(layout.grid.y, 2)?),
        )
    })?;
    let max_x = resources.checked_grid_add(max_x, 6)?;
    let max_y = resources.checked_grid_add(max_y, 6)?;
    let occupied = occupied_grid_cells(layouts, resources, execution)?;
    let mut open = BinaryHeap::new();
    let mut cost_so_far = HashMap::new();
    let mut came_from = HashMap::<GridCoord, GridCoord>::new();
    open.try_reserve(1)
        .map_err(|_| layout_allocation_failed())?;
    try_reserve_hash_map(&mut cost_so_far, 1)?;
    cost_so_far.insert(start, 0usize);
    open.push(OpenEntry {
        coord: start,
        cost: 0,
        priority: grid_heuristic(start, target, resources)?,
        sequence: 0,
    });
    let mut sequence = 0usize;

    while let Some(entry) = open.pop() {
        checkpoint_layout(execution)?;
        resources.charge_layout_work(1)?;
        let current = entry.coord;
        if cost_so_far
            .get(&current)
            .is_some_and(|known| entry.cost > *known)
        {
            continue;
        }
        if current == target {
            let path_capacity = resources.checked_work_add(entry.cost, 1)?;
            let mut path = Vec::new();
            path.try_reserve(path_capacity)
                .map_err(|_| layout_allocation_failed())?;
            path.push(current);
            let mut cursor = current;
            while let Some(previous) = came_from.get(&cursor).copied() {
                checkpoint_layout(execution)?;
                resources.charge_layout_work(1)?;
                path.push(previous);
                cursor = previous;
            }
            path.reverse();
            return Ok(Some(path));
        }

        for next in grid_neighbors(current, max_x, max_y).into_iter().flatten() {
            checkpoint_layout(execution)?;
            resources.charge_layout_work(1)?;
            if occupied.contains(&next) && next != target {
                continue;
            }

            let new_cost = resources.checked_work_add(cost_so_far[&current], 1)?;
            if cost_so_far
                .get(&next)
                .is_none_or(|current_cost| new_cost < *current_cost)
            {
                if !cost_so_far.contains_key(&next) {
                    try_reserve_hash_map(&mut cost_so_far, 1)?;
                    try_reserve_hash_map(&mut came_from, 1)?;
                }
                cost_so_far.insert(next, new_cost);
                let priority = resources
                    .checked_work_add(new_cost, grid_heuristic(next, target, resources)?)?;
                sequence = resources.checked_work_add(sequence, 1)?;
                open.try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
                open.push(OpenEntry {
                    coord: next,
                    cost: new_cost,
                    priority,
                    sequence,
                });
                came_from.insert(next, current);
            }
        }
    }

    Ok(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenEntry {
    coord: GridCoord,
    cost: usize,
    priority: usize,
    sequence: usize,
}

impl Ord for OpenEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.sequence.cmp(&self.sequence))
            .then_with(|| other.coord.y.cmp(&self.coord.y))
            .then_with(|| other.coord.x.cmp(&self.coord.x))
    }
}

impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn occupied_grid_cells(
    layouts: &[NodeLayout],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<HashSet<GridCoord>> {
    const NODE_GRID_FOOTPRINT: usize = 9;
    let capacity = resources.checked_work_mul(layouts.len(), NODE_GRID_FOOTPRINT)?;
    let mut occupied = HashSet::new();
    occupied
        .try_reserve(capacity)
        .map_err(|_| layout_allocation_failed())?;
    for layout in layouts {
        for y_offset in 0..=2 {
            for x_offset in 0..=2 {
                checkpoint_layout(execution)?;
                resources.charge_layout_work(1)?;
                occupied.insert(GridCoord {
                    x: resources.checked_grid_add(layout.grid.x, x_offset)?,
                    y: resources.checked_grid_add(layout.grid.y, y_offset)?,
                });
            }
        }
    }
    Ok(occupied)
}

fn try_reserve_hash_map<K: Eq + Hash, V>(map: &mut HashMap<K, V>, additional: usize) -> Result<()> {
    map.try_reserve(additional)
        .map_err(|_| layout_allocation_failed())
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

fn grid_neighbors(coord: GridCoord, max_x: usize, max_y: usize) -> [Option<GridCoord>; 4] {
    [
        (coord.x < max_x).then(|| GridCoord {
            x: coord.x + 1,
            y: coord.y,
        }),
        coord.x.checked_sub(1).map(|x| GridCoord { x, y: coord.y }),
        (coord.y < max_y).then(|| GridCoord {
            x: coord.x,
            y: coord.y + 1,
        }),
        coord.y.checked_sub(1).map(|y| GridCoord { x: coord.x, y }),
    ]
}

fn grid_heuristic(a: GridCoord, b: GridCoord, resources: &ResourceContext) -> Result<usize> {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    dx.checked_add(dy).ok_or_else(|| resources.grid_overflow())
}

fn merge_grid_path(
    path: Vec<GridCoord>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<GridCoord>> {
    if path.len() <= 2 {
        return Ok(path);
    }

    let mut merged = Vec::new();
    merged
        .try_reserve(path.len())
        .map_err(|_| layout_allocation_failed())?;
    merged.push(path[0]);
    for window in path.windows(3) {
        checkpoint_layout(execution)?;
        resources.charge_layout_work(1)?;
        let previous = step_direction(window[0], window[1]);
        let next = step_direction(window[1], window[2]);
        if previous != next {
            merged.push(window[1]);
        }
    }
    merged.push(*path.last().expect("path has at least one element"));
    Ok(merged)
}

fn checkpoint_layout(execution: AsciiExecution<'_>) -> Result<()> {
    execution.checkpoint(OperationPhase::Layout)
}

pub(super) fn step_direction(from: GridCoord, to: GridCoord) -> StepDirection {
    if from.x == to.x {
        if from.y < to.y {
            StepDirection::Down
        } else {
            StepDirection::Up
        }
    } else if from.x < to.x {
        StepDirection::Right
    } else {
        StepDirection::Left
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::label::GraphLabel;
    use crate::graph::model::{GraphNodeShape, GraphNodeStyle};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use merman_core::{OperationControl, OperationPhase};

    #[test]
    fn fixed_port_policy_does_not_substitute_directional_candidates() {
        let from = node("from", 0, 0);
        let blocker = node("blocker", 1, 2);
        let to = node("to", 5, 5);
        let layouts = vec![from.clone(), blocker, to.clone()];

        assert!(
            route_grid_path(
                &layouts,
                &from,
                &to,
                GridPathPortPolicy::Fixed(PortPair::new(Port::Down, Port::Left)),
            )
            .is_none()
        );

        let route = route_grid_path(
            &layouts,
            &from,
            &to,
            GridPathPortPolicy::Fixed(PortPair::new(Port::Right, Port::Up)),
        )
        .expect("secondary directional ports should be reachable");

        assert_eq!(route.ports, PortPair::new(Port::Right, Port::Up));
    }

    #[test]
    fn directional_shortest_policy_selects_reachable_directional_candidate() {
        let from = node("from", 0, 0);
        let blocker = node("blocker", 1, 2);
        let to = node("to", 5, 5);
        let layouts = vec![from.clone(), blocker, to.clone()];

        let route = route_grid_path(
            &layouts,
            &from,
            &to,
            GridPathPortPolicy::DirectionalShortest,
        )
        .expect("directional policy should select the reachable candidate");

        assert_eq!(route.ports, PortPair::new(Port::Right, Port::Up));
    }

    #[test]
    fn grid_frontier_expansion_honors_layout_work_limit() {
        let from = node("from", 0, 0);
        let blocker = node("blocker", 1, 2);
        let to = node("to", 5, 5);
        let layouts = vec![from.clone(), blocker, to.clone()];
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("frontier work limit should be valid");
        let mut resources = ResourceContext::new(policy);

        let error = route_grid_path_with_resources(
            &layouts,
            &from,
            &to,
            GridPathPortPolicy::DirectionalShortest,
            &mut resources,
        )
        .expect_err("frontier expansion should exceed one work unit");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert!(details.actual > details.max);
    }

    #[test]
    fn grid_path_cancellation_wins_before_the_next_work_debit() {
        let from = node("from", 0, 0);
        let blocker = node("blocker", 1, 2);
        let to = node("to", 5, 5);
        let layouts = vec![from.clone(), blocker, to.clone()];
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);

        let error = route_grid_path_with_resources_and_execution(
            &layouts,
            &from,
            &to,
            GridPathPortPolicy::DirectionalShortest,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("routing should observe cancellation before exhausting work");

        assert!(matches!(
            error,
            crate::AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 1);
    }

    #[test]
    fn heap_search_is_deterministic_for_equal_cost_detours() {
        let from = node("from", 0, 3);
        let blocker = node("blocker", 4, 3);
        let to = node("to", 8, 3);
        let layouts = vec![from.clone(), blocker, to.clone()];

        let expected = route_grid_path(
            &layouts,
            &from,
            &to,
            GridPathPortPolicy::Fixed(PortPair::new(Port::Right, Port::Left)),
        )
        .expect("one of the equal-cost detours should be reachable");

        for _ in 0..16 {
            let actual = route_grid_path(
                &layouts,
                &from,
                &to,
                GridPathPortPolicy::Fixed(PortPair::new(Port::Right, Port::Left)),
            )
            .expect("repeated equal-cost routing should stay reachable");
            assert_eq!(actual, expected);
        }
    }

    fn node(id: &str, grid_x: usize, grid_y: usize) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            label: GraphLabel::new(id),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            grid: GridCoord {
                x: grid_x,
                y: grid_y,
            },
            x: grid_x * 4,
            y: grid_y * 4,
            width: 3,
            height: 3,
        }
    }
}
