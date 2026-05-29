use super::charset::GraphCharset;
use super::layout::{CanvasCoord, GraphLayout, GridCoord, NodeLayout};
use super::model::{
    AsciiGraphEdge, GraphDirection, GraphEdgeArrow, GraphEdgeStroke, GraphNodeShape,
};
use crate::canvas::Canvas;
use crate::text::display_width;
use std::collections::HashSet;

mod path;

use path::{Port, StepDirection, merge_grid_path, route_grid_path, step_direction};

type RouteCells = HashSet<(usize, usize)>;

#[derive(Debug, Clone, Copy)]
struct EdgeLayouts<'a> {
    from: &'a NodeLayout,
    to: &'a NodeLayout,
}

pub(super) fn edge_canvas_extent(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    direction: GraphDirection,
) -> (usize, usize) {
    let mut width = 0;
    let mut height = 0;
    if direction != GraphDirection::LeftRight {
        return (width, height);
    }

    for edge in edges.iter().filter(|edge| edge.from == edge.to) {
        let Some(layout) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        width = width.max(self_loop_right_x(layouts, layout) + 1);
        height = height.max(self_loop_bottom_y(layout) + 1);
    }
    for edge in edges.iter().filter(|edge| edge.from != edge.to) {
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
            continue;
        };
        if from.center_y() == to.center_y() && from.x > to.x {
            width = width.max(from.center_x() + 1);
            height = height.max(left_right_back_edge_bottom_y(from) + 1);
        }
    }

    (width, height)
}

pub(super) fn draw_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    edge: &AsciiGraphEdge,
    direction: GraphDirection,
    charset: &GraphCharset,
) {
    let layouts = &graph_layout.nodes;
    let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
        return;
    };
    let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
        return;
    };

    match direction {
        GraphDirection::LeftRight => draw_left_right_edge(
            canvas,
            route_cells,
            graph_layout,
            edges,
            EdgeLayouts { from, to },
            edge,
            charset,
        ),
        GraphDirection::TopDown => draw_top_down_edge(canvas, route_cells, from, to, edge, charset),
    }

    draw_edge_label(canvas, from, to, edge, direction);
}

fn draw_left_right_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    endpoints: EdgeLayouts<'_>,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let from = endpoints.from;
    let to = endpoints.to;

    if from.id == to.id {
        draw_left_right_self_edge(
            canvas,
            route_cells,
            &graph_layout.nodes,
            from,
            edge,
            charset,
        );
        return;
    }

    if from.center_y() == to.center_y() && from.x > to.x {
        draw_left_right_back_edge(canvas, route_cells, from, to, edge, charset);
        return;
    }

    if draw_left_right_grid_path_edge(canvas, route_cells, graph_layout, from, to, edge, charset) {
        return;
    }

    if from.center_y() < to.center_y() && to.x > from.x {
        draw_left_right_down_then_right_edge(
            canvas,
            route_cells,
            &graph_layout.nodes,
            edges,
            endpoints,
            edge,
            charset,
        );
        return;
    }

    if from.center_y() < to.center_y() && to.x == from.x {
        draw_left_right_down_edge(canvas, route_cells, from, to, edge, charset);
        return;
    }

    if from.center_y() > to.center_y() && to.x > from.x {
        draw_left_right_right_then_up_edge(
            canvas,
            route_cells,
            &graph_layout.nodes,
            edges,
            endpoints,
            edge,
            charset,
        );
        return;
    }

    if to.x <= from.right() + 1 {
        return;
    }

    let y = from.center_y();
    if from.shape != GraphNodeShape::Diamond {
        canvas.set(from.right(), y, charset.right_connector);
    }
    let start = from.right() + 1;
    let end = to.x - 1;
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    for x in start..end {
        set_route_cell(canvas, route_cells, x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, end, y, line),
        GraphEdgeArrow::Point => canvas.set(end, y, charset.arrow_right),
    }
}

fn draw_left_right_grid_path_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> bool {
    let Some((path, start_port, end_port)) = route_grid_path(&graph_layout.nodes, from, to) else {
        return false;
    };
    if path.len() < 2 {
        return false;
    }

    let path = merge_grid_path(path);
    let (lines_drawn, line_dirs) =
        draw_grid_path(canvas, route_cells, graph_layout, &path, edge, charset);
    if lines_drawn.is_empty() || line_dirs.is_empty() {
        return false;
    }
    draw_grid_corners(canvas, route_cells, graph_layout, &path, charset);
    draw_grid_box_start(canvas, lines_drawn[0].as_slice(), start_port, charset);
    draw_grid_arrow_head(
        canvas,
        lines_drawn.last().map(Vec::as_slice).unwrap_or_default(),
        *line_dirs.last().unwrap_or(&end_port.step_fallback()),
        edge,
        charset,
    );
    true
}

