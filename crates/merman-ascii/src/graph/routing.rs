use super::charset::GraphCharset;
use super::layout::{CanvasCoord, GraphLayout, GridCoord, NodeLayout};
use super::model::{
    AsciiGraphEdge, GraphDirection, GraphEdgeArrow, GraphEdgeStyle, GraphNodeShape,
};
use crate::canvas::{Canvas, CanvasColor};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::text::display_width;

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{edge_line_char, set_edge_arrow, set_edge_line, set_route_cell};
pub(super) use label::{EdgeLabel, draw_routed_label};
use label::{
    push_label_on_canvas_lines, push_label_on_horizontal_line, push_label_on_vertical_line,
};
use path::{Port, StepDirection, merge_grid_path, route_grid_path, step_direction};
use plan::{PlannedRouteCellKind, RoutePlan, plan_top_down_direct_route};

pub(super) struct RouteDrawing<'a> {
    canvas: &'a mut Canvas,
    route_cells: &'a mut RouteCells,
    labels: &'a mut Vec<EdgeLabel>,
}

impl<'a> RouteDrawing<'a> {
    pub(super) fn new(
        canvas: &'a mut Canvas,
        route_cells: &'a mut RouteCells,
        labels: &'a mut Vec<EdgeLabel>,
    ) -> Self {
        Self {
            canvas,
            route_cells,
            labels,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EdgeLayouts<'a> {
    from: &'a NodeLayout,
    to: &'a NodeLayout,
}

#[derive(Debug, Clone, Copy)]
struct EdgeContext {
    parallel_index: usize,
}

pub(super) fn edge_canvas_extent(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    direction: GraphDirection,
) -> (usize, usize) {
    let mut width = 0;
    let mut height = 0;

    for edge in edges.iter().filter(|edge| edge.from == edge.to) {
        let Some(layout) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        width = width.max(self_loop_right_x(layouts, layout) + 1);
        height = height.max(self_loop_bottom_y_for_edges(layouts, edges, layout) + 1);
    }
    for (edge_index, edge) in edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.from != edge.to)
    {
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
            continue;
        };
        let context = edge_context(edges, edge_index);
        match direction.canonical() {
            GraphDirection::LeftRight => {
                if from.center_y() == to.center_y() && (from.x > to.x || context.parallel_index > 0)
                {
                    width = width.max(from.center_x().max(to.center_x()) + 3);
                    height = height.max(left_right_back_edge_bottom_y(from) + 1);
                }
            }
            GraphDirection::TopDown => {
                if from.center_y() > to.center_y() {
                    let lane_x = top_down_back_edge_lane_x(from, to);
                    width = width.max(lane_x + 3);
                    if let Some(label) = edge.label.as_deref() {
                        let label_start = lane_x.saturating_sub(display_width(label) / 2);
                        width = width.max(label_start + display_width(label) + 1);
                    }
                }
            }
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    (width, height)
}

pub(super) fn transform_routed_label(
    label: &EdgeLabel,
    mut transform: impl FnMut(CanvasCoord) -> CanvasCoord,
) -> EdgeLabel {
    EdgeLabel {
        start: transform(label.start),
        end: transform(label.end),
        text: label.text.clone(),
        color: label.color,
    }
}

pub(super) fn draw_edge(
    drawing: &mut RouteDrawing<'_>,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    edge_index: usize,
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
    let context = edge_context(edges, edge_index);
    let labels_start = drawing.labels.len();
    let before =
        (edge.style.line.is_some() || edge.style.arrow.is_some()).then(|| drawing.canvas.clone());

    match direction.canonical() {
        GraphDirection::LeftRight => draw_left_right_edge(
            drawing,
            graph_layout,
            edges,
            EdgeLayouts { from, to },
            context,
            edge,
            charset,
        ),
        GraphDirection::TopDown => draw_top_down_edge(drawing, from, to, edge, charset),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }

    if let Some(before) = &before {
        apply_edge_style_delta(drawing.canvas, before, edge.style);
    }
    if let Some(color) = edge.style.label {
        for label in &mut drawing.labels[labels_start..] {
            label.color = Some(color);
        }
    }
}

fn draw_left_right_edge(
    drawing: &mut RouteDrawing<'_>,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    endpoints: EdgeLayouts<'_>,
    context: EdgeContext,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let from = endpoints.from;
    let to = endpoints.to;

    if from.id == to.id {
        draw_left_right_self_edge(
            drawing.canvas,
            drawing.route_cells,
            &graph_layout.nodes,
            edges,
            from,
            edge,
            charset,
        );
        return;
    }

    if from.center_y() == to.center_y() && from.x < to.x && context.parallel_index > 0 {
        draw_left_right_bottom_lane_edge(drawing, from, to, edge, charset);
        return;
    }

    if from.center_y() == to.center_y() && from.x > to.x {
        if has_self_loop(edges, &to.id) {
            draw_left_right_reverse_over_self_loop(
                drawing,
                &graph_layout.nodes,
                from,
                to,
                edge,
                charset,
            );
        } else {
            draw_left_right_bottom_lane_edge(drawing, from, to, edge, charset);
        }
        return;
    }

    if draw_left_right_grid_path_edge(drawing, graph_layout, from, to, edge, charset) {
        return;
    }

    if from.center_y() < to.center_y() && to.x > from.x {
        draw_left_right_down_then_right_edge(
            drawing.canvas,
            drawing.route_cells,
            &graph_layout.nodes,
            edges,
            endpoints,
            edge,
            charset,
        );
        return;
    }

    if from.center_y() < to.center_y() && to.x == from.x {
        draw_left_right_down_edge(drawing.canvas, drawing.route_cells, from, to, edge, charset);
        return;
    }

    if from.center_y() > to.center_y() && to.x > from.x {
        draw_left_right_right_then_up_edge(
            drawing.canvas,
            drawing.route_cells,
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
        set_edge_line(drawing.canvas, from.right(), y, charset.right_connector);
    }
    let start = from.right() + 1;
    let end = to.x - 1;
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    for x in start..end {
        set_route_cell(drawing.canvas, drawing.route_cells, x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(drawing.canvas, drawing.route_cells, end, y, line),
        GraphEdgeArrow::Point => set_edge_arrow(drawing.canvas, end, y, charset.arrow_right),
    }
    push_label_on_horizontal_line(drawing.labels, start, end, y, edge.label.as_deref());
}

fn draw_left_right_grid_path_edge(
    drawing: &mut RouteDrawing<'_>,
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
    let (lines_drawn, line_dirs) = draw_grid_path(
        drawing.canvas,
        drawing.route_cells,
        graph_layout,
        &path,
        edge,
        charset,
    );
    if lines_drawn.is_empty() || line_dirs.is_empty() {
        return false;
    }
    draw_grid_corners(
        drawing.canvas,
        drawing.route_cells,
        graph_layout,
        &path,
        charset,
    );
    draw_grid_box_start(
        drawing.canvas,
        lines_drawn[0].as_slice(),
        start_port,
        charset,
    );
    draw_grid_arrow_head(
        drawing.canvas,
        lines_drawn.last().map(Vec::as_slice).unwrap_or_default(),
        *line_dirs.last().unwrap_or(&end_port.step_fallback()),
        edge,
        charset,
    );
    push_label_on_canvas_lines(drawing.labels, &lines_drawn, edge.label.as_deref());
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
        StepDirection::Up => set_edge_line(canvas, from.x, from.y + 1, charset.up_connector),
        StepDirection::Down => set_edge_line(
            canvas,
            from.x,
            from.y.saturating_sub(1),
            charset.down_connector,
        ),
        StepDirection::Left => set_edge_line(canvas, from.x + 1, from.y, charset.left_connector),
        StepDirection::Right => set_edge_line(
            canvas,
            from.x.saturating_sub(1),
            from.y,
            charset.right_connector,
        ),
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
    set_edge_arrow(canvas, last.x, last.y, ch);
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

fn apply_edge_style_delta(canvas: &mut Canvas, before: &Canvas, style: GraphEdgeStyle) {
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if before.get(x, y) == canvas.get(x, y)
                && before.get_color(x, y) == canvas.get_color(x, y)
            {
                continue;
            }
            let Some(ch) = canvas.get(x, y) else {
                continue;
            };
            let Some(color) = edge_delta_color(ch, canvas.get_color(x, y), style) else {
                continue;
            };
            canvas.set_color(x, y, ch, color);
        }
    }
}

fn edge_delta_color(
    ch: char,
    color: Option<CanvasColor>,
    style: GraphEdgeStyle,
) -> Option<AsciiRgb> {
    match color {
        Some(CanvasColor::Role(AsciiColorRole::EdgeArrow)) => style.arrow.or(style.line),
        Some(CanvasColor::Role(AsciiColorRole::EdgeLine | AsciiColorRole::Junction)) => style.line,
        Some(CanvasColor::Role(_)) | Some(CanvasColor::Direct(_)) | None => {
            if is_edge_arrow_char(ch) {
                style.arrow.or(style.line)
            } else if is_edge_line_char(ch) {
                style.line
            } else {
                None
            }
        }
    }
}

fn is_edge_arrow_char(ch: char) -> bool {
    matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼')
}

fn is_edge_line_char(ch: char) -> bool {
    matches!(
        ch,
        '-' | '|'
            | '+'
            | '='
            | '#'
            | '─'
            | '┄'
            | '━'
            | '│'
            | '┆'
            | '┃'
            | '┌'
            | '┐'
            | '└'
            | '┘'
            | '├'
            | '┤'
            | '┬'
            | '┴'
            | '┼'
            | '╭'
            | '╮'
            | '╰'
            | '╯'
    )
}

fn draw_left_right_bottom_lane_edge(
    drawing: &mut RouteDrawing<'_>,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let start_x = from.center_x();
    let end_x = to.center_x();
    if start_x == end_x {
        return;
    }

    let bottom_y = left_right_back_edge_bottom_y(from);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let min_x = start_x.min(end_x);
    let max_x = start_x.max(end_x);

    drawing
        .canvas
        .set(start_x, from.bottom(), charset.down_connector);
    for y in (from.bottom() + 1)..bottom_y {
        set_route_cell(drawing.canvas, drawing.route_cells, start_x, y, vertical);
    }
    let start_corner = if start_x < end_x {
        charset.corner_down_right
    } else {
        charset.bottom_right
    };
    set_route_cell(
        drawing.canvas,
        drawing.route_cells,
        start_x,
        bottom_y,
        start_corner,
    );

    for x in (min_x + 1)..max_x {
        set_route_cell(drawing.canvas, drawing.route_cells, x, bottom_y, horizontal);
    }
    let end_corner = if start_x < end_x {
        charset.bottom_right
    } else {
        charset.corner_down_right
    };
    set_route_cell(
        drawing.canvas,
        drawing.route_cells,
        end_x,
        bottom_y,
        end_corner,
    );

    let arrow_y = bottom_y - 1;
    match edge.arrow {
        GraphEdgeArrow::Open => set_edge_line(drawing.canvas, end_x, arrow_y, vertical),
        GraphEdgeArrow::Point => set_edge_arrow(drawing.canvas, end_x, arrow_y, charset.arrow_up),
    }
    push_label_on_horizontal_line(
        drawing.labels,
        min_x,
        max_x,
        bottom_y,
        edge.label.as_deref(),
    );
}

fn draw_left_right_reverse_over_self_loop(
    drawing: &mut RouteDrawing<'_>,
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let lane_x = self_loop_right_x(layouts, to);
    if lane_x <= to.right() || from.x <= lane_x {
        return;
    }

    let y = to.center_y();
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    set_edge_line(drawing.canvas, from.x, y, charset.left_connector);
    set_route_cell(
        drawing.canvas,
        drawing.route_cells,
        lane_x,
        y,
        charset.down_junction,
    );
    for x in (lane_x + 1)..from.x {
        set_route_cell(drawing.canvas, drawing.route_cells, x, y, horizontal);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => {
            set_route_cell(
                drawing.canvas,
                drawing.route_cells,
                to.right() + 1,
                y,
                horizontal,
            );
        }
        GraphEdgeArrow::Point => {
            set_edge_arrow(drawing.canvas, to.right() + 1, y, charset.arrow_left)
        }
    }
    for x in (to.right() + 2)..lane_x {
        set_route_cell(drawing.canvas, drawing.route_cells, x, y, horizontal);
    }
    push_label_on_horizontal_line(
        drawing.labels,
        to.right() + 1,
        from.x.saturating_sub(1),
        y,
        edge.label.as_deref(),
    );
}

fn left_right_back_edge_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
}

fn draw_left_right_self_edge(
    canvas: &mut Canvas,
    route_cells: &mut RouteCells,
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let y = from.center_y();
    let loop_x = self_loop_right_x(layouts, from);
    let bottom_y = self_loop_bottom_y_for_edges(layouts, edges, from);
    if loop_x <= from.right() || bottom_y <= y + 1 {
        return;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    if from.shape != GraphNodeShape::Diamond {
        set_edge_line(canvas, from.right(), y, charset.right_connector);
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

    let arrow_y = from.bottom() + 1;
    for line_y in (arrow_y + 1)..bottom_y {
        set_route_cell(canvas, route_cells, from.center_x(), line_y, vertical);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_edge_line(canvas, from.center_x(), arrow_y, vertical),
        GraphEdgeArrow::Point => set_edge_arrow(canvas, from.center_x(), arrow_y, charset.arrow_up),
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

fn self_loop_bottom_y_for_edges(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
) -> usize {
    if has_same_row_reverse_edge_into(layouts, edges, from) {
        from.bottom() + 3
    } else {
        self_loop_bottom_y(from)
    }
}

fn has_same_row_reverse_edge_into(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        if edge.to != target.id || edge.from == target.id {
            return false;
        }
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        from.center_y() == target.center_y() && from.x > target.x
    })
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
    set_edge_line(canvas, x, from.bottom(), charset.down_connector);
    for y in start..end {
        set_route_cell(canvas, route_cells, x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(canvas, route_cells, x, end, line),
        GraphEdgeArrow::Point => set_edge_arrow(canvas, x, end, charset.arrow_down),
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
    set_edge_line(canvas, source_x, from.bottom(), charset.down_connector);
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
        GraphEdgeArrow::Point => set_edge_arrow(canvas, end, to.center_y(), charset.arrow_right),
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
    set_edge_line(canvas, source_x, from.y, charset.up_connector);
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
        GraphEdgeArrow::Point => set_edge_arrow(canvas, end, to.center_y(), charset.arrow_right),
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
    set_edge_line(canvas, x, from.bottom(), charset.down_connector);
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
        GraphEdgeArrow::Point => set_edge_arrow(canvas, end, corner_y, charset.arrow_right),
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
    set_edge_line(canvas, from.right(), y, charset.right_connector);
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
        GraphEdgeArrow::Point => set_edge_arrow(canvas, corner_x, arrow_y, charset.arrow_up),
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
    drawing: &mut RouteDrawing<'_>,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if from.center_y() > to.center_y() {
        draw_top_down_back_edge(drawing, from, to, edge, charset);
        return;
    }

    if from.center_x() != to.center_x() {
        draw_top_down_bent_edge(drawing, from, to, edge, charset);
        return;
    }

    if to.y <= from.bottom() + 1 {
        return;
    }

    if let Some(plan) = plan_top_down_direct_route(from, to, edge, charset) {
        paint_route_plan(drawing, &plan);
    }
}

fn paint_route_plan(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) {
    for cell in &plan.cells {
        match cell.kind {
            PlannedRouteCellKind::EdgeLine => {
                set_edge_line(drawing.canvas, cell.coord.x, cell.coord.y, cell.ch)
            }
            PlannedRouteCellKind::RouteCell => set_route_cell(
                drawing.canvas,
                drawing.route_cells,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
            ),
            PlannedRouteCellKind::EdgeArrow => {
                set_edge_arrow(drawing.canvas, cell.coord.x, cell.coord.y, cell.ch)
            }
        }
    }

    drawing
        .labels
        .extend(plan.labels.iter().map(|label| EdgeLabel {
            start: label.start,
            end: label.end,
            text: label.text.clone(),
            color: None,
        }));
}

fn draw_top_down_bent_edge(
    drawing: &mut RouteDrawing<'_>,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if to.y <= from.center_y() + 1 {
        return;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let source_y = from.center_y();
    let target_x = to.center_x();
    let end_y = to.y - 1;

    if target_x > from.center_x() {
        drawing
            .canvas
            .set(from.right(), source_y, charset.right_connector);
        for x in (from.right() + 1)..target_x {
            set_route_cell(drawing.canvas, drawing.route_cells, x, source_y, horizontal);
        }
    } else {
        set_edge_line(drawing.canvas, from.x, source_y, charset.left_connector);
        for x in (target_x + 1)..from.x {
            set_route_cell(drawing.canvas, drawing.route_cells, x, source_y, horizontal);
        }
    }

    set_route_cell(
        drawing.canvas,
        drawing.route_cells,
        target_x,
        source_y,
        charset.corner_down_right,
    );
    for y in (source_y + 1)..end_y {
        set_route_cell(drawing.canvas, drawing.route_cells, target_x, y, vertical);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => set_route_cell(
            drawing.canvas,
            drawing.route_cells,
            target_x,
            end_y,
            vertical,
        ),
        GraphEdgeArrow::Point => {
            set_edge_arrow(drawing.canvas, target_x, end_y, charset.arrow_down)
        }
    }
    push_label_on_vertical_line(
        drawing.labels,
        target_x,
        source_y + 1,
        end_y,
        edge.label.as_deref(),
    );
}

fn draw_top_down_back_edge(
    drawing: &mut RouteDrawing<'_>,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let lane_x = top_down_back_edge_lane_x(from, to);
    let source_y = from.center_y();
    let target_y = to.center_y();
    if source_y <= target_y || lane_x <= from.right() {
        return;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);

    drawing
        .canvas
        .set(from.right(), source_y, charset.right_connector);
    for x in (from.right() + 1)..lane_x {
        set_route_cell(drawing.canvas, drawing.route_cells, x, source_y, horizontal);
    }
    set_route_cell(
        drawing.canvas,
        drawing.route_cells,
        lane_x,
        source_y,
        charset.corner_right_up,
    );

    for y in (target_y + 1)..source_y {
        set_route_cell(drawing.canvas, drawing.route_cells, lane_x, y, vertical);
    }

    set_route_cell(
        drawing.canvas,
        drawing.route_cells,
        lane_x,
        target_y,
        charset.top_right,
    );
    match edge.arrow {
        GraphEdgeArrow::Open => {
            for x in (to.right() + 1)..lane_x {
                set_route_cell(drawing.canvas, drawing.route_cells, x, target_y, horizontal);
            }
        }
        GraphEdgeArrow::Point => {
            drawing
                .canvas
                .set(to.right() + 1, target_y, charset.arrow_left);
            for x in (to.right() + 2)..lane_x {
                set_route_cell(drawing.canvas, drawing.route_cells, x, target_y, horizontal);
            }
        }
    }
    push_label_on_vertical_line(
        drawing.labels,
        lane_x,
        target_y,
        source_y,
        edge.label.as_deref(),
    );
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

fn top_down_back_edge_lane_x(from: &NodeLayout, to: &NodeLayout) -> usize {
    from.right().max(to.right()) + 4
}

fn edge_context(edges: &[AsciiGraphEdge], edge_index: usize) -> EdgeContext {
    let Some(edge) = edges.get(edge_index) else {
        return EdgeContext { parallel_index: 0 };
    };
    let parallel_index = edges[..edge_index]
        .iter()
        .filter(|previous| same_edge_pair(previous, edge))
        .count();
    EdgeContext { parallel_index }
}

fn same_edge_pair(left: &AsciiGraphEdge, right: &AsciiGraphEdge) -> bool {
    (left.from == right.from && left.to == right.to)
        || (left.from == right.to && left.to == right.from)
}

fn has_self_loop(edges: &[AsciiGraphEdge], node_id: &str) -> bool {
    edges
        .iter()
        .any(|edge| edge.from == node_id && edge.to == node_id)
}
