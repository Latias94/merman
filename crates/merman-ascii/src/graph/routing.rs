use super::charset::GraphCharset;
use super::layout::{CanvasCoord, GraphLayout, NodeLayout};
use super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeStyle};
use crate::canvas::{Canvas, CanvasColor};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::text::display_width;

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{set_edge_arrow, set_edge_line, set_route_cell};
pub(super) use label::{EdgeLabel, draw_routed_label};
use plan::{
    PlannedRouteCellKind, RoutePlan, left_right_back_edge_bottom_y,
    plan_left_right_bottom_lane_route, plan_left_right_direct_route, plan_left_right_down_route,
    plan_left_right_down_then_right_route, plan_left_right_grid_path_route,
    plan_left_right_reverse_over_self_loop_route, plan_left_right_right_then_up_route,
    plan_left_right_self_loop_route, plan_top_down_back_route, plan_top_down_bent_route,
    plan_top_down_direct_route, self_loop_bottom_y_for_edges, self_loop_right_x,
    top_down_back_edge_lane_x,
};

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
        if let Some(plan) =
            plan_left_right_self_loop_route(&graph_layout.nodes, edges, from, edge, charset)
        {
            paint_route_plan(drawing, &plan);
        }
        return;
    }

    if from.center_y() == to.center_y() && from.x < to.x && context.parallel_index > 0 {
        if let Some(plan) = plan_left_right_bottom_lane_route(from, to, edge, charset) {
            paint_route_plan(drawing, &plan);
        }
        return;
    }

    if from.center_y() == to.center_y() && from.x > to.x {
        if has_self_loop(edges, &to.id) {
            if let Some(plan) = plan_left_right_reverse_over_self_loop_route(
                &graph_layout.nodes,
                from,
                to,
                edge,
                charset,
            ) {
                paint_route_plan(drawing, &plan);
            }
        } else if let Some(plan) = plan_left_right_bottom_lane_route(from, to, edge, charset) {
            paint_route_plan(drawing, &plan);
        }
        return;
    }

    if from.center_y() == to.center_y() && from.x < to.x {
        if let Some(plan) =
            plan_left_right_direct_route(&graph_layout.nodes, from, to, edge, charset)
        {
            paint_route_plan(drawing, &plan);
            return;
        }
    }

    if draw_left_right_grid_path_edge(drawing, graph_layout, from, to, edge, charset) {
        return;
    }

    if from.center_y() < to.center_y() && to.x > from.x {
        if let Some(plan) = plan_left_right_down_then_right_route(
            &graph_layout.nodes,
            edges,
            from,
            to,
            edge,
            charset,
        ) {
            paint_route_plan(drawing, &plan);
        }
        return;
    }

    if from.center_y() < to.center_y() && to.x == from.x {
        if let Some(plan) = plan_left_right_down_route(from, to, edge, charset) {
            paint_route_plan(drawing, &plan);
        }
        return;
    }

    if from.center_y() > to.center_y() && to.x > from.x {
        if let Some(plan) =
            plan_left_right_right_then_up_route(&graph_layout.nodes, edges, from, to, edge, charset)
        {
            paint_route_plan(drawing, &plan);
        }
    }
}

fn draw_left_right_grid_path_edge(
    drawing: &mut RouteDrawing<'_>,
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> bool {
    let Some(plan) = plan_left_right_grid_path_route(graph_layout, from, to, edge, charset) else {
        return false;
    };
    paint_route_plan(drawing, &plan);
    true
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

fn draw_top_down_edge(
    drawing: &mut RouteDrawing<'_>,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if from.center_y() > to.center_y() {
        if let Some(plan) = plan_top_down_back_route(from, to, edge, charset) {
            paint_route_plan(drawing, &plan);
        }
        return;
    }

    if from.center_x() != to.center_x() {
        if let Some(plan) = plan_top_down_bent_route(from, to, edge, charset) {
            paint_route_plan(drawing, &plan);
        }
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