fn draw_grid_path(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    graph_layout: &GraphLayout,
    path: &[GridCoord],
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> (Vec<Vec<CanvasCoord>>, Vec<StepDirection>) {
    let mut lines_drawn = Vec::new();
    let mut line_dirs = Vec::new();

    for segment in path.windows(2) {
        let direction = step_direction(segment[0], segment[1]);
        let line = draw_grid_line(
            canvas,
            route_cells,
            graph_layout.grid_to_canvas(segment[0]),
            graph_layout.grid_to_canvas(segment[1]),
            direction,
            edge,
            charset,
        );
        if !line.is_empty() {
            lines_drawn.push(line);
            line_dirs.push(direction);
        }
    }

    (lines_drawn, line_dirs)
}

fn draw_grid_line(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    from: CanvasCoord,
    to: CanvasCoord,
    direction: StepDirection,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Vec<CanvasCoord> {
    let mut drawn = Vec::new();
    match direction {
        StepDirection::Right => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in (from.x + 1)..to.x {
                set_route_cell(canvas, route_cells, x, from.y, line);
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Left => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in ((to.x + 1)..from.x).rev() {
                set_route_cell(canvas, route_cells, x, from.y, line);
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Down => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in (from.y + 1)..to.y {
                set_route_cell(canvas, route_cells, from.x, y, line);
                drawn.push(CanvasCoord { x: from.x, y });
            }
        }
        StepDirection::Up => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in ((to.y + 1)..from.y).rev() {
                set_route_cell(canvas, route_cells, from.x, y, line);
                drawn.push(CanvasCoord { x: from.x, y });
            }
        }
    }
    drawn
}

fn draw_grid_corners(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    graph_layout: &GraphLayout,
    path: &[GridCoord],
    charset: &GraphCharset,
) {
    for index in 1..path.len().saturating_sub(1) {
        let previous = step_direction(path[index - 1], path[index]);
        let next = step_direction(path[index], path[index + 1]);
        let coord = graph_layout.grid_to_canvas(path[index]);
        set_route_cell(
            canvas,
            route_cells,
            coord.x,
            coord.y,
            grid_corner_char(previous, next, charset),
        );
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

fn draw_grid_box_start(
    canvas: &mut Canvas,
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

    match start_port.step_fallback() {
        StepDirection::Up => canvas.set(from.x, from.y + 1, charset.up_connector),
        StepDirection::Down => canvas.set(from.x, from.y.saturating_sub(1), charset.down_connector),
        StepDirection::Left => canvas.set(from.x + 1, from.y, charset.left_connector),
        StepDirection::Right => {
            canvas.set(from.x.saturating_sub(1), from.y, charset.right_connector)
        }
    }
}

fn draw_grid_arrow_head(
    canvas: &mut Canvas,
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
    canvas.set(last.x, last.y, ch);
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

fn draw_left_right_back_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let start_x = from.center_x();
    let end_x = to.center_x();
    if start_x <= end_x {
        return;
    }

    let bottom_y = left_right_back_edge_bottom_y(from);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);

    canvas.set(start_x, from.bottom(), charset.down_connector);
    for y in (from.bottom() + 1)..bottom_y {
        set_route_cell(canvas, route_cells, start_x, y, vertical);
    }
    set_route_cell(canvas, route_cells, start_x, bottom_y, charset.bottom_right);

    for x in (end_x + 1)..start_x {
        set_route_cell(canvas, route_cells, x, bottom_y, horizontal);
    }
    set_route_cell(
        canvas,
        route_cells,
        end_x,
        bottom_y,
        charset.corner_down_right,
    );

    let arrow_y = bottom_y - 1;
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(end_x, arrow_y, vertical),
        GraphEdgeArrow::Point => canvas.set(end_x, arrow_y, charset.arrow_up),
    }
}

fn left_right_back_edge_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
}

fn draw_left_right_self_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    layouts: &[NodeLayout],
    from: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let y = from.center_y();
    let loop_x = self_loop_right_x(layouts, from);
    let bottom_y = self_loop_bottom_y(from);
    if loop_x <= from.right() || bottom_y <= y + 1 {
        return;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    if from.shape != GraphNodeShape::Diamond {
        canvas.set(from.right(), y, charset.right_connector);
    }
    for x in (from.right() + 1)..loop_x {
        set_route_cell(canvas, route_cells, x, y, horizontal);
    }
    let top_corner = if self_loop_has_right_neighbor(layouts, from) {
        charset.down_junction
    } else {
        charset.top_right
    };
    set_route_cell(canvas, route_cells, loop_x, y, top_corner);

    for line_y in (y + 1)..bottom_y {
        set_route_cell(canvas, route_cells, loop_x, line_y, vertical);
    }
    set_route_cell(canvas, route_cells, loop_x, bottom_y, charset.bottom_right);

    for x in (from.center_x() + 1)..loop_x {
        set_route_cell(canvas, route_cells, x, bottom_y, horizontal);
    }
    set_route_cell(
        canvas,
        route_cells,
        from.center_x(),
        bottom_y,
        charset.corner_down_right,
    );

    let arrow_y = bottom_y - 1;
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(from.center_x(), arrow_y, vertical),
        GraphEdgeArrow::Point => canvas.set(from.center_x(), arrow_y, charset.arrow_up),
    }
}

