use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, GraphLayout, GridCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow};
use super::super::cell::edge_line_char;
use super::super::label::{
    RoutedLabelPlacement, routed_label_placement, routed_label_right_of_vertical_route_placement,
};
use super::super::path::{
    Port, StepDirection, merge_grid_path, route_grid_path_with_ports, step_direction,
};
use super::{
    PlannedRouteCell, PlannedRouteLabel, PlannedRouteSegment, RoutePlan,
    edge_arrow_cell_in_segment, edge_line_cell_in_segment, route_cell_in_segment, route_turn_char,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct GridRouteOptions {
    start_port: Option<Port>,
    end_port: Option<Port>,
    segment: PlannedRouteSegment,
    label_mode: GridRouteLabelMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridRouteLabelMode {
    InlineLongestSegment,
    FirstVerticalTransitLane,
    LastVerticalTransitLane,
}

impl GridRouteOptions {
    pub(super) fn direct() -> Self {
        Self {
            start_port: None,
            end_port: None,
            segment: PlannedRouteSegment::Direct,
            label_mode: GridRouteLabelMode::InlineLongestSegment,
        }
    }

    pub(super) fn with_ports(start_port: Option<Port>, end_port: Option<Port>) -> Self {
        Self {
            start_port,
            end_port,
            segment: PlannedRouteSegment::Direct,
            label_mode: GridRouteLabelMode::InlineLongestSegment,
        }
    }

    pub(super) fn with_segment(mut self, segment: PlannedRouteSegment) -> Self {
        self.segment = segment;
        self
    }

    pub(super) fn with_first_vertical_transit_label(mut self) -> Self {
        self.label_mode = GridRouteLabelMode::FirstVerticalTransitLane;
        self
    }

    pub(super) fn with_last_vertical_transit_label(mut self) -> Self {
        self.label_mode = GridRouteLabelMode::LastVerticalTransitLane;
        self
    }
}

pub(super) fn plan_left_right_grid_path_route(
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    plan_left_right_grid_path_route_with_options(
        graph_layout,
        from,
        to,
        edge,
        charset,
        GridRouteOptions::direct(),
    )
}

pub(super) fn plan_left_right_grid_path_route_with_options(
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    options: GridRouteOptions,
) -> Option<RoutePlan> {
    let (path, start_port, end_port) = route_grid_path_with_ports(
        &graph_layout.nodes,
        from,
        to,
        options.start_port,
        options.end_port,
    )?;
    if path.len() < 2 {
        return None;
    }

    let path = merge_grid_path(path);
    let segment = options.segment;
    let (mut cells, lines_drawn, line_dirs) =
        plan_grid_path(graph_layout, &path, edge, charset, segment);
    if lines_drawn.is_empty() || line_dirs.is_empty() {
        return None;
    }
    plan_grid_corners(&mut cells, graph_layout, &path, charset, segment);
    plan_grid_box_start(
        &mut cells,
        lines_drawn[0].as_slice(),
        start_port,
        charset,
        segment,
    );
    plan_grid_arrow_head(
        &mut cells,
        lines_drawn.last().map(Vec::as_slice).unwrap_or_default(),
        *line_dirs.last().unwrap_or(&end_port.terminal_direction()),
        edge,
        charset,
        segment,
    );
    let labels = planned_grid_label(
        edge.label.as_deref(),
        &lines_drawn,
        &line_dirs,
        options.label_mode,
    )
    .into_iter()
    .collect();

    Some(RoutePlan { cells, labels })
}

fn planned_grid_label(
    label: Option<&str>,
    lines: &[Vec<CanvasCoord>],
    directions: &[StepDirection],
    mode: GridRouteLabelMode,
) -> Option<PlannedRouteLabel> {
    let label = label.filter(|label| !label.is_empty())?;
    let (line, direction) = grid_label_line(lines, directions, mode)?;
    let first = line.first().copied()?;
    let last = line.last().copied()?;
    let placement = grid_label_placement(label, first, last, mode, direction)?;
    Some(PlannedRouteLabel {
        text: label.to_string(),
        placement,
    })
}

fn grid_label_line<'a>(
    lines: &'a [Vec<CanvasCoord>],
    directions: &[StepDirection],
    mode: GridRouteLabelMode,
) -> Option<(&'a Vec<CanvasCoord>, StepDirection)> {
    let candidates = lines.iter().zip(directions.iter().copied());
    match mode {
        GridRouteLabelMode::InlineLongestSegment => candidates.max_by_key(|(line, _)| line.len()),
        GridRouteLabelMode::FirstVerticalTransitLane => first_vertical_grid_label_line(candidates),
        GridRouteLabelMode::LastVerticalTransitLane => last_vertical_grid_label_line(candidates),
    }
}

fn first_vertical_grid_label_line<'a>(
    mut candidates: impl Iterator<Item = (&'a Vec<CanvasCoord>, StepDirection)>,
) -> Option<(&'a Vec<CanvasCoord>, StepDirection)> {
    candidates.find(|(_, direction)| matches!(direction, StepDirection::Up | StepDirection::Down))
}

fn last_vertical_grid_label_line<'a>(
    candidates: impl Iterator<Item = (&'a Vec<CanvasCoord>, StepDirection)>,
) -> Option<(&'a Vec<CanvasCoord>, StepDirection)> {
    candidates
        .filter(|(_, direction)| matches!(direction, StepDirection::Up | StepDirection::Down))
        .last()
}

fn grid_label_placement(
    label: &str,
    first: CanvasCoord,
    last: CanvasCoord,
    mode: GridRouteLabelMode,
    direction: StepDirection,
) -> Option<RoutedLabelPlacement> {
    match mode {
        GridRouteLabelMode::InlineLongestSegment => routed_label_placement(first, last, label),
        GridRouteLabelMode::FirstVerticalTransitLane
        | GridRouteLabelMode::LastVerticalTransitLane => match direction {
            StepDirection::Up | StepDirection::Down => {
                routed_label_right_of_vertical_route_placement(first, last, label)
            }
            StepDirection::Left | StepDirection::Right => None,
        },
    }
}

fn plan_grid_path(
    graph_layout: &GraphLayout,
    path: &[GridCoord],
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
) -> (
    Vec<PlannedRouteCell>,
    Vec<Vec<CanvasCoord>>,
    Vec<StepDirection>,
) {
    let mut cells = Vec::new();
    let mut lines_drawn = Vec::new();
    let mut line_dirs = Vec::new();

    for path_segment in path.windows(2) {
        let direction = step_direction(path_segment[0], path_segment[1]);
        let (line_cells, line) = plan_grid_line(
            graph_layout.grid_to_canvas(path_segment[0]),
            graph_layout.grid_to_canvas(path_segment[1]),
            direction,
            edge,
            charset,
            segment,
        );
        cells.extend(line_cells);
        if !line.is_empty() {
            lines_drawn.push(line);
            line_dirs.push(direction);
        }
    }

    (cells, lines_drawn, line_dirs)
}

fn plan_grid_line(
    from: CanvasCoord,
    to: CanvasCoord,
    direction: StepDirection,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
) -> (Vec<PlannedRouteCell>, Vec<CanvasCoord>) {
    let mut cells = Vec::new();
    let mut drawn = Vec::new();
    match direction {
        StepDirection::Right => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in (from.x + 1)..to.x {
                cells.push(route_cell_in_segment(x, from.y, line, segment));
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Left => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in ((to.x + 1)..from.x).rev() {
                cells.push(route_cell_in_segment(x, from.y, line, segment));
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Down => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in (from.y + 1)..to.y {
                cells.push(route_cell_in_segment(from.x, y, line, segment));
                drawn.push(CanvasCoord { x: from.x, y });
            }
        }
        StepDirection::Up => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in ((to.y + 1)..from.y).rev() {
                cells.push(route_cell_in_segment(from.x, y, line, segment));
                drawn.push(CanvasCoord { x: from.x, y });
            }
        }
    }
    (cells, drawn)
}

fn plan_grid_corners(
    cells: &mut Vec<PlannedRouteCell>,
    graph_layout: &GraphLayout,
    path: &[GridCoord],
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
) {
    for index in 1..path.len().saturating_sub(1) {
        let previous = step_direction(path[index - 1], path[index]);
        let next = step_direction(path[index], path[index + 1]);
        let coord = graph_layout.grid_to_canvas(path[index]);
        cells.push(route_cell_in_segment(
            coord.x,
            coord.y,
            route_turn_char(previous, next, charset),
            segment,
        ));
    }
}

fn plan_grid_box_start(
    cells: &mut Vec<PlannedRouteCell>,
    first_line: &[CanvasCoord],
    start_port: Port,
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
) {
    if !charset.unicode {
        return;
    }
    let Some(from) = first_line.first().copied() else {
        return;
    };

    let cell = match start_port.terminal_direction() {
        StepDirection::Up => {
            edge_line_cell_in_segment(from.x, from.y + 1, charset.up_connector, segment)
        }
        StepDirection::Down => edge_line_cell_in_segment(
            from.x,
            from.y.saturating_sub(1),
            charset.down_connector,
            segment,
        ),
        StepDirection::Left => {
            edge_line_cell_in_segment(from.x + 1, from.y, charset.left_connector, segment)
        }
        StepDirection::Right => edge_line_cell_in_segment(
            from.x.saturating_sub(1),
            from.y,
            charset.right_connector,
            segment,
        ),
    };
    cells.push(cell);
}

fn plan_grid_arrow_head(
    cells: &mut Vec<PlannedRouteCell>,
    last_line: &[CanvasCoord],
    default_direction: StepDirection,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
) {
    if edge.arrow == GraphEdgeArrow::Open {
        return;
    }
    let Some(last) = last_line.last().copied() else {
        return;
    };
    let direction = last_line
        .first()
        .and_then(|first| canvas_line_direction(*first, last))
        .unwrap_or(default_direction);
    let ch = match direction {
        StepDirection::Up => charset.arrow_up,
        StepDirection::Down => charset.arrow_down,
        StepDirection::Left => charset.arrow_left,
        StepDirection::Right => charset.arrow_right,
    };
    cells.push(edge_arrow_cell_in_segment(last.x, last.y, ch, segment));
}

fn canvas_line_direction(from: CanvasCoord, to: CanvasCoord) -> Option<StepDirection> {
    if from.x == to.x {
        if from.y < to.y {
            Some(StepDirection::Down)
        } else if from.y > to.y {
            Some(StepDirection::Up)
        } else {
            None
        }
    } else if from.x < to.x {
        Some(StepDirection::Right)
    } else {
        Some(StepDirection::Left)
    }
}
