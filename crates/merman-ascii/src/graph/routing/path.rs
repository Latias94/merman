use super::super::layout::{GridCoord, NodeLayout};

pub(super) fn route_grid_path_with_ports(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    start_override: Option<Port>,
    end_override: Option<Port>,
) -> Option<(Vec<GridCoord>, Port, Port)> {
    let (preferred_start, preferred_end, alternative_start, alternative_end) =
        determine_left_right_ports(from, to);
    let preferred_start = start_override.unwrap_or(preferred_start);
    let preferred_end = end_override.unwrap_or(preferred_end);
    let alternative_start = start_override.unwrap_or(alternative_start);
    let alternative_end = end_override.unwrap_or(alternative_end);
    let preferred_path = find_grid_path(
        layouts,
        from.grid_for_port(preferred_start),
        to.grid_for_port(preferred_end),
    )
    .map(merge_grid_path);
    let alternative_path = find_grid_path(
        layouts,
        from.grid_for_port(alternative_start),
        to.grid_for_port(alternative_end),
    )
    .map(merge_grid_path);

    match (preferred_path, alternative_path) {
        (Some(preferred), Some(alternative)) if alternative.len() < preferred.len() => {
            Some((alternative, alternative_start, alternative_end))
        }
        (Some(preferred), _) => Some((preferred, preferred_start, preferred_end)),
        (None, Some(alternative)) => Some((alternative, alternative_start, alternative_end)),
        (None, None) => None,
    }
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

    pub(super) fn step_fallback(self) -> StepDirection {
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
    fn grid_for_port(&self, port: Port) -> GridCoord;
}

impl NodeGridPort for NodeLayout {
    fn grid_for_port(&self, port: Port) -> GridCoord {
        let (x, y) = port.offset();
        GridCoord {
            x: self.grid.x + x,
            y: self.grid.y + y,
        }
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

fn determine_left_right_ports(from: &NodeLayout, to: &NodeLayout) -> (Port, Port, Port, Port) {
    match relative_direction(from.grid, to.grid) {
        RelativeDirection::LowerRight => (Port::Down, Port::Left, Port::Right, Port::Up),
        RelativeDirection::UpperRight => (Port::Up, Port::Left, Port::Right, Port::Down),
        RelativeDirection::LowerLeft => (Port::Down, Port::Down, Port::Left, Port::Up),
        RelativeDirection::UpperLeft => (Port::Down, Port::Down, Port::Left, Port::Down),
        RelativeDirection::Left => (Port::Down, Port::Down, Port::Left, Port::Right),
        RelativeDirection::Right => (Port::Right, Port::Left, Port::Right, Port::Left),
        RelativeDirection::Down => (Port::Down, Port::Up, Port::Down, Port::Up),
        RelativeDirection::Up => (Port::Up, Port::Down, Port::Up, Port::Down),
        RelativeDirection::Middle => (Port::Middle, Port::Middle, Port::Middle, Port::Middle),
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
) -> Option<Vec<GridCoord>> {
    let max_x = layouts
        .iter()
        .map(|layout| layout.grid.x + 2)
        .max()
        .unwrap_or_default()
        + 6;
    let max_y = layouts
        .iter()
        .map(|layout| layout.grid.y + 2)
        .max()
        .unwrap_or_default()
        + 6;
    let mut open = vec![(start, 0usize)];
    let mut cost_so_far = std::collections::HashMap::from([(start, 0usize)]);
    let mut came_from = std::collections::HashMap::<GridCoord, GridCoord>::new();

    while !open.is_empty() {
        let best_index = open
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, priority))| *priority)
            .map(|(index, _)| index)
            .unwrap_or_default();
        let (current, _) = open.remove(best_index);
        if current == target {
            let mut path = vec![current];
            let mut cursor = current;
            while let Some(previous) = came_from.get(&cursor).copied() {
                path.insert(0, previous);
                cursor = previous;
            }
            return Some(path);
        }

        for next in grid_neighbors(current, max_x, max_y) {
            if grid_occupied(layouts, next) && next != target {
                continue;
            }

            let new_cost = cost_so_far[&current] + 1;
            if cost_so_far
                .get(&next)
                .is_none_or(|current_cost| new_cost < *current_cost)
            {
                cost_so_far.insert(next, new_cost);
                let priority = new_cost + grid_heuristic(next, target);
                open.push((next, priority));
                came_from.insert(next, current);
            }
        }
    }

    None
}

fn grid_neighbors(coord: GridCoord, max_x: usize, max_y: usize) -> Vec<GridCoord> {
    let mut neighbors = Vec::with_capacity(4);
    if coord.x < max_x {
        neighbors.push(GridCoord {
            x: coord.x + 1,
            y: coord.y,
        });
    }
    if coord.x > 0 {
        neighbors.push(GridCoord {
            x: coord.x - 1,
            y: coord.y,
        });
    }
    if coord.y < max_y {
        neighbors.push(GridCoord {
            x: coord.x,
            y: coord.y + 1,
        });
    }
    if coord.y > 0 {
        neighbors.push(GridCoord {
            x: coord.x,
            y: coord.y - 1,
        });
    }
    neighbors
}

fn grid_heuristic(a: GridCoord, b: GridCoord) -> usize {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    if dx == 0 || dy == 0 {
        dx + dy
    } else {
        dx + dy + 1
    }
}

fn grid_occupied(layouts: &[NodeLayout], coord: GridCoord) -> bool {
    layouts.iter().any(|layout| {
        (layout.grid.x..=(layout.grid.x + 2)).contains(&coord.x)
            && (layout.grid.y..=(layout.grid.y + 2)).contains(&coord.y)
    })
}

pub(super) fn merge_grid_path(path: Vec<GridCoord>) -> Vec<GridCoord> {
    if path.len() <= 2 {
        return path;
    }

    let mut merged = Vec::with_capacity(path.len());
    merged.push(path[0]);
    for window in path.windows(3) {
        let previous = step_direction(window[0], window[1]);
        let next = step_direction(window[1], window[2]);
        if previous != next {
            merged.push(window[1]);
        }
    }
    merged.push(*path.last().expect("path has at least one element"));
    merged
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
