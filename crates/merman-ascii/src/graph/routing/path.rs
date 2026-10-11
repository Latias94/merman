use super::super::layout::{
    CanvasCoord, GraphLayout, GridCoord, GroupLayout, NodeLayout, ROUTING_GRID_MARGIN,
};
use super::super::model::GraphGroupKind;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::AsciiResourceLimitPhase;
use crate::resource::ResourceContext;
use merman_core::OperationPhase;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
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

#[cfg(test)]
pub(super) fn route_grid_path_with_resources_and_execution(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    port_policy: GridPathPortPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    route_grid_path_with_geometry(layouts, from, to, port_policy, None, resources, execution)
}

pub(super) fn route_grid_path_for_layout_with_resources_and_execution(
    layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    port_policy: GridPathPortPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    let geometry = (!layout.groups.is_empty()).then_some(GroupRouteGeometry { layout, from, to });
    route_grid_path_with_geometry(
        &layout.nodes,
        from,
        to,
        port_policy,
        geometry,
        resources,
        execution,
    )
}

#[allow(clippy::too_many_arguments)]
fn route_grid_path_with_geometry(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    port_policy: GridPathPortPolicy,
    geometry: Option<GroupRouteGeometry<'_>>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    match port_policy {
        GridPathPortPolicy::DirectionalShortest => select_shortest_reachable_grid_path(
            layouts,
            from,
            to,
            directional_left_right_port_pairs(from, to),
            geometry,
            resources,
            execution,
        ),
        GridPathPortPolicy::Fixed(ports) => {
            plan_grid_path_for_ports(layouts, from, to, ports, geometry, resources, execution)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_shortest_reachable_grid_path(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    candidates: [PortPair; 2],
    geometry: Option<GroupRouteGeometry<'_>>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    let mut selected: Option<GridPathRoute> = None;
    for ports in candidates {
        let Some(route) =
            plan_grid_path_for_ports(layouts, from, to, ports, geometry, resources, execution)?
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

#[allow(clippy::too_many_arguments)]
fn plan_grid_path_for_ports(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    ports: PortPair,
    geometry: Option<GroupRouteGeometry<'_>>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<GridPathRoute>> {
    let start = from.grid_for_port(ports.start, resources)?;
    let target = to.grid_for_port(ports.end, resources)?;
    let Some(path) = find_grid_path(
        layouts, start, target, ports, geometry, resources, execution,
    )?
    else {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum StepDirection {
    Up,
    Down,
    Left,
    Right,
}

impl StepDirection {
    pub(super) const fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
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

#[allow(clippy::too_many_arguments)]
fn find_grid_path(
    layouts: &[NodeLayout],
    start: GridCoord,
    target: GridCoord,
    ports: PortPair,
    geometry: Option<GroupRouteGeometry<'_>>,
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
    let max_x = resources.checked_grid_add(max_x, ROUTING_GRID_MARGIN)?;
    let max_y = resources.checked_grid_add(max_y, ROUTING_GRID_MARGIN)?;
    let occupied = occupied_grid_cells(layouts, resources, execution)?;
    let mut open = BinaryHeap::new();
    let mut cost_so_far = HashMap::default();
    let mut came_from = HashMap::<SearchPosition, SearchPosition>::default();
    let start_position = SearchPosition {
        coord: start,
        crossing_direction: None,
    };
    open.try_reserve(1)
        .map_err(|_| layout_allocation_failed())?;
    try_reserve_hash_map(&mut cost_so_far, 1)?;
    cost_so_far.insert(start_position, 0usize);
    open.push(OpenEntry {
        position: start_position,
        cost: 0,
        priority: grid_heuristic(start, target, resources)?,
        sequence: 0,
    });
    let mut sequence = 0usize;

    while let Some(entry) = open.pop() {
        checkpoint_layout(execution)?;
        resources.charge_layout_work(1)?;
        let current = entry.position.coord;
        if cost_so_far
            .get(&entry.position)
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
            let mut cursor = entry.position;
            while let Some(previous) = came_from.get(&cursor).copied() {
                checkpoint_layout(execution)?;
                resources.charge_layout_work(1)?;
                path.push(previous.coord);
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

            let next_position = if let Some(geometry) = geometry {
                let direction = step_direction(current, next);
                if entry
                    .position
                    .crossing_direction
                    .is_some_and(|incoming| incoming != direction)
                    || (current == start && direction != ports.start.terminal_direction())
                    || (next == target && direction != ports.end.terminal_direction().opposite())
                {
                    continue;
                }
                let Some(position) =
                    geometry.next_position(current, next, direction, resources, execution)?
                else {
                    continue;
                };
                position
            } else {
                SearchPosition {
                    coord: next,
                    crossing_direction: None,
                }
            };
            let new_cost = resources.checked_work_add(cost_so_far[&entry.position], 1)?;
            if cost_so_far
                .get(&next_position)
                .is_none_or(|current_cost| new_cost < *current_cost)
            {
                if !cost_so_far.contains_key(&next_position) {
                    try_reserve_hash_map(&mut cost_so_far, 1)?;
                    try_reserve_hash_map(&mut came_from, 1)?;
                }
                cost_so_far.insert(next_position, new_cost);
                let priority = resources
                    .checked_work_add(new_cost, grid_heuristic(next, target, resources)?)?;
                sequence = resources.checked_work_add(sequence, 1)?;
                open.try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
                open.push(OpenEntry {
                    position: next_position,
                    cost: new_cost,
                    priority,
                    sequence,
                });
                came_from.insert(next_position, entry.position);
            }
        }
    }

    Ok(None)
}

/// Only a waypoint on a crossed frame needs directional state. All other coarse cells keep
/// the original single search state; the no-group path retains its original work admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SearchPosition {
    coord: GridCoord,
    crossing_direction: Option<StepDirection>,
}

#[derive(Clone, Copy)]
struct GroupRouteGeometry<'a> {
    layout: &'a GraphLayout,
    from: &'a NodeLayout,
    to: &'a NodeLayout,
}

impl GroupRouteGeometry<'_> {
    /// A boundary waypoint can continue only on the same straight transverse segment.
    /// Checking each projected coarse step avoids both corner turns and strokes along a frame.
    fn next_position(
        self,
        from: GridCoord,
        to: GridCoord,
        direction: StepDirection,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Option<SearchPosition>> {
        let next_grid = to;
        let from = self.layout.grid_to_canvas(from);
        let to = self.layout.grid_to_canvas(to);
        let mut crossing_direction = None;
        for (index, group) in self.layout.groups.iter().enumerate() {
            execution.checkpoint_loop(OperationPhase::Layout, index)?;
            resources.charge_layout_work(1)?;
            if group.kind != GraphGroupKind::Container {
                continue;
            }
            let contains_from = group_contains_node(group, self.from);
            let contains_to = group_contains_node(group, self.to);
            if !group_allows_segment(group, contains_from, contains_to, from, to) {
                return Ok(None);
            }
            if group_perimeter_contains(group, to) {
                crossing_direction = Some(direction);
            }
        }
        Ok(Some(SearchPosition {
            coord: next_grid,
            crossing_direction,
        }))
    }
}

fn group_contains_node(group: &GroupLayout, node: &NodeLayout) -> bool {
    group.x < node.x
        && node.right() < group.right()
        && group.y < node.y
        && node.bottom() < group.bottom()
}

fn group_perimeter_contains(group: &GroupLayout, coord: CanvasCoord) -> bool {
    ((coord.x == group.x || coord.x == group.right())
        && (group.y..=group.bottom()).contains(&coord.y))
        || ((coord.y == group.y || coord.y == group.bottom())
            && (group.x..=group.right()).contains(&coord.x))
}

fn group_allows_segment(
    group: &GroupLayout,
    contains_from: bool,
    contains_to: bool,
    from: CanvasCoord,
    to: CanvasCoord,
) -> bool {
    let min_x = from.x.min(to.x);
    let max_x = from.x.max(to.x);
    let min_y = from.y.min(to.y);
    let max_y = from.y.max(to.y);
    if contains_from && contains_to {
        return group.x < min_x
            && max_x < group.right()
            && group.y < min_y
            && max_y < group.bottom();
    }
    let intersects =
        min_x <= group.right() && max_x >= group.x && min_y <= group.bottom() && max_y >= group.y;
    if !intersects {
        return true;
    }
    if !contains_from && !contains_to {
        return false;
    }
    if from.y == to.y {
        group.y < from.y && from.y < group.bottom()
    } else {
        group.x < from.x && from.x < group.right()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenEntry {
    position: SearchPosition,
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
            .then_with(|| other.position.coord.y.cmp(&self.position.coord.y))
            .then_with(|| other.position.coord.x.cmp(&self.position.coord.x))
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
    let mut occupied = HashSet::default();
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

    #[test]
    fn compound_steps_cross_owned_sides_transversely_without_touching_corners() {
        let group = group();
        for (from, to) in [
            ((5, 15), (15, 15)),
            ((15, 15), (25, 15)),
            ((15, 5), (15, 15)),
            ((15, 15), (15, 25)),
        ] {
            let from = CanvasCoord {
                x: from.0,
                y: from.1,
            };
            let to = CanvasCoord { x: to.0, y: to.1 };
            assert!(group_allows_segment(&group, true, false, from, to));
            assert!(group_allows_segment(&group, false, true, to, from));
            assert!(!group_allows_segment(&group, false, false, from, to));
            assert!(!group_allows_segment(&group, true, true, from, to));
        }
        for (from, to) in [
            ((5, 10), (15, 10)),
            ((15, 20), (25, 20)),
            ((10, 5), (10, 15)),
            ((20, 15), (20, 25)),
        ] {
            let from = CanvasCoord {
                x: from.0,
                y: from.1,
            };
            let to = CanvasCoord { x: to.0, y: to.1 };
            assert!(!group_allows_segment(&group, true, false, from, to));
        }
        let inside = CanvasCoord { x: 12, y: 15 };
        let other_inside = CanvasCoord { x: 18, y: 15 };
        assert!(group_allows_segment(
            &group,
            true,
            true,
            inside,
            other_inside
        ));
        assert!(!group_allows_segment(
            &group,
            false,
            false,
            inside,
            other_inside
        ));
    }

    fn group() -> GroupLayout {
        GroupLayout {
            id: "group".into(),
            kind: GraphGroupKind::Container,
            title: GraphLabel::empty_with_profile(crate::options::TerminalWidthProfile::Unicode),
            style: crate::graph::model::GraphGroupStyle::default(),
            divider_span: None,
            x: 10,
            y: 10,
            width: 11,
            height: 11,
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
