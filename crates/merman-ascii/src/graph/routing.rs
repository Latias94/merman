use super::charset::GraphCharset;
use super::label::GraphLabel;
use super::layout::{CanvasCoord, GraphLayout, GridCoord, GroupLayout, NodeLayout};
use super::model::{
    AsciiGraph, AsciiGraphEdge, GraphDirection, GraphEdgeStyle, GraphNodeShape, GraphNodeStyle,
};
use crate::canvas::Canvas;
use crate::error::{AsciiError, Result};

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{set_edge_arrow_with_color, set_edge_line_with_color, set_route_cell_with_color};
use label::routed_label_placement;
pub(super) use label::{EdgeLabel, draw_routed_label};
pub(super) use plan::RouteLabelAnchor;
use plan::{
    EdgeRouteRequest, PlannedRouteCellKind, PlannedRouteLabel, RoutePlan, plan_edge_route,
    route_canvas_extent,
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
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    direction: GraphDirection,
    charset: &GraphCharset,
) -> (usize, usize) {
    let layouts = endpoint_layouts_for_extent(graph_layout);
    let (mut width, mut height) = route_canvas_extent(graph, &layouts, edges, direction);
    let (label_width, label_height) =
        planned_route_label_canvas_extent(graph, graph_layout, edges, charset);
    width = width.max(label_width);
    height = height.max(label_height);
    (width, height)
}

pub(super) fn transform_routed_label(
    label: &EdgeLabel,
    mut transform: impl FnMut(CanvasCoord) -> CanvasCoord,
    mut transform_anchor: impl FnMut(RouteLabelAnchor) -> RouteLabelAnchor,
) -> EdgeLabel {
    EdgeLabel {
        start: transform(label.start),
        end: transform(label.end),
        text: label.text.clone(),
        anchor: transform_anchor(label.anchor),
        color: label.color,
    }
}

pub(super) fn draw_edge(
    drawing: &mut RouteDrawing<'_>,
    request: DrawEdgeRequest<'_>,
) -> Result<()> {
    let Some(from) = endpoint_layout(request.graph_layout, &request.edge.from) else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: request.graph.diagram_type(),
            feature: "edges with missing endpoint layouts",
        });
    };
    let Some(to) = endpoint_layout(request.graph_layout, &request.edge.to) else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: request.graph.diagram_type(),
            feature: "edges with missing endpoint layouts",
        });
    };
    let Some(plan) = plan_edge_route(EdgeRouteRequest {
        graph: request.graph,
        graph_layout: request.graph_layout,
        edges: request.edges,
        from: &from,
        to: &to,
        edge_index: request.edge_index,
        edge: request.edge,
        charset: request.charset,
    }) else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: request.graph.diagram_type(),
            feature: "unroutable graph edges",
        });
    };
    paint_route_plan(drawing, &plan, request.edge.style);

    Ok(())
}

fn endpoint_layout(graph_layout: &GraphLayout, endpoint_id: &str) -> Option<NodeLayout> {
    graph_layout
        .nodes
        .iter()
        .find(|layout| layout.id == endpoint_id)
        .cloned()
        .or_else(|| {
            graph_layout
                .groups
                .iter()
                .find(|layout| layout.id == endpoint_id)
                .map(group_endpoint_layout)
        })
}

fn endpoint_layouts_for_extent(graph_layout: &GraphLayout) -> Vec<NodeLayout> {
    let mut layouts = graph_layout.nodes.clone();
    layouts.extend(graph_layout.groups.iter().map(group_endpoint_layout));
    layouts
}

fn group_endpoint_layout(group: &GroupLayout) -> NodeLayout {
    NodeLayout {
        id: group.id.clone(),
        label: GraphLabel::new(""),
        shape: GraphNodeShape::Rect,
        style: GraphNodeStyle::default(),
        grid: GridCoord { x: 0, y: 0 },
        x: group.x,
        y: group.y,
        width: group.width,
        height: group.height,
    }
}

fn planned_route_label_canvas_extent(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    charset: &GraphCharset,
) -> (usize, usize) {
    let mut width = 0;
    let mut height = 0;

    for (edge_index, edge) in edges.iter().enumerate() {
        let Some(from) = endpoint_layout(graph_layout, &edge.from) else {
            continue;
        };
        let Some(to) = endpoint_layout(graph_layout, &edge.to) else {
            continue;
        };
        let Some(plan) = plan_edge_route(EdgeRouteRequest {
            graph,
            graph_layout,
            edges,
            from: &from,
            to: &to,
            edge_index,
            edge,
            charset,
        }) else {
            continue;
        };

        for label in &plan.labels {
            let (label_width, label_height) = planned_label_canvas_extent(label);
            width = width.max(label_width);
            height = height.max(label_height);
        }
    }

    (width, height)
}

fn planned_label_canvas_extent(label: &PlannedRouteLabel) -> (usize, usize) {
    routed_label_placement(label.start, label.end, &label.text, label.anchor)
        .map(|placement| placement.canvas_extent())
        .unwrap_or((0, 0))
}