fn self_loop_has_right_neighbor(layouts: &[NodeLayout], from: &NodeLayout) -> bool {
    layouts.iter().any(|layout| {
        layout.id != from.id && layout.center_y() == from.center_y() && layout.x > from.x
    })
}

fn self_loop_right_x(layouts: &[NodeLayout], from: &NodeLayout) -> usize {
    layouts
        .iter()
        .filter(|layout| {
            layout.id != from.id && layout.center_y() == from.center_y() && layout.x > from.x
        })
        .map(|layout| layout.x)
        .min()
        .map(|right_x| (from.right() + right_x) / 2)
        .unwrap_or_else(|| from.right() + 2)
}

fn self_loop_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
}

fn draw_left_right_down_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if to.y <= from.bottom() + 1 {
        return;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    canvas.set(x, from.bottom(), charset.down_connector);
    for y in start..end {
        set_route_cell(canvas, route_cells, x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, x, end, line),
        GraphEdgeArrow::Point => canvas.set(x, end, charset.arrow_down),
    }
}

fn draw_left_right_down_then_right_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    endpoints: EdgeLayouts<'_>,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let from = endpoints.from;
    let to = endpoints.to;

    if !has_left_right_crossing_pair(layouts, edges, from, to) {
        draw_left_right_basic_down_then_right_edge(canvas, route_cells, from, to, edge, charset);
        return;
    }

    let source_x = from.center_x();
    let lane_x = lane_x_between(from, to);
    let lane_y = lane_y_between(from, to);
    if lane_y <= from.bottom() || to.x <= lane_x + 1 {
        return;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    canvas.set(source_x, from.bottom(), charset.down_connector);
    for y in (from.bottom() + 1)..lane_y {
        set_route_cell(canvas, route_cells, source_x, y, vertical);
    }
    set_route_cell(
        canvas,
        route_cells,
        source_x,
        lane_y,
        charset.corner_down_right,
    );

    for line_x in (source_x + 1)..lane_x {
        set_route_cell(canvas, route_cells, line_x, lane_y, horizontal);
    }
    set_route_cell(canvas, route_cells, lane_x, lane_y, charset.top_right);

    for y in (lane_y + 1)..to.center_y() {
        set_route_cell(canvas, route_cells, lane_x, y, vertical);
    }
    let end = to.x - 1;
    set_route_cell(
        canvas,
        route_cells,
        lane_x,
        to.center_y(),
        charset.corner_down_right,
    );
    for line_x in (lane_x + 1)..end {
        set_route_cell(canvas, route_cells, line_x, to.center_y(), horizontal);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, end, to.center_y(), horizontal),
        GraphEdgeArrow::Point => canvas.set(end, to.center_y(), charset.arrow_right),
    }
}

