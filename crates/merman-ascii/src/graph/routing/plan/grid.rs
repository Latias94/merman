use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, GraphLayout, GridCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow};
use super::super::cell::edge_line_char;
use super::super::path::{Port, StepDirection, merge_grid_path, route_grid_path, step_direction};
use super::{
    PlannedRouteCell, RoutePlan, edge_arrow_cell, edge_line_cell, planned_label_on_canvas_lines,
    route_cell,
};

pub(in crate::graph::routing) fn plan_left_right_grid_path_route(
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let (path, start_port, end_port) = route_grid_path(&graph_layout.nodes, from, to)?;
    if path.len() < 2 {
        return None;
    }

    let path = merge_grid_path(path);
    let (mut cells, lines_drawn, line_dirs) = plan_grid_path(graph_layout, &path, edge, charset);
    if lines_drawn.is_empty() || line_dirs.is_empty() {
        return None;
    }
    plan_grid_corners(&mut cells, graph_layout, &path, charset);
    plan_grid_box_start(&mut cells, lines_drawn[0].as_slice(), start_port, charset);
    plan_grid_arrow_head(
        &mut cells,
        lines_drawn.last().map(Vec::as_slice).unwrap_or_default(),
        *line_dirs.last().unwrap_or(&end_port.step_fallback()),
        edge,
        charset,
    );
    let labels = planned_label_on_canvas_lines(edge.label.as_deref(), &lines_drawn)
        .into_iter()
        .collect();

    Some(RoutePlan { cells, labels })
}

fn plan_grid_path(
    graph_layout: &GraphLayout,
    path: &[GridCoord],
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> (
    Vec<PlannedRouteCell>,
    Vec<Vec<CanvasCoord>>,
    Vec<StepDirection>,
) {
    let mut cells = Vec::new();
    let mut lines_drawn = Vec::new();
    let mut line_dirs = Vec::new();

    for segment in path.windows(2) {
        let direction = step_direction(segment[0], segment[1]);
        let (line_cells, line) = plan_grid_line(
            graph_layout.grid_to_canvas(segment[0]),
            graph_layout.grid_to_canvas(segment[1]),
            direction,
            edge,
            charset,
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
) -> (Vec<PlannedRouteCell>, Vec<CanvasCoord>) {
    let mut cells = Vec::new();
    let mut drawn = Vec::new();
    match direction {
        StepDirection::Right => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in (from.x + 1)..to.x {
                cells.push(route_cell(x, from.y, line));
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Left => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in ((to.x + 1)..from.x).rev() {
                cells.push(route_cell(x, from.y, line));
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Down => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in (from.y + 1)..to.y {
                cells.push(route_cell(from.x, y, line));
                drawn.push(CanvasCoord { x: from.x, y });
            }
        }
        StepDirection::Up => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in ((to.y + 1)..from.y).rev() {
                cells.push(route_cell(from.x, y, line));
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
) {
    for index in 1..path.len().saturating_sub(1) {
        let previous = step_direction(path[index - 1], path[index]);
        let next = step_direction(path[index], path[index + 1]);
        let coord = graph_layout.grid_to_canvas(path[index]);
        cells.push(route_cell(
            coord.x,
            coord.y,
            grid_corner_char(previous, next, charset),
        ));
    }
}

fn grid_corner_char(previous: StepDirection, next: StepDirection, charset: &GraphCharset) -> char {
    if !charset.unicode {
        return '+';
    }

    match (previous, next) {
        (StepDirection::Right, StepDirection::Down) | (StepDirection::Up, StepDirection::Left) => {
            charset.top_right
        }
        (StepDirection::Right, StepDirection::Up) | (StepDirection::Down, StepDirection::Left) => {
            charset.corner_right_up
        }
        (StepDirection::Left, StepDirection::Down) | (StepDirection::Up, StepDirection::Right) => {
            charset.top_left
        }
        (StepDirection::Left, StepDirection::Up) | (StepDirection::Down, StepDirection::Right) => {
            charset.corner_down_right
        }
        _ => '+',
    }
}

fn plan_grid_box_start(
    cells: &mut Vec<PlannedRouteCell>,
    first_line: &[CanvasCoord],
    start_port: Port,
    charset: &GraphCharset,
) {
    if !charset.unicode {
        return;
    }
    let Some(from) = first_line.first().copied() else {
        return;
    };

    let cell = match start_port.step_fallback() {
        StepDirection::Up => edge_line_cell(from.x, from.y + 1, charset.up_connector),
        StepDirection::Down => {
            edge_line_cell(from.x, from.y.saturating_sub(1), charset.down_connector)
        }
        StepDirection::Left => edge_line_cell(from.x + 1, from.y, charset.left_connector),
        StepDirection::Right => {
            edge_line_cell(from.x.saturating_sub(1), from.y, charset.right_connector)
        }
    };
    cells.push(cell);
}

fn plan_grid_arrow_head(
    cells: &mut Vec<PlannedRouteCell>,
    last_line: &[CanvasCoord],
    fallback: StepDirection,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
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
        .unwrap_or(fallback);
    let ch = match direction {
        StepDirection::Up => charset.arrow_up,
        StepDirection::Down => charset.arrow_down,
        StepDirection::Left => charset.arrow_left,
        StepDirection::Right => charset.arrow_right,
    };
    cells.push(edge_arrow_cell(last.x, last.y, ch));
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
