use super::charset::GraphCharset;
use super::layout::{CanvasCoord, GraphLayout, NodeLayout};
use super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection, GraphEdgeStyle};
use crate::canvas::{Canvas, CanvasColor};
use crate::color::{AsciiColorRole, AsciiRgb};

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{set_edge_arrow, set_edge_line, set_route_cell};
pub(super) use label::{EdgeLabel, draw_routed_label};
use plan::{
    EdgeRouteRequest, PlannedRouteCellKind, RoutePlan, plan_edge_route, route_canvas_extent,
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

pub(super) fn edge_canvas_extent(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    direction: GraphDirection,
) -> (usize, usize) {
    route_canvas_extent(graph, layouts, edges, direction)
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
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    edge_index: usize,
    edge: &AsciiGraphEdge,
    _direction: GraphDirection,
    charset: &GraphCharset,
) {
    let layouts = &graph_layout.nodes;
    let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
        return;
    };
    let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
        return;
    };
    let labels_start = drawing.labels.len();
    let before =
        (edge.style.line.is_some() || edge.style.arrow.is_some()).then(|| drawing.canvas.clone());

    if let Some(plan) = plan_edge_route(EdgeRouteRequest {
        graph,
        graph_layout,
        edges,
        from,
        to,
        edge_index,
        edge,
        charset,
    }) {
        paint_route_plan(drawing, &plan);
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