fn draw_left_right_right_then_up_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    endpoints: EdgeLayouts<'_>,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let from = endpoints.from;
    let to = endpoints.to;

    if !has_left_right_reverse_crossing_pair(layouts, edges, from, to) {
        draw_left_right_basic_right_then_up_edge(canvas, route_cells, from, to, edge, charset);
        return;
    }

    let source_x = from.center_x();
    let lane_x = lane_x_between(from, to);
    let lane_y = lane_y_between(to, from);
    if lane_x <= source_x || from.y <= lane_y || lane_y <= to.bottom() {
        return;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    canvas.set(source_x, from.y, charset.up_connector);
    for y in (lane_y + 1)..from.y {
        set_route_cell(canvas, route_cells, source_x, y, vertical);
    }
    set_route_cell(canvas, route_cells, source_x, lane_y, charset.top_left);

    for x in (source_x + 1)..lane_x {
        set_route_cell(canvas, route_cells, x, lane_y, horizontal);
    }
    set_route_cell(canvas, route_cells, lane_x, lane_y, charset.corner_right_up);

    for y in (to.center_y() + 1)..lane_y {
        set_route_cell(canvas, route_cells, lane_x, y, vertical);
    }
    set_route_cell(canvas, route_cells, lane_x, to.center_y(), charset.top_left);

    let end = to.x - 1;
    for x in (lane_x + 1)..end {
        set_route_cell(canvas, route_cells, x, to.center_y(), horizontal);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, end, to.center_y(), horizontal),
        GraphEdgeArrow::Point => canvas.set(end, to.center_y(), charset.arrow_right),
    }
}

fn draw_left_right_basic_down_then_right_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let x = from.center_x();
    let corner_y = to.center_y();
    if corner_y <= from.bottom() || to.x <= x + 1 {
        return;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    canvas.set(x, from.bottom(), charset.down_connector);
    for y in (from.bottom() + 1)..corner_y {
        set_route_cell(canvas, route_cells, x, y, vertical);
    }
    set_route_cell(canvas, route_cells, x, corner_y, charset.corner_down_right);

    let end = to.x - 1;
    for line_x in (x + 1)..end {
        set_route_cell(canvas, route_cells, line_x, corner_y, horizontal);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, end, corner_y, horizontal),
        GraphEdgeArrow::Point => canvas.set(end, corner_y, charset.arrow_right),
    }
}

fn draw_left_right_basic_right_then_up_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let y = from.center_y();
    let corner_x = to.center_x();
    if corner_x <= from.right() || y <= to.bottom() + 1 {
        return;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    canvas.set(from.right(), y, charset.right_connector);
    for x in (from.right() + 1)..corner_x {
        set_route_cell(canvas, route_cells, x, y, horizontal);
    }
    set_route_cell(canvas, route_cells, corner_x, y, charset.corner_right_up);

    let arrow_y = to.bottom() + 1;
    for line_y in (arrow_y + 1)..y {
        set_route_cell(canvas, route_cells, corner_x, line_y, vertical);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, corner_x, arrow_y, vertical),
        GraphEdgeArrow::Point => canvas.set(corner_x, arrow_y, charset.arrow_up),
    }
}

fn has_left_right_crossing_pair(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    upper_source: &NodeLayout,
    lower_target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        let Some(other_source) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        let Some(other_target) = layouts.iter().find(|layout| layout.id == edge.to) else {
            return false;
        };
        other_source.x == upper_source.x
            && other_target.x == lower_target.x
            && other_source.center_y() > upper_source.center_y()
            && other_target.center_y() < lower_target.center_y()
    })
}

fn has_left_right_reverse_crossing_pair(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    lower_source: &NodeLayout,
    upper_target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        let Some(other_source) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        let Some(other_target) = layouts.iter().find(|layout| layout.id == edge.to) else {
            return false;
        };
        other_source.x == lower_source.x
            && other_target.x == upper_target.x
            && other_source.center_y() < lower_source.center_y()
            && other_target.center_y() > upper_target.center_y()
    })
}

fn draw_top_down_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if to.y <= from.bottom() + 1 {
        return;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    for y in start..end {
        set_route_cell(canvas, route_cells, x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, x, end, line),
        GraphEdgeArrow::Point => canvas.set(x, end, charset.arrow_down),
    }
}

fn lane_x_between(from: &NodeLayout, to: &NodeLayout) -> usize {
    if from.x < to.x {
        (from.right() + to.x) / 2
    } else {
        (to.right() + from.x) / 2
    }
}

fn lane_y_between(upper: &NodeLayout, lower: &NodeLayout) -> usize {
    (upper.bottom() + lower.y) / 2
}

fn set_route_cell(canvas: &mut Canvas, route_cells: &mut RouteCells, x: usize, y: usize, ch: char) {
    let Some(existing) = canvas.get(x, y) else {
        return;
    };
    let merged = if route_cells.contains(&(x, y)) || is_arrow(existing) {
        merge_route_chars(existing, ch)
    } else {
        ch
    };
    canvas.set(x, y, merged);
    route_cells.insert((x, y));
}