fn paint_route_plan(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan, style: GraphEdgeStyle) {
    for cell in &plan.cells {
        let color = match cell.kind {
            PlannedRouteCellKind::EdgeArrow => style.arrow.or(style.line),
            PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::RouteCell => style.line,
        };
        match cell.kind {
            PlannedRouteCellKind::EdgeLine => {
                set_edge_line_with_color(drawing.canvas, cell.coord.x, cell.coord.y, cell.ch, color)
            }
            PlannedRouteCellKind::RouteCell => set_route_cell_with_color(
                drawing.canvas,
                drawing.route_cells,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                color,
            ),
            PlannedRouteCellKind::EdgeArrow => set_edge_arrow_with_color(
                drawing.canvas,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                color,
            ),
        }
    }

    drawing
        .labels
        .extend(plan.labels.iter().map(|label| EdgeLabel {
            start: label.start,
            end: label.end,
            text: label.text.clone(),
            anchor: label.anchor,
            color: style.label,
        }));
}

#[cfg(test)]
mod tests {
    use super::plan::{PlannedRouteCell, PlannedRouteLabel, PlannedRouteSegment};
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::color::AsciiRgb;
    use crate::graph::layout::layout_graph;
    use crate::graph::model::GraphEdgeAttrs;

    #[test]
    fn edge_style_is_applied_to_route_plan_cells_and_labels() {
        let line = AsciiRgb::new(1, 2, 3);
        let arrow = AsciiRgb::new(4, 5, 6);
        let label = AsciiRgb::new(7, 8, 9);
        let plan = RoutePlan {
            cells: vec![
                planned_cell(0, 0, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(1, 0, '-', PlannedRouteCellKind::RouteCell),
                planned_cell(2, 0, '>', PlannedRouteCellKind::EdgeArrow),
            ],
            labels: vec![PlannedRouteLabel {
                start: CanvasCoord { x: 0, y: 0 },
                end: CanvasCoord { x: 2, y: 0 },
                text: "label".to_string(),
                anchor: RouteLabelAnchor::Inline,
            }],
        };

        let mut canvas = Canvas::new(3, 1);
        let mut route_cells = RouteCells::new();
        let mut labels = Vec::new();
        let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells, &mut labels);

        paint_route_plan(
            &mut drawing,
            &plan,
            GraphEdgeStyle {
                line: Some(line),
                arrow: Some(arrow),
                label: Some(label),
            },
        );

        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::terminal::CanvasColor::Direct(line))
        );
        assert_eq!(
            canvas.get_color(1, 0),
            Some(crate::terminal::CanvasColor::Direct(line))
        );
        assert_eq!(
            canvas.get_color(2, 0),
            Some(crate::terminal::CanvasColor::Direct(arrow))
        );
        assert_eq!(labels[0].color, Some(label));
    }

    #[test]
    fn edge_arrow_style_falls_back_to_line_style() {
        let line = AsciiRgb::new(10, 11, 12);
        let plan = RoutePlan {
            cells: vec![planned_cell(0, 0, '>', PlannedRouteCellKind::EdgeArrow)],
            labels: Vec::new(),
        };

        let mut canvas = Canvas::new(1, 1);
        let mut route_cells = RouteCells::new();
        let mut labels = Vec::new();
        let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells, &mut labels);

        paint_route_plan(
            &mut drawing,
            &plan,
            GraphEdgeStyle {
                line: Some(line),
                arrow: None,
                label: None,
            },
        );

        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::terminal::CanvasColor::Direct(line))
        );
    }

    #[test]
    fn edge_canvas_extent_accounts_for_boundary_grid_path_label_width() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("y", "Y");
        graph.add_group_with_style(
            "one",
            "LR Group",
            Some(GraphDirection::LeftRight),
            vec!["a".to_string(), "b".to_string()],
            Default::default(),
        );
        graph.add_edge("a", "b");
        graph.add_edge_with_attrs(
            "b",
            "y",
            GraphEdgeAttrs {
                label: Some("boundary label with enough width".to_string()),
                ..Default::default()
            },
        );
        let graph_layout = layout_graph(&graph, &options);
        let edge = &graph.edges[1];
        let from = endpoint_layout(&graph_layout, &edge.from).expect("source layout should exist");
        let to = endpoint_layout(&graph_layout, &edge.to).expect("target layout should exist");
        let plan = plan_edge_route(EdgeRouteRequest {
            graph: &graph,
            graph_layout: &graph_layout,
            edges: &graph.edges,
            from: &from,
            to: &to,
            edge_index: 1,
            edge,
            charset: &charset,
        })
        .expect("boundary route should plan");
        let label = plan.labels.first().expect("boundary route should label");
        let label_width = crate::text::display_width(&label.text);
        let min_x = label.start.x.min(label.end.x);
        let max_x = label.start.x.max(label.end.x);
        let middle_x = min_x + (max_x - min_x) / 2;
        let required_width = middle_x.saturating_sub(label_width / 2) + label_width;

        let (edge_width, _) = edge_canvas_extent(
            &graph,
            &graph_layout,
            &graph.edges,
            graph.direction,
            &charset,
        );

        assert!(
            edge_width >= required_width,
            "edge canvas extent should reserve boundary label width {required_width}, got {edge_width}; plan: {plan:?}"
        );
    }

    #[test]
    fn draw_edge_reports_missing_endpoint_layouts() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("b", "missing");
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
        .expect_err("edge with a missing endpoint layout should be unsupported");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "edges with missing endpoint layouts",
            }
        );
    }

    fn planned_cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
        PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch,
            kind,
            segment: PlannedRouteSegment::Direct,
        }
    }
}
