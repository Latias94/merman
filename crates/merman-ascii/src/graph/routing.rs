use super::charset::GraphCharset;
use super::layout::{CanvasCoord, GraphLayout, NodeLayout};
use super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection, GraphEdgeStyle};
use crate::canvas::Canvas;
use crate::error::{AsciiError, Result};

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{set_edge_arrow_with_color, set_edge_line_with_color, set_route_cell_with_color};
pub(super) use label::{EdgeLabel, draw_routed_label};
use plan::{
    EdgeRouteRequest, PlannedRouteCellKind, RoutePlan, plan_edge_route, route_canvas_extent,
};

pub(super) struct RouteDrawing<'a> {
    canvas: &'a mut Canvas,
    route_cells: &'a mut RouteCells,
    labels: &'a mut Vec<EdgeLabel>,
}

pub(super) struct DrawEdgeRequest<'a> {
    pub(super) graph: &'a AsciiGraph,
    pub(super) graph_layout: &'a GraphLayout,
    pub(super) edges: &'a [AsciiGraphEdge],
    pub(super) edge_index: usize,
    pub(super) edge: &'a AsciiGraphEdge,
    pub(super) charset: &'a GraphCharset,
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
    request: DrawEdgeRequest<'_>,
) -> Result<()> {
    let layouts = &request.graph_layout.nodes;
    let Some(from) = layouts.iter().find(|layout| layout.id == request.edge.from) else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "edges with missing endpoint layouts",
        });
    };
    let Some(to) = layouts.iter().find(|layout| layout.id == request.edge.to) else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "edges with missing endpoint layouts",
        });
    };
    let Some(mut plan) = plan_edge_route(EdgeRouteRequest {
        graph: request.graph,
        graph_layout: request.graph_layout,
        edges: request.edges,
        from,
        to,
        edge_index: request.edge_index,
        edge: request.edge,
        charset: request.charset,
    }) else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "unroutable graph edges",
        });
    };
    apply_edge_style_to_plan(&mut plan, request.edge.style);
    paint_route_plan(drawing, &plan);

    Ok(())
}

fn apply_edge_style_to_plan(plan: &mut RoutePlan, style: GraphEdgeStyle) {
    for cell in &mut plan.cells {
        cell.color = match cell.kind {
            PlannedRouteCellKind::EdgeArrow => style.arrow.or(style.line),
            PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::RouteCell => style.line,
        };
    }
    for label in &mut plan.labels {
        label.color = style.label;
    }
}

fn paint_route_plan(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) {
    for cell in &plan.cells {
        match cell.kind {
            PlannedRouteCellKind::EdgeLine => set_edge_line_with_color(
                drawing.canvas,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.color,
            ),
            PlannedRouteCellKind::RouteCell => set_route_cell_with_color(
                drawing.canvas,
                drawing.route_cells,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.color,
            ),
            PlannedRouteCellKind::EdgeArrow => set_edge_arrow_with_color(
                drawing.canvas,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.color,
            ),
        }
    }

    drawing
        .labels
        .extend(plan.labels.iter().map(|label| EdgeLabel {
            start: label.start,
            end: label.end,
            text: label.text.clone(),
            color: label.color,
        }));
}

#[cfg(test)]
mod tests {
    use super::plan::{PlannedRouteCell, PlannedRouteLabel, PlannedRouteSegment};
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::color::AsciiRgb;
    use crate::graph::layout::layout_graph;

    #[test]
    fn edge_style_is_applied_to_route_plan_cells_and_labels() {
        let line = AsciiRgb::new(1, 2, 3);
        let arrow = AsciiRgb::new(4, 5, 6);
        let label = AsciiRgb::new(7, 8, 9);
        let mut plan = RoutePlan {
            cells: vec![
                planned_cell(0, 0, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(1, 0, '-', PlannedRouteCellKind::RouteCell),
                planned_cell(2, 0, '>', PlannedRouteCellKind::EdgeArrow),
            ],
            labels: vec![PlannedRouteLabel {
                start: CanvasCoord { x: 0, y: 0 },
                end: CanvasCoord { x: 2, y: 0 },
                text: "label".to_string(),
                color: None,
            }],
        };

        apply_edge_style_to_plan(
            &mut plan,
            GraphEdgeStyle {
                line: Some(line),
                arrow: Some(arrow),
                label: Some(label),
            },
        );

        assert_eq!(plan.cells[0].color, Some(line));
        assert_eq!(plan.cells[1].color, Some(line));
        assert_eq!(plan.cells[2].color, Some(arrow));
        assert_eq!(plan.labels[0].color, Some(label));
    }

    #[test]
    fn edge_arrow_style_falls_back_to_line_style() {
        let line = AsciiRgb::new(10, 11, 12);
        let mut plan = RoutePlan {
            cells: vec![planned_cell(0, 0, '>', PlannedRouteCellKind::EdgeArrow)],
            labels: Vec::new(),
        };

        apply_edge_style_to_plan(
            &mut plan,
            GraphEdgeStyle {
                line: Some(line),
                arrow: None,
                label: None,
            },
        );

        assert_eq!(plan.cells[0].color, Some(line));
    }

    #[test]
    fn draw_edge_reports_unroutable_edges() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("b", "a");
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&graph, &options);
        let charset = GraphCharset::for_options(&options);
        let mut canvas = Canvas::new(80, 20);
        let mut route_cells = RouteCells::new();
        let mut labels = Vec::new();
        let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells, &mut labels);
        let edge = &graph.edges[0];

        let error = draw_edge(
            &mut drawing,
            DrawEdgeRequest {
                graph: &graph,
                graph_layout: &graph_layout,
                edges: &graph.edges,
                edge_index: 0,
                edge,
                charset: &charset,
            },
        )
        .expect_err("same-rank right-to-left edge in top-down graph should be unsupported");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "unroutable graph edges",
            }
        );
    }

    fn planned_cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
        PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch,
            kind,
            segment: PlannedRouteSegment::Direct,
            color: None,
        }
    }
}