fn merge_route_chars(existing: char, incoming: char) -> char {
    if existing == ' ' || existing == incoming {
        return incoming;
    }
    if incoming == ' ' {
        return existing;
    }
    if is_arrow(incoming) {
        return incoming;
    }
    if is_arrow(existing) {
        return existing;
    }
    if is_ascii_route_char(existing) || is_ascii_route_char(incoming) {
        return merge_ascii_route_chars(existing, incoming);
    }

    let existing_dirs = unicode_route_dirs(existing);
    let incoming_dirs = unicode_route_dirs(incoming);
    if existing_dirs == 0 || incoming_dirs == 0 {
        return incoming;
    }
    unicode_route_char(existing_dirs | incoming_dirs)
}

fn is_arrow(ch: char) -> bool {
    matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼')
}

fn is_ascii_route_char(ch: char) -> bool {
    matches!(ch, '-' | '|' | '+')
}

fn merge_ascii_route_chars(existing: char, incoming: char) -> char {
    match (existing, incoming) {
        (' ', ch) | (ch, ' ') => ch,
        ('-', '-') => '-',
        ('|', '|') => '|',
        ('+' | '-' | '|', '+' | '-' | '|') => '+',
        (_, ch) => ch,
    }
}

const DIR_UP: u8 = 1;
const DIR_RIGHT: u8 = 2;
const DIR_DOWN: u8 = 4;
const DIR_LEFT: u8 = 8;

fn unicode_route_dirs(ch: char) -> u8 {
    match ch {
        '─' | '┄' => DIR_LEFT | DIR_RIGHT,
        '│' | '┆' => DIR_UP | DIR_DOWN,
        '┌' | '╭' => DIR_RIGHT | DIR_DOWN,
        '┐' | '╮' => DIR_LEFT | DIR_DOWN,
        '└' | '╰' => DIR_UP | DIR_RIGHT,
        '┘' | '╯' => DIR_UP | DIR_LEFT,
        '├' => DIR_UP | DIR_RIGHT | DIR_DOWN,
        '┤' => DIR_UP | DIR_DOWN | DIR_LEFT,
        '┬' => DIR_RIGHT | DIR_DOWN | DIR_LEFT,
        '┴' => DIR_UP | DIR_RIGHT | DIR_LEFT,
        '┼' => DIR_UP | DIR_RIGHT | DIR_DOWN | DIR_LEFT,
        _ => 0,
    }
}

fn unicode_route_char(dirs: u8) -> char {
    match dirs {
        dirs if dirs == (DIR_LEFT | DIR_RIGHT) => '─',
        dirs if dirs == (DIR_UP | DIR_DOWN) => '│',
        dirs if dirs == (DIR_RIGHT | DIR_DOWN) => '┌',
        dirs if dirs == (DIR_DOWN | DIR_LEFT) => '┐',
        dirs if dirs == (DIR_UP | DIR_RIGHT) => '└',
        dirs if dirs == (DIR_UP | DIR_LEFT) => '┘',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_DOWN) => '├',
        dirs if dirs == (DIR_UP | DIR_DOWN | DIR_LEFT) => '┤',
        dirs if dirs == (DIR_RIGHT | DIR_DOWN | DIR_LEFT) => '┬',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_LEFT) => '┴',
        dirs if dirs == (DIR_UP | DIR_RIGHT | DIR_DOWN | DIR_LEFT) => '┼',
        _ => '┼',
    }
}

fn edge_line_char(
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    direction: GraphDirection,
) -> char {
    match (edge.stroke, direction) {
        (GraphEdgeStroke::Normal, GraphDirection::LeftRight) => charset.horizontal,
        (GraphEdgeStroke::Normal, GraphDirection::TopDown) => charset.vertical,
        (GraphEdgeStroke::Dotted, GraphDirection::LeftRight) => charset.dotted_horizontal,
        (GraphEdgeStroke::Dotted, GraphDirection::TopDown) => charset.dotted_vertical,
    }
}

fn draw_edge_label(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    direction: GraphDirection,
) {
    let Some(label) = edge.label.as_deref() else {
        return;
    };

    match direction {
        GraphDirection::LeftRight => {
            let start = from.right() + 1;
            let end = to.x.saturating_sub(1);
            let available = end.saturating_sub(start).saturating_add(1);
            let width = display_width(label);
            let x = start + available.saturating_sub(width) / 2;
            canvas.write_text(x, from.y.saturating_sub(1), label);
        }
        GraphDirection::TopDown => {
            let start = from.bottom() + 1;
            let end = to.y.saturating_sub(1);
            let available = end.saturating_sub(start).saturating_add(1);
            let y = start + available / 2;
            let width = display_width(label);
            let x = from.center_x().saturating_sub(width / 2);
            canvas.write_text(x, y, label);
        }
    }
}
